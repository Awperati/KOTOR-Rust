//! What an item's properties do for whoever wears or holds it. `PropertiesList` in the item's file names rows of
//! `itempropdef.2da`, with a `Subtype`, and a `CostTable` and `CostValue` that say how much through `iprp_costtable.2da`. A
//! property with `UpgradeType` waits for that row of `upgrade.2da` to be in one of the item's slots.
//!
//! **Read from the install:** all of that. **Assumed, not compared with the game:** enhancement adds to attack and to damage,
//! keen doubles the threat range, flat damage is multiplied on a critical and bonus dice are not, a massive critical adds on a
//! critical only, and what an item in another slot gives to attack it gives to both hands. **Not built:** damage types, so what
//! a type resists or adds is the same as any; on-hit effects, immunities, regeneration, deflection and bonus feats.

use crate::rules::Save;
use crate::world::{ObjId, World};
use kotor_gff::Struct;

const ABILITY: i32 = 0;
const ARMOR: i32 = 1;
const ENHANCEMENT: i32 = 5;
const ATTACK_PENALTY: i32 = 8;
const DAMAGE: i32 = 11;
const DAMAGE_VS_SIDE: i32 = 12;
const DAMAGE_VS_RACE: i32 = 13;
const DAMAGE_PENALTY: i32 = 15;
const ABILITY_DOWN: i32 = 19;
const ARMOR_DOWN: i32 = 20;
const SKILL_DOWN: i32 = 21;
const SAVES_AGAINST: i32 = 26;
const SAVE: i32 = 27;
const KEEN: i32 = 28;
const SAVES_DOWN: i32 = 33;
const SAVE_DOWN: i32 = 34;
const SKILL: i32 = 36;
const ATTACK: i32 = 38;
const TO_HIT_PENALTY: i32 = 41;
const ONLY_SIDE: i32 = 43;
const ONLY_CLASS: i32 = 44;
const ONLY_RACE: i32 = 45;
const MASSIVE_CRITICAL: i32 = 49;
const ONLY_FEAT: i32 = 57;
/// `iprp_saveelement`'s row for everything, and `iprp_savingthrow`'s.
const AGAINST_ALL: i32 = 0;
/// `iprp_aligngrp`'s rows, and the band in the middle of the scale.
const LIMIT_NEUTRAL: i32 = 1;
const LIMIT_LIGHT: i32 = 2;
const LIMIT_DARK: i32 = 3;
const LIMIT_BAND: (i32, i32) = (40, 60);
const SAVE_ALL: i32 = 0;
const RIGHT_HAND: i32 = crate::items::SLOT_RIGHT_HAND;
const LEFT_HAND: i32 = crate::items::SLOT_LEFT_HAND;

/// One line of an item's `PropertiesList`.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ItemProperty {
    kind: i32,
    subtype: i32,
    /// What it adds, or takes away, if a number.
    flat: i32,
    /// How many dice of how many sides it adds, if dice.
    dice: (i32, i32),
    waits_for: Option<i32>,
}

/// Whether an alignment may use what is limited to a row of `iprp_aligngrp`. The middle may use both sides' things, by the
/// owner's order through Top Dog.
fn side_allows(row: i32, alignment: i32) -> bool {
    match row {
        LIMIT_LIGHT => alignment > LIMIT_BAND.0,
        LIMIT_DARK => alignment < LIMIT_BAND.1,
        LIMIT_NEUTRAL => (LIMIT_BAND.0..=LIMIT_BAND.1).contains(&alignment),
        _ => true,
    }
}

/// Whether what a limit asks for is among what a creature is. No limit of a kind means anyone.
fn passes(limits: &[i32], is: impl Fn(i32) -> bool) -> bool {
    limits.is_empty() || limits.iter().any(|l| is(*l))
}

impl World {
    /// The properties in an item's file, with their amounts looked up.
    pub(crate) fn read_properties(&mut self, list: &[Struct]) -> Vec<ItemProperty> {
        let names = self.table("iprp_costtable");
        list.iter()
            .map(|p| {
                let int = |name: &str| p.int(name).map(|v| v as i32);
                let mut out = ItemProperty {
                    kind: int("PropertyName").unwrap_or(-1),
                    subtype: int("Subtype").unwrap_or(0),
                    waits_for: int("UpgradeType"),
                    ..ItemProperty::default()
                };
                let table = int("CostTable")
                    .and_then(|t| {
                        names
                            .as_ref()?
                            .get(t.max(0) as usize, "name")
                            .map(str::to_ascii_lowercase)
                    })
                    .and_then(|name| self.table(&name));
                if let Some(t) = table {
                    let row = int("CostValue").unwrap_or(0).max(0) as usize;
                    // Damage's table has no number of its own: its label is the number, or it has dice.
                    out.flat = t
                        .int(row, "value")
                        .map(|v| v as i32)
                        .or_else(|| t.get(row, "label")?.parse().ok())
                        .unwrap_or(0);
                    out.dice = (
                        t.int(row, "numdice").unwrap_or(0) as i32,
                        t.int(row, "die").unwrap_or(0) as i32,
                    );
                }
                out
            })
            .collect()
    }

    /// What an item has working, from its file and the upgrades in its slots.
    fn working(&self, item: ObjId) -> Vec<ItemProperty> {
        let Some(o) = self.objects.get(&item) else {
            return Vec::new();
        };
        let Some(t) = self.item_templates.get(&o.template.to_ascii_lowercase()) else {
            return Vec::new();
        };
        t.properties
            .iter()
            .filter(|p| {
                p.waits_for
                    .is_none_or(|row| o.upgrades.values().any(|r| *r == row))
            })
            .copied()
            .collect()
    }

    /// What all a creature wears and holds has working, with the slot each is in.
    fn gear(&self, id: ObjId) -> Vec<(i32, ItemProperty)> {
        let Some(o) = self.objects.get(&id) else {
            return Vec::new();
        };
        o.equipped
            .iter()
            .flat_map(|(slot, item)| self.working(*item).into_iter().map(|p| (*slot, p)))
            .collect()
    }

    /// The properties of the weapon in `slot`, with what the rest of the gear gives every attack.
    fn attacking(&self, id: ObjId, slot: i32) -> Vec<ItemProperty> {
        self.gear(id)
            .into_iter()
            .filter(|(s, _)| *s == slot || (*s != RIGHT_HAND && *s != LEFT_HAND))
            .map(|(_, p)| p)
            .collect()
    }

    fn gear_sum(&self, id: ObjId, kinds: &[i32], subtype: Option<i32>) -> i32 {
        self.gear(id)
            .iter()
            .filter(|(_, p)| kinds.contains(&p.kind) && subtype.is_none_or(|s| s == p.subtype))
            .map(|(_, p)| p.flat)
            .sum()
    }

    /// What the gear adds to an ability score.
    pub(crate) fn gear_ability(&self, id: ObjId, ability: usize) -> i32 {
        self.gear_sum(id, &[ABILITY, ABILITY_DOWN], Some(ability as i32))
    }

    /// What the gear adds to defense.
    pub(crate) fn gear_defense(&self, id: ObjId) -> i32 {
        self.gear_sum(id, &[ARMOR, ARMOR_DOWN], None)
    }

    /// What the gear adds to a skill.
    pub(crate) fn gear_skill(&self, id: ObjId, skill: usize) -> i32 {
        self.gear_sum(id, &[SKILL, SKILL_DOWN], Some(skill as i32))
    }

    /// What the gear adds to a saving throw. Saves against one thing only are not counted, as nothing asks against what.
    pub(crate) fn gear_save(&self, id: ObjId, save: Save) -> i32 {
        let row = match save {
            Save::Fortitude => 1,
            Save::Reflex => 2,
            Save::Will => 3,
        };
        self.gear(id)
            .iter()
            .filter(|(_, p)| match p.kind {
                SAVE | SAVE_DOWN => p.subtype == SAVE_ALL || p.subtype == row,
                SAVES_AGAINST | SAVES_DOWN => p.subtype == AGAINST_ALL,
                _ => false,
            })
            .map(|(_, p)| p.flat)
            .sum()
    }

    /// What the gear adds to the d20 with the weapon in a slot.
    pub(crate) fn gear_attack(&self, id: ObjId, slot: i32) -> i32 {
        self.attacking(id, slot)
            .iter()
            .filter(|p| {
                matches!(
                    p.kind,
                    ENHANCEMENT | ATTACK | ATTACK_PENALTY | TO_HIT_PENALTY
                )
            })
            .map(|p| p.flat)
            .sum()
    }

    /// What the gear adds to a hit as a plain number, and its dice, against anyone.
    pub(crate) fn gear_damage(&self, id: ObjId, slot: i32) -> (i32, Vec<(i32, i32)>) {
        let mut flat = 0;
        let mut dice = Vec::new();
        for p in self.attacking(id, slot) {
            match p.kind {
                ENHANCEMENT | DAMAGE_PENALTY => flat += p.flat,
                DAMAGE => {
                    flat += p.flat;
                    if p.dice.0 > 0 {
                        dice.push(p.dice);
                    }
                }
                _ => {}
            }
        }
        (flat, dice)
    }

    /// What the gear adds to a hit on this target in particular. A critical adds what a massive critical gives.
    fn gear_damage_on(
        &self,
        id: ObjId,
        slot: i32,
        target: ObjId,
        critical: bool,
    ) -> (i32, Vec<(i32, i32)>) {
        let race = self.objects.get(&target).map_or(-1, |o| o.race);
        let side = self.alignment_side(target);
        let mut flat = 0;
        let mut dice = Vec::new();
        for p in self.attacking(id, slot) {
            let counts = match p.kind {
                DAMAGE_VS_RACE => p.subtype == race,
                DAMAGE_VS_SIDE => p.subtype == side,
                MASSIVE_CRITICAL => critical,
                _ => false,
            };
            if counts {
                flat += p.flat;
                if p.dice.0 > 0 {
                    dice.push(p.dice);
                }
            }
        }
        (flat, dice)
    }

    /// Rolls the dice that gear adds to a hit, and what it adds for this target and this kind of hit.
    pub(crate) fn roll_gear_damage(
        &mut self,
        id: ObjId,
        slot: i32,
        target: ObjId,
        critical: bool,
    ) -> i32 {
        let (_, mut dice) = self.gear_damage(id, slot);
        let (mut total, more) = self.gear_damage_on(id, slot, target, critical);
        dice.extend(more);
        for (count, sides) in dice {
            for _ in 0..count {
                total += self.roll(sides);
            }
        }
        total
    }

    /// How much the gear widens the weapon's threat range.
    pub fn threat_factor(&self, id: ObjId, slot: i32) -> i32 {
        if self.attacking(id, slot).iter().any(|p| p.kind == KEEN) {
            2
        } else {
            1
        }
    }

    /// Whether a creature passes an item's own limits, by side, then class, race and feat.
    pub(crate) fn passes_item_limits(&self, who: ObjId, item: ObjId) -> (bool, bool) {
        let Some(c) = self.objects.get(&who) else {
            return (false, false);
        };
        let of = |kind: i32| -> Vec<i32> {
            self.working(item)
                .iter()
                .filter(|p| p.kind == kind)
                .map(|p| p.subtype)
                .collect()
        };
        (
            passes(&of(ONLY_SIDE), |s| side_allows(s, c.alignment)),
            passes(&of(ONLY_CLASS), |k| k == c.class)
                && passes(&of(ONLY_RACE), |r| r == c.race)
                && passes(&of(ONLY_FEAT), |f| c.feats.contains(&f)),
        )
    }

    /// Reads the files of the items there are, for what asks about them without being able to read.
    /// What kind of thing a carried item is, from `baseitems.2da`.
    fn base_label(&mut self, item: ObjId) -> String {
        let Some(row) = self.object(item).map(|o| o.base_item) else {
            return String::new();
        };
        self.table("baseitems")
            .and_then(|t| t.get(row.max(0) as usize, "label").map(str::to_string))
            .unwrap_or_default()
    }

    /// Whether someone may try a lock with the Security skill, and the spikes they carry. The talk table's row 256: points in
    /// the skill make it an option on a target, spikes carried are options too, and with no points they cannot try.
    pub fn lock_ways(&mut self, who: ObjId) -> (bool, Vec<ObjId>) {
        const SKILL_SECURITY: usize = 6;
        let trained = self.skill_ranks(who, SKILL_SECURITY) > 0;
        let carried = self.carried(who);
        let spikes = carried
            .into_iter()
            .filter(|i| self.base_label(*i) == "Security_Spikes")
            .collect();
        (trained, spikes)
    }

    /// What someone carries to use on themselves, for the quick slots: what heals, and then the rest. The video at 7:50 has
    /// a Medpac in the first of these and Adrenal Stamina in the next. **Assumed:** a repair kit goes with what heals.
    pub fn quick_items(&mut self, who: ObjId) -> [Vec<ObjId>; 2] {
        const HEALS: [&str; 3] = [
            "Medical_Equipment",
            "Squad_Recovery_kit",
            "Droid_Repair_Equipment",
        ];
        let thrown = self.combat_throwables(who);
        let mut out = [Vec::new(), Vec::new()];
        for item in self.carried(who) {
            let for_self = self
                .item_view(item)
                .is_some_and(|v| v.power.is_some() && v.slots == 0);
            if thrown.contains(&item) || !for_self {
                continue;
            }
            let label = self.base_label(item);
            out[usize::from(!HEALS.contains(&label.as_str()))].push(item);
        }
        out
    }

    pub(crate) fn remember_items(&mut self) {
        let templates: Vec<String> = self
            .objects
            .values()
            .filter(|o| o.kind == crate::world::Kind::Item)
            .map(|o| o.template.clone())
            .collect();
        for t in templates {
            self.item_template(&t);
        }
    }
}
