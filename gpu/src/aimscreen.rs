//! Modern combat's aim: the right button held in a fight, with an enemy in sight, or with Attack mode on. The camera comes in
//! over the right shoulder, the view narrows, the body turns to the aim, and a crosshair shows. See `docs/MODERN_COMBAT.md`.
//!
//! **Assumed, all of it:** the sizes below. The game has no such thing to measure. It also has no drawn weapon to go by, so a
//! fight, a seen enemy or Attack mode stand for one.

use super::Stage;
use kotor_ui::Crosshair;

/// How fast the camera goes in and out, a second.
pub(super) const AIM_RATE: f32 = 12.0;
/// How far right of the player the camera stands, in metres.
pub(super) const AIM_SHOULDER: f32 = 0.55;
/// How far back it stands, in metres.
const AIM_ARM: f32 = 2.0;
/// How high it looks, over the feet, in metres.
const AIM_LOOKS_AT: f32 = 1.5;
/// How much narrower the view is, in degrees.
const AIM_NARROWER: f32 = 10.0;
/// How far down the mouse may look, and up, while aiming. Negative `pitch` puts the camera under the player's head.
pub(super) const AIM_PITCH: (f32, f32) = (-0.55, 0.9);
/// How much slower the mouse turns while aiming.
pub(super) const AIM_MOUSE: f32 = 0.6;
/// The lowest `pitch` when not aiming, as `Stage::look` has it, and how fast the camera comes back to it.
const PLAIN_LOWEST: f32 = -0.2;
const PITCH_BACK_RATE: f32 = 6.0;
const LAYOUT_HIGH: i32 = 480;
/// The least time between aimed shots, in seconds. `kotor_game::fire_times` has the clips' shots about this far apart.
const SHOT_SECONDS: f32 = 0.5;
/// Where a bolt goes from over the player's middle, and how far short of where the ray ends, in metres.
const BOLT_SHORT: f32 = 0.05;

impl Stage {
    /// The aim button, down or up. It is the look button: holding it does nothing new until aiming may begin.
    pub fn set_aim_held(&mut self, held: bool) {
        self.aim_held = held;
    }

    /// Whether the player is aiming now. Clicks fire, and the window holds the cursor.
    pub fn aiming(&self) -> bool {
        self.aim_on
    }

    /// How far the camera has gone from plain to aiming, 0 to 1.
    pub fn aim_share(&self) -> f32 {
        self.aim_share
    }

    /// Whether the aim can begin or go on now.
    fn may_aim(&self) -> bool {
        if !self.aim_held || !self.modern_combat() || self.menu_open() {
            return false;
        }
        let w = &self.game.world;
        let busy = w.in_dialogue()
            || w.opened.is_some()
            || w.game_over
            || self.turret_on()
            || w.in_race()
            || self.beat_camera().is_some()
            || w.object(w.pc).is_none_or(|o| o.down || o.dead);
        !busy && (self.party_blocked() || w.attack_mode() || self.seen_enemy().is_some())
    }

    /// Each frame of the game.
    pub(super) fn aim_tick(&mut self, dt: f32) {
        let on = self.may_aim();
        if on && !self.aim_on {
            // A click's walk ends: the keys steer from here.
            self.game.world.pc_mover.stop();
        }
        self.aim_on = on;
        self.aim_hot = false;
        let want = if on { 1.0 } else { 0.0 };
        self.aim_share += (want - self.aim_share) * (1.0 - (-AIM_RATE * dt).exp());
        if self.aim_share < 0.001 {
            self.aim_share = 0.0;
        }
        self.game.world.pc_aim = on.then_some(self.yaw);
        if on {
            let pc = self.game.world.pc;
            self.game.hold_stance(pc);
            // Rounds are not for the player while aiming: each shot is a click.
            self.game.world.stop_attacking(pc);
            let (eye, dir) = self.aim_ray();
            let reach = self.aim_reach(eye, dir);
            let who = self.game.world.aim_hit(eye, dir, reach).who;
            self.hover_target = who;
            self.aim_hot = who.is_some();
        } else if self.pitch < PLAIN_LOWEST {
            self.pitch += (PLAIN_LOWEST - self.pitch) * (1.0 - (-PITCH_BACK_RATE * dt).exp());
        }
    }

    /// The ray through the crosshair: where it starts, and its unit direction.
    pub fn aim_ray(&self) -> ([f32; 3], [f32; 3]) {
        let c = self.camera();
        let d = [
            c.target[0] - c.eye[0],
            c.target[1] - c.eye[1],
            c.target[2] - c.eye[2],
        ];
        let long = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-4);
        (c.eye, [d[0] / long, d[1] / long, d[2] / long])
    }

    /// Whether the crosshair lies on what a shot would hit.
    pub fn aim_hot(&self) -> bool {
        self.aim_hot
    }

    /// How far the ray goes before the room stops it.
    pub fn aim_reach(&self, eye: [f32; 3], dir: [f32; 3]) -> f32 {
        self.shell
            .first_hit(eye, dir, kotor_game::AIM_REACH)
            .unwrap_or(kotor_game::AIM_REACH)
    }

    /// A click while aiming fires, at the pace of the weapon. A blade or a bare hand does nothing yet. Not in a pause: there is
    /// no queue in Modern combat.
    pub(super) fn aim_click(&mut self) {
        let now = self.game.world.time;
        // A save from earlier in the game puts the clock back: a wait longer than one shot's is left from before it.
        let waiting = now < self.aim_next_shot && self.aim_next_shot - now <= SHOT_SECONDS;
        if self.paused() || waiting {
            return;
        }
        let (eye, dir) = self.aim_ray();
        let reach = self.aim_reach(eye, dir);
        let Some(hit) = self.game.world.fire_aimed(eye, dir, reach) else {
            return;
        };
        self.aim_next_shot = now + SHOT_SECONDS;
        let w = &self.game.world;
        let (gun, pc) = (w.pc_gun(), w.pc);
        let to = [
            hit.point[0] - dir[0] * BOLT_SHORT,
            hit.point[1] - dir[1] * BOLT_SHORT,
            hit.point[2] - dir[2] * BOLT_SHORT,
        ];
        if let Some((bolt, color, shot)) = self.gun_look(pc) {
            self.effects.fire(gun, to, &bolt, color);
            if let Some(s) = shot {
                self.effect_from(&s, gun);
            }
        }
    }

    /// How high the camera looks, over the feet.
    pub(super) fn looks_at(&self) -> f32 {
        super::CAMERA_LOOKS_AT + (AIM_LOOKS_AT - super::CAMERA_LOOKS_AT) * self.aim_share
    }

    /// How much the aim takes off the view.
    pub(super) fn aim_narrows(&self) -> f32 {
        AIM_NARROWER * self.aim_share
    }

    /// How far the arm may reach as the aim comes in.
    pub(super) fn arm_reach(&self) -> f32 {
        self.distance + (AIM_ARM.min(self.distance) - self.distance) * self.aim_share
    }

    /// What the HUD draws for the aim.
    pub(super) fn crosshair_view(&self) -> Option<Crosshair> {
        self.aim_on.then_some(Crosshair {
            x: self.ui_width as i32 / 2,
            y: LAYOUT_HIGH / 2,
            hot: self.aim_hot,
        })
    }
}
