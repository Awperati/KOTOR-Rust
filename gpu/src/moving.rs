//! The window's side of Modern movement: the camera's lag and widening, the roll key, the roll's and the landing's
//! pose, and their dust and sound. The body itself is `kotor_game::Body`.
//!
//! **Assumed:** every number here is a feel choice. There is no roll or landing clip in the game's supermodels, so the
//! joints are bent here, by the keys in `roll_pose.txt` and `landing_pose.txt`, which the owner can edit: see
//! `docs/MOVEMENT_POSE.md`. A droid has none of these joints and only turns.

use super::bodypose::{self, PoseFile, Side, JOINTS};
use super::{placement, Mat, Stage};
use kotor_core::ResType;
use kotor_game::{Clip, MovementMode, ObjId};
use kotor_model::{Model, Pose};
use kotor_render::{Rig, Scene};
use kotor_tables::TwoDa;
use std::collections::HashMap;
use std::f32::consts::TAU;
use std::path::PathBuf;
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
/// How quickly the keys' direction catches up with a turning camera in Modern, seconds: nine tenths of the way in 2.3 of these.
const STEER_SECONDS: f32 = 0.1;
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

/// The two files of keys, and what is wrong with them.
pub(super) struct BodyPoses {
    roll: PoseFile,
    landing: PoseFile,
    /// Where the files are. None: the defaults, and nothing is read or written.
    folder: Option<PathBuf>,
    problems: Vec<String>,
    /// For the preview: this pose, whatever the body is doing.
    preview: Option<Bend>,
}

impl Default for BodyPoses {
    fn default() -> Self {
        BodyPoses {
            roll: default_roll(),
            landing: default_landing(),
            folder: None,
            problems: Vec::new(),
            preview: None,
        }
    }
}

fn default_roll() -> PoseFile {
    PoseFile::parse(bodypose::DEFAULT_ROLL).expect("the roll that comes with the game reads")
}

fn default_landing() -> PoseFile {
    PoseFile::parse(bodypose::DEFAULT_LANDING).expect("the landing that comes with the game reads")
}

/// One of the pose files, what the game has written before, and the default.
struct PoseSource {
    name: &'static str,
    default_text: &'static str,
    shipped: &'static [&'static str],
    default: fn() -> PoseFile,
}

const ROLL_SOURCE: PoseSource = PoseSource {
    name: bodypose::ROLL_FILE,
    default_text: bodypose::DEFAULT_ROLL,
    shipped: &bodypose::SHIPPED_ROLLS,
    default: default_roll,
};
const LANDING_SOURCE: PoseSource = PoseSource {
    name: bodypose::LANDING_FILE,
    default_text: bodypose::DEFAULT_LANDING,
    shipped: &bodypose::SHIPPED_LANDINGS,
    default: default_landing,
};

/// A file's keys. A missing file, or one nobody edited since the game wrote an older default, gets the new default on disk. An
/// edited one stays, whatever its age, and a note says so. One that cannot be used gives the default and says why.
fn read_pose(
    folder: &std::path::Path,
    from: &PoseSource,
    problems: &mut Vec<String>,
    notes: &mut Vec<String>,
) -> PoseFile {
    let path = folder.join(from.name);
    let default = from.default;
    let write_default = |problems: &mut Vec<String>| {
        let written =
            std::fs::create_dir_all(folder).and_then(|()| std::fs::write(&path, from.default_text));
        if let Err(e) = written {
            problems.push(format!("{}: could not be written: {e}", from.name));
        }
    };
    let Ok(bytes) = std::fs::read(&path) else {
        write_default(problems);
        return default();
    };
    let text = String::from_utf8_lossy(&bytes);
    if from
        .shipped
        .iter()
        .any(|old| bodypose::same_text(&text, old))
    {
        write_default(problems);
        notes.push(format!(
            "{}: was the game's older default, and is now its newest.",
            from.name
        ));
        return default();
    }
    match PoseFile::parse(&text) {
        Ok(f) => {
            let newest = default().version;
            if f.version < newest {
                notes.push(format!(
                    "{}: is your own edit, and it is kept. It began as version {} and the game's newest is {newest}: delete the file to get that.",
                    from.name, f.version
                ));
            }
            f
        }
        Err(e) => {
            problems.push(format!("{}: {e}", from.name));
            default()
        }
    }
}

impl Stage {
    /// Keep the pose files in this folder: read them now, and write those that are not there.
    pub fn use_pose_folder(&mut self, folder: PathBuf) {
        self.body_poses.folder = Some(folder);
        self.read_poses();
    }

    fn read_poses(&mut self) {
        let Some(folder) = self.body_poses.folder.clone() else {
            return;
        };
        let (mut problems, mut notes) = (Vec::new(), Vec::new());
        self.body_poses.roll = read_pose(&folder, &ROLL_SOURCE, &mut problems, &mut notes);
        self.body_poses.landing = read_pose(&folder, &LANDING_SOURCE, &mut problems, &mut notes);
        for p in &problems {
            eprintln!("pose: {p}");
        }
        let mut report = if problems.is_empty() {
            "No problems: the game is using both pose files.\n".to_string()
        } else {
            let mut text = "The game is using its own pose for each file named here:\n".to_string();
            for p in &problems {
                text.push_str(p);
                text.push('\n');
            }
            text
        };
        for n in &notes {
            report.push_str(n);
            report.push('\n');
        }
        // Only a help to the reader: the game goes on without it.
        let _ = std::fs::write(folder.join("pose_problems.txt"), report);
        self.body_poses.problems = problems;
    }

    /// F8: read the pose files again, and say on screen how that went.
    pub fn reload_poses(&mut self) {
        if self.body_poses.folder.is_none() {
            return;
        }
        self.read_poses();
        let words = if self.body_poses.problems.is_empty() {
            "Poses reloaded".to_string()
        } else {
            "Pose file mistake: using the default. See pose_problems.txt".to_string()
        };
        self.say_over_player(words);
    }

    /// What was wrong with the pose files when they were last read.
    pub fn pose_problems(&self) -> &[String] {
        &self.body_poses.problems
    }

    /// Draw the player in the roll, this share of the way through, whatever it is doing. For the preview and a test.
    pub fn preview_roll(&mut self, p: f32) {
        self.body_poses.preview = Some(Bend::Roll(p));
    }

    /// Draw the player in the landing, this share of the way through. For the preview and a test.
    pub fn preview_landing(&mut self, p: f32) {
        self.body_poses.preview = Some(Bend::Land(p));
    }

    /// Where the player's joints are in the body's own frame, feet at the origin, facing +y, without the body's somersault.
    pub fn bent_joints(&mut self) -> Option<HashMap<String, [f32; 3]>> {
        let pc = self.game.world.pc;
        let pose = self.game.poses().into_iter().find(|p| p.id == pc)?;
        let rig = self.rig_of(pc)?;
        let bend = self.body_bend(pc)?;
        let (bent, ..) = self.bent_pose(&rig, &pose.clip, bend)?;
        Some(
            rig.joint_frames(Some(&bent))
                .into_iter()
                .map(|(name, m, _)| (name, [m[0][3], m[1][3], m[2][3]]))
                .collect(),
        )
    }

    /// The look button is held or let go.
    pub fn set_looking(&mut self, looking: bool) {
        self.looking = looking;
    }

    /// Which way the movement keys point now: the way the camera looks, read every frame and never kept from the moment a
    /// key went down. In Classic exactly that. In Modern it follows the camera over a moment, so a held key's path curves.
    pub(super) fn keys_yaw(&mut self, dt: f32) -> f32 {
        let (ahead, right, _) = self.steer;
        let held = ahead != 0.0 || right != 0.0;
        if !held || !self.settings().movement_modern || self.pilot.is_some() {
            self.steer_yaw = self.yaw;
        } else {
            let tau = std::f32::consts::TAU;
            let off = (self.yaw - self.steer_yaw + tau * 1.5).rem_euclid(tau) - tau / 2.0;
            self.steer_yaw += off * (1.0 - (-dt / STEER_SECONDS).exp());
        }
        self.steer_yaw
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
        if id != w.pc {
            return None;
        }
        if let Some(preview) = self.body_poses.preview {
            return Some(preview);
        }
        if w.movement != MovementMode::Modern {
            return None;
        }
        let body = &w.pc_body;
        if body.rolling() {
            Some(Bend::Roll(body.roll_progress(&w.tuning)))
        } else if body.landing() {
            Some(Bend::Land(1.0 - body.land_dip()))
        } else {
            None
        }
    }

    /// The body bent: the joints, how far the whole of it is turned over, in radians, and how much it rests on the floor, 0 to 1.
    fn bent_pose(&self, rig: &Rig, clip: &Option<Clip>, bend: Bend) -> Option<(Pose, f32, f32)> {
        let name = clip.as_ref().map_or("pause1", |c| c.name);
        let t = clip.as_ref().map_or(0.0, |c| c.time);
        let sampled = rig
            .animation_length(name)
            .filter(|l| *l > 0.0)
            .and_then(|l| rig.sample(name, t % l));
        // The skeleton's own stance, which is even from left to right. The idle clip has one foot forward.
        let stand = rig.bind_pose();
        // A clip that was playing shows through as the bend comes and goes.
        let pose = sampled.unwrap_or_else(|| stand.clone());
        let (file, p, roll) = match bend {
            Bend::Roll(p) => (&self.body_poses.roll, p, true),
            Bend::Land(p) => (&self.body_poses.landing, p, false),
        };
        let keys = file.sample(p);
        // The clip's swing goes as the bend comes, or the legs would scissor in the tuck.
        let mut pose = kotor_model::blend(&pose, &stand, keys.tuck);
        turn_joints(&mut pose, rig, &keys.angles);
        if let Some(root) = pose.get_mut("rootdummy") {
            root.position[2] -= keys.lower;
        }
        Some(if roll {
            (pose, somersault(p) * TAU * file.turns, keys.tuck.sqrt())
        } else {
            (pose, 0.0, 0.0)
        })
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
        let Some((pose, over, rests)) = self.bent_pose(rig, clip, bend) else {
            return false;
        };
        let Ok(mut body) = rig.posed(Some(&pose)) else {
            return false;
        };
        let angle = heading - std::f32::consts::FRAC_PI_2;
        let mut xf = times(&tilt_matrix(at, heading, over), &placement(at, angle));
        // A roll is on the floor all the way round: what is lowest now touches it, whichever part that is. Before the items
        // in the hands, which do not hold the body up.
        if rests > 0.0 {
            let lowest = body
                .meshes
                .iter()
                .flat_map(|m| m.positions.iter())
                .map(|p| xf[2][0] * p[0] + xf[2][1] * p[1] + xf[2][2] * p[2] + xf[2][3])
                .fold(f32::MAX, f32::min);
            if lowest < f32::MAX {
                xf = times(&shift([0.0, 0.0, (at[2] - lowest) * rests]), &xf);
            }
        }
        for (node, item) in hands {
            rig.attach(&mut body, Some(&pose), node, item);
        }
        scene.append_transformed(&body, &xf);
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

/// What the player's body is doing: how far through a roll or a landing, 0 to 1.
#[derive(Clone, Copy)]
enum Bend {
    Roll(f32),
    Land(f32),
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// How far round a roll is, 0 to 1, `p` of the way through it: going over from the first moment, or it reads as a squat.
fn somersault(p: f32) -> f32 {
    (p + smooth(p)) / 2.0
}

/// `[w, x, y, z]` of a turn about a line of length 1.
fn about(axis: [f32; 3], degrees: f32) -> [f32; 4] {
    let (s, c) = (degrees.to_radians() / 2.0).sin_cos();
    [c, s * axis[0], s * axis[1], s * axis[2]]
}

fn quat_times(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
        a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
        a[0] * b[2] - a[1] * b[3] + a[2] * b[0] + a[3] * b[1],
        a[0] * b[3] + a[1] * b[2] - a[2] * b[1] + a[3] * b[0],
    ]
}

/// A line in the body's frame, as the frame `m` has it: `m`'s turn, undone.
fn into_frame(m: &Mat, v: [f32; 3]) -> [f32; 3] {
    [0, 1, 2].map(|c| m[0][c] * v[0] + m[1][c] * v[1] + m[2][c] * v[2])
}

fn unit(v: [f32; 3]) -> Option<[f32; 3]> {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    (l > 1e-4).then(|| v.map(|x| x / l))
}

/// Turn the joints by these angles, in the order of `JOINTS`.
///
/// The bones' own axes cannot be used: `rcollar_g` is flipped against `lcollar_g`, and an arm's frame leans with the arm, so
/// a turn about a parent's own x goes a different way on each side, and twists the limb. So each line is said in the body's
/// frame of the standing pose, and carried into the frame of the joint's parent there. A knee's line still turns with the thigh.
fn turn_joints(pose: &mut Pose, rig: &Rig, angles: &[[f32; 3]]) {
    let frames: HashMap<String, (Mat, Mat)> = rig
        .joint_frames(None)
        .into_iter()
        .map(|(name, m, parent)| (name, (m, parent)))
        .collect();
    let place = |m: &Mat| [m[0][3], m[1][3], m[2][3]];
    for (joint, angle) in JOINTS.iter().zip(angles) {
        if *angle == [0.0; 3] {
            continue;
        }
        let (Some(node), Some((own, parent))) = (pose.get_mut(joint.name), frames.get(joint.name))
        else {
            continue;
        };
        // Which way "out" and "twist" go, so that left and right are alike.
        let side = if joint.side == Side::Right { -1.0 } else { 1.0 };
        // Up the limb, from the joint it reaches for, or up the body.
        let up = joint
            .reaches
            .and_then(|r| frames.get(r))
            .and_then(|(reach, _)| {
                let (a, b) = (place(own), place(reach));
                unit([a[0] - b[0], a[1] - b[1], a[2] - b[2]])
            })
            .unwrap_or([0.0, 0.0, 1.0]);
        let bend = about(into_frame(parent, [1.0, 0.0, 0.0]), joint.flex * angle[0]);
        let out = about(into_frame(parent, [0.0, 1.0, 0.0]), side * angle[1]);
        let twist = about(into_frame(parent, up), side * angle[2]);
        node.orientation = quat_times(bend, quat_times(out, quat_times(twist, node.orientation)));
    }
}

/// A body at `at` facing `heading`, turned `over` radians over the shoulder around its middle.
fn tilt_matrix(at: [f32; 3], heading: f32, over: f32) -> Mat {
    // Models face +y at rest.
    let angle = heading - std::f32::consts::FRAC_PI_2;
    let middle = [at[0], at[1], at[2] + TUCK_UP];
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
        let m = tilt_matrix(at, 0.7, 0.0);
        for p in [at, [3.0, -2.0, 2.8], [3.5, -1.5, 1.2]] {
            assert!(near(apply(&m, p), p), "{p:?}");
        }
    }

    #[test]
    fn a_whole_turn_at_the_end_of_a_roll_is_no_turn() {
        let at = [3.0, -2.0, 1.0];
        let roll = default_roll();
        assert_eq!(roll.sample(1.0).tuck, 0.0);
        let m = tilt_matrix(at, 0.7, somersault(1.0) * TAU * roll.turns);
        let p = [3.0, -2.0, 2.8];
        assert!(near(apply(&m, p), p), "{:?}", apply(&m, p));
    }

    #[test]
    fn a_quarter_turn_puts_the_head_forward_along_the_heading() {
        let heading = 0.7f32;
        let m = tilt_matrix([0.0; 3], heading, std::f32::consts::FRAC_PI_2);
        let (c, s) = (heading.cos(), heading.sin());
        let head = apply(&m, [c * TUCK_FORWARD, s * TUCK_FORWARD, TUCK_UP + 1.0]);
        let ahead = 1.0 + TUCK_FORWARD;
        assert!((head[0] - heading.cos() * ahead).abs() < 1e-4, "{head:?}");
        assert!((head[1] - heading.sin() * ahead).abs() < 1e-4, "{head:?}");
        assert!((head[2] - TUCK_UP).abs() < 1e-4, "{head:?}");
    }

    #[test]
    fn turning_a_joint_turns_a_shin_forward() {
        // A point below a joint turned forward by 90 degrees about x comes out in front of it.
        let x_line = [1.0, 0.0, 0.0];
        let q = about(x_line, 90.0);
        let (w, x) = (q[0], q[1]);
        // The y of (0, 0, -1) under this turn is 2 w x.
        assert!((2.0 * w * x - 1.0).abs() < 1e-5);
        assert!(near(
            [quat_times(q, about(x_line, -90.0))[0], 0.0, 0.0],
            [1.0, 0.0, 0.0]
        ));
    }

    #[test]
    fn a_line_of_the_body_is_carried_into_a_flipped_frame() {
        // A parent turned half way round about y, as a flipped collar is: the body's x is its -x.
        let flipped: Mat = [
            [-1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, -1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        assert_eq!(into_frame(&flipped, [1.0, 0.0, 0.0]), [-1.0, 0.0, 0.0]);
        assert_eq!(
            into_frame(&shift([1.0, 2.0, 3.0]), [1.0, 0.0, 0.0]),
            [1.0, 0.0, 0.0]
        );
    }

    fn roll_named(name: &'static str) -> PoseSource {
        PoseSource {
            name,
            ..ROLL_SOURCE
        }
    }

    #[test]
    fn a_file_with_a_mistake_gives_the_default_and_says_why() {
        let dir = std::env::temp_dir().join(format!("kotor_pose_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (mut problems, mut notes) = (Vec::new(), Vec::new());
        let name = "bad_roll.txt";
        std::fs::write(dir.join(name), "[key]\nat = 0\nlthigh_g = 999\n").unwrap();
        let f = read_pose(&dir, &roll_named(name), &mut problems, &mut notes);
        assert_eq!(f.version, default_roll().version);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("line 3"), "{problems:?}");
        assert!(std::fs::read_to_string(dir.join(name))
            .unwrap()
            .contains("999"));

        // One that is missing is written for editing, and is no mistake.
        problems.clear();
        let fresh = "fresh_roll.txt";
        read_pose(&dir, &roll_named(fresh), &mut problems, &mut notes);
        assert!(
            problems.is_empty() && notes.is_empty(),
            "{problems:?} {notes:?}"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join(fresh)).unwrap(),
            bodypose::DEFAULT_ROLL
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The owner's own file, as the last version wrote it: it must not keep the old roll.
    #[test]
    fn a_default_nobody_edited_is_replaced_and_an_edited_one_is_kept() {
        let dir = std::env::temp_dir().join(format!("kotor_pose_old_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (mut problems, mut notes) = (Vec::new(), Vec::new());
        let old = bodypose::SHIPPED_ROLLS[0];

        // Line ends do not make it an edit.
        for (name, text) in [
            ("a.txt", old.replace('\r', "")),
            ("b.txt", old.replace('\r', "").replace('\n', "\r\n")),
        ] {
            std::fs::write(dir.join(name), &text).unwrap();
            let f = read_pose(&dir, &roll_named(name), &mut problems, &mut notes);
            assert_eq!(f.version, default_roll().version);
            assert_eq!(
                std::fs::read_to_string(dir.join(name)).unwrap(),
                bodypose::DEFAULT_ROLL
            );
        }
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(notes.len(), 2, "{notes:?}");

        // One number changed: it is the owner's, and it stays.
        notes.clear();
        let name = "edited.txt";
        let edited = old.replace("120    8    0", "125    8    0");
        assert_ne!(edited, old);
        std::fs::write(dir.join(name), &edited).unwrap();
        let f = read_pose(&dir, &roll_named(name), &mut problems, &mut notes);
        assert_eq!(f.version, 0);
        assert_eq!(std::fs::read_to_string(dir.join(name)).unwrap(), edited);
        assert!(problems.is_empty(), "{problems:?}");
        assert!(notes[0].contains("kept"), "{notes:?}");

        // The new default, read again, is left alone with nothing to say.
        notes.clear();
        let name = "newest.txt";
        std::fs::write(dir.join(name), bodypose::DEFAULT_ROLL).unwrap();
        read_pose(&dir, &roll_named(name), &mut problems, &mut notes);
        assert!(notes.is_empty(), "{notes:?}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
