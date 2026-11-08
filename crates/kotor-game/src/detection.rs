//! How far each enemy has noticed the player's side, for the detection bars on the screen and for when attack mode ends.
//!
//! Each enemy has a number from 0 to 1. It rises while the enemy perceives the leader or a companion (range, a clear line
//! and the Stealth check are in [`World::update_perception`]), and falls when it does not. At 1 the enemy is alerted. An
//! enemy that is attacking one of the party is at 1. Attack mode goes off by itself only when nobody is fighting and every
//! number has fallen to 0, and then for [`crate::stance::AUTO_OFF_SECONDS`].
//!
//! **Assumed:** the game's AI is not delayed by the number: an enemy that perceives the party reacts at once, as it always
//! did, so the bar mostly fills while it is already coming. Closer is faster: `FILL_SECONDS_NEAR` beside the enemy to
//! `FILL_SECONDS_FAR` at the end of its perception range.
//!
//! **Sound** scales the rate ([`World::noise_rate`]): standing still is quiet, running is loud, a roll louder, a fight loudest,
//! and Stealth mode muffles all of it. **Light:** the game's files give no light level for a place, so there is no light
//! factor. That is left out on purpose, not guessed.
use crate::world::{Kind, ObjId, World};
use std::collections::BTreeMap;

/// Seconds to fill from empty to alerted with the party beside the enemy.
pub const FILL_SECONDS_NEAR: f32 = 0.8;
/// The same at the end of the enemy's perception range.
pub const FILL_SECONDS_FAR: f32 = 3.5;
/// Seconds to drain from full to empty once the enemy no longer perceives the party.
pub const DRAIN_SECONDS: f32 = 3.0;
/// How fast the bar fills while the leader stands still, walks (under `RUN_NOISE_SPEED`), runs, rolls, or the party fights.
pub const NOISE_STILL: f32 = 0.7;
pub const NOISE_WALK: f32 = 1.0;
pub const NOISE_RUN: f32 = 1.3;
pub const NOISE_ROLL: f32 = 1.6;
pub const NOISE_FIGHT: f32 = 1.8;
/// Stealth mode multiplies the rate by this.
pub const NOISE_STEALTH: f32 = 0.5;
/// Leader speed (m/s) under which they are standing still, and from which they are running.
const STILL_SPEED: f32 = 0.3;
const RUN_NOISE_SPEED: f32 = 3.5;
/// Reputation at or under which a creature is an enemy.
const ENEMY: i32 = 10;

/// One enemy's bar, for the screen.
#[derive(Clone, Debug, PartialEq)]
pub struct DetectionMark {
    pub who: ObjId,
    /// 0 to 1.
    pub fill: f32,
    pub position: [f32; 3],
    /// Whether it perceives the party now (a bar that only drains is fading).
    pub sees: bool,
}

impl World {
    /// The enemies, how far each has noticed the party, once a frame.
    pub(crate) fn update_detection(&mut self, dt: f32) {
        let group: Vec<ObjId> = std::iter::once(self.pc)
            .chain(self.party.iter().copied())
            .filter(|id| self.objects.get(id).is_some_and(|o| !o.dead))
            .collect();
        let noise = self.noise_rate();
        let mut next: BTreeMap<ObjId, f32> = BTreeMap::new();
        let enemies: Vec<(ObjId, [f32; 3], f32)> = self
            .objects
            .values()
            .filter(|o| o.kind == Kind::Creature && !o.dead && !group.contains(&o.id))
            .map(|o| (o.id, o.position, o.perception))
            .collect();
        for (a, apos, range) in enemies {
            let hostile = group.iter().any(|b| self.reputation(a, *b) <= ENEMY);
            if !hostile {
                continue;
            }
            let attacking = self
                .combat
                .target
                .get(&a)
                .is_some_and(|t| group.contains(t));
            let nearest = group
                .iter()
                .filter(|b| self.combat.perceived.contains(&(a, **b)))
                .filter_map(|b| self.objects.get(b))
                .map(|o| crate::follow::dist2d(apos, o.position))
                .fold(None, |m: Option<f32>, d| Some(m.map_or(d, |m| m.min(d))));
            let was = self.detection.get(&a).copied().unwrap_or(0.0);
            let now = if attacking {
                1.0
            } else if let Some(d) = nearest {
                let far = (d / range.max(1.0)).clamp(0.0, 1.0);
                let seconds = FILL_SECONDS_NEAR + (FILL_SECONDS_FAR - FILL_SECONDS_NEAR) * far;
                (was + dt * noise / seconds).min(1.0)
            } else {
                (was - dt / DRAIN_SECONDS).max(0.0)
            };
            if now > 0.0 {
                next.insert(a, now);
            }
        }
        self.detection = next;
    }

    /// How loud the party is now, as a multiplier on how fast a bar fills.
    pub fn noise_rate(&self) -> f32 {
        let fighting = std::iter::once(self.pc)
            .chain(self.party.iter().copied())
            .any(|b| self.combat.in_combat(b));
        let speed = self.pc_body.speed();
        let base = if fighting {
            NOISE_FIGHT
        } else if self.pc_body.rolling() {
            NOISE_ROLL
        } else if speed >= RUN_NOISE_SPEED {
            NOISE_RUN
        } else if speed < STILL_SPEED {
            NOISE_STILL
        } else {
            NOISE_WALK
        };
        if self.stealth_mode {
            base * NOISE_STEALTH
        } else {
            base
        }
    }

    /// Whether any enemy has noticed the party, even a little: it has not gone cold yet.
    pub fn is_watched(&self) -> bool {
        self.detection.values().any(|f| *f > 0.0)
    }

    /// How far an enemy has noticed the party: 0 to 1.
    pub fn detection_of(&self, enemy: ObjId) -> f32 {
        self.detection.get(&enemy).copied().unwrap_or(0.0)
    }

    /// Every enemy that has noticed the party, for the bars on the screen.
    pub fn detection_marks(&self) -> Vec<DetectionMark> {
        self.detection
            .iter()
            .filter_map(|(id, fill)| {
                let o = self.objects.get(id)?;
                let sees = self
                    .party
                    .iter()
                    .copied()
                    .chain(std::iter::once(self.pc))
                    .any(|b| self.combat.perceived.contains(&(*id, b)));
                Some(DetectionMark {
                    who: *id,
                    fill: *fill,
                    position: o.position,
                    sees,
                })
            })
            .collect()
    }
}
