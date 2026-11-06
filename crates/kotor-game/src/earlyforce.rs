//! Early Force sensitivity, the owner's change: see `docs/EARLYFORCE.md`. All the numbers here are proposals, not the game's.

use crate::combat::ability_mod;
use crate::world::{ObjId, World};

/// Force Push in `spells.2da`.
pub const FORCE_PUSH: i32 = 23;
const WISDOM: usize = 4;
const START_POINTS: i32 = 12;
const POINTS_A_LEVEL: i32 = 2;
/// A point comes back in this many seconds outside a fight.
const SECONDS_A_POINT: f32 = 2.0;
/// One power at every level that divides by this.
const LEVELS_A_PICK: i32 = 2;

fn is_jedi(class: i32) -> bool {
    (3..=5).contains(&class)
}

impl World {
    /// Whether the early rules hold for this creature now: the main character, in a game with them on, and no Jedi yet.
    pub fn is_early(&self, id: ObjId) -> bool {
        self.early_force && id == self.hero && self.object(id).is_some_and(|o| !is_jedi(o.class))
    }

    fn wisdom_bonus(&mut self, id: ObjId) -> i32 {
        ability_mod(self.ability_score(id, WISDOM)).max(0)
    }

    /// What a new character starts with under the early rules.
    pub(crate) fn start_early(&mut self, id: ObjId) {
        if !self.is_early(id) {
            return;
        }
        let points = START_POINTS + self.wisdom_bonus(id);
        if let Some(o) = self.object_mut(id) {
            o.max_force = points;
            o.force = points;
            o.spells.insert(FORCE_PUSH);
        }
    }

    /// Force points a level gives under the early rules.
    pub(crate) fn early_points_a_level(&mut self, id: ObjId) -> i32 {
        POINTS_A_LEVEL + self.wisdom_bonus(id)
    }

    /// Force powers this creature chooses at a level.
    pub fn power_slots_for(&mut self, id: ObjId, level: i32) -> i32 {
        if self.is_early(id) {
            return (level % LEVELS_A_PICK == 0) as i32;
        }
        let Some(class) = self.object(id).map(|o| o.class) else {
            return 0;
        };
        self.power_slots_at(class, level)
    }

    /// What a power costs this creature: under the early rules a power of the other side costs double.
    /// **Assumption:** the factor, and the line at alignment 50.
    pub fn power_cost(&mut self, id: ObjId, spell: i32) -> Option<i32> {
        let (cost, _) = self.spell_cost_and_range(spell)?;
        if !self.early_force || id != self.hero {
            return Some(cost);
        }
        let side = self
            .table("spells")
            .and_then(|t| t.get(spell.max(0) as usize, "goodevil").map(str::to_string))
            .unwrap_or_default();
        let light = self.object(id).map_or(50, |o| o.alignment) >= 50;
        let other =
            (light && side.eq_ignore_ascii_case("E")) || (!light && side.eq_ignore_ascii_case("G"));
        Some(if other { cost * 2 } else { cost })
    }

    /// `AddMultiClass`: this creature is of this class from now on. Under the early rules what it has carries over and
    /// nothing is paid twice.
    pub(crate) fn add_multi_class(&mut self, id: ObjId, class: i32) {
        let early = self.is_early(id);
        let first = self.force_for_level(class, 1);
        let Some(o) = self.object_mut(id) else {
            return;
        };
        o.class = class;
        if early {
            let raised = (first - o.max_force).max(0);
            o.max_force += raised;
            o.force += raised;
        } else {
            o.max_force += first;
            o.force += first;
        }
    }

    /// The Force comes back slowly outside a fight, for the main character of a game with the early rules.
    pub(crate) fn update_early_force(&mut self, dt: f32) {
        if !self.early_force || !self.fight_is_over() {
            return;
        }
        let hero = self.hero;
        let Some(short) = self.object(hero).map(|o| o.force < o.max_force) else {
            return;
        };
        if !short {
            self.force_regen = 0.0;
            return;
        }
        self.force_regen += dt;
        if self.force_regen >= SECONDS_A_POINT {
            self.force_regen -= SECONDS_A_POINT;
            if let Some(o) = self.object_mut(hero) {
                o.force = (o.force + 1).min(o.max_force);
            }
        }
    }
}
