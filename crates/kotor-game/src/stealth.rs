//! Stealth mode (GAPS row 15): a player who wears a stealth unit can sneak, and an enemy that would notice them must
//! pass an Awareness check against their Stealth skill first.
//!
//! **Read from the install:** the skills' words. Stealth: "If a character equips such a device and activates Stealth Mode,
//! opponents must make an Awareness check versus the character's Stealth skill or be unable to detect them, even if the
//! character is in a direct line of sight. ... Combat cancels Stealth Mode. Stealth Mode will not work without a
//! camouflage unit." The unit is base item 88, `Stealth_Unit` in `baseitems.2da`; the skills are rows 2 (Stealth) and 3
//! (Awareness) of `skills.2da`.
//! **Assumed, not measured:** the check is d20 + the watcher's Awareness against the player's whole Stealth skill, made
//! once for each watcher every `LOOK_SECONDS`; "invisibility" (the effect) is not part of this. **Not built:** the button
//! and key that turn it on in the interface, and the camouflage's own look.
//!
//! Stealth XP (`AwardStealthXP`, `GetCurrentStealthXP` and the rest) is a pool the scripts fill: this keeps the numbers and
//! pays the pool to the party when a script asks, **assumed** to mean everything in it, once.

use crate::world::{ObjId, World};

/// The skill rows in `skills.2da`.
const STEALTH: usize = 2;
const AWARENESS: usize = 3;
/// A watcher who failed its check tries again after this long. **Assumed.**
const LOOK_SECONDS: f32 = 2.0;

/// The stealth XP pool the area's scripts keep.
#[derive(Clone, Copy, Debug, Default)]
pub struct StealthXp {
    pub enabled: bool,
    pub current: i32,
    pub max: i32,
    pub decrement: i32,
}

impl World {
    /// Whether the creature wears a stealth unit.
    pub fn has_stealth_unit(&mut self, id: ObjId) -> bool {
        let bases: Vec<i32> = self
            .objects
            .get(&id)
            .map(|o| o.equipped.values().copied().collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|i| self.objects.get(&i).map(|i| i.base_item))
            .collect();
        let Some(t) = self.table("baseitems") else {
            return false;
        };
        bases.into_iter().any(|b| {
            t.get(b.max(0) as usize, "label")
                .is_some_and(|l| l.eq_ignore_ascii_case("Stealth_Unit"))
        })
    }

    /// Turn the player's Stealth Mode on or off. On needs a stealth unit and no fight.
    pub fn set_stealth(&mut self, on: bool) -> Result<(), &'static str> {
        let pc = self.pc;
        if on {
            if !self.has_stealth_unit(pc) {
                return Err("Stealth Mode needs a stealth unit.");
            }
            if self.in_combat(pc) {
                return Err("Not in combat.");
            }
        }
        if self.stealth_mode != on {
            self.stealth_mode = on;
            self.stealth_looks.clear();
            self.events
                .push(format!("stealth mode {}", if on { "on" } else { "off" }));
        }
        Ok(())
    }

    /// Combat and taking off the unit end Stealth Mode.
    pub(crate) fn update_stealth(&mut self) {
        if !self.stealth_mode {
            return;
        }
        let pc = self.pc;
        if self.in_combat(pc) || !self.has_stealth_unit(pc) {
            self.stealth_mode = false;
            self.stealth_looks.clear();
            self.events.push("stealth mode off".into());
        }
    }

    /// Whether a watcher notices the player right now: always when the player is not sneaking, else by the check.
    pub(crate) fn stealth_lets_notice(&mut self, watcher: ObjId) -> bool {
        if !self.stealth_mode {
            return true;
        }
        if let Some((until, noticed)) = self.stealth_looks.get(&watcher).copied() {
            if self.time < until {
                return noticed;
            }
        }
        let pc = self.pc;
        let stealth = self.skill_ranks(pc, STEALTH) + self.skill_modifier(pc, STEALTH);
        let awareness =
            self.skill_ranks(watcher, AWARENESS) + self.skill_modifier(watcher, AWARENESS);
        let noticed = self.roll(20) + awareness >= stealth;
        self.stealth_looks
            .insert(watcher, (self.time + LOOK_SECONDS, noticed));
        noticed
    }

    /// Pay the stealth pool to the party and empty it, if the scripts have it on.
    pub fn award_stealth_xp(&mut self) -> i32 {
        let pool = self.stealth_xp;
        if !pool.enabled || pool.current <= 0 {
            return 0;
        }
        let amount = pool
            .current
            .min(if pool.max > 0 { pool.max } else { pool.current });
        self.stealth_xp.current = 0;
        for id in self.standing_party() {
            for up in self.grant_xp(id, amount) {
                self.events
                    .push(format!("{} reaches level {}", self.describe(id), up.level));
            }
        }
        amount
    }
}
