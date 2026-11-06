//! What the slots under an enemy's bar offer, as the owner has them: on the left the feats that change the next attack, in
//! the middle the Force powers that hurt, on the right what can be thrown. The numbers are the talk table's own words for
//! each feat.
//!
//! A power is for an enemy when `spells.2da` fills its `forcehostile`, as a test checks. **Assumed:** "while using this feat" in Flurry's
//! words is the combat round.

use crate::world::{ObjId, World};

/// The rows of `baseitems.2da` of the game's grenades and its thermal detonator, read from their item files.
const BASE_THROWN: std::ops::RangeInclusive<i32> = 25..=34;

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
    /// False when it is shown greyed: a power too dear for now.
    pub usable: bool,
}

/// How a feat changes one attack.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct FeatBoost {
    pub ranged: bool,
    pub to_hit: i32,
    pub damage: i32,
    pub threat: i32,
    pub extra_attack: bool,
    /// A hit stuns for `STUN_SECONDS` unless the target saves against the level plus this ability's modifier.
    pub stun_ability: Option<usize>,
    /// Off Defense while the feat is used and for `DEFENSE_SECONDS` afterwards.
    pub defense: i32,
}

/// Places in `abilities`, as the feats' words name them.
const STRENGTH: usize = 0;
const INTELLIGENCE: usize = 3;
pub(crate) const STUN_SECONDS: f32 = 6.0;
pub(crate) const DEFENSE_SECONDS: f32 = 3.0;

const fn hard(ranged: bool, damage: i32) -> FeatBoost {
    FeatBoost {
        ranged,
        to_hit: -3,
        damage,
        threat: 1,
        extra_attack: false,
        stun_ability: None,
        defense: 0,
    }
}

const fn keen(ranged: bool, threat: i32) -> FeatBoost {
    FeatBoost {
        ranged,
        to_hit: 0,
        damage: 0,
        threat,
        extra_attack: false,
        stun_ability: Some(if ranged { INTELLIGENCE } else { STRENGTH }),
        defense: 0,
    }
}

const fn quick(ranged: bool, to_hit: i32) -> FeatBoost {
    FeatBoost {
        ranged,
        to_hit,
        damage: 0,
        threat: 1,
        extra_attack: true,
        stun_ability: None,
        defense: -to_hit,
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
                    usable: true,
                },
            ));
        }
        out.into_iter().map(|(_, c)| c).collect()
    }

    /// What a creature carries that it can throw at an enemy with `use_item_on`. One with no power cannot be used: the
    /// flash grenade's file has none.
    pub fn combat_throwables(&mut self, id: ObjId) -> Vec<ObjId> {
        let thrown: Vec<ObjId> = self
            .carried(id)
            .into_iter()
            .filter(|i| {
                self.object(*i)
                    .is_some_and(|o| BASE_THROWN.contains(&o.base_item))
            })
            .collect();
        thrown
            .into_iter()
            .filter(|i| self.item_view(*i).is_some_and(|v| v.power.is_some()))
            .collect()
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
            let cost = self.power_cost(id, spell).unwrap_or(i32::MAX);
            // Under the early rules one too dear stays on its slot, greyed. As the game has it, it goes.
            let usable = cost <= force;
            if t.int(row, "forcehostile").is_some() && (usable || self.is_early(id)) {
                out.push(CombatChoice {
                    action: CombatAction::Power(spell),
                    name: t.int(row, "name").unwrap_or(0) as u32,
                    icon: t.get(row, "iconresref").unwrap_or("").to_ascii_lowercase(),
                    usable,
                });
            }
        }
        out
    }
}
