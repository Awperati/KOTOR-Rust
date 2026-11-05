//! The playable scene: a [`Game`] with everything a window needs from it each frame. The camera, the posed bodies
//! of creatures and doors, the dialogue box and HUD, and what to play. Nothing here touches the GPU or a window,
//! so it runs in a headless test.

use kotor_archive::Install;
use kotor_core::ResType;
use kotor_game::{pixel_ray, Beat, CameraShot, Clip, Game, Kind, ObjId, World};
use kotor_gff::{Gff, Value};
use kotor_image::Image;
use kotor_model::{parse_model, Model};
use kotor_render::{Camera, Floor, Rig, Scene};
use kotor_tables::TwoDa;
use kotor_ui::{DialogueView, HudMember, Ui};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

type Mat = [[f32; 4]; 4];

/// Creatures farther than this from the player are not drawn (metres).
const DRAW_RANGE: f32 = 45.0;
/// Animation ids from here index a stunt model's animations (see `kotor_game::cutscene`).
const FIRST_STUNT_ANIMATION: u32 = 1200;
/// The follow camera: metres behind the player and above its feet, and the height it looks at. As `render_walk`.
const CAMERA_FOV: f32 = 55.0;
/// Metres within which the E key finds something to use.
const USE_REACH: f32 = 2.5;
const CAMERA_BEHIND: f32 = 3.3;
const CAMERA_ABOVE: f32 = 1.7;
const CAMERA_LOOKS_AT: f32 = 1.2;

/// Something the window should play. Files are found here; decoding and output belong to the window.
#[derive(Clone, Debug, PartialEq)]
pub enum Cue {
    Voice(PathBuf),
    StopVoice,
    Music(PathBuf),
    StopMusic,
}

struct DoorBody {
    model: Model,
    xf: Mat,
}

/// Z rotation by `angle` radians, then translation.
fn placement(p: [f32; 3], angle: f32) -> Mat {
    let (s, c) = angle.sin_cos();
    [
        [c, -s, 0.0, p[0]],
        [s, c, 0.0, p[1]],
        [0.0, 0.0, 1.0, p[2]],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// `placement`, with the body laid flat on its back first.
fn placement_flat(p: [f32; 3], angle: f32) -> Mat {
    let (s, c) = angle.sin_cos();
    [
        [c, 0.0, s, p[0]],
        [s, 0.0, -c, p[1]],
        [0.0, 1.0, 0.0, p[2] + 0.15],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// The `CameraList` of an area. A camera stands `Height` above its `Position`, turned about z by `Orientation`, and
/// tilted up from straight down by `Pitch` degrees.
///
/// **Assumption, fits the opening's cameras but not checked against the game:** `Orientation` is (w, x, y, z), and
/// with no turn a camera looks along +y.
fn fixed_cameras(inst: &mut Install, area: &str) -> HashMap<i32, Camera> {
    let mut out = HashMap::new();
    let Some(git) = inst
        .read(area, ResType::GIT)
        .ok()
        .and_then(|b| Gff::parse(&b).ok())
    else {
        return out;
    };
    for c in git.root.list("CameraList").unwrap_or_default() {
        let field = |label: &str| c.fields.iter().find(|(l, _)| l == label).map(|(_, v)| v);
        let (Some(Value::Vector(p)), Some(Value::Orientation(q)), Some(id)) =
            (field("Position"), field("Orientation"), c.int("CameraID"))
        else {
            continue;
        };
        let turn = 2.0 * q[3].atan2(q[0]);
        let pitch = (c.float("Pitch").unwrap_or(90.0) as f32).to_radians();
        let eye = [p[0], p[1], p[2] + c.float("Height").unwrap_or(0.0) as f32];
        out.insert(
            id as i32,
            Camera {
                eye,
                target: [
                    eye[0] - turn.sin() * pitch.sin(),
                    eye[1] + turn.cos() * pitch.sin(),
                    eye[2] - pitch.cos(),
                ],
                fov: c.float("FieldOfView").unwrap_or(CAMERA_FOV as f64) as f32,
            },
        );
    }
    out
}

fn load_model(inst: &mut Install, name: &str) -> Option<Model> {
    let mdl = inst.read(name, ResType::MDL).ok()?;
    let mdx = inst.read(name, ResType::MDX).unwrap_or_default();
    parse_model(&mdl, &mdx).ok()
}

pub struct Stage {
    pub game: Game,
    inst: Install,
    ui: Ui<'static>,
    dir: PathBuf,
    original: bool,
    /// Degrees per second the camera turns round the player by itself while nothing else moves it.
    pub orbit: f32,
    module: String,
    floor: Option<Floor>,
    static_scene: Scene,
    /// Goes up whenever `static_scene` is replaced.
    pub static_version: u64,
    rigs: HashMap<(i32, u32), Option<Rc<Rig>>>,
    doors: HashMap<ObjId, Option<Rc<DoorBody>>>,
    textures: HashMap<String, Option<Image>>,
    /// Camera direction in radians (0 = looking along +x) and distance behind the player.
    pub yaw: f32,
    pub distance: f32,
    /// Degrees of camera turning asked for this frame, per second.
    pub turn: f32,
    /// Camera height angle in radians, above the level of the chest.
    pub pitch: f32,
    /// WASD this frame: forward and rightward, each -1, 0 or 1, and whether to walk instead of run.
    pub steer: (f32, f32, bool),
    hover_reply: usize,
    hover_target: Option<ObjId>,
    ui_width: u32,
    overlay: Option<Image>,
    overlay_key: String,
    /// Goes up whenever the overlay changes.
    pub overlay_version: u64,
    cues: Vec<Cue>,
    events_seen: usize,
    line_key: Option<usize>,
    /// The cutscene beat on now, its age in seconds, and the stunt and camera model names it reads.
    beat: Option<Beat>,
    beat_time: f32,
    camera_model: String,
    stunts: Vec<(String, String)>,
    models: HashMap<String, Option<Rc<Model>>>,
    /// The area's fixed cameras by `CameraID`.
    fixed_cameras: HashMap<i32, Camera>,
    /// Seconds until a line with no replies moves on by itself.
    line_left: Option<f32>,
    /// Lines of conversation shown so far.
    pub lines_shown: usize,
    last_status: String,
}

impl Stage {
    /// Load `module` from its original files and enter its area. Conversations wait for the player.
    pub fn new(module: &str) -> Result<Stage, String> {
        Stage::open(module, true)
    }

    /// `new`; without `original` a `.mod` that an installed mod left beside a module's `.rim` pair wins.
    pub fn open(module: &str, original: bool) -> Result<Stage, String> {
        let e = |e: kotor_core::Error| e.to_string();
        let dir = kotor_core::install_dir().ok_or("no KotOR install found")?;
        let mut base = Install::open(&dir).map_err(e)?;
        base.set_prefer_rim(original);
        let mut world = World::new(base).map_err(e)?;
        world.interactive_dialogue = true;
        world.load_module(module).map_err(e)?;
        // **Assumption:** until there is a character creation screen the player is the recommended Soldier.
        let spec = world
            .recommended_character("Player", 0, 1)
            .ok_or("no recommended character")?;
        world.create_character(&spec)?;
        let mut inst = Install::open(&dir).map_err(e)?;
        inst.set_prefer_rim(original);
        inst.load_texture_packs().map_err(e)?;
        let ui_inst: &'static mut Install = Box::leak(Box::new(Install::open(&dir).map_err(e)?));
        let mut s = Stage {
            game: Game::new(world),
            inst,
            ui: Ui::new(ui_inst),
            dir,
            original,
            orbit: 0.0,
            module: String::new(),
            floor: None,
            static_scene: Scene::default(),
            static_version: 0,
            rigs: HashMap::new(),
            doors: HashMap::new(),
            textures: HashMap::new(),
            yaw: 0.0,
            distance: CAMERA_BEHIND,
            turn: 0.0,
            pitch: ((CAMERA_ABOVE - CAMERA_LOOKS_AT) / CAMERA_BEHIND).atan(),
            steer: (0.0, 0.0, false),
            hover_reply: 0,
            hover_target: None,
            ui_width: 853,
            overlay: None,
            overlay_key: String::new(),
            overlay_version: 0,
            cues: Vec::new(),
            events_seen: 0,
            line_key: None,
            beat: None,
            beat_time: 0.0,
            camera_model: String::new(),
            stunts: Vec::new(),
            models: HashMap::new(),
            fixed_cameras: HashMap::new(),
            line_left: None,
            lines_shown: 0,
            last_status: String::new(),
        };
        s.enter_module()?;
        s.yaw = s.game.world.pc_mover.heading;
        s.game.world.enter_area();
        s.set_view((16, 9));
        Ok(s)
    }

    fn area_name(&self) -> String {
        let w = &self.game.world;
        w.object(w.area).map(|a| a.tag.clone()).unwrap_or_default()
    }

    /// The world is in a new module: rebuild what is drawn and start its music.
    fn enter_module(&mut self) -> Result<(), String> {
        let e = |e: kotor_core::Error| e.to_string();
        self.module = self.game.world.module.clone();
        self.inst.load_module(&self.module).map_err(e)?;
        let area = self.area_name();
        let mut scene = kotor_render::load_area(&mut self.inst, &area).map_err(e)?;
        kotor_render::add_area_objects_with(&mut scene, &mut self.inst, &area, false).map_err(e)?;
        self.static_scene = scene;
        self.static_version += 1;
        self.floor = Floor::load(&mut self.inst, &area).ok();
        self.doors.clear();
        self.events_seen = 0;
        self.fixed_cameras = fixed_cameras(&mut self.inst, &area);
        let music = (|| {
            let git = Gff::parse(&self.inst.read(&area, ResType::GIT).ok()?).ok()?;
            let props = git.root.strukt("AreaProperties")?;
            Some(props.int("MusicDay")? as usize)
        })();
        match music.and_then(|row| self.music_file(row)) {
            Some(p) => self.cues.push(Cue::Music(p)),
            None => self.cues.push(Cue::StopMusic),
        }
        Ok(())
    }

    /// The file of an `ambientmusic.2da` row.
    fn music_file(&mut self, row: usize) -> Option<PathBuf> {
        let t = TwoDa::parse(&self.inst.read("ambientmusic", ResType::TWODA).ok()?).ok()?;
        let name = t.get(row, "resource")?.to_string();
        let p = self.dir.join("streammusic").join(format!("{name}.wav"));
        p.is_file().then_some(p)
    }

    fn sound_file(&self, resref: &str) -> Option<PathBuf> {
        if resref.is_empty() {
            return None;
        }
        ["streamwaves", "streamsounds"]
            .iter()
            .map(|d| self.dir.join(d).join(format!("{resref}.wav")))
            .find(|p| p.is_file())
    }

    /// Advance the game by `dt` seconds of real time. Returns what to play.
    pub fn update(&mut self, dt: f32) -> Vec<Cue> {
        let (ahead, right, walk) = self.steer;
        let (cos, sin) = (self.yaw.cos(), self.yaw.sin());
        let dir = [cos * ahead + sin * right, sin * ahead - cos * right];
        let len = dir[0].hypot(dir[1]);
        self.game.world.steer = (len > 0.01).then(|| ([dir[0] / len, dir[1] / len], !walk));
        self.game.advance(dt);
        if self.game.world.module != self.module {
            if let Err(e) = self.enter_module() {
                self.game.world.errors.push(format!("stage: {e}"));
            }
        }
        self.scan_music_events();
        self.update_line(dt);
        self.update_camera(dt);
        self.refresh_overlay();
        std::mem::take(&mut self.cues)
    }

    /// Scripts change the music with `MusicBackgroundChangeDay(area, row)` and `MusicBackgroundStop(area)`.
    fn scan_music_events(&mut self) {
        let n = self.game.world.events.len();
        let new: Vec<String> = self.game.world.events[self.events_seen.min(n)..].to_vec();
        self.events_seen = n;
        for ev in new {
            if ev.starts_with("MusicBackgroundStop(") {
                self.cues.push(Cue::StopMusic);
            } else if let Some(args) = ev.strip_prefix("MusicBackgroundChangeDay(") {
                let row = args
                    .trim_end_matches(')')
                    .rsplit(", ")
                    .next()
                    .and_then(|r| r.parse::<usize>().ok());
                if let Some(p) = row.and_then(|r| self.music_file(r)) {
                    self.cues.push(Cue::Music(p));
                }
            }
        }
    }

    /// A new node starts its voice, its beat and its timer; a node with no replies moves on when the timer ends.
    fn update_line(&mut self, dt: f32) {
        let Some((_, _, vo, replies)) = self.game.world.dialogue_view() else {
            if self.line_key.take().is_some() {
                self.cues.push(Cue::StopVoice);
            }
            self.line_left = None;
            self.beat = None;
            return;
        };
        let key = self.game.world.dialogue_serial();
        if self.line_key != key {
            self.line_key = key;
            self.lines_shown += 1;
            self.cues.push(match self.sound_file(&vo) {
                Some(p) => Cue::Voice(p),
                None => Cue::StopVoice,
            });
            self.beat = self.game.world.dialogue_beat();
            self.face_speaker();
            self.beat_time = 0.0;
            if let Some(c) = self.game.world.dialogue_conversation() {
                self.camera_model = c.camera_model;
                self.stunts = c.stunts;
            }
            let names: Vec<String> = std::iter::once(self.camera_model.clone())
                .chain(self.stunts.iter().map(|(_, m)| m.clone()))
                .collect();
            for n in names {
                self.model(&n);
            }
            let seconds = self.beat.as_ref().map_or(1.5, |b| b.length + b.hold);
            self.line_left = replies.is_empty().then_some(seconds);
        } else {
            self.beat_time += dt;
        }
        if let Some(left) = self.line_left.as_mut() {
            *left -= dt;
            if *left <= 0.0 {
                self.line_left = None;
                self.game.world.dialogue_choose(0);
            }
        }
    }

    /// A model by name, parsed once.
    fn model(&mut self, name: &str) -> Option<Rc<Model>> {
        let key = name.to_ascii_lowercase();
        if !self.models.contains_key(&key) {
            let m = load_model(&mut self.inst, &key).map(Rc::new);
            self.models.insert(key.clone(), m);
        }
        self.models[&key].clone()
    }

    fn loaded(&self, name: &str) -> Option<&Rc<Model>> {
        self.models.get(&name.to_ascii_lowercase())?.as_ref()
    }

    /// What the beat's camera sees, if the node names one. The camera model's `camerahook` is in world space and
    /// looks along its local -z with y up. Measured: in the opening's 18 shots the stunt's head lies at -0.91 to -1.00
    /// along that axis.
    fn beat_camera(&self) -> Option<Camera> {
        match self.beat.as_ref()?.camera.as_ref()? {
            CameraShot::Animated { animation, fov, .. } => {
                let model = self.loaded(&self.camera_model)?;
                let a = model
                    .animations
                    .get(animation.checked_sub(FIRST_STUNT_ANIMATION)? as usize)?;
                let pose = a.sample(model, self.beat_time.min(a.length));
                let mats = model.world_matrices(Some(&pose));
                let m = mats
                    .iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case("camerahook"))?
                    .1;
                let eye = [m[0][3], m[1][3], m[2][3]];
                Some(Camera {
                    eye,
                    target: [eye[0] - m[0][2], eye[1] - m[1][2], eye[2] - m[2][2]],
                    fov: fov.unwrap_or(CAMERA_FOV),
                })
            }
            CameraShot::Static { id, fov } => {
                let mut c = *self.fixed_cameras.get(id)?;
                if let Some(f) = fov {
                    c.fov = *f;
                }
                Some(c)
            }
            CameraShot::Angle { .. } => None,
        }
    }

    /// A close shot of whoever owns the conversation, from the player's side.
    ///
    /// **Assumption:** this stands in for the game's dialogue camera angles, whose codes are not modelled. A script may
    /// start a talk from metres away (Trask's first talk is from 7.5 m), and this keeps the speaker in shot.
    fn speaker_camera(&self) -> Option<Camera> {
        let w = &self.game.world;
        let o = w.object(w.dialogue_owner()?)?;
        if o.kind != Kind::Creature || o.id == w.pc {
            return None;
        }
        let (at, here) = (o.position, self.pc_position());
        let (dx, dy) = (here[0] - at[0], here[1] - at[1]);
        let len = dx.hypot(dy).max(0.01);
        let (dx, dy) = (dx / len, dy / len);
        Some(Camera {
            eye: [
                at[0] + dx * 1.3 - dy * 0.35,
                at[1] + dy * 1.3 + dx * 0.35,
                at[2] + 1.6,
            ],
            target: [at[0], at[1], at[2] + 1.5],
            fov: 40.0,
        })
    }

    /// Speaker and player turn to each other.
    fn face_speaker(&mut self) {
        let w = &self.game.world;
        let pc = w.pc;
        let Some(owner) = w.dialogue_owner().filter(|o| *o != pc) else {
            return;
        };
        let is_creature = w.object(owner).is_some_and(|o| o.kind == Kind::Creature);
        let at = w.object(owner).map_or([0.0; 3], |o| o.position);
        let here = self.pc_position();
        let to_speaker = (at[1] - here[1]).atan2(at[0] - here[0]);
        self.game.face(pc, to_speaker);
        if is_creature {
            self.game.face(owner, to_speaker + std::f32::consts::PI);
        }
    }

    /// The pose a creature's cutscene stunt puts it in this beat.
    fn stunt_pose(&self, id: ObjId) -> Option<kotor_model::Pose> {
        let w = &self.game.world;
        let beat = self.beat.as_ref()?;
        let tag = w.object(id)?.tag.clone();
        let (who, anim) = beat.step.animations.iter().find(|(p, _)| {
            (id == w.pc && p.eq_ignore_ascii_case("PLAYER")) || p.eq_ignore_ascii_case(&tag)
        })?;
        let (_, name) = self
            .stunts
            .iter()
            .find(|(p, _)| p.eq_ignore_ascii_case(who))?;
        let stunt = self.loaded(name)?;
        let a = stunt
            .animations
            .get(anim.checked_sub(FIRST_STUNT_ANIMATION)? as usize)?;
        Some(a.sample(stunt, self.beat_time.min(a.length)))
    }

    fn update_camera(&mut self, dt: f32) {
        self.yaw += self.turn.to_radians() * dt;
        let w = &self.game.world;
        if !w.pc_walking() && !w.in_dialogue() {
            self.yaw += self.orbit.to_radians() * dt;
        }
        // In a conversation look past the player at the speaker; on a walk swing slowly round behind the player.
        let here = self.pc_position();
        let wanted = match w.dialogue_owner().and_then(|id| w.object(id)) {
            Some(o) if o.id != w.pc => {
                Some((o.position[1] - here[1]).atan2(o.position[0] - here[0]))
            }
            _ => w.pc_walking().then_some(w.pc_mover.heading),
        };
        if let Some(a) = wanted {
            let tau = std::f32::consts::TAU;
            let d = (a - self.yaw + tau * 1.5).rem_euclid(tau) - tau / 2.0;
            self.yaw += d * (1.5 * dt).min(1.0);
        }
    }

    fn pc_position(&self) -> [f32; 3] {
        let w = &self.game.world;
        w.object(w.pc).map_or([0.0; 3], |o| o.position)
    }

    /// The camera behind the player, drawn in where the floor ends so that it stays on this side of the walls.
    pub fn camera(&self) -> Camera {
        if let Some(c) = self.beat_camera().or_else(|| self.speaker_camera()) {
            return c;
        }
        let p = self.pc_position();
        let target = [p[0], p[1], p[2] + CAMERA_LOOKS_AT];
        let (dx, dy) = (self.yaw.cos(), self.yaw.sin());
        let (flat, up) = (self.pitch.cos(), self.pitch.sin());
        let mut d = 0.8;
        if let Some(f) = &self.floor {
            while d < self.distance
                && f.height(
                    p[0] - dx * flat * (d + 0.3),
                    p[1] - dy * flat * (d + 0.3),
                    p[2] + 1.0,
                )
                .is_some()
            {
                d += 0.1;
            }
        } else {
            d = self.distance;
        }
        Camera {
            eye: [
                p[0] - dx * flat * d,
                p[1] - dy * flat * d,
                p[2] + CAMERA_LOOKS_AT + up * d,
            ],
            target,
            fov: CAMERA_FOV,
        }
    }

    /// Drag with the right button: turn the camera round the player and tilt it.
    pub fn look(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * 0.005;
        self.pitch = (self.pitch + dy * 0.004).clamp(-0.2, 1.2);
    }

    /// What the replies on screen are, in the conversation or the open container.
    fn choice_view(&self) -> Option<DialogueView> {
        let w = &self.game.world;
        if let Some((speaker, subtitle, _, replies)) = w.dialogue_view() {
            return Some(DialogueView {
                speaker,
                subtitle,
                replies,
                selected: self.hover_reply,
            });
        }
        let id = w.opened?;
        let mut replies: Vec<String> = w.container_items().into_iter().map(|i| i.1).collect();
        if !replies.is_empty() {
            replies.push("Take everything".to_string());
        }
        replies.push("Close".to_string());
        Some(DialogueView {
            speaker: w.object(id).map(|o| o.name.clone()).unwrap_or_default(),
            subtitle: String::new(),
            replies,
            selected: self.hover_reply,
        })
    }

    /// The row of the choice list under a pixel.
    fn reply_at(&mut self, pixel: (f32, f32), size: (u32, u32)) -> Option<usize> {
        let view = self.choice_view()?;
        if view.replies.is_empty() || size.0 == 0 || size.1 == 0 {
            return None;
        }
        let rows = self.ui.reply_rows(&view)?;
        let x = pixel.0 * self.ui_width as f32 / size.0 as f32;
        let y = pixel.1 * 480.0 / size.1 as f32;
        rows.iter().position(|r| {
            x >= r[0] as f32
                && x < (r[0] + r[2]) as f32
                && y >= r[1] as f32
                && y < (r[1] + r[3]) as f32
        })
    }

    /// The cursor moved: light the reply under it, or name what is under it.
    pub fn hover(&mut self, pixel: (f32, f32), size: (u32, u32)) {
        if self.choice_view().is_some() {
            if let Some(i) = self.reply_at(pixel, size) {
                self.hover_reply = i;
            }
            return;
        }
        let cam = self.camera();
        let (o, d) = pixel_ray(cam.eye, cam.target, cam.fov, size, pixel);
        self.hover_target = self.game.world.pick(o, d);
    }

    /// Use a thing: an enemy is attacked, anything else is walked to and talked to, opened or used.
    fn use_thing(&mut self, id: ObjId) {
        let pc = self.game.world.pc;
        let w = &mut self.game.world;
        let enemy =
            w.object(id).is_some_and(|t| t.kind == Kind::Creature) && w.reputation(pc, id) <= 10;
        if enemy {
            w.attack(pc, id);
            let at = w.object(id).map(|t| t.position);
            if let Some(at) = at {
                w.walk_to(at);
            }
        } else {
            w.click(id);
        }
    }

    /// The E key: use the nearest thing in reach.
    pub fn interact(&mut self) {
        if self.choice_view().is_some() {
            return;
        }
        if let Some(id) = self.game.world.nearest_usable(USE_REACH) {
            self.use_thing(id);
        }
    }

    /// A left click at `pixel` of a window of `size`: acknowledge a line, or use what is under the cursor, or walk.
    pub fn click(&mut self, pixel: (f32, f32), size: (u32, u32)) {
        if self.choice_view().is_some() {
            match self.reply_at(pixel, size) {
                Some(i) => self.number_key(i + 1),
                None => self.continue_line(),
            }
            return;
        }
        let cam = self.camera();
        let (o, d) = pixel_ray(cam.eye, cam.target, cam.fov, size, pixel);
        if let Some(id) = self.game.world.pick(o, d) {
            self.use_thing(id);
            return;
        }
        if let Some(p) = self.floor_under_ray(o, d) {
            self.game.world.walk_to(p);
        }
    }

    /// The first walkable floor a ray meets (a 5 cm march, at most 60 m).
    pub fn floor_under_ray(&self, o: [f32; 3], d: [f32; 3]) -> Option<[f32; 3]> {
        let f = self.floor.as_ref()?;
        (1..1200).find_map(|i| {
            let t = i as f32 * 0.05;
            let p = [o[0] + d[0] * t, o[1] + d[1] * t, o[2] + d[2] * t];
            let h = f.height(p[0], p[1], p[2] + 0.1)?;
            (p[2] <= h + 0.05).then_some([p[0], p[1], h])
        })
    }

    /// Key `n` (1-9): take that reply.
    pub fn number_key(&mut self, n: usize) {
        self.hover_reply = 0;
        if !self.game.world.in_dialogue() && self.game.world.opened.is_some() {
            let items = self.game.world.container_items();
            if n >= 1 && n <= items.len() {
                self.game.world.take_item(items[n - 1].0);
            } else if n == items.len() + 1 && !items.is_empty() {
                for (i, _) in items {
                    self.game.world.take_item(i);
                }
            } else if n >= 1 && n <= items.len() + 2 {
                self.game.world.opened = None;
            }
            return;
        }
        let count = self
            .game
            .world
            .dialogue_view()
            .map_or(0, |(_, _, _, r)| r.len());
        if n >= 1 && n <= count {
            self.line_left = None;
            self.game.world.dialogue_choose(n - 1);
        }
    }

    /// Space or a click: finish a line that has no replies.
    pub fn continue_line(&mut self) {
        if !self.game.world.in_dialogue() {
            self.game.world.opened = None;
            return;
        }
        let waiting = self
            .game
            .world
            .dialogue_view()
            .is_some_and(|(_, _, _, r)| r.is_empty());
        if waiting {
            self.line_left = None;
            self.game.world.dialogue_choose(0);
        }
    }

    pub fn static_scene(&self) -> &Scene {
        &self.static_scene
    }

    /// A named texture, decoded once.
    pub fn texture(&mut self, name: &str) -> Option<Image> {
        let inst = &mut self.inst;
        self.textures
            .entry(name.to_string())
            .or_insert_with(|| kotor_render::load_texture(inst, name))
            .clone()
    }

    fn rig_of(&mut self, id: ObjId) -> Option<Rc<Rig>> {
        let (appearance, template) = {
            let o = self.game.world.object(id)?;
            (o.appearance, o.template.clone())
        };
        let var = self
            .inst
            .read(&template, ResType(2027))
            .ok()
            .and_then(|b| Gff::parse(&b).ok())
            .and_then(|g| g.root.int("TextureVar"))
            .unwrap_or(1) as u32;
        let inst = &mut self.inst;
        self.rigs
            .entry((appearance, var))
            .or_insert_with(|| {
                kotor_render::rig_for_appearance(inst, appearance as usize, var)
                    .ok()
                    .map(Rc::new)
            })
            .clone()
    }

    fn door_body(&mut self, id: ObjId) -> Option<Rc<DoorBody>> {
        if let Some(b) = self.doors.get(&id) {
            return b.clone();
        }
        let body = self.load_door(id).map(Rc::new);
        self.doors.insert(id, body.clone());
        body
    }

    /// A door's model comes from `genericdoors.2da`; its bearing is in the GIT entry at the door's position.
    fn load_door(&mut self, id: ObjId) -> Option<DoorBody> {
        let (pos, template) = {
            let o = self.game.world.object(id)?;
            (o.position, o.template.clone())
        };
        let area = self.area_name();
        let git = Gff::parse(&self.inst.read(&area, ResType::GIT).ok()?).ok()?;
        let at = |s: &kotor_gff::Struct, l: &str| s.float(l).unwrap_or(0.0) as f32;
        let bearing = git
            .root
            .list("Door List")?
            .iter()
            .find(|d| (at(d, "X") - pos[0]).abs() + (at(d, "Y") - pos[1]).abs() < 0.01)
            .map_or(0.0, |d| at(d, "Bearing"));
        let utd = Gff::parse(&self.inst.read(&template, ResType(2042)).ok()?).ok()?;
        let table = TwoDa::parse(&self.inst.read("genericdoors", ResType::TWODA).ok()?).ok()?;
        let name = table
            .get(utd.root.int("GenericType")? as usize, "modelname")?
            .to_string();
        Some(DoorBody {
            model: load_model(&mut self.inst, &name)?,
            xf: placement(pos, bearing),
        })
    }

    /// Everything that moves, posed for this frame: creatures near the player and every door.
    pub fn dynamic_scene(&mut self) -> Scene {
        let mut scene = Scene::default();
        let here = self.pc_position();
        for pose in self.game.poses() {
            match pose.kind {
                Kind::Creature => {
                    let (dx, dy) = (pose.position[0] - here[0], pose.position[1] - here[1]);
                    if (dx * dx + dy * dy).sqrt() > DRAW_RANGE {
                        continue;
                    }
                    let Some(rig) = self.rig_of(pose.id) else {
                        continue;
                    };
                    if let Some(p) = self.stunt_pose(pose.id) {
                        // A stunt is in world space, like the camera beside it.
                        if let Ok(body) = rig.posed(Some(&p)) {
                            scene.append_transformed(&body, &placement([0.0; 3], 0.0));
                        }
                        continue;
                    }
                    self.add_creature(&mut scene, &rig, &pose.clip, pose.position, pose.heading);
                }
                Kind::Door => {
                    let Some(body) = self.door_body(pose.id) else {
                        continue;
                    };
                    let sampled = pose.clip.as_ref().and_then(|c| {
                        body.model
                            .animation(c.name)
                            .map(|a| a.sample(&body.model, c.time.min(a.length)))
                    });
                    scene.add_model_with(&body.model, sampled.as_ref(), &body.xf, &[], None, None);
                }
                _ => {}
            }
        }
        scene
    }

    fn add_creature(
        &self,
        scene: &mut Scene,
        rig: &Rig,
        clip: &Option<Clip>,
        at: [f32; 3],
        heading: f32,
    ) {
        // Models face +y at rest; turn so +y points along the heading.
        let angle = heading - std::f32::consts::FRAC_PI_2;
        let name = clip.as_ref().map_or("pause1", |c| c.name);
        let len = rig.animation_length(name);
        // **Assumption:** a body whose supermodels have no `die` clip is laid flat instead.
        let flat = name == "die" && len.is_none();
        let (name, len) = if len.is_some() {
            (name, len)
        } else {
            ("pause1", rig.animation_length("pause1"))
        };
        let t = clip.as_ref().map_or(0.0, |c| c.time);
        let t = len
            .filter(|l| *l > 0.0)
            .map_or(0.0, |l| if name == "die" { t.min(l) } else { t % l });
        let Ok(body) = rig.pose(len.map(|_| (name, t))) else {
            return;
        };
        let xf = if flat {
            placement_flat(at, angle)
        } else {
            placement(at, angle)
        };
        scene.append_transformed(&body, &xf);
    }

    /// The picture over the scene: the dialogue box in a conversation, otherwise the HUD.
    pub fn overlay(&self) -> Option<&Image> {
        self.overlay.as_ref()
    }

    fn refresh_overlay(&mut self) {
        let view = self.choice_view();
        let party = self.party_view();
        let target = self.target_view();
        let cut = self.beat_camera().is_some();
        let key = match &view {
            Some(v) => format!(
                "{}d{:?}{:?}{}{}{cut}",
                self.module, v.speaker, v.replies, v.subtitle, v.selected
            ),
            None => format!(
                "{}h{:?}{target:?}{cut}",
                self.module,
                party
                    .iter()
                    .map(|m| (&m.portrait, m.hp, m.max_hp, m.force, m.max_force))
                    .collect::<Vec<_>>()
            ),
        };
        if key == self.overlay_key {
            return;
        }
        self.overlay_key = key;
        self.overlay = match view {
            // A wordless beat: the scene alone.
            Some(v) if v.subtitle.trim().is_empty() && v.replies.is_empty() => {
                self.ui.blank_overlay()
            }
            Some(v) => self.ui.dialogue_overlay(&v),
            None => {
                let t = target.as_ref();
                self.ui.hud_overlay(
                    &party,
                    t.map(|t| (t.0.as_str(), t.1, t.2)),
                    t.map_or("", |t| t.3),
                )
            }
        };
        let label = self.label();
        self.overlay = self.overlay.take().and_then(|o| self.ui.caption(o, &label));
        self.overlay_version += 1;
    }

    /// The thing under the cursor, else the nearest in reach: name, hit points, and what using it does.
    fn target_view(&self) -> Option<(String, i32, i32, &'static str)> {
        let w = &self.game.world;
        let id = self
            .hover_target
            .filter(|id| w.object(*id).is_some_and(|o| !o.dead))
            .or_else(|| w.nearest_usable(USE_REACH))?;
        let o = w.object(id)?;
        let name = if o.name.is_empty() {
            o.tag.clone()
        } else {
            o.name.clone()
        };
        Some(match o.kind {
            Kind::Creature if w.reputation(w.pc, id) <= 10 => (name, o.hp, o.max_hp, "Attack"),
            Kind::Creature => (name, o.hp, o.max_hp, "Talk"),
            Kind::Door => (name, 1, 1, if o.open { "Close" } else { "Open" }),
            _ => (name, 1, 1, "Use"),
        })
    }

    fn party_view(&self) -> Vec<HudMember> {
        let w = &self.game.world;
        std::iter::once(w.pc)
            .chain(w.party.iter().copied().filter(|id| *id != w.pc))
            .filter_map(|id| w.object(id))
            .take(3)
            .map(|o| HudMember {
                portrait: if o.portrait.is_empty() {
                    "po_pfha2".to_string()
                } else {
                    o.portrait.clone()
                },
                hp: o.hp,
                max_hp: o.max_hp,
                force: o.force,
                max_force: o.max_force,
            })
            .collect()
    }

    /// What is on screen, in words: the module, which files it came from, the area and the camera.
    pub fn label(&self) -> String {
        let installed = !self.original
            && self
                .dir
                .join("modules")
                .join(format!("{}.mod", self.module))
                .is_file();
        format!(
            "kotor-rs | {} | {} | area {} | camera {}",
            self.module,
            if installed {
                "installed .mod"
            } else {
                "original files"
            },
            self.area_name(),
            if self.beat_camera().is_some() {
                "from the cutscene"
            } else {
                "on the player"
            }
        )
    }

    /// The window's shape: the interface is laid out 480 units high and as wide as that makes it.
    pub fn set_view(&mut self, size: (u32, u32)) {
        if let Some(width) = (480 * size.0).checked_div(size.1) {
            self.ui_width = width.max(640);
            self.ui.set_width(width);
            self.overlay_key.clear();
            self.refresh_overlay();
        }
    }

    /// One line of what is going on, when it changes (the window logs it).
    pub fn status(&mut self) -> Option<String> {
        let s = match self.game.world.dialogue_view() {
            Some((who, text, _, r)) => format!("{who}: {text} [{} replies]", r.len()),
            None => format!("{} at {:?}", self.module, self.pc_position()),
        };
        if self.game.world.in_dialogue() && s != self.last_status {
            self.last_status = s.clone();
            return Some(s);
        }
        None
    }
}
