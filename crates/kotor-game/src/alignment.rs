//! Light and dark, and how a faction feels about someone. `AdjustAlignment` and `AdjustReputation` say what they do in
//! `nwscript.nss`: neutral moves towards 50 and stops there, and a faction's change is for the whole faction.
//!
//! The talk table has the rule for what powers cost, at 41887: "The further you stray to the dark side, the more expensive
//! light side powers become, and the less expensive dark side powers become and vice versa."
//! **Assumed:** the numbers. At 100 or 0 a power of your side costs half and one of the other side double, in a straight line
//! from 50. The words say nothing of how far. Also that `AdjustReputation` works both ways, as nothing else says how the
//! player then sees them, and that the party goes with the player. **Not compared:** I have no frame of a power's cost.

use crate::world::{ObjId, World};

pub const ALIGNMENT_NEUTRAL_VALUE: i32 = 50;
const ALIGNMENT_TOP: i32 = 100;
/// `ALIGNMENT_` in `nwscript.nss`.
const ALIGNMENT_NEUTRAL: i32 = 1;
const ALIGNMENT_LIGHT_SIDE: i32 = 2;
const ALIGNMENT_DARK_SIDE: i32 = 3;
/// `GetAlignmentGoodEvil`'s lines. **Assumed:** where the sheet's mist changes. No stock script asks it.
const LIGHT_FROM: i32 = 60;
const DARK_FROM: i32 = 40;

impl World {
    /// `AdjustAlignment`.
    pub fn adjust_alignment(&mut self, who: ObjId, side: i32, shift: i32) {
        let Some(o) = self.object_mut(who) else {
            return;
        };
        let to_middle = shift.min((o.alignment - ALIGNMENT_NEUTRAL_VALUE).abs());
        o.alignment = match side {
            ALIGNMENT_LIGHT_SIDE => o.alignment + shift,
            ALIGNMENT_DARK_SIDE => o.alignment - shift,
            ALIGNMENT_NEUTRAL if o.alignment > ALIGNMENT_NEUTRAL_VALUE => o.alignment - to_middle,
            ALIGNMENT_NEUTRAL => o.alignment + to_middle,
            _ => o.alignment,
        }
        .clamp(0, ALIGNMENT_TOP);
    }

    /// `GetAlignmentGoodEvil`.
    pub(crate) fn alignment_side(&self, who: ObjId) -> i32 {
        match self.object(who).map(|o| o.alignment) {
            None => -1,
            Some(a) if a >= LIGHT_FROM => ALIGNMENT_LIGHT_SIDE,
            Some(a) if a <= DARK_FROM => ALIGNMENT_DARK_SIDE,
            Some(_) => ALIGNMENT_NEUTRAL,
        }
    }

    /// What a power costs this creature, as far as it is from the middle.
    pub fn power_cost(&mut self, id: ObjId, spell: i32) -> Option<i32> {
        let (cost, _) = self.spell_cost_and_range(spell)?;
        let side = self
            .table("spells")
            .and_then(|t| t.get(spell.max(0) as usize, "goodevil").map(str::to_string))
            .unwrap_or_default();
        let from_middle = self
            .object(id)
            .map_or(0, |o| o.alignment - ALIGNMENT_NEUTRAL_VALUE);
        let leaning = side_leaning(&side) * from_middle;
        let half_way = ALIGNMENT_TOP - ALIGNMENT_NEUTRAL_VALUE;
        Some(if leaning >= 0 {
            cost - cost * leaning / (half_way * 2)
        } else {
            cost - cost * leaning / half_way
        })
    }

    /// `AdjustReputation`.
    pub fn adjust_reputation(&mut self, target: ObjId, source: ObjId, by: i32) {
        let (Some(s), Some(t)) = (self.object(source), self.object(target)) else {
            return;
        };
        let key = (
            self.reputation_side(source, s.faction),
            self.reputation_side(target, t.faction),
        );
        *self.reputation_shifts.entry(key).or_insert(0) += by;
    }

    /// How far a faction has changed its mind about another.
    pub(crate) fn reputation_shift(&self, source: (ObjId, i32), target: (ObjId, i32)) -> i32 {
        if self.reputation_shifts.is_empty() {
            return 0;
        }
        let (s, t) = (
            self.reputation_side(source.0, source.1),
            self.reputation_side(target.0, target.1),
        );
        self.reputation_shifts
            .get(&(s, t))
            .or_else(|| self.reputation_shifts.get(&(t, s)))
            .copied()
            .unwrap_or(0)
    }

    /// The faction someone counts as: the party has its own, but is on the player's side.
    fn reputation_side(&self, id: ObjId, faction: i32) -> i32 {
        if id == self.hero || id == self.pc || self.party.contains(&id) {
            self.object(self.hero).map_or(faction, |o| o.faction)
        } else {
            faction
        }
    }
}

/// `spells.2da`'s `goodevil`: 1 for a light power, -1 for a dark one, 0 for neither.
fn side_leaning(side: &str) -> i32 {
    if side.eq_ignore_ascii_case("G") {
        1
    } else if side.eq_ignore_ascii_case("E") {
        -1
    } else {
        0
    }
}
