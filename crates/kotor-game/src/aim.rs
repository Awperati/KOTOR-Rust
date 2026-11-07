//! Modern combat's aimed shot, in the game's own terms: what a ray through the crosshair hits, and the shot itself. The window
//! gives the ray and how far the room lets it go. See `docs/MODERN_COMBAT.md`.
//!
//! **Assumed, all of it:** the body's shape and the reach. A shot on the body never misses outright: `resolve_aimed` rolls a
//! graze, a hit or a critical. The enemies' attacks keep their rounds and the near misses of the no-misses rule.

use crate::world::{Kind, ObjId, Object, World};

/// How far a shot goes, in metres, against 12 for a round's shot.
pub const AIM_REACH: f32 = 30.0;
/// A standing body, as a shot sees it: a pole this high and this thick round the feet.
const BODY_HIGH: f32 = 1.8;
const BODY_THICK: f32 = 0.4;
/// Where a shot leaves the player, over the feet.
const GUN_HIGH: f32 = 1.2;
/// The slot of the right hand, and the left's after it.
const RIGHT_HAND: i32 = 4;

/// How wide the crosshair drifts standing still, with a pistol and no feats, in radians. All the sizes here are my guesses.
const SWAY_STILL: f32 = 0.009;
/// How much wider for each m/s the body goes.
const SWAY_PER_SPEED: f32 = 0.6;
/// How much wider just after a shot, and how long that takes to go.
const SWAY_KICK: f32 = 2.0;
const SWAY_KICK_SECONDS: f32 = 0.6;
/// A rifle is steadier than a pistol, a heavy weapon less so, and a Weapon Focus or Specialization feat each takes a quarter off.
const SWAY_RIFLE: f32 = 0.7;
const SWAY_HEAVY: f32 = 1.4;
const SWAY_FEAT: f32 = 0.75;

/// Where the crosshair is on its drift at this time of the game, each way from -1 to 1. By the clock and not by dice: the same
/// time gives the same place.
pub fn sway_path(time: f32) -> (f32, f32) {
    (
        (time * 0.83).sin() * 0.7 + (time * 2.1 + 1.3).sin() * 0.3,
        (time * 1.13 + 0.7).sin() * 0.7 + (time * 2.7 + 2.1).sin() * 0.3,
    )
}

/// What a ray through the crosshair comes to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AimHit {
    /// A creature a shot would hit.
    pub who: Option<ObjId>,
    /// Where the ray stops, on the creature or as far as it was let go.
    pub point: [f32; 3],
}

/// How far along a ray of unit `dir` from `eye` it comes to a body standing at `feet`.
fn along_to_body(eye: [f32; 3], dir: [f32; 3], feet: [f32; 3]) -> Option<f32> {
    let (ox, oy) = (eye[0] - feet[0], eye[1] - feet[1]);
    let flat = dir[0] * dir[0] + dir[1] * dir[1];
    // Straight up or down, it meets the pole only if it starts in it.
    let t = if flat < 1e-6 {
        0.0
    } else {
        (-(ox * dir[0] + oy * dir[1]) / flat).max(0.0)
    };
    let off = (ox + dir[0] * t).hypot(oy + dir[1] * t);
    let high = eye[2] + dir[2] * t - feet[2];
    (off <= BODY_THICK && (0.0..=BODY_HIGH).contains(&high)).then_some(t)
}

impl World {
    /// Whether a shot may hit this creature: alive, on its feet and no friend. A neutral only with Attack mode on.
    fn shot_may_hit(&self, o: &Object) -> bool {
        o.id != self.pc
            && o.kind == Kind::Creature
            && !o.dead
            && !o.down
            && !self.on_player_team(o.id)
            && (self.attack_mode || self.reputation(self.pc, o.id) <= 10)
    }

    /// What a ray of unit `dir` through the crosshair comes to, within `reach` metres. A body behind the player, or with a wall
    /// or a closed door between it and the gun, is not hit.
    pub fn aim_hit(&self, eye: [f32; 3], dir: [f32; 3], reach: f32) -> AimHit {
        let reach = reach.min(AIM_REACH);
        let at = |t: f32| {
            [
                eye[0] + dir[0] * t,
                eye[1] + dir[1] * t,
                eye[2] + dir[2] * t,
            ]
        };
        let gun = self.pc_gun();
        let mut best: Option<(f32, ObjId)> = None;
        for o in self.objects.values().filter(|o| self.shot_may_hit(o)) {
            let ahead = (o.position[0] - gun[0]) * dir[0] + (o.position[1] - gun[1]) * dir[1];
            let Some(t) = along_to_body(eye, dir, o.position) else {
                continue;
            };
            if ahead > 0.0 && t <= reach && best.is_none_or(|(b, _)| t < b) {
                best = Some((t, o.id));
            }
        }
        let clear = |p: [f32; 3]| {
            self.nav
                .as_ref()
                .is_none_or(|nav| !nav.crosses_block(gun, p) && !nav.sight_blocked(gun, p))
        };
        match best.filter(|(t, _)| clear(at(*t))) {
            Some((t, id)) => AimHit {
                who: Some(id),
                point: at(t),
            },
            None => AimHit {
                who: None,
                point: at(reach),
            },
        }
    }

    /// How wide the player's crosshair drifts now, in radians, `since_shot` seconds after the last shot.
    pub fn sway_size(&mut self, since_shot: f32) -> f32 {
        let pc = self.pc;
        let base = self
            .objects
            .get(&pc)
            .and_then(|o| o.equipped.get(&RIGHT_HAND))
            .and_then(|i| self.objects.get(i))
            .map(|i| i.base_item.max(0) as usize);
        let wield = base.and_then(|b| self.table("baseitems")?.int(b, "weaponwield"));
        let weapon = match wield {
            Some(4) => 1.0,
            Some(6) => SWAY_HEAVY,
            _ => SWAY_RIFLE,
        };
        let mut feats = 1.0;
        if self.weapon_focus_bonus(pc, RIGHT_HAND) > 0 {
            feats *= SWAY_FEAT;
        }
        if self.weapon_spec_bonus(pc, RIGHT_HAND) > 0 {
            feats *= SWAY_FEAT;
        }
        // A save from earlier in the game can put the last shot ahead of the clock: no kick from that.
        let kick = if since_shot < 0.0 {
            0.0
        } else {
            (1.0 - since_shot / SWAY_KICK_SECONDS).max(0.0)
        };
        let moving = 1.0 + self.pc_body.speed() * SWAY_PER_SPEED + kick * SWAY_KICK;
        SWAY_STILL * weapon * feats * moving
    }

    /// Where a shot leaves the player.
    pub fn pc_gun(&self) -> [f32; 3] {
        let p = self.pc_mover.pos;
        let h = self.pc_mover.heading;
        [p[0] + h.cos() * 0.5, p[1] + h.sin() * 0.5, p[2] + GUN_HIGH]
    }

    /// The player's aimed shot, by a click. It says where it went, or nothing with no blaster in hand. Each weapon in hand
    /// fires. The window keeps the pace.
    pub fn fire_aimed(&mut self, eye: [f32; 3], dir: [f32; 3], reach: f32) -> Option<AimHit> {
        let pc = self.pc;
        if !self.weapon_of(pc).ranged {
            return None;
        }
        let hit = self.aim_hit(eye, dir, reach);
        if let Some(who) = hit.who {
            self.combat.last_hostile.insert(pc, who);
            self.resolve_aimed(pc, who, RIGHT_HAND);
            let standing = self.objects.get(&who).is_some_and(|o| !o.dead && !o.down);
            if self.hand_weapon(pc, RIGHT_HAND + 1).is_some() && standing {
                self.resolve_aimed(pc, who, RIGHT_HAND + 1);
            }
        }
        Some(hit)
    }
}
