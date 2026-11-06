//! The window's side of Modern movement: the camera's lag and widening, the roll key, the roll's and the landing's
//! pose, and their dust and sound. The body itself is `kotor_game::Body`.
//!
//! **Assumed:** every number here is a feel choice. There is no roll or landing clip in the game's supermodels, so the
//! joints are bent here: knees to the chest in a roll, a crouch in a landing. A droid has none of these joints and only turns.

use super::{placement, Mat, Stage};
use kotor_core::ResType;
use kotor_game::{Clip, MovementMode, ObjId};
use kotor_model::{Model, Pose};
use kotor_render::{Rig, Scene};
use kotor_tables::TwoDa;
use std::f32::consts::TAU;
use std::rc::Rc;

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
/// A body in a tight tuck turns around this point of its standing pose, metres forward and up from the feet.
const TUCK_FORWARD: f32 = 0.15;
const TUCK_UP: f32 = 1.15;
/// And that point is this high over the floor at the middle of a roll: as far as the head and feet reach from it.
const TUCK_RADIUS: f32 = 0.85;
/// Share of a roll spent going into the tuck, and coming out of it.
const TUCK_IN: f32 = 0.18;
const TUCK_OUT: f32 = 0.22;
/// The joints in a full tuck, by how many degrees they turn forward: knees to the chest, the back curled, the arms round
/// the shins. A joint turns about the side to side line of the joint above it.
const TUCK: [(&str, f32); 12] = [
    ("lthigh_g", 125.0),
    ("rthigh_g", 125.0),
    ("lshin_g", -140.0),
    ("rshin_g", -140.0),
    ("lfoot_g", 25.0),
    ("rfoot_g", 25.0),
    ("torso_g", -30.0),
    ("torsoupr_g", -30.0),
    ("necklwr_g", -20.0),
    ("neck_g", -15.0),
    ("lbicep_g", 55.0),
    ("rbicep_g", 55.0),
];
const TUCK_ELBOWS: [(&str, f32); 2] = [("lforearm_g", 85.0), ("rforearm_g", 85.0)];
/// A hard landing bends the knees this many degrees at its deepest, with the feet staying on the floor.
const LAND_KNEES: f32 = 55.0;
/// Thigh and shin lengths of the humanoid skeleton, metres (`S_Male01`).
const THIGH: f32 = 0.45;
const SHIN: f32 = 0.39;
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

    /// What the player's body is doing that no clip has, now.
    fn body_bend(&self, id: ObjId) -> Option<Bend> {
        let w = &self.game.world;
        if id != w.pc || w.movement != MovementMode::Modern {
            return None;
        }
        let body = &w.pc_body;
        if body.rolling() {
            Some(Bend::Roll(body.roll_progress(&w.tuning)))
        } else if body.landing() {
            Some(Bend::Land(body.land_dip()))
        } else {
            None
        }
    }

    /// Draw the player tucked or crouching, if it is. Otherwise false, and nothing is drawn.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn add_bent_body(
        &self,
        scene: &mut Scene,
        id: ObjId,
        rig: &Rig,
        hands: &[(&'static str, Rc<Model>)],
        clip: &Option<Clip>,
        at: [f32; 3],
        heading: f32,
    ) -> bool {
        let Some(bend) = self.body_bend(id) else {
            return false;
        };
        let name = clip.as_ref().map_or("pause1", |c| c.name);
        let t = clip.as_ref().map_or(0.0, |c| c.time);
        let sampled = rig
            .animation_length(name)
            .filter(|l| *l > 0.0)
            .and_then(|l| rig.sample(name, t % l));
        // A clip that was playing shows through as the bend comes and goes.
        let Some(mut pose) = sampled.or_else(|| rig.sample("pause1", 0.0)) else {
            return false;
        };
        // The clip's swing goes as the bend comes, or the legs would scissor in the tuck.
        let rest = rig.sample("pause1", 0.0);
        let mut settle = |share: f32| {
            if let Some(r) = &rest {
                pose = kotor_model::blend(&pose, r, share);
            }
        };
        let xf = match bend {
            Bend::Roll(p) => {
                let (over, tuck) = roll_pose(p);
                settle(tuck);
                bend_joints(&mut pose, tuck);
                tilt_matrix(at, heading, over, tuck)
            }
            Bend::Land(dip) => {
                settle(dip);
                crouch(&mut pose, dip);
                tilt_matrix(at, heading, 0.0, 0.0)
            }
        };
        let Ok(mut body) = rig.posed(Some(&pose)) else {
            return false;
        };
        for (node, item) in hands {
            rig.attach(&mut body, Some(&pose), node, item);
        }
        let angle = heading - std::f32::consts::FRAC_PI_2;
        scene.append_transformed(&body, &times(&xf, &placement(at, angle)));
        true
    }

    /// How far under its feet the player's body reaches now, for a test: `None` when it is not tucked or crouching.
    pub fn bent_body_sinks(&mut self) -> Option<f32> {
        let pc = self.game.world.pc;
        let pose = self.game.poses().into_iter().find(|p| p.id == pc)?;
        let rig = self.rig_of(pc)?;
        let hands = self.hands(pc);
        let mut alone = Scene::default();
        self.add_bent_body(
            &mut alone,
            pc,
            &rig,
            &hands,
            &pose.clip,
            pose.position,
            pose.heading,
        )
        .then_some(())?;
        let lowest = alone
            .meshes
            .iter()
            .flat_map(|m| m.positions.iter().map(|p| p[2]))
            .fold(f32::MAX, f32::min);
        Some(pose.position[2] - lowest)
    }
}

/// What the player's body is doing: how far through a roll, or how deep in a landing.
#[derive(Clone, Copy)]
enum Bend {
    Roll(f32),
    Land(f32),
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// How far a roll has turned the body over, in radians, and how tucked it is, 0 to 1, `p` of the way through it.
fn roll_pose(p: f32) -> (f32, f32) {
    (
        smooth(p) * TAU,
        smooth(p / TUCK_IN) * smooth((1.0 - p) / TUCK_OUT),
    )
}

/// `[w, x, y, z]` of a turn about the x axis, the body's side to side line.
fn about_x(degrees: f32) -> [f32; 4] {
    let (s, c) = (degrees.to_radians() / 2.0).sin_cos();
    [c, s, 0.0, 0.0]
}

fn quat_times(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
        a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
        a[0] * b[2] - a[1] * b[3] + a[2] * b[0] + a[3] * b[1],
        a[0] * b[3] + a[1] * b[2] - a[2] * b[1] + a[3] * b[0],
    ]
}

/// A joint a share of the way from what the clip has to the turn from the standing pose, about its parent's side to side line.
fn turn_joint(pose: &mut Pose, joint: &str, degrees: f32) {
    if let Some(n) = pose.get_mut(joint) {
        n.orientation = quat_times(about_x(degrees), n.orientation);
    }
}

/// Knees to the chest, the back curled and the arms in, `share` of the way there.
fn bend_joints(pose: &mut Pose, share: f32) {
    for (joint, degrees) in TUCK.iter().chain(&TUCK_ELBOWS) {
        turn_joint(pose, joint, degrees * share);
    }
}

/// The knees bent, `dip` of the way to the deepest, with the feet flat and the body lowered so that they stay on the floor.
fn crouch(pose: &mut Pose, dip: f32) {
    let a = LAND_KNEES * dip;
    for (joint, degrees) in [
        ("lthigh_g", a),
        ("rthigh_g", a),
        ("lshin_g", -2.0 * a),
        ("rshin_g", -2.0 * a),
        ("lfoot_g", a),
        ("rfoot_g", a),
        ("torso_g", -0.4 * a),
    ] {
        turn_joint(pose, joint, degrees);
    }
    if let Some(root) = pose.get_mut("rootdummy") {
        root.position[2] -= (THIGH + SHIN) * (1.0 - a.to_radians().cos());
    }
}

/// A body at `at` facing `heading`, turned `over` radians over the shoulder around its middle, which comes down from its
/// standing height to `TUCK_RADIUS` over the floor as it tucks, so that the head and feet clear the floor as they come round.
fn tilt_matrix(at: [f32; 3], heading: f32, over: f32, tuck: f32) -> Mat {
    // Models face +y at rest.
    let angle = heading - std::f32::consts::FRAC_PI_2;
    // Late, so that the body only comes down once the legs have folded.
    let height = TUCK_UP + (TUCK_RADIUS - TUCK_UP) * tuck * tuck;
    let middle = [at[0], at[1], at[2] + height];
    let inner = times(
        &times(&shift([0.0, TUCK_FORWARD, 0.0]), &turn_over(over)),
        &times(&shift([0.0, -TUCK_FORWARD, -TUCK_UP]), &turn_z(-angle)),
    );
    times(
        &times(&shift(middle), &turn_z(angle)),
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
    fn a_whole_turn_at_the_end_of_a_roll_is_no_turn() {
        let at = [3.0, -2.0, 1.0];
        let (over, tuck) = roll_pose(1.0);
        assert_eq!(tuck, 0.0);
        let m = tilt_matrix(at, 0.7, over, tuck);
        let p = [3.0, -2.0, 2.8];
        assert!(near(apply(&m, p), p), "{:?}", apply(&m, p));
    }

    #[test]
    fn a_quarter_turn_puts_the_head_forward_along_the_heading() {
        let heading = 0.7f32;
        let m = tilt_matrix([0.0; 3], heading, std::f32::consts::FRAC_PI_2, 0.0);
        let (c, s) = (heading.cos(), heading.sin());
        let head = apply(&m, [c * TUCK_FORWARD, s * TUCK_FORWARD, TUCK_UP + 1.0]);
        let ahead = 1.0 + TUCK_FORWARD;
        assert!((head[0] - heading.cos() * ahead).abs() < 1e-4, "{head:?}");
        assert!((head[1] - heading.sin() * ahead).abs() < 1e-4, "{head:?}");
        assert!((head[2] - TUCK_UP).abs() < 1e-4, "{head:?}");
    }

    #[test]
    fn a_roll_tucks_in_and_comes_out() {
        assert_eq!(roll_pose(0.0).1, 0.0);
        assert_eq!(roll_pose(0.5).1, 1.0);
        assert_eq!(roll_pose(1.0).1, 0.0);
    }

    #[test]
    fn turning_a_joint_turns_a_shin_forward() {
        // A point below a joint turned forward by 90 degrees about x comes out in front of it.
        let q = about_x(90.0);
        let (w, x) = (q[0], q[1]);
        // The y of (0, 0, -1) under this turn is 2 w x.
        assert!((2.0 * w * x - 1.0).abs() < 1e-5);
        assert!(near(
            [quat_times(q, about_x(-90.0))[0], 0.0, 0.0],
            [1.0, 0.0, 0.0]
        ));
    }
}
