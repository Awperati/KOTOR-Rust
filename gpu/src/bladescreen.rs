//! Modern combat's blade: left click with a blade in hand in a fight. Clicks in time chain the five clips of the stance, at twice
//! their speed and cut at the blow. A blow lunges at its target and strikes everyone in an arc in front, by the ordinary rolls. See
//! `docs/MODERN_COMBAT.md`.
//!
//! **The tuning table, every number my guess but `HIT_SECONDS`, for the owner to set:**

use super::{ResType, Stage, TwoDa};
use kotor_game::ObjId;

/// How much faster than written the clips play.
const BLOW_RATE: f32 = 2.0;
/// How much of an ordinary attack's damage a blow does. One blow in about 0.5 s against one attack in 3 s, so a blow must do less.
const BLOW_SHARE: f32 = 0.25;
/// How long after a blow the next click goes on with the chain, in seconds.
const CHAIN_WINDOW: f32 = 0.7;
/// The arc a blow strikes: how far from the feet in metres, and how far each way from the facing in radians.
const ARC_REACH: f32 = 2.0;
const ARC_HALF: f32 = 1.05;
/// A lunge: who it goes for, in metres and radians off the view, where it stops short of them, and its most speed in m/s.
const LUNGE_REACH: f32 = 4.0;
const LUNGE_CONE: f32 = 1.2;
const LUNGE_STAND: f32 = 1.3;
const LUNGE_MOST: f32 = 9.0;
/// When the blade lands in each clip, in seconds as written: the clips' own `Hit` events, for stances 2, 3 and 4. Read from the
/// install. That the event is the moment the blade lands is assumed.
const HIT_SECONDS: [[f32; 5]; 3] = [
    [1.233, 0.967, 1.2, 0.833, 0.8],
    [1.233, 0.967, 1.2, 0.933, 0.8],
    [0.733, 0.967, 1.2, 0.933, 0.8],
];

/// Where the player is in a chain.
#[derive(Default)]
pub(super) struct Blows {
    /// The clip of the chain, 0 to 4, that plays next or plays now.
    step: usize,
    /// When the blade lands, if it is on its way.
    hit_at: Option<f32>,
    /// Until when the next click goes on with the chain.
    until: f32,
    /// A click is waiting for the blade to land.
    wanted: bool,
    facing: f32,
    stance: i32,
}

/// How far apart two directions are, from -pi to pi.
fn off_by(a: f32, b: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    (a - b + tau / 2.0).rem_euclid(tau) - tau / 2.0
}

impl Stage {
    /// A click in a fight with a blade in hand: a blow, and nothing else happens to it. In a pause it does nothing: there is no
    /// queue in Modern combat.
    pub(super) fn blow_click(&mut self) -> bool {
        if !self.in_fight_now() {
            return false;
        }
        let pc = self.game.world.pc;
        let Some(stance) = self.game.blade_stance(pc) else {
            return false;
        };
        // Behind a Block the blade does not strike.
        if !self.paused() && !self.game.world.pc_guard {
            self.blows.wanted = true;
            self.blows.stance = stance;
        }
        true
    }

    /// Each frame of the game.
    pub(super) fn blow_tick(&mut self) {
        if self.menu_open() || self.paused() {
            return;
        }
        let pc = self.game.world.pc;
        // Right click with a blade is the Block, and not the aim (`aimscreen::may_aim`): held, in a fight, in Modern combat.
        let block = self.aim_held && self.modern_mode() && self.in_fight_now();
        self.game.world.set_block(block);
        if !self.in_fight_now() {
            self.blows = Blows::default();
            self.game.world.pc_blowing = false;
            return;
        }
        let now = self.game.world.time;
        if self.blows.hit_at.is_some_and(|at| now >= at) {
            self.blows.hit_at = None;
            self.strike();
        }
        // The swing before it must have played out: a click in the meantime waits, once, and is not a second swing.
        if self.blows.hit_at.is_none() && self.blows.wanted && !self.game.world.attack_busy(pc) {
            self.blows.wanted = false;
            self.start_blow();
        }
        if self.blows.hit_at.is_some() {
            self.game.world.pc_blowing = true;
            self.game.world.pc_aim = Some(self.blows.facing);
        } else if now > self.blows.until {
            self.blows.step = 0;
            self.game.world.pc_blowing = false;
        }
    }

    /// What a blow goes for: the lock, or what the crosshair is on, or the nearest in front of the view.
    fn blow_target(&mut self) -> Option<ObjId> {
        if self.aim_on {
            let (eye, dir) = self.aim_ray();
            let reach = self.aim_reach(eye, dir);
            let on = self.game.world.aim_hit(eye, dir, reach).who;
            if on.is_some() {
                return on;
            }
        }
        let w = &self.game.world;
        let me = w.pc_mover.pos;
        let near = |id: ObjId| {
            w.object(id).map_or(f32::MAX, |o| {
                (o.position[0] - me[0]).hypot(o.position[1] - me[1])
            })
        };
        if let Some(id) = self.lock_on.filter(|id| w.may_lock(*id, LUNGE_REACH)) {
            return Some(id);
        }
        w.objects
            .values()
            .filter(|o| w.may_lock(o.id, LUNGE_REACH))
            .filter(|o| {
                let way = (o.position[1] - me[1]).atan2(o.position[0] - me[0]);
                off_by(way, self.yaw).abs() <= LUNGE_CONE
            })
            .map(|o| o.id)
            .min_by(|a, b| near(*a).total_cmp(&near(*b)))
    }

    /// The blade starts on its way: the clip, the turn and the lunge.
    fn start_blow(&mut self) {
        let target = self.blow_target();
        let aiming = self.aim_on;
        let (step, stance) = (self.blows.step, self.blows.stance);
        let w = &mut self.game.world;
        let pc = w.pc;
        let now = w.time;
        w.stop_attacking(pc);
        w.pc_mover.stop();
        let me = w.pc_mover.pos;
        let there = target.and_then(|t| w.object(t)).map(|o| o.position);
        let mut facing = self.yaw;
        if let Some(at) = there {
            let (dx, dy) = (at[0] - me[0], at[1] - me[1]);
            let way = dy.atan2(dx);
            if !aiming {
                facing = way;
            }
            let short = dx.hypot(dy) - LUNGE_STAND;
            if short > 0.0 {
                // As fast as stops it where it is to stop, given how the body slows.
                let speed = (2.0 * w.tuning.decel * short).sqrt().min(LUNGE_MOST);
                w.pc_body.vel = [way.cos() * speed, way.sin() * speed];
            }
        }
        self.game.play_blow(pc, stance, step, BLOW_RATE);
        self.game
            .world
            .commit_attack(pc, kotor_game::BLOW_CLIP_SECONDS / BLOW_RATE);
        self.game.hold_stance(pc);
        let row = (stance - 2).clamp(0, 2) as usize;
        let at = now + HIT_SECONDS[row][step] / BLOW_RATE;
        self.blows.hit_at = Some(at);
        self.blows.until = at + CHAIN_WINDOW;
        self.blows.facing = facing;
    }

    /// The blade lands: everyone in the arc, by the ordinary rolls.
    fn strike(&mut self) {
        let (step, facing) = (self.blows.step, self.blows.facing);
        self.blows.step = (step + 1) % HIT_SECONDS[0].len();
        let w = &mut self.game.world;
        let (pc, me) = (w.pc, w.pc_mover.pos);
        let struck: Vec<ObjId> = w
            .objects
            .values()
            .filter(|o| w.may_lock(o.id, ARC_REACH))
            .filter(|o| {
                let way = (o.position[1] - me[1]).atan2(o.position[0] - me[0]);
                off_by(way, facing).abs() <= ARC_HALF
            })
            .map(|o| o.id)
            .collect();
        for id in &struck {
            w.combat.last_hostile.insert(pc, *id);
            w.resolve_blow(pc, *id, step, BLOW_SHARE);
        }
        // A blow on someone has its sounds by the log: one on no one only cuts the air.
        if struck.is_empty() {
            self.swing_sound();
        }
    }

    fn swing_sound(&mut self) {
        let w = &self.game.world;
        let pc = w.pc;
        let at = w.pc_mover.pos;
        let base = w
            .object(pc)
            .and_then(|o| o.equipped.get(&super::RIGHT_HAND_SLOT))
            .and_then(|i| w.object(*i))
            .map(|i| i.base_item as usize);
        let row = base
            .and_then(|b| self.baseitem(b, "weaponmattype"))
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
        let swing = self
            .inst
            .read("weaponsounds", ResType::TWODA)
            .ok()
            .and_then(|b| TwoDa::parse(&b).ok())
            .and_then(|t| {
                t.get(row, "swingshort0")
                    .filter(|v| !v.is_empty())
                    .map(str::to_string)
            });
        if let Some(s) = swing {
            self.effect_from(&s, at);
        }
    }

    /// Which clip of the chain plays next, 0 to 4, for a test.
    pub fn blow_step(&self) -> usize {
        self.blows.step
    }
}
