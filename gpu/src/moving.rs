//! The window's side of Modern movement: the camera's lag and widening, the roll key, the roll's and the landing's
//! pose, and their dust and sound. The body itself is `kotor_game::Body`.
//!
//! **Assumed:** every number here is a feel choice. There is no roll or landing clip in the game's supermodels, so the
//! body is turned over and pressed down whole around its hips.

use super::{Mat, Stage};
use kotor_core::ResType;
use kotor_game::{MovementMode, ObjId};
use kotor_tables::TwoDa;
use std::f32::consts::TAU;

/// Seconds the camera's focus is behind the player, at speed.
const LAG_SECONDS: f32 = 0.07;
/// How quickly the lag closes on what it should be, per second.
const LAG_RATE: f32 = 6.0;
/// How quickly the focus follows the player's height, per second: a fall is seen, a stair is not felt.
const HEIGHT_RATE: f32 = 10.0;
/// The most the focus is left behind in height, in metres.
const HEIGHT_LAG: f32 = 1.0;
/// Degrees wider at a sprint.
const FOV_KICK: f32 = 6.0;
/// How quickly the field of view moves, per second.
const FOV_RATE: f32 = 5.0;
/// From this speed the view begins to widen, metres per second.
const FOV_FROM: f32 = 2.0;
/// The body turns over around its middle, this high when it stands, metres. Pressed down around it to the same share, the
/// head never reaches further from the middle than the feet do, so a turned body stays out of the floor.
const TUCK_HEIGHT: f32 = 0.9;
/// How far a roll presses the body down at its middle, and a landing at its start.
const ROLL_SQUASH: f32 = 0.55;
const LAND_SQUASH: f32 = 0.22;
/// A stock effect model of the game.
const DUST: &str = "fx_step_dust";
/// Slower landings make no sound.
const LOUD_LANDING: f32 = 2.0;

fn times(a: &Mat, b: &Mat) -> Mat {
    let mut m = [[0.0; 4]; 4];
    for (row, out) in m.iter_mut().enumerate() {
        for (col, v) in out.iter_mut().enumerate() {
            *v = (0..4).map(|k| a[row][k] * b[k][col]).sum();
        }
    }
    m
}

fn shift(p: [f32; 3]) -> Mat {
    [
        [1.0, 0.0, 0.0, p[0]],
        [0.0, 1.0, 0.0, p[1]],
        [0.0, 0.0, 1.0, p[2]],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn turn_z(a: f32) -> Mat {
    let (s, c) = a.sin_cos();
    [
        [c, -s, 0.0, 0.0],
        [s, c, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// Over the shoulder: at a quarter turn the top of a body that faces +y points along +y.
fn turn_over(a: f32) -> Mat {
    let (s, c) = a.sin_cos();
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, c, s, 0.0],
        [0.0, -s, c, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

impl Stage {
    /// The look button is held or let go. While it is held the keys keep the direction they had: looking round does not
    /// steer. With no key down, they take whatever direction the look ends on.
    pub fn set_looking(&mut self, looking: bool) {
        let (ahead, right, _) = self.steer;
        self.steer_yaw = match (looking, self.steer_yaw) {
            (false, _) => None,
            (true, Some(y)) if ahead != 0.0 || right != 0.0 => Some(y),
            (true, _) => Some(self.yaw),
        };
    }

    /// Put the player on a spot facing `heading`, with the camera behind it, for a test driver.
    pub fn place_player(&mut self, at: [f32; 3], heading: f32) {
        let w = &mut self.game.world;
        let pc = w.pc;
        w.teleport(pc, at);
        w.pc_mover.pos = at;
        w.pc_mover.heading = heading;
        self.yaw = heading;
    }

    /// How many played effects are alive, for a test.
    pub fn effects_playing(&self) -> usize {
        self.effects.playing()
    }

    /// Write the settings file now, for a test.
    pub fn save_settings_now(&mut self) {
        self.settings_changed();
    }

    /// The roll key. It asks; the world says no in a conversation or a minigame, or when the player is down.
    pub fn roll_key(&mut self) {
        let s = self.settings();
        let held = !s.combat_movement && self.party_blocked();
        if !s.movement_modern
            || held
            || self.menu_open()
            || self.turret_on()
            || self.pilot.is_some()
        {
            return;
        }
        self.game.world.roll_request = true;
    }

    /// The camera's lag and widening, and the sound of what the body did.
    pub(super) fn movement_tick(&mut self, dt: f32) {
        let w = &mut self.game.world;
        let cues = std::mem::take(&mut w.body_cues);
        if w.movement != MovementMode::Modern {
            self.camera_lag = [0.0; 3];
            self.fov_extra = 0.0;
            return;
        }
        let tuning = w.tuning;
        let body = &w.pc_body;
        let (vel, speed, z) = (body.vel, body.speed(), body.pos[2]);
        let share = 1.0 - (-LAG_RATE * dt).exp();
        let lag = self.camera_lag;
        self.camera_lag[0] = lag[0] + (-vel[0] * LAG_SECONDS - lag[0]) * share;
        self.camera_lag[1] = lag[1] + (-vel[1] * LAG_SECONDS - lag[1]) * share;
        // Height: what the focus was, less how far the player has moved since.
        let moved = z - self.camera_z.unwrap_or(z);
        self.camera_z = Some(z);
        let high = (lag[2] - moved) * (-HEIGHT_RATE * dt).exp();
        self.camera_lag[2] = high.clamp(-HEIGHT_LAG, HEIGHT_LAG);
        let wide = ((speed - FOV_FROM) / (tuning.sprint - FOV_FROM)).clamp(0.0, 1.0) * FOV_KICK;
        self.fov_extra += (wide - self.fov_extra) * (1.0 - (-FOV_RATE * dt).exp());
        if cues.roll_started || cues.landed.is_some_and(|v| v > LOUD_LANDING) {
            self.body_cue();
        }
    }

    /// The game's own dust at the feet, and a footfall from the table the steps use.
    fn body_cue(&mut self) {
        if let Some(m) = self.model(DUST) {
            let at = self.game.world.pc_mover.pos;
            self.effects.play_model(&m, at);
        }
        let sound = TwoDa::parse(
            &self
                .inst
                .read("footstepsounds", ResType::TWODA)
                .unwrap_or_default(),
        )
        .ok()
        .and_then(|t| t.get(1, "metal0").map(str::to_string));
        if let Some(sound) = sound {
            self.effect(&sound);
        }
    }

    /// Where the camera looks from: the player, with the lag.
    pub(super) fn lagged(&self, p: [f32; 3]) -> [f32; 3] {
        let l = self.camera_lag;
        [p[0] + l[0], p[1] + l[1], p[2] + l[2]]
    }

    /// How the player's body is turned over and pressed down now, in the world. None when it is not.
    pub(super) fn body_tilt(&self, id: ObjId, at: [f32; 3], heading: f32) -> Option<Mat> {
        let w = &self.game.world;
        if id != w.pc || w.movement != MovementMode::Modern {
            return None;
        }
        let body = &w.pc_body;
        let (over, squash) = if body.rolling() {
            roll_pose(body.roll_progress(&w.tuning))
        } else if body.landing() {
            (0.0, LAND_SQUASH * body.land_dip())
        } else {
            return None;
        };
        Some(tilt_matrix(at, heading, over, squash))
    }
}

/// How far a roll has turned the body over and pressed it down, `p` of the way through it.
fn roll_pose(p: f32) -> (f32, f32) {
    let eased = p * p * (3.0 - 2.0 * p);
    (eased * TAU, ROLL_SQUASH * (p * TAU / 2.0).sin())
}

/// Around the middle of a body at `at` facing `heading`: `over` radians over the shoulder, and pressed down by `squash`.
fn tilt_matrix(at: [f32; 3], heading: f32, over: f32, squash: f32) -> Mat {
    // Models face +y at rest.
    let angle = heading - std::f32::consts::FRAC_PI_2;
    let press = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0 - squash, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let hips = [at[0], at[1], at[2] + TUCK_HEIGHT * (1.0 - squash)];
    let inner = times(
        &times(&turn_over(over), &press),
        &times(&shift([0.0, 0.0, -TUCK_HEIGHT]), &turn_z(-angle)),
    );
    times(
        &times(&shift(hips), &turn_z(angle)),
        &times(&inner, &shift([-at[0], -at[1], -at[2]])),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(m: &Mat, p: [f32; 3]) -> [f32; 3] {
        [0, 1, 2].map(|r| m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2] + m[r][3])
    }

    fn near(a: [f32; 3], b: [f32; 3]) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-4)
    }

    #[test]
    fn no_tilt_leaves_the_body_where_it_is() {
        let at = [3.0, -2.0, 1.0];
        let m = tilt_matrix(at, 0.7, 0.0, 0.0);
        for p in [at, [3.0, -2.0, 2.8], [3.5, -1.5, 1.2]] {
            assert!(near(apply(&m, p), p), "{p:?}");
        }
    }

    #[test]
    fn the_hips_stay_put_through_a_turn() {
        let at = [3.0, -2.0, 1.0];
        let hips = [at[0], at[1], at[2] + TUCK_HEIGHT];
        let m = tilt_matrix(at, 0.7, 1.9, 0.0);
        assert!(near(apply(&m, hips), hips));
    }

    #[test]
    fn a_quarter_turn_puts_the_head_forward_along_the_heading() {
        let heading = 0.7f32;
        let at = [0.0; 3];
        let m = tilt_matrix(at, heading, std::f32::consts::FRAC_PI_2, 0.0);
        let head = apply(&m, [0.0, 0.0, TUCK_HEIGHT + 1.0]);
        assert!((head[0] - heading.cos()).abs() < 1e-4, "{head:?}");
        assert!((head[1] - heading.sin()).abs() < 1e-4, "{head:?}");
        assert!((head[2] - TUCK_HEIGHT).abs() < 1e-4, "{head:?}");
    }

    #[test]
    fn a_turned_body_stays_out_of_the_floor_all_through_a_roll() {
        let at = [2.0, -1.0, 3.0];
        for k in 0..=100 {
            let (over, squash) = roll_pose(k as f32 / 100.0);
            let m = tilt_matrix(at, 0.4, over, squash);
            // Along the body, from the feet to the top of a 1.8 m head.
            for h in 0..=18 {
                let z = apply(&m, [at[0], at[1], at[2] + h as f32 * 0.1])[2];
                assert!(z >= at[2] - 1e-3, "{z} at {k}% and height {h}");
            }
        }
    }

    #[test]
    fn pressing_down_keeps_the_feet_on_the_floor() {
        let at = [1.0, 1.0, 2.0];
        let m = tilt_matrix(at, 0.3, 0.0, 0.2);
        assert!(near(apply(&m, at), at));
        let head = apply(&m, [1.0, 1.0, 3.8]);
        assert!((head[2] - (2.0 + 1.8 * 0.8)).abs() < 1e-4, "{head:?}");
    }
}
