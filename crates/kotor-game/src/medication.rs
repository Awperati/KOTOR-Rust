//! Medication Sickness. Using a medical item while the cooldown from the last one still runs is allowed, with a price: the
//! item heals less, and the user's aim and damage get worse, for as long as the cooldown runs. Each use restarts it.
//!
//! The numbers are in `docs/medication.tsv`. The prompt and the white square that drains on the consumables pill belong to
//! the menus: [`World::medication_cooldown`] is the share of the cooldown left, and [`World::use_consumable_request`] says
//! whether the menus must ask first.
//!
//! **Assumed:** only items whose base item is `Medical_Equipment` count (medpacs and stimulants); repair kits and squad
//! recovery kits do not. The first use is free and only starts the cooldown. The second use inside it is the first
//! "extra", the third and every later one is the second. The stack and its penalties end when the cooldown runs out,
//! which is 20 s after the last use. A hit that the penalty turns into a miss follows the rules for a miss. The prompt is
//! shown once for a cooldown window, and again only if it was cancelled.
use crate::world::{ObjId, World};
use std::sync::OnceLock;

/// The numbers of `docs/medication.tsv`.
#[derive(Clone, Debug)]
pub struct MedicationTable {
    pub cooldown_seconds: f32,
    pub heal_scale: [f32; 2],
    pub aim_penalty: [f32; 2],
    pub damage_penalty: [f32; 2],
}

const TABLE_TEXT: &str = include_str!("../../../docs/medication.tsv");

impl MedicationTable {
    pub fn get() -> &'static MedicationTable {
        static T: OnceLock<MedicationTable> = OnceLock::new();
        T.get_or_init(|| {
            let value = |key: &str, default: f32| {
                TABLE_TEXT
                    .lines()
                    .filter_map(|l| {
                        let mut c = l.split('\t');
                        if c.next()? == key {
                            c.next()?.trim().parse().ok()
                        } else {
                            None
                        }
                    })
                    .next()
                    .unwrap_or(default)
            };
            MedicationTable {
                cooldown_seconds: value("cooldown_seconds", 20.0),
                heal_scale: [value("heal_scale_1", 0.5), value("heal_scale_2", 0.25)],
                aim_penalty: [value("aim_penalty_1", 0.5), value("aim_penalty_2", 0.8)],
                damage_penalty: [
                    value("damage_penalty_1", 0.25),
                    value("damage_penalty_2", 0.5),
                ],
            }
        })
    }
}

/// One character's sickness.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sickness {
    /// When the cooldown ends, in world seconds.
    pub until: f32,
    /// 0 for a first use, 1 and 2 for the extra uses inside the cooldown.
    pub stack: u8,
    /// The player has accepted the warning for this cooldown window.
    pub warned: bool,
}

/// What the menus do before a medical item is used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UseRequest {
    /// Go ahead.
    Ready,
    /// Ask first: "warning, continuing will cause medication sickness. Are you sure you would like to continue?" Then call
    /// [`World::accept_medication_warning`] on Yes and use the item, or do nothing on No.
    NeedsConfirm,
}

/// What using a consumable did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UseOutcome {
    /// It was used.
    Used,
    /// Nothing happened: ask the player first, and call again with `confirmed` set if the answer is Yes.
    NeedsConfirm,
    /// It cannot be used.
    Refused,
}

impl World {
    /// Use a consumable for the menus. A medical item inside the cooldown asks first, once for each cooldown window (kept in the
    /// world, so a save keeps it): with `confirmed` false it returns `NeedsConfirm` and does nothing, and with `confirmed` true it
    /// takes the warning as answered Yes and uses the item.
    pub fn use_consumable(&mut self, who: ObjId, item: ObjId, confirmed: bool) -> UseOutcome {
        if self.use_consumable_request(who, item) == UseRequest::NeedsConfirm {
            if !confirmed {
                return UseOutcome::NeedsConfirm;
            }
            self.accept_medication_warning(who);
        }
        match self.use_item(who, item) {
            Ok(()) => UseOutcome::Used,
            Err(_) => UseOutcome::Refused,
        }
    }

    /// Whether the item is a medical consumable that the cooldown applies to.
    pub fn is_medical(&mut self, item: ObjId) -> bool {
        self.base_label(item) == "Medical_Equipment"
    }

    fn sickness_now(&self, who: ObjId) -> Option<&Sickness> {
        self.medication.get(&who).filter(|s| self.time < s.until)
    }

    /// The share of the cooldown left for someone, 1 just after a use and 0 when it is over. The menus draw the white square
    /// on the pill from it.
    pub fn medication_cooldown(&self, who: ObjId) -> f32 {
        self.sickness_now(who).map_or(0.0, |s| {
            ((s.until - self.time) / MedicationTable::get().cooldown_seconds).clamp(0.0, 1.0)
        })
    }

    /// How many extra uses are on someone now: 0, 1 or 2. Above 0 they are sick, and the character panel says so.
    pub fn medication_stack(&self, who: ObjId) -> u8 {
        self.sickness_now(who).map_or(0, |s| s.stack)
    }

    /// Whether a use of this item must be confirmed first: it is medical, the cooldown still runs, and the player has not
    /// accepted the warning in this window.
    pub fn use_consumable_request(&mut self, who: ObjId, item: ObjId) -> UseRequest {
        if self.is_medical(item) && self.sickness_now(who).is_some_and(|s| !s.warned) {
            UseRequest::NeedsConfirm
        } else {
            UseRequest::Ready
        }
    }

    /// The player said Yes to the warning.
    pub fn accept_medication_warning(&mut self, who: ObjId) {
        if let Some(s) = self.medication.get_mut(&who) {
            s.warned = true;
        }
    }

    /// What a medical item heals for this user now, as a share of its full heal.
    pub(crate) fn medication_heal_scale(&self, who: ObjId) -> f32 {
        match self.medication_stack(who) {
            0 => 1.0,
            1 => MedicationTable::get().heal_scale[0],
            _ => MedicationTable::get().heal_scale[1],
        }
    }

    /// A medical item was used. The cooldown restarts, and a use inside it adds to the stack, to 2 at most. The first use
    /// inside a cooldown is the first extra, and it is the one that brings the stack to 1.
    pub(crate) fn medication_used(&mut self, who: ObjId) {
        let until = self.time + MedicationTable::get().cooldown_seconds;
        let entry = match self.sickness_now(who).cloned() {
            Some(s) => Sickness {
                until,
                stack: (s.stack + 1).min(2),
                warned: s.warned,
            },
            None => Sickness {
                until,
                stack: 0,
                warned: false,
            },
        };
        self.medication.insert(who, entry);
    }

    /// The share of the sick character's hits that become misses.
    pub(crate) fn medication_aim_penalty(&self, who: ObjId) -> f32 {
        match self.medication_stack(who) {
            0 => 0.0,
            1 => MedicationTable::get().aim_penalty[0],
            _ => MedicationTable::get().aim_penalty[1],
        }
    }

    /// The share taken off the sick character's damage.
    pub(crate) fn medication_damage_penalty(&self, who: ObjId) -> f32 {
        match self.medication_stack(who) {
            0 => 0.0,
            1 => MedicationTable::get().damage_penalty[0],
            _ => MedicationTable::get().damage_penalty[1],
        }
    }
}
