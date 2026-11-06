//! What the player can queue in a fight: the feats that change the next attack and the Force powers that hurt. The
//! numbers are the talk table's own words for each feat.
//!
//! **Not built:** the stun on a critical from Critical Strike and Sniper Shot, and the Defense penalty of Flurry and Rapid
//! Shot. Their other effects are here. **Assumed:** a power is for an enemy when `spells.2da` fills its `forcehostile`.

use crate::world::{ObjId, World};

/// What the player has queued for their next attacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombatAction {
    Feat(i32),
    Power(i32),
}

/// One thing a target slot offers.
#[derive(Clone, Debug, PartialEq)]
pub struct CombatChoice {
    pub action: CombatAction,
    pub name: u32,
    pub icon: String,
}

/// How a feat changes one attack.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct FeatBoost {
    pub ranged: bool,
    pub to_hit: i32,
    pub damage: i32,
    pub threat: i32,
    pub extra_attack: bool,
}

const fn hard(ranged: bool, damage: i32) -> FeatBoost {
    FeatBoost {
        ranged,
        to_hit: -3,
        damage,
        threat: 1,
        extra_attack: false,
    }
}

const fn keen(ranged: bool, threat: i32) -> FeatBoost {
    FeatBoost {
        ranged,
        to_hit: 0,
        damage: 0,
        threat,
        extra_attack: false,
    }
}

const fn quick(ranged: bool, to_hit: i32) -> FeatBoost {
    FeatBoost {
        ranged,
        to_hit,
        damage: 0,
        threat: 1,
        extra_attack: true,
    }
}

/// Rows of `feat.2da`. Best first, so that a chain offers its strongest only.
const FEATS: [(i32, FeatBoost); 18] = [
    (83, hard(false, 10)),
    (17, hard(false, 8)),
    (28, hard(false, 5)),
    (82, hard(true, 10)),
    (18, hard(true, 8)),
    (29, hard(true, 5)),
    (81, keen(false, 4)),
    (19, keen(false, 3)),
    (8, keen(false, 2)),
    (77, keen(true, 4)),
    (20, keen(true, 3)),
    (31, keen(true, 2)),
    (53, quick(false, -1)),
    (91, quick(false, -2)),
    (11, quick(false, -4)),
    (26, quick(true, -1)),
    (92, quick(true, -2)),
    (30, quick(true, -4)),
];

/// The same kind of feat at another strength, as `FEATS` lists them in threes.
fn same_chain(a: usize, b: usize) -> bool {
    a / 3 == b / 3
}

pub(crate) fn feat_boost(feat: i32) -> FeatBoost {
    FEATS
        .iter()
        .find(|(f, _)| *f == feat)
        .map_or(FeatBoost::default(), |(_, b)| *b)
}

impl World {
    /// The feats a creature can use with what it holds, the strongest of each kind.
    pub fn combat_feats(&mut self, id: ObjId) -> Vec<CombatChoice> {
        let ranged = self.weapon_of(id).ranged;
        let Some(owned) = self.objects.get(&id).map(|o| o.feats.clone()) else {
            return Vec::new();
        };
        let Some(t) = self.table("feat") else {
            return Vec::new();
        };
        let mut out: Vec<(usize, CombatChoice)> = Vec::new();
        for (n, (feat, boost)) in FEATS.iter().enumerate() {
            let taken = out.iter().any(|(m, _)| same_chain(*m, n));
            if boost.ranged != ranged || !owned.contains(feat) || taken {
                continue;
            }
            let row = *feat as usize;
            out.push((
                n,
                CombatChoice {
                    action: CombatAction::Feat(*feat),
                    name: t.int(row, "name").unwrap_or(0) as u32,
                    icon: t.get(row, "icon").unwrap_or("").to_ascii_lowercase(),
                },
            ));
        }
        out.into_iter().map(|(_, c)| c).collect()
    }

    /// The powers a creature knows that are for an enemy and it has the Force for.
    pub fn combat_powers(&mut self, id: ObjId) -> Vec<CombatChoice> {
        let Some((known, force)) = self.objects.get(&id).map(|o| (o.spells.clone(), o.force))
        else {
            return Vec::new();
        };
        let Some(t) = self.table("spells") else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for spell in known {
            let row = spell.max(0) as usize;
            let cost = self.spell_cost_and_range(spell).map_or(i32::MAX, |c| c.0);
            if t.int(row, "forcehostile").is_some() && cost <= force {
                out.push(CombatChoice {
                    action: CombatAction::Power(spell),
                    name: t.int(row, "name").unwrap_or(0) as u32,
                    icon: t.get(row, "iconresref").unwrap_or("").to_ascii_lowercase(),
                });
            }
        }
        out
    }
}
