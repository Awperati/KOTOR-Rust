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

#[path = "menus.rs"]
mod menus;
pub use menus::{default_saves_dir, default_settings_file, Settings};

type Mat = [[f32; 4]; 4];

/// Creatures farther than this from the player are not drawn (metres).
const DRAW_RANGE: f32 = 45.0;
/// Animation ids from here index a stunt model's animations (see `kotor_game::cutscene`).
const FIRST_STUNT_ANIMATION: u32 = 1200;
/// The follow camera: metres behind the player and above its feet, and the height it looks at. As `render_walk`.
const CAMERA_FOV: f32 = 55.0;
/// How near the camera's line may pass a closed door (metres): about half a door.
const DOOR_CAMERA_MARGIN: f32 = 1.2;
/// `Equip_ItemList` slots: the armour is measured from the data (`g_a_*` items have this bit); the right hand is the one
/// combat reads.
const ARMOUR_SLOT: i32 = 1;
const RIGHT_HAND_SLOT: i32 = 4;
const LEFT_HAND_SLOT: i32 = 5;
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
    /// A short sound, as the bytes of its file.
    Effect(Vec<u8>),
    /// The player's levels for music, voice and effects, 0 to 1.
    Levels(f32, f32, f32),
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

/// Templates (lower case) of an area's placeables that hold things and have a `close2open` animation.
fn chest_templates(inst: &mut Install, area: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Some(git) = inst
        .read(area, ResType::GIT)
        .ok()
        .and_then(|b| Gff::parse(&b).ok())
    else {
        return out;
    };
    let Some(table) = inst
        .read("placeables", ResType::TWODA)
        .ok()
        .and_then(|b| TwoDa::parse(&b).ok())
    else {
        return out;
    };
    for p in git.root.list("Placeable List").unwrap_or_default() {
        let Some(t) = p.str("TemplateResRef").map(str::to_ascii_lowercase) else {
            continue;
        };
        if out.contains(&t) {
            continue;
        }
        let Some(u) = inst
            .read(&t, ResType(2044))
            .ok()
            .and_then(|b| Gff::parse(&b).ok())
        else {
            continue;
        };
        if u.root.int("HasInventory").unwrap_or(0) == 0 {
            continue;
        }
        let name = u
            .root
            .int("Appearance")
            .and_then(|r| table.get(r as usize, "modelname").map(str::to_string));
        let opens = name
            .and_then(|n| load_model(inst, &n))
            .is_some_and(|m| m.animation("close2open").is_some());
        if opens {
            out.push(t);
        }
    }
    out
}

/// Where upscaled pictures live: outside the repository and the install, since they are made from game files.
pub fn default_hd_dir() -> Option<PathBuf> {
    Some(
        PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
            .join("kotor-rs")
            .join("hd"),
    )
}

/// An upscaled 3D texture: `<name>.png`, with its alpha from `<name>__a.png` when there is one.
fn upscaled(dir: &std::path::Path, name: &str) -> Option<Image> {
    let key = name.to_ascii_lowercase();
    let read = |n: String| kotor_image::parse_png(&std::fs::read(dir.join(n)).ok()?);
    let mut img = read(format!("{key}.png"))?;
    if let Some(a) = read(format!("{key}__a.png")) {
        if (a.width, a.height) != (img.width, img.height) {
            return None;
        }
        for (p, m) in img.rgba.chunks_mut(4).zip(a.rgba.chunks(4)) {
            p[3] = m[0];
        }
    }
    Some(img)
}

/// The stock icon for a target's action, read from the texture names in `swpc_tex_gui.erf`.
fn action_icon(action: &str) -> &'static str {
    match action {
        "Attack" => "i_attack",
        "Talk" => "i_dialog",
        "Open" | "Close" => "i_opendoor",
        _ => "i_openplace",
    }
}

fn load_model(inst: &mut Install, name: &str) -> Option<Model> {
    let mdl = inst.read(name, ResType::MDL).ok()?;
    let mdx = inst.read(name, ResType::MDX).unwrap_or_default();
    parse_model(&mdl, &mdx).ok()
}

pub struct Stage {
    pub game: Game,
    /// Game seconds to each second of wall-clock time. Only game time speeds up: see `docs/FASTTEST.md`.
    pub speed: f32,
    /// A scripted player at the controls. It alone moves conversations on.
    pub pilot: Option<kotor_game::Autopilot>,
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
    rigs: HashMap<(i32, u32, Option<char>), Option<Rc<Rig>>>,
    pub effects: crate::effects::Effects,
    /// Templates of the placeables that hold things, drawn each frame so that their lids can move.
    chests: Vec<String>,
    chest_bodies: HashMap<ObjId, Option<Rc<DoorBody>>>,
    /// Seconds since each chest was opened.
    lids: HashMap<ObjId, f32>,
    log_seen: usize,
    door_seen: HashMap<ObjId, bool>,
    /// Seconds to the player's next footstep, and which of the three sounds.
    step_due: f32,
    step_turn: usize,
    /// Upscaled 3D textures, when `scripts/upscale_area.ps1` has made them.
    hd3d: Option<PathBuf>,
    baseitems: Option<TwoDa>,
    /// An item's model, by item.
    held: HashMap<ObjId, Option<Rc<Model>>>,
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
    menus: menus::Menus,
    /// A game is under way: a module is loaded and drawn. Off on the main menu.
    started: bool,
    /// Where New Game starts.
    first_module: String,
}

impl Stage {
    /// Load `module` from its original files and enter its area. Conversations wait for the player.
    pub fn new(module: &str) -> Result<Stage, String> {
        Stage::open(module, true)
    }

    /// `new`; without `original` a `.mod` that an installed mod left beside a module's `.rim` pair wins.
    pub fn open(module: &str, original: bool) -> Result<Stage, String> {
        let mut s = Stage::boot(original)?;
        s.first_module = module.to_string();
        s.start_module(module)?;
        Ok(s)
    }

    /// Open on the main menu, with no game running. New Game starts `module`.
    pub fn menu(module: &str, original: bool) -> Result<Stage, String> {
        let mut s = Stage::boot(original)?;
        s.first_module = module.to_string();
        s.menus.stack.push(menus::Screen::Main);
        s.refresh_overlay();
        Ok(s)
    }

    /// A world for a new game: the player exists and no module is loaded.
    fn fresh_world(dir: &std::path::Path, original: bool) -> Result<World, String> {
        let e = |e: kotor_core::Error| e.to_string();
        let mut base = Install::open(dir).map_err(e)?;
        base.set_prefer_rim(original);
        let mut world = World::new(base).map_err(e)?;
        world.interactive_dialogue = true;
        world.hold_levels = true;
        world.party_screen = true;
        Ok(world)
    }

    /// Everything but the game: the files, the interface and an empty scene.
    fn boot(original: bool) -> Result<Stage, String> {
        let e = |e: kotor_core::Error| e.to_string();
        let dir = kotor_core::install_dir().ok_or("no KotOR install found")?;
        let world = Stage::fresh_world(&dir, original)?;
        let mut inst = Install::open(&dir).map_err(e)?;
        inst.set_prefer_rim(original);
        inst.load_texture_packs().map_err(e)?;
        let ui_inst: &'static mut Install = Box::leak(Box::new(Install::open(&dir).map_err(e)?));
        let mut s = Stage {
            game: Game::new(world),
            speed: 1.0,
            pilot: None,
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
            baseitems: None,
            effects: Default::default(),
            chests: Vec::new(),
            chest_bodies: HashMap::new(),
            lids: HashMap::new(),
            log_seen: 0,
            door_seen: HashMap::new(),
            step_due: 0.0,
            step_turn: 0,
            hd3d: default_hd_dir()
                .and_then(|d| d.parent().map(|p| p.join("hd3d")))
                .filter(|d| d.is_dir()),
            held: HashMap::new(),
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
            menus: menus::Menus::default(),
            started: false,
            first_module: String::new(),
        };
        // Upscaled interface pictures, when `scripts/upscale_ui.ps1` has made them.
        let hd = default_hd_dir().filter(|d| d.is_dir());
        s.ui.set_hd_dir(hd);
        s.set_view((1280, 720));
        Ok(s)
    }

    /// A new game in `module`, in place of whatever was running.
    fn start_module(&mut self, module: &str) -> Result<(), String> {
        let e = |e: kotor_core::Error| e.to_string();
        let mut world = Stage::fresh_world(&self.dir, self.original)?;
        world.load_module(module).map_err(e)?;
        // **Assumption:** until there is a character creation screen the player is the recommended Soldier.
        let spec = world
            .recommended_character("Player", 0, 1)
            .ok_or("no recommended character")?;
        world.create_character(&spec)?;
        self.game = Game::new(world);
        self.apply_difficulty();
        self.forget_view();
        self.enter_module()?;
        self.started = true;
        self.yaw = self.game.world.pc_mover.heading;
        self.game.world.enter_area();
        self.refresh_overlay();
        Ok(())
    }

    /// Everything kept by object id or about the last game. Ids start over in a new world.
    fn forget_view(&mut self) {
        self.held.clear();
        self.doors.clear();
        self.hover_reply = 0;
        self.hover_target = None;
        self.line_key = None;
        self.line_left = None;
        self.beat = None;
        self.overlay_key.clear();
        self.events_seen = 0;
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
        let chests = chest_templates(&mut self.inst, &area);
        kotor_render::add_area_objects_except(&mut scene, &mut self.inst, &area, false, &chests)
            .map_err(e)?;
        self.chests = chests;
        self.chest_bodies.clear();
        self.lids.clear();
        self.static_scene = scene;
        self.static_version += 1;
        self.floor = Floor::load(&mut self.inst, &area).ok();
        self.doors.clear();
        self.events_seen = 0;
        self.fixed_cameras = fixed_cameras(&mut self.inst, &area);
        self.effects.load_area(&mut self.inst, &area);
        self.log_seen = 0;
        self.door_seen.clear();
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
        self.take_party_request();
        self.notices_tick();
        if self.menu_open() && !self.container_up() {
            self.refresh_overlay();
            return std::mem::take(&mut self.cues);
        }
        if self.pause_tick() {
            self.update_camera(dt);
            self.refresh_overlay();
            return std::mem::take(&mut self.cues);
        }
        let (ahead, right, walk) = self.steer;
        let (cos, sin) = (self.yaw.cos(), self.yaw.sin());
        let dir = [cos * ahead + sin * right, sin * ahead - cos * right];
        let len = dir[0].hypot(dir[1]);
        let held = !self.settings().combat_movement && self.party_blocked();
        self.game.world.steer =
            (len > 0.01 && !held).then(|| ([dir[0] / len, dir[1] / len], !walk));
        let speed = self.speed;
        if let Some(p) = self.pilot.as_mut() {
            // A scripted player opens no screens: levels and forced party members come straight away.
            self.game.world.hold_levels = false;
            self.game.world.party_screen = false;
            let n = self.game.steps_due(dt, speed);
            p.run_steps(&mut self.game, n);
        } else {
            self.game.advance_at(dt, speed);
        }
        let dt_game = dt * speed;
        if self.game.world.module != self.module {
            if let Err(e) = self.enter_module() {
                self.game.world.errors.push(format!("stage: {e}"));
            }
            self.autosave();
        }
        self.scan_music_events();
        self.play_world_sounds(dt_game);
        if let Some(id) = self.game.world.opened {
            self.lids.entry(id).or_insert(0.0);
        }
        for t in self.lids.values_mut() {
            *t += dt_game;
        }
        let eye = self.camera().eye;
        self.effects.update(dt_game, eye);
        self.update_line(dt_game);
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
            self.visual_event(&ev);
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

    /// A visual effect or a Force power shows itself. Scripts record `ApplyEffectToObject(duration, Engine(0, handle),
    /// #target, seconds)`; a power records `... casts power N on ...`.
    fn visual_event(&mut self, ev: &str) {
        if let Some(i) = ev.find(" casts power ") {
            let spell = ev[i + 13..]
                .split(' ')
                .next()
                .and_then(|n| n.parse::<i32>().ok());
            if let Some(spell) = spell {
                self.power_visual(spell);
            }
            return;
        }
        let Some(args) = ev.strip_prefix("ApplyEffectToObject(") else {
            return;
        };
        let handle = args
            .split("Engine(0, ")
            .nth(1)
            .and_then(|r| r.split(')').next())
            .and_then(|n| n.parse::<u32>().ok());
        let target = args
            .split('#')
            .nth(1)
            .and_then(|r| r.split(',').next())
            .and_then(|n| n.trim_end_matches(')').parse::<ObjId>().ok());
        if let (Some(h), Some(t)) = (handle, target) {
            self.effect_visual(h, t, 0);
        }
    }

    /// The visuals in an effect record and whatever it is linked with. A handle is an index plus one.
    fn effect_visual(&mut self, handle: u32, target: ObjId, depth: u32) {
        let Some(rec) = handle
            .checked_sub(1)
            .and_then(|i| self.game.world.effects.get(i as usize))
            .cloned()
        else {
            return;
        };
        if depth > 8 {
            return;
        }
        match rec.name.as_str() {
            "EffectVisualEffect" => {
                if let Some(row) = rec.nums.first() {
                    self.play_visual(*row as i32, target);
                }
            }
            "EffectBeam" => {
                let from = rec.args.iter().find_map(|a| {
                    a.strip_prefix("Object(")?
                        .trim_end_matches(')')
                        .parse::<ObjId>()
                        .ok()
                });
                let row = rec.nums.first().map_or(0, |r| *r as i32);
                let label = self.visual_row(row).map_or(String::new(), |r| r.0);
                let texture = if label.contains("LIGHTNING") {
                    "fx_lightning"
                } else if label.contains("DRAIN") {
                    "fx_drain"
                } else {
                    "fx_beam01"
                };
                let w = &self.game.world;
                let chest = |id: ObjId| {
                    w.object(id)
                        .map(|o| [o.position[0], o.position[1], o.position[2] + 1.1])
                };
                if let (Some(a), Some(b)) = (from.and_then(chest), chest(target)) {
                    self.effects.beam(a, b, texture, 0.8);
                }
            }
            _ => {}
        }
        for l in rec.links {
            self.effect_visual(l, target, depth + 1);
        }
    }

    /// A row of `visualeffects.2da` by its label number: its name and (impact model, height on the body).
    fn visual_row(&mut self, row: i32) -> Option<(String, Option<(String, f32)>)> {
        let t = TwoDa::parse(&self.inst.read("visualeffects", ResType::TWODA).ok()?).ok()?;
        let i = t.labels.iter().position(|l| l == &row.to_string())?;
        let name = t.get(i, "label").unwrap_or("").to_string();
        // **Assumption:** the node columns' heights on a person: head, chest, feet.
        let model = [
            ("imp_headcon_node", 1.6),
            ("imp_impact_node", 1.0),
            ("imp_root_m_node", 0.0),
            ("imp_root_s_node", 0.0),
            ("imp_root_l_node", 0.0),
            ("imp_root_h_node", 0.0),
        ]
        .iter()
        .find_map(|(c, up)| {
            t.get(i, c)
                .filter(|v| !v.is_empty())
                .map(|v| (v.to_string(), *up))
        });
        Some((name, model))
    }

    fn play_visual(&mut self, row: i32, target: ObjId) {
        let Some((_, Some((model, up)))) = self.visual_row(row) else {
            return;
        };
        let Some(at) = self.game.world.object(target).map(|o| o.position) else {
            return;
        };
        if let Some(m) = self.model(&model) {
            self.effects.play_model(&m, [at[0], at[1], at[2] + up]);
        }
    }

    /// A caster's hand visual and sound, from `spells.2da`.
    fn power_visual(&mut self, spell: i32) {
        let caster = self
            .game
            .world
            .last_power
            .iter()
            .find(|(_, p)| p.0 == spell)
            .map(|(id, _)| *id);
        let Some(t) = self
            .inst
            .read("spells", ResType::TWODA)
            .ok()
            .and_then(|b| TwoDa::parse(&b).ok())
        else {
            return;
        };
        let get = |c: &str| {
            t.get(spell.max(0) as usize, c)
                .filter(|v| !v.is_empty() && *v != "****")
                .map(str::to_string)
        };
        let (hand, sound) = (get("casthandvisual"), get("castsound"));
        if let (Some(id), Some(name)) = (caster, hand) {
            let at = self.game.world.object(id).map(|o| o.position);
            if let (Some(at), Some(m)) = (at, self.model(&name)) {
                self.effects.play_model(&m, [at[0], at[1], at[2] + 1.1]);
            }
        }
        if let Some(s) = sound {
            self.effect(&s);
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
        if self.game.world.conversation_paused() {
            return;
        }
        if self.pilot.is_some() {
            return;
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

    /// Whether the line has been said and the replies are waiting.
    pub fn answering(&self) -> bool {
        let said = self.beat.as_ref().map_or(0.0, |b| b.length);
        self.game
            .world
            .dialogue_view()
            .is_some_and(|(_, _, _, replies)| !replies.is_empty())
            && self.beat_time >= said
    }

    /// A close shot of whoever owns the conversation while the line is said, then of the player while the replies wait.
    /// The reference video cuts like this, see `docs/REFERENCE.md`.
    ///
    /// **Assumption:** this stands in for the game's dialogue camera angles, whose codes are not modelled. A script may
    /// start a talk from metres away (Trask's first talk is from 7.5 m), and this keeps the speaker in shot.
    pub fn speaker_camera(&self) -> Option<Camera> {
        let w = &self.game.world;
        let o = w.object(w.dialogue_owner()?)?;
        if o.kind != Kind::Creature || o.id == w.pc {
            return None;
        }
        let (mut at, mut here) = (o.position, self.pc_position());
        if self.answering() {
            std::mem::swap(&mut at, &mut here);
        }
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
        let back = [-dx * flat, -dy * flat, up];
        let mut d = self.distance;
        if let Some(f) = &self.floor {
            // Walls in the walkmesh, and where the floor ends.
            if let Some(hit) = f.ray_distance(target, back, self.distance) {
                d = d.min(hit - 0.25);
            }
            let mut t = 0.8;
            while t < d {
                let q = [p[0] + back[0] * (t + 0.3), p[1] + back[1] * (t + 0.3)];
                if f.height(q[0], q[1], p[2] + 1.0).is_none() {
                    d = t;
                    break;
                }
                t += 0.1;
            }
        }
        // Closed doors are not in the walkmesh.
        for o in self.game.world.objects.values() {
            if o.kind != Kind::Door || o.open {
                continue;
            }
            let (ox, oy) = (o.position[0] - p[0], o.position[1] - p[1]);
            let along = ox * back[0] + oy * back[1];
            let across = (ox - back[0] * along).hypot(oy - back[1] * along);
            if along > 0.0 && along < d && across < DOOR_CAMERA_MARGIN {
                d = (along - 0.4).max(0.8);
            }
        }
        let d = d.max(0.8);
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
        let dy = if self.settings().invert_camera {
            -dy
        } else {
            dy
        };
        // A half on the Mouse Settings slider is the speed this always had.
        let speed = self.settings().mouse_sensitivity * 2.0;
        self.yaw -= dx * 0.005 * speed;
        self.pitch = (self.pitch + dy * 0.004 * speed).clamp(-0.2, 1.2);
    }

    /// What the replies on screen are, in the conversation.
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
        None
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
        if self.menu_open() {
            return self.menu_hover(pixel, size);
        }
        if self.choice_view().is_some() {
            if let Some(i) = self.reply_at(pixel, size) {
                self.hover_reply = i;
            }
            return;
        }
        let at = (
            pixel.0 * self.ui_width as f32 / size.0.max(1) as f32,
            pixel.1 * 480.0 / size.1.max(1) as f32,
        );
        let on_hud = self.ui.hud_hit(at.0, at.1);
        self.set_hud_hover(on_hud);
        let cam = self.camera();
        let (o, d) = pixel_ray(cam.eye, cam.target, cam.fov, size, pixel);
        // The bar's slots stay where they are while the cursor is on them, whatever lies behind.
        if !self.over_target_slots() {
            self.hover_target = self.game.world.pick(o, d);
        }
    }

    /// Use a thing: an enemy is attacked, anything else is walked to and talked to, opened or used.
    fn use_thing(&mut self, id: ObjId) {
        let pc = self.game.world.pc;
        let w = &mut self.game.world;
        let enemy = w
            .object(id)
            .is_some_and(|t| t.kind == Kind::Creature && !t.dead)
            && w.reputation(pc, id) <= 10;
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
        if self.menu_open() || self.choice_view().is_some() {
            return;
        }
        if let Some(id) = self.game.world.nearest_usable(USE_REACH) {
            self.use_thing(id);
        }
    }

    /// A left click at `pixel` of a window of `size`: acknowledge a line, or use what is under the cursor, or walk.
    pub fn click(&mut self, pixel: (f32, f32), size: (u32, u32)) {
        if self.menu_open() {
            return self.menu_click(pixel, size);
        }
        if self.choice_view().is_some() {
            match self.reply_at(pixel, size) {
                Some(i) => self.number_key(i + 1),
                None => self.continue_line(),
            }
            return;
        }
        // The first action slot (`LBH_BORDER1` in `maininterface.gui`) uses the target it shows.
        let (x, y) = (
            pixel.0 * self.ui_width as f32 / size.0.max(1) as f32,
            pixel.1 * 480.0 / size.1.max(1) as f32,
        );
        if let Some(tag) = self.ui.hud_hit(x, y) {
            self.hud_click_sound(&tag);
            if tag == "BTN_ACTION0" || tag == "BTN_TARGET0" {
                if let Some(id) = self.target() {
                    self.use_thing(id);
                }
            } else if let Some(slot) = menus::target_slot(&tag) {
                if self.queue_slot(slot) {
                    if let Some(id) = self.target() {
                        self.use_thing(id);
                    }
                }
            } else {
                self.hud_button(&tag);
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
            if self.settings().combat_movement || !self.party_blocked() {
                // A click on the floor ends a fight, or the chase would hold the player where it can still reach its target.
                let pc = self.game.world.pc;
                self.game.world.stop_attacking(pc);
                self.game.world.walk_to(p);
            }
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
        if self.menu_open() && !self.container_up() {
            return;
        }
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
                self.game.world.close_container();
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

    /// Tab: the next party member takes over.
    pub fn next_leader(&mut self) {
        if !self.game.world.in_dialogue() {
            self.game.world.next_leader();
        }
    }

    /// Space or a click: finish a line that has no replies.
    pub fn continue_line(&mut self) {
        if self.menu_open() {
            return self.menu_enter();
        }
        if !self.game.world.in_dialogue() {
            self.game.world.close_container();
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
        let hd = self.hd3d.clone();
        self.textures
            .entry(name.to_string())
            .or_insert_with(|| {
                hd.and_then(|d| upscaled(&d, name))
                    .or_else(|| kotor_render::load_texture(inst, name))
            })
            .clone()
    }

    /// The particles to draw this frame.
    pub fn particles(&self) -> Vec<crate::effects::ParticleBatch> {
        self.effects.batches(&self.camera())
    }

    fn effect(&mut self, name: &str) {
        if let Ok(bytes) = self.inst.read(name, ResType::WAV) {
            self.cues.push(Cue::Effect(bytes));
        }
    }

    /// Sounds and bolts of what happened since the last frame: attacks, doors and the player's steps. Sound names come
    /// from `ammunitiontypes.2da`, `weaponsounds.2da`, `placeableobjsnds.2da` and `footstepsounds.2da`.
    ///
    /// **Assumed:** which `weaponsounds.2da` row a melee weapon uses (by its `baseitems.2da` label and wield), that the
    /// floor sounds like metal, and the pace of steps.
    fn play_world_sounds(&mut self, dt: f32) {
        let n = self.game.world.combat.log.len();
        let new: Vec<_> = self.game.world.combat.log[self.log_seen.min(n)..].to_vec();
        self.log_seen = n;
        for l in new {
            self.attack_effects(l.attacker, l.target, l.outcome != kotor_game::Outcome::Miss);
        }
        let doors: Vec<(ObjId, bool, String)> = self
            .game
            .world
            .objects
            .values()
            .filter(|o| o.kind == Kind::Door)
            .map(|o| (o.id, o.open, o.template.clone()))
            .collect();
        for (id, open, template) in doors {
            let was = self.door_seen.insert(id, open);
            if was.is_some_and(|w| w != open) {
                let column = if open { "opened" } else { "closed" };
                if let Some(sound) = self.door_sound(&template, column) {
                    self.effect(&sound);
                }
            }
        }
        let pc = self.game.world.pc;
        let pace = match self.game.clip(pc).map(|c| c.name) {
            Some("run") => Some(0.33),
            Some("walk") => Some(0.5),
            _ => None,
        };
        self.step_due -= dt;
        if let Some(pace) = pace {
            if self.step_due <= 0.0 {
                self.step_due = pace;
                self.step_turn = (self.step_turn + 1) % 3;
                let column = format!("metal{}", self.step_turn);
                let sound = TwoDa::parse(
                    &self
                        .inst
                        .read("footstepsounds", ResType::TWODA)
                        .unwrap_or_default(),
                )
                .ok()
                .and_then(|t| t.get(1, &column).map(str::to_string));
                if let Some(sound) = sound {
                    self.effect(&sound);
                }
            }
        }
    }

    fn door_sound(&mut self, template: &str, column: &str) -> Option<String> {
        let utd = Gff::parse(&self.inst.read(template, ResType(2042)).ok()?).ok()?;
        let doors = TwoDa::parse(&self.inst.read("genericdoors", ResType::TWODA).ok()?).ok()?;
        let row = doors
            .int(utd.root.int("GenericType")? as usize, "soundapptype")?
            .max(0) as usize;
        let sounds =
            TwoDa::parse(&self.inst.read("placeableobjsnds", ResType::TWODA).ok()?).ok()?;
        sounds
            .get(row, column)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    }

    fn attack_effects(&mut self, attacker: ObjId, target: ObjId, hit: bool) {
        let w = &self.game.world;
        let (Some(a), Some(t)) = (w.object(attacker), w.object(target)) else {
            return;
        };
        let (from, to) = (a.position, t.position);
        let armoured = t.equipped.contains_key(&ARMOUR_SLOT);
        let base = a
            .equipped
            .get(&RIGHT_HAND_SLOT)
            .and_then(|i| w.object(*i))
            .map(|i| i.base_item as usize);
        let ammo = base
            .and_then(|b| self.baseitem(b, "ammunitiontype"))
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|a| *a > 0);
        if let Some(row) = ammo {
            let Some(table) = self
                .inst
                .read("ammunitiontypes", ResType::TWODA)
                .ok()
                .and_then(|b| TwoDa::parse(&b).ok())
            else {
                return;
            };
            let get = |c: &str| {
                table
                    .get(row, c)
                    .filter(|v| !v.is_empty())
                    .map(str::to_string)
            };
            let (shot, impact, model) = (get("shotsound0"), get("impactsound0"), get("model"));
            let (bolt, color) = model
                .and_then(|m| self.model(&m))
                .and_then(|m| crate::effects::bolt_look(&m))
                .unwrap_or_else(|| ("fx_laser_01".to_string(), [1.0, 0.25, 0.2]));
            let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
            let len = dx.hypot(dy).max(0.01);
            let muzzle = [
                from[0] + dx / len * 0.6,
                from[1] + dy / len * 0.6,
                from[2] + 1.15,
            ];
            self.effects
                .fire(muzzle, [to[0], to[1], to[2] + 1.15], &bolt, color);
            if let Some(s) = shot {
                self.effect(&s);
            }
            if hit {
                if let Some(s) = impact {
                    self.effect(&s);
                }
            }
            return;
        }
        let label = base
            .and_then(|b| self.baseitem(b, "label"))
            .unwrap_or_default();
        let wield = base.and_then(|b| self.baseitem(b, "weaponwield"));
        let row = if base.is_none() {
            0
        } else if label.contains("Lightsaber") {
            1
        } else if wield.as_deref() == Some("3") {
            3
        } else {
            2
        };
        let Some(table) = self
            .inst
            .read("weaponsounds", ResType::TWODA)
            .ok()
            .and_then(|b| TwoDa::parse(&b).ok())
        else {
            return;
        };
        let get = |c: &str| {
            table
                .get(row, c)
                .filter(|v| !v.is_empty())
                .map(str::to_string)
        };
        let swing = get("swingshort0");
        let blow = get(if armoured { "armor0" } else { "leather0" });
        if let Some(s) = swing {
            self.effect(&s);
        }
        if hit {
            if let Some(s) = blow {
                self.effect(&s);
            }
        }
    }

    fn rig_of(&mut self, id: ObjId) -> Option<Rc<Rig>> {
        let (appearance, template, armour) = {
            let w = &self.game.world;
            let o = w.object(id)?;
            let armour = o
                .equipped
                .get(&ARMOUR_SLOT)
                .and_then(|i| w.object(*i))
                .map(|i| (i.base_item, i.template.clone()));
            (o.appearance, o.template.clone(), armour)
        };
        self.rig_for(appearance, &template, armour)
    }

    /// The body of a creature file's appearance in some armour: its base item and template.
    fn rig_for(
        &mut self,
        appearance: i32,
        template: &str,
        armour: Option<(i32, String)>,
    ) -> Option<Rc<Rig>> {
        let number = |inst: &mut Install, name: &str, ty: ResType, field: &str| {
            inst.read(name, ty)
                .ok()
                .and_then(|b| Gff::parse(&b).ok())
                .and_then(|g| g.root.int(field))
        };
        let mut var = number(&mut self.inst, template, ResType(2027), "TextureVar").unwrap_or(1);
        let mut letter = None;
        if let Some((base, item)) = armour {
            letter = self
                .baseitem(base as usize, "bodyvar")
                .and_then(|v| v.chars().next());
            if let Some(v) = number(&mut self.inst, &item, ResType(2025), "TextureVar") {
                var = v;
            }
        }
        let key = (appearance, var as u32, letter);
        let inst = &mut self.inst;
        self.rigs
            .entry(key)
            .or_insert_with(|| {
                kotor_render::rig_for_outfit(inst, appearance as usize, var as u32, letter)
                    .ok()
                    .map(Rc::new)
            })
            .clone()
    }

    fn baseitem(&mut self, row: usize, column: &str) -> Option<String> {
        if self.baseitems.is_none() {
            self.baseitems = TwoDa::parse(&self.inst.read("baseitems", ResType::TWODA).ok()?).ok();
        }
        self.baseitems
            .as_ref()?
            .get(row, column)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    }

    /// The model of a held item: `itemclass` of its `baseitems.2da` row and its `ModelVariation`, for example
    /// `w_Blstrrfl_001`, else the row's `defaultmodel`.
    fn item_model(&mut self, item: ObjId) -> Option<Rc<Model>> {
        if let Some(m) = self.held.get(&item) {
            return m.clone();
        }
        let (base, template) = {
            let o = self.game.world.object(item)?;
            (o.base_item as usize, o.template.clone())
        };
        let variation = self
            .inst
            .read(&template, ResType(2025))
            .ok()
            .and_then(|b| Gff::parse(&b).ok())
            .and_then(|g| g.root.int("ModelVariation"))
            .unwrap_or(1);
        let class = self.baseitem(base, "itemclass").unwrap_or_default();
        let named = format!("{class}_{variation:03}");
        let model = self.model(&named).or_else(|| {
            let d = self.baseitem(base, "defaultmodel")?;
            self.model(&d)
        });
        self.held.insert(item, model.clone());
        model
    }

    /// What a creature holds: hand node and model.
    fn hands(&mut self, id: ObjId) -> Vec<(&'static str, Rc<Model>)> {
        let items: Vec<(&'static str, ObjId)> = self
            .game
            .world
            .object(id)
            .map(|o| {
                [(RIGHT_HAND_SLOT, "rhand"), (LEFT_HAND_SLOT, "lhand")]
                    .iter()
                    .filter_map(|(slot, node)| Some((*node, *o.equipped.get(slot)?)))
                    .collect()
            })
            .unwrap_or_default();
        items
            .into_iter()
            .filter_map(|(node, item)| Some((node, self.item_model(item)?)))
            .collect()
    }

    /// The body of a placeable that holds things; `None` for any other placeable, which is in the fixed scene.
    fn chest_body(&mut self, id: ObjId) -> Option<Rc<DoorBody>> {
        if let Some(b) = self.chest_bodies.get(&id) {
            return b.clone();
        }
        let body = self.load_chest(id).map(Rc::new);
        self.chest_bodies.insert(id, body.clone());
        body
    }

    fn load_chest(&mut self, id: ObjId) -> Option<DoorBody> {
        let (pos, template) = {
            let o = self.game.world.object(id)?;
            (o.position, o.template.to_ascii_lowercase())
        };
        if !self.chests.contains(&template) {
            return None;
        }
        let area = self.area_name();
        let git = Gff::parse(&self.inst.read(&area, ResType::GIT).ok()?).ok()?;
        let at = |s: &kotor_gff::Struct, l: &str| s.float(l).unwrap_or(0.0) as f32;
        let bearing = git
            .root
            .list("Placeable List")?
            .iter()
            .find(|d| (at(d, "X") - pos[0]).abs() + (at(d, "Y") - pos[1]).abs() < 0.01)
            .map_or(0.0, |d| at(d, "Bearing"));
        let utp = Gff::parse(&self.inst.read(&template, ResType(2044)).ok()?).ok()?;
        let table = TwoDa::parse(&self.inst.read("placeables", ResType::TWODA).ok()?).ok()?;
        let name = table
            .get(utp.root.int("Appearance")? as usize, "modelname")?
            .to_string();
        Some(DoorBody {
            model: load_model(&mut self.inst, &name)?,
            xf: placement(pos, bearing),
        })
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
        if !self.started {
            return scene;
        }
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
                    let hands = self.hands(pose.id);
                    if let Some(p) = self.stunt_pose(pose.id) {
                        // A stunt is in world space, like the camera beside it.
                        if let Ok(mut body) = rig.posed(Some(&p)) {
                            for (node, item) in &hands {
                                rig.attach(&mut body, Some(&p), node, item);
                            }
                            scene.append_transformed(&body, &placement([0.0; 3], 0.0));
                        }
                        continue;
                    }
                    self.add_creature(
                        &mut scene,
                        &rig,
                        &hands,
                        &pose.clip,
                        pose.position,
                        pose.heading,
                    );
                }
                Kind::Placeable => {
                    let Some(body) = self.chest_body(pose.id) else {
                        continue;
                    };
                    // Shut until opened; then `close2open` plays once and it stays `open`.
                    let sampled = match self.lids.get(&pose.id) {
                        Some(t) => body.model.animation("close2open").and_then(|a| {
                            if *t < a.length {
                                Some(a.sample(&body.model, *t))
                            } else {
                                body.model
                                    .animation("open")
                                    .map(|o| o.sample(&body.model, 0.0))
                            }
                        }),
                        None => body
                            .model
                            .animation("close")
                            .map(|a| a.sample(&body.model, 0.0)),
                    };
                    scene.add_model_with(&body.model, sampled.as_ref(), &body.xf, &[], None, None);
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
        hands: &[(&'static str, Rc<Model>)],
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
        let pose = len.and_then(|_| rig.sample(name, t));
        let Ok(mut body) = rig.posed(pose.as_ref()) else {
            return;
        };
        for (node, item) in hands {
            rig.attach(&mut body, pose.as_ref(), node, item);
        }
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
        if let Some(view) = self.menu_view() {
            let key = format!("{view:?}{}", self.ui_width);
            if key != self.overlay_key {
                self.overlay_key = key;
                self.overlay = self.ui.menu_overlay(&view);
                self.apply_brightness();
                self.overlay_version += 1;
            }
            return;
        }
        let mut view = self.choice_view();
        if !self.settings().subtitles {
            // **Assumed:** with subtitles off the line being said goes, and the replies stay.
            if let Some(v) = view.as_mut() {
                v.subtitle.clear();
            }
        }
        let party = self.party_view();
        let target = self.target_view();
        let target_at = self.target_on_screen();
        self.ui.set_target_at(target_at);
        let minimap = self.minimap_view();
        let combat = self.combat_view(target.as_ref().map(|t| t.3));
        let notices = self.notices_view();
        let floats = self.floats_view();
        let pause = self.pause_view();
        let cut = self.beat_camera().is_some();
        let key = match &view {
            Some(v) => format!(
                "{}d{:?}{:?}{}{}{cut}",
                self.module, v.speaker, v.replies, v.subtitle, v.selected
            ),
            None => format!(
                "{}h{:?}{target:?}{target_at:?}{cut}{minimap:?}{combat:?}{notices:?}{floats:?}{pause:?}",
                self.module,
                party
                    .iter()
                    .map(|m| (
                        &m.portrait,
                        m.hp,
                        m.max_hp,
                        m.force,
                        m.max_force,
                        m.level_up
                    ))
                    .collect::<Vec<_>>()
            ),
        };
        if key == self.overlay_key {
            return;
        }
        self.overlay_key = key;
        self.ui.set_minimap(minimap);
        let engaged = combat.engaged || !notices.is_empty();
        self.ui.set_notices(notices);
        self.ui.set_floats(floats);
        self.ui.set_pause(pause.0, pause.1);
        self.ui.set_combat(combat);
        self.overlay = match view {
            // A wordless beat: the scene alone.
            Some(v) if v.subtitle.trim().is_empty() && v.replies.is_empty() => {
                self.ui.letterbox_overlay()
            }
            Some(v) => self.ui.dialogue_overlay(&v),
            None => {
                let t = target.as_ref();
                let action = t.map_or("", |t| t.3);
                self.ui.hud_overlay(
                    &party,
                    t.map(|t| (t.0.as_str(), t.1, t.2)),
                    action,
                    action_icon(action),
                )
            }
        };
        // What is on screen in words, as far as the menu buttons leave room.
        let label = self.label();
        let short = label.split(" | ").take(3).collect::<Vec<_>>().join(" | ");
        // In a fight that place has the line about combat mode, and the notices are there too.
        if !engaged {
            self.overlay = self.overlay.take().and_then(|o| self.ui.caption(o, &short));
        }
        self.apply_brightness();
        self.overlay_version += 1;
    }

    /// The thing under the cursor, else the nearest in reach.
    fn target(&self) -> Option<ObjId> {
        let w = &self.game.world;
        self.hover_target
            .filter(|id| w.object(*id).is_some_and(|o| !o.dead) || w.lootable(*id))
            .or_else(|| w.nearest_usable(USE_REACH))
    }

    /// Whether a module is being played, and not only the main menu shown.
    pub fn in_game(&self) -> bool {
        !self.module.is_empty()
    }

    /// Put the player on the floor beside an object with this tag. For test scenes.
    pub fn stand_near(&mut self, tag: &str) -> bool {
        let w = &mut self.game.world;
        let Some(at) = w
            .by_tag(tag)
            .first()
            .and_then(|i| w.object(*i))
            .map(|o| o.position)
        else {
            return false;
        };
        let spot = (0..8).find_map(|k| {
            let a = k as f32 * std::f32::consts::FRAC_PI_4;
            let (x, y) = (at[0] + a.cos() * 1.0, at[1] + a.sin() * 1.0);
            let z = w.nav.as_ref()?.walkable_at(x, y, at[2] + 1.0)?;
            Some([x, y, z])
        });
        match spot {
            Some(p) => {
                let pc = w.pc;
                w.teleport(pc, p);
                true
            }
            None => false,
        }
    }

    /// The point on a window of `size` in the middle of a control of the HUD.
    pub fn hud_pixel(&mut self, tag: &str, size: (u32, u32)) -> Option<(f32, f32)> {
        let (x, y) = self.ui.hud_point(tag)?;
        Some((
            x * size.0 as f32 / self.ui_width as f32,
            y * size.1 as f32 / 480.0,
        ))
    }

    /// One line for a test driver watching the live window.
    pub fn state_line(&mut self) -> String {
        // Where the slots are, in layout units, for a driver to click: they move with the target's bar.
        let layout = (self.ui_width, 480);
        let at =
            |p: Option<(f32, f32)>| p.map_or("-".to_string(), |p| format!("{:.0},{:.0}", p.0, p.1));
        let attack_slot = at(self.hud_pixel("BTN_TARGET0", layout));
        let feat_slot = at(self.hud_pixel("BTN_TARGET1", layout));
        let (paused, note) = self.pause_view();
        let floats = self.floats_shown().len();
        let foe = self
            .seen_enemy()
            .and_then(|id| self.game.world.object(id))
            .and_then(|o| self.on_screen(o.position, 1.0))
            .map_or("-".to_string(), |p| format!("{},{}", p.0, p.1));
        let w = &self.game.world;
        let here = self.pc_position();
        let view = w.dialogue_view();
        format!(
            "state menu={} saves={} gold={} level={} xp={} hp={} worn={} module={} pc={:.2},{:.2} clip={} dlg={} replies={} said={} opened={} items={} inv={} pilot={} engaged={} paused={paused} note={} attacks={} queue={} floats={floats} width={} slot0={attack_slot} slot1={feat_slot} foe={foe} target={}",
            self.menu_name(),
            self.saves_listed(),
            w.gold,
            w.object(w.pc).map_or(0, |o| o.level),
            w.object(w.pc).map_or(0, |o| o.xp),
            w.object(w.pc).map_or(0, |o| o.hp),
            w.object(w.pc).map_or(0, |o| o.equipped.len()),
            if self.module.is_empty() { "-" } else { &self.module },
            here[0],
            here[1],
            self.game.clip(w.pc).map_or("-", |c| c.name),
            view.is_some(),
            view.map_or(0, |v| v.3.len()),
            w.transcript.len(),
            w.opened.is_some(),
            w.container_items().len(),
            w.object(w.pc).map_or(0, |o| o.inventory.len()),
            self.pilot.is_some(),
            w.combat.target.contains_key(&w.pc),
            note.lines().next().unwrap_or("-").replace(' ', "_"),
            w.combat.log.iter().filter(|a| a.attacker == w.pc).count(),
            w.combat_queue.len(),
            self.ui_width,
            self.target_view().map_or("-".to_string(), |t| t.0.replace(' ', "_")),
        )
    }

    /// Where the target's name bar goes, in layout units: a little over the target, as the game has it.
    fn target_on_screen(&self) -> Option<(i32, i32)> {
        let w = &self.game.world;
        let o = w.object(self.target()?)?;
        let up = match o.kind {
            Kind::Door => 2.6,
            Kind::Creature if !o.dead => 2.1,
            _ => 1.1,
        };
        self.on_screen(o.position, up)
    }

    /// Where a point of the world, `up` above the floor, is in the layout.
    fn on_screen(&self, at: [f32; 3], up: f32) -> Option<(i32, i32)> {
        let cam = self.camera();
        let v = [
            at[0] - cam.eye[0],
            at[1] - cam.eye[1],
            at[2] + up - cam.eye[2],
        ];
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let norm = |a: [f32; 3]| {
            let l = dot(a, a).sqrt().max(1e-6);
            [a[0] / l, a[1] / l, a[2] / l]
        };
        let fwd = norm([
            cam.target[0] - cam.eye[0],
            cam.target[1] - cam.eye[1],
            cam.target[2] - cam.eye[2],
        ]);
        let right = norm([fwd[1], -fwd[0], 0.0]);
        let above = [
            right[1] * fwd[2] - right[2] * fwd[1],
            right[2] * fwd[0] - right[0] * fwd[2],
            right[0] * fwd[1] - right[1] * fwd[0],
        ];
        let depth = dot(v, fwd);
        if depth < 0.2 {
            return None;
        }
        let t = (cam.fov.to_radians() / 2.0).tan();
        let wide = self.ui_width as f32;
        let nx = dot(v, right) / (depth * t * wide / 480.0);
        let ny = dot(v, above) / (depth * t);
        let x = ((nx + 1.0) / 2.0 * wide) as i32;
        let y = ((1.0 - ny) / 2.0 * 480.0) as i32;
        Some((x.clamp(80, wide as i32 - 80), y.clamp(2, 440)))
    }

    /// The thing under the cursor, else the nearest in reach: name, hit points, and what using it does.
    fn target_view(&self) -> Option<(String, i32, i32, &'static str)> {
        let w = &self.game.world;
        let id = self.target()?;
        let o = w.object(id)?;
        let name = if o.name.is_empty() {
            o.tag.clone()
        } else {
            o.name.clone()
        };
        Some(match o.kind {
            Kind::Creature if o.dead => (name, 1, 1, "Use"),
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
                level_up: w.level_ready(o.id),
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

    /// Stop using upscaled pictures.
    pub fn no_hd(&mut self) {
        self.ui.set_hd_dir(None);
        self.hd3d = None;
        self.textures.clear();
        self.overlay_key.clear();
        self.refresh_overlay();
    }

    /// The window's size: the interface is laid out 480 units high, as wide as that makes it, and drawn pixel for pixel.
    pub fn set_view(&mut self, size: (u32, u32)) {
        if let Some(width) = (480 * size.0 + size.1 / 2).checked_div(size.1) {
            self.ui_width = width.max(640);
            self.ui.set_view(size);
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
