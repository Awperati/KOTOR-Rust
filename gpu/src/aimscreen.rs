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
        let want = if on { 1.0 } else { 0.0 };
        self.aim_share += (want - self.aim_share) * (1.0 - (-AIM_RATE * dt).exp());
        if self.aim_share < 0.001 {
            self.aim_share = 0.0;
        }
        self.game.world.pc_aim = on.then_some(self.yaw);
        if on {
            let pc = self.game.world.pc;
            self.game.hold_stance(pc);
        } else if self.pitch < PLAIN_LOWEST {
            self.pitch += (PLAIN_LOWEST - self.pitch) * (1.0 - (-PITCH_BACK_RATE * dt).exp());
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
            hot: false,
        })
    }
}
