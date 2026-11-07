//! The playable scene: a [`Game`] with everything a window needs from it each frame. The camera, the posed bodies
//! of creatures and doors, the dialogue box and HUD, and what to play. Nothing here touches the GPU or a window,
//! so it runs in a headless test.

use kotor_archive::Install;
use kotor_core::ResType;
use kotor_game::{pixel_ray, Beat, CameraShot, Clip, Game, Kind, MovementMode, ObjId, World};
use kotor_gff::{Gff, Value};
use kotor_image::Image;
use kotor_model::{parse_model, Model};
use kotor_render::{Camera, Floor, Rig, Scene};
use kotor_tables::TwoDa;
use kotor_ui::{DialogueView, HudMember, MenuView, Ui};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

#[path = "bodypose.rs"]
mod bodypose;
#[path = "deathscreen.rs"]
mod deathscreen;
#[path = "menus.rs"]
mod menus;
#[path = "moving.rs"]
mod moving;
#[path = "nameplates.rs"]
mod nameplates;
#[path = "racescreen.rs"]
mod racescreen;
#[path = "roomshell.rs"]
mod roomshell;
#[path = "seamless_stage.rs"]
mod seamless_stage;
pub use menus::{
    about_words, choose_override_use, default_saves_dir, default_settings_file, Settings,
};
#[path = "overridescan.rs"]
mod overridescan;
pub use overridescan::{scan_override_folder, scan_override_in_background};

type Mat = [[f32; 4]; 4];

/// A sound from a place is heard whole this near the player, and not at all this far (metres). **Assumed:** a duel 43 m from
/// where a new game starts played at full strength for good. The video has none of it there.
const HEARD_WHOLE: f32 = 8.0;
const HEARD_AT_MOST: f32 = 32.0;
/// Creatures farther than this from the player are not drawn (metres).
const DRAW_RANGE: f32 = 45.0;
/// Animation ids from here index a stunt model's animations (see `kotor_game::cutscene`).
const FIRST_STUNT_ANIMATION: u32 = 1200;
/// The follow camera: metres behind the player and above its feet, and the height it looks at. As `render_walk`.
const CAMERA_FOV: f32 = 55.0;
/// How far the camera keeps from a wall or ceiling (metres): its near plane is 0.05, and a frame needs some way past that.
const CAMERA_OFF_SURFACE: f32 = 0.2;
/// How high a closed door stops the camera, over the door's foot, in metres. A door's frame and wall go on above it.
const DOOR_CAMERA_HEIGHT: f32 = 4.0;
/// `Equip_ItemList` slots: the armour is measured from the data (`g_a_*` items have this bit); the right hand is the one
/// combat reads.
const ARMOUR_SLOT: i32 = 1;
/// A fixed conversation camera nearer than this to the speaker is not used.
const TOO_CLOSE_METRES: f32 = 1.2;
const CLOTHES_LETTER: char = 'B';
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
    /// A short sound from somewhere off, and how much of it reaches the player, 0 to 1.
    FaintEffect(Vec<u8>, f32),
    /// The player's levels for music, voice and effects, 0 to 1.
    Levels(f32, f32, f32),
    StopMusic,
}

struct DoorBody {
    model: Model,
    xf: Mat,
}

/// A shooter's attack in progress: the look of its bolt and its sound, and how far into its clip the last shot was.
struct Volley {
    target: ObjId,
    bolt: String,
    color: [f32; 3],
    shot: Option<String>,
    seen: bool,
    last: f32,
    age: f32,
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

/// Steps of `Stage::voice_energy` in a second.
const VOICE_RATE: f32 = 50.0;

/// How loud a voice file is, 50 times a second, from 0 (a pause between words) to 1: the mean size of the samples over 20 ms
/// against the 90th percentile of them. Empty when the file cannot be read.
fn voice_energy(path: &std::path::Path) -> Vec<f32> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    let Ok(audio) = kotor_audio::decode(&bytes) else {
        return Vec::new();
    };
    let channels = usize::from(audio.channels.max(1));
    let step = (audio.sample_rate as f32 / VOICE_RATE).max(1.0) as usize * channels;
    let mut steps: Vec<f32> = audio
        .samples
        .chunks(step)
        .map(|c| c.iter().map(|s| f32::from(*s).abs()).sum::<f32>() / c.len() as f32)
        .collect();
    let mut sorted = steps.clone();
    sorted.sort_by(f32::total_cmp);
    let top = sorted
        .get(sorted.len() * 9 / 10)
        .copied()
        .unwrap_or(0.0)
        .max(1.0);
    for v in &mut steps {
        let level = (*v / top).min(1.0);
        *v = if level < 0.1 {
            0.0
        } else {
            0.35 + 0.65 * level
        };
    }
    steps
}

/// Ids from here are rows of `dialoganimations.2da`, in a conversation node's `AnimList`.
const DIALOGUE_ANIMATION_BASE: u32 = 10000;

/// The clip a `dialoganimations.2da` label asks for. Labels with no clip here make no gesture.
fn dialogue_clip_name(label: &str) -> Option<&'static str> {
    Some(match label.to_ascii_lowercase().as_str() {
        "talk_normal" => "tlknorm",
        "talk_pleading" => "tlkplead",
        "talk_forceful" => "tlkforce",
        "talk_laughing" => "tlklaugh",
        "talk_sad" => "tlksad",
        "listen" => "listen",
        "use_computer_lp" => "usecomplp",
        "use_computer" => "usecomp",
        "greeting" => "greeting",
        "taunt" => "taunt",
        "salute" => "salute",
        "bow" => "bow",
        "victory" => "victory",
        "inject" => "inject",
        "flirt" => "flirt",
        "persuade" => "persuade",
        "sleep" => "sleep",
        "horror" => "horror",
        "activate" => "activate",
        "scratch_head" => "pause2",
        "dead" => "dead",
        _ => return None,
    })
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

/// The owner's own pictures, outside the repository and the install: `KOTOR_ASSETS_DIR`, or this folder.
pub fn default_assets_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("KOTOR_ASSETS_DIR") {
        return Some(PathBuf::from(dir));
    }
    Some(
        PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
            .join("kotor-rs")
            .join("assets"),
    )
}

/// Where upscaled pictures live: outside the repository and the install, since they are made from game files.
pub fn default_hd_dir() -> Option<PathBuf> {
    Some(
        PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
            .join("kotor-rs")
            .join("hd"),
    )
}

/// A stock picture takes 2 or 3 ms to read.
const PICTURES_A_FRAME: u32 = 3;
const RIGS_A_FRAME: u32 = 1;

/// The thread that reads upscaled pictures.
struct HdReader {
    ask: std::sync::mpsc::Sender<String>,
    given: std::sync::mpsc::Receiver<(String, Option<Image>)>,
    asked: HashSet<String>,
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

/// A rig as the underwear style has it.
fn styled(mut rig: Rig, conventional: bool) -> Rc<Rig> {
    if conventional {
        rig.soften_underwear_shading();
    }
    Rc::new(rig)
}

/// How far over a head its bar stays, in layout units.
const TARGET_BAR_CLEAR: i32 = 6;
/// The box holding two boxes: left, top, width, height.
fn union_box(a: Option<[u32; 4]>, b: Option<[u32; 4]>) -> Option<[u32; 4]> {
    match (a, b) {
        (Some(a), Some(b)) => {
            let (left, top) = (a[0].min(b[0]), a[1].min(b[1]));
            let right = (a[0] + a[2]).max(b[0] + b[2]);
            let bottom = (a[1] + a[3]).max(b[1] + b[3]);
            Some([left, top, right - left, bottom - top])
        }
        (one, other) => one.or(other),
    }
}

/// How near the line from a body's feet to its head the camera is when that body goes, and when it comes back, in metres.
const FADE_FROM: f32 = 0.6;
const GONE_WITHIN: f32 = 0.25;
const BODY_TALL: f32 = 1.8;
const SLOTS_AND_GAP: i32 = kotor_ui::TARGET_BAR_AND_SLOTS_HIGH - kotor_ui::TARGET_BAR_HIGH;

/// The stock icon for a target's action, read from the texture names in `swpc_tex_gui.erf`.
fn action_icon(action: &str) -> &'static str {
    match action {
        "Attack" => "i_attack",
        "Talk" => "i_dialog",
        "Open" | "Close" => "i_opendoor",
        _ => "i_openplace",
    }
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>().sqrt()
}

/// How much of a sound this far from the player reaches it, 0 to 1.
fn heard_share(metres: f32) -> f32 {
    ((HEARD_AT_MOST - metres) / (HEARD_AT_MOST - HEARD_WHOLE)).clamp(0.0, 1.0)
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
    /// The rooms, for the camera to stay inside.
    shell: roomshell::RoomShell,
    plates: nameplates::PlateState,
    /// Party members the camera is too close to draw.
    hud_under_plates: Option<Image>,
    /// What moves all the time on the HUD, as last drawn.
    /// The HUD is what was last drawn, and not a conversation's box, so part of it can be drawn again.
    hud_is_up: bool,
    /// More of the small map has been seen since it was drawn.
    map_seen_more: bool,
    plate_key: String,
    plate_box: Option<[u32; 4]>,
    /// The version the plates began moving at, the latest one, and the box they have moved in since.
    plates_moved: Option<(u64, u64, [u32; 4])>,
    static_scene: Scene,
    seam: seamless_stage::Seam,
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
    /// A thread that reads upscaled pictures: a 1024 square one takes 50 to 100 ms, which no frame can pay.
    hd_reader: Option<HdReader>,
    /// Pictures standing in, by the stock one, for an upscaled one the thread is reading.
    stand_ins: HashSet<String>,
    /// Upscaled pictures that have come, for the renderer to ask for again.
    hd_come: Vec<String>,
    /// Stock pictures that may still be read this frame, when `pace_loading`.
    pictures_left: u32,
    /// Bodies that may still be read this frame, when set.
    rigs_left: Option<u32>,
    /// A window is drawing, and a frame must not pay for all that comes into view at once. A test or a driver that wants
    /// everything at once leaves this off.
    pub pace_loading: bool,
    /// "Underwear style: Conventional". See `kotor_render::underwear`.
    conventional_underwear: bool,
    /// The underwear the character creation screen's figures wear: a variant's id, empty for the standard paint.
    preview_underwear: String,
    underwear_plans: HashMap<String, Option<kotor_render::underwear::Plan>>,
    neck_plans: HashMap<String, Option<kotor_render::underwear::NeckPlan>>,
    baseitems: Option<TwoDa>,
    /// The clips `animations.2da` marks `hideequ`: whoever plays one holds nothing in sight (`inject`, picking a lock).
    hide_equipment: Option<std::collections::BTreeSet<String>>,
    /// Shooters whose attack clip is on or coming: each shot leaves at its moment of the clip (`kotor_game::fire_times`).
    volleys: std::collections::BTreeMap<ObjId, Volley>,
    /// An item's model, by item.
    held: HashMap<ObjId, Option<Rc<Model>>>,
    doors: HashMap<ObjId, Option<Rc<DoorBody>>>,
    textures: HashMap<String, Option<Image>>,
    /// The owner's folder of textures that stand in for the game's, and what was wrong with the pictures in it.
    texture_folder: Option<std::path::PathBuf>,
    texture_problems: Vec<String>,
    /// What was wrong with the owner's animation files when last read (`anim_problems.txt` has it too).
    anim_problems: Vec<String>,
    /// The owner's garments (`kotor-rs\meshes`), what was wrong with the files, which one the player wears, and the bodies drawn in them.
    garments: Vec<kotor_render::custom_mesh::Garment>,
    mesh_problems: Vec<String>,
    worn_garment: Option<usize>,
    dressed: HashMap<(i32, usize), Rc<Rig>>,
    /// Which of the owner's clips the debug key has the player in, if any.
    shown_clip: Option<usize>,
    forced_at: f32,
    /// Camera direction in radians (0 = looking along +x) and distance behind the player.
    pub yaw: f32,
    pub distance: f32,
    /// Degrees of camera turning asked for this frame, per second.
    pub turn: f32,
    /// Camera height angle in radians, above the level of the chest.
    pub pitch: f32,
    /// WASD this frame: forward and rightward, each -1, 0 or 1, and whether to walk instead of run.
    pub steer: (f32, f32, bool),
    /// The way the movement keys point: the camera's, a moment behind it in Modern.
    steer_yaw: f32,
    /// The look button is held.
    looking: bool,
    /// How far behind the player the camera's focus is, and the height it was last at.
    camera_lag: [f32; 3],
    /// How far out the camera stands, easing in Modern. None: as the room says, at once.
    camera_arm: Option<f32>,
    /// Where the player was when the arm last looked.
    arm_at: [f32; 3],
    /// How far the camera has risen over something close behind the player, radians.
    camera_rise: f32,
    /// How far to the player's right the camera has stepped, metres, to get out from behind something.
    camera_side: f32,
    /// Seconds since the player was last in the air, or in a landing.
    since_fall: f32,
    /// The roll's and the landing's poses.
    body_poses: moving::BodyPoses,
    camera_z: Option<f32>,
    /// Degrees added to the field of view at speed.
    fov_extra: f32,
    hover_reply: usize,
    hover_target: Option<ObjId>,
    ui_width: u32,
    overlay: Option<Image>,
    overlay_key: String,
    /// A game someone is playing saves before each fight. A test's or a driver's must not touch their saves.
    pub combat_autosave: bool,
    /// A test driver has given someone this: `--grenadier`.
    pub armed: Option<ObjId>,
    /// The one `--grenadier` armed, and where it stood: a thrower is put back there when it closes in, as the AI throws only
    /// at a foe farther than about 4 m and a driver's check has to see it throw.
    pub armed_home: Option<(ObjId, [f32; 3])>,
    /// A test driver has the turret aim for it.
    pub turret_assist: bool,
    /// Goes up whenever the overlay changes.
    pub overlay_version: u64,
    /// The screen the overlay last showed, and the version of it that is.
    menu_drawn: Option<(u64, MenuView)>,
    /// How long the last look at the overlay took, for `--frame-log`.
    pub overlay_ms: f32,
    /// What it last did on the HUD: 0 nothing, 1 the plates, 2 part of it, 3 the whole, 4 a conversation's box, 5 the whole
    /// after part of it could not be drawn.
    pub hud_drawn: u8,
    cues: Vec<Cue>,
    events_seen: usize,
    line_key: Option<usize>,
    /// The cutscene beat on now, its age in seconds, and the stunt and camera model names it reads.
    beat: Option<Beat>,
    beat_time: f32,
    /// How loud the voice of the line on now is, in 50ths of a second from 0 to 1: what opens the speaker's mouth. Empty for a
    /// line with no voice, which is then spoken by its length alone.
    voice_energy: Vec<f32>,
    /// A creature held in one clip for a look at it (`force_clip`), and the clip's name.
    forced_clip: Option<(ObjId, &'static str)>,
    camera_model: String,
    stunts: Vec<(String, String)>,
    /// Seconds of real time: what bodies shown on screens with no game running (the character picker, the party screen)
    /// idle by.
    idle_clock: f32,
    /// The `name` of each row of `dialoganimations.2da`.
    dialogue_labels: Vec<String>,
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

/// How often the pilot says how its clicks are going.
const CLICKS_TO_REPORT: usize = 200;

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
            shell: Default::default(),
            plates: Default::default(),
            hud_under_plates: None,
            hud_is_up: false,
            map_seen_more: false,
            plate_key: String::new(),
            plate_box: None,
            plates_moved: None,
            static_scene: Scene::default(),
            seam: Default::default(),
            static_version: 0,
            rigs: HashMap::new(),
            baseitems: None,
            hide_equipment: None,
            volleys: Default::default(),
            effects: Default::default(),
            chests: Vec::new(),
            chest_bodies: HashMap::new(),
            lids: HashMap::new(),
            log_seen: 0,
            door_seen: HashMap::new(),
            step_due: 0.0,
            step_turn: 0,
            hd_reader: None,
            stand_ins: HashSet::new(),
            hd_come: Vec::new(),
            pictures_left: 0,
            rigs_left: None,
            pace_loading: false,
            hd3d: default_hd_dir()
                .and_then(|d| d.parent().map(|p| p.join("hd3d")))
                .filter(|d| d.is_dir()),
            conventional_underwear: true,
            preview_underwear: String::new(),
            underwear_plans: HashMap::new(),
            neck_plans: HashMap::new(),
            held: HashMap::new(),
            doors: HashMap::new(),
            textures: HashMap::new(),
            texture_folder: crate::texture_override::folder().filter(|d| d.is_dir()),
            texture_problems: Vec::new(),
            anim_problems: Vec::new(),
            garments: Vec::new(),
            mesh_problems: Vec::new(),
            worn_garment: None,
            dressed: HashMap::new(),
            shown_clip: None,
            forced_at: 0.0,
            yaw: 0.0,
            distance: CAMERA_BEHIND,
            turn: 0.0,
            pitch: ((CAMERA_ABOVE - CAMERA_LOOKS_AT) / CAMERA_BEHIND).atan(),
            steer: (0.0, 0.0, false),
            steer_yaw: 0.0,
            looking: false,
            camera_lag: [0.0; 3],
            camera_arm: None,
            arm_at: [0.0; 3],
            camera_rise: 0.0,
            camera_side: 0.0,
            since_fall: 0.0,
            body_poses: moving::BodyPoses::default(),
            camera_z: None,
            fov_extra: 0.0,
            hover_reply: 0,
            hover_target: None,
            ui_width: 853,
            overlay: None,
            overlay_key: String::new(),
            combat_autosave: false,
            armed: None,
            armed_home: None,
            turret_assist: false,
            overlay_version: 0,
            menu_drawn: None,
            overlay_ms: 0.0,
            hud_drawn: 0,
            cues: Vec::new(),
            events_seen: 0,
            line_key: None,
            beat: None,
            beat_time: 0.0,
            voice_energy: Vec::new(),
            forced_clip: None,
            camera_model: String::new(),
            stunts: Vec::new(),
            idle_clock: 0.0,
            dialogue_labels: Vec::new(),
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
        s.ui.set_assets_dir(default_assets_dir());
        s.set_view((1280, 720));
        Ok(s)
    }

    /// A new game in `module`, in place of whatever was running.
    fn start_module(&mut self, module: &str) -> Result<(), String> {
        let e = |e: kotor_core::Error| e.to_string();
        let mut world = Stage::fresh_world(&self.dir, self.original)?;
        world.load_module(module).map_err(e)?;
        world.early_force = self.menus.early_force_on && self.settings().early_force;
        // **Assumption:** with no character made on the screens the player is the recommended Soldier.
        let spec = match self.take_new_character() {
            Some(spec) => spec,
            None => world
                .recommended_character("Player", 0, 1)
                .ok_or("no recommended character")?,
        };
        world.create_character(&spec)?;
        // The one on the character screen's figure: the world under those screens is not this one.
        world.set_player_underwear(&self.preview_underwear);
        self.preview_underwear.clear();
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
        self.plates.kept = None;
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
        // A seamless crossing has the new module's picture ready.
        let ready = self.seamless_arrive();
        self.module = self.game.world.module.clone();
        self.inst.load_module(&self.module).map_err(e)?;
        let area = self.area_name();
        // The turret's sky moves, so it is drawn with what moves.
        let turret = self.game.world.minigame.is_some();
        let preloaded = ready.is_some();
        let (mut scene, chests) = match ready {
            Some((scene, chests)) => (scene, chests),
            None => {
                let scene = if turret {
                    Scene::default()
                } else {
                    kotor_render::load_area(&mut self.inst, &area).map_err(e)?
                };
                (scene, chest_templates(&mut self.inst, &area))
            }
        };
        self.shell = roomshell::RoomShell::of(&scene);
        if !turret && !preloaded {
            kotor_render::add_area_objects_except(
                &mut scene,
                &mut self.inst,
                &area,
                false,
                &chests,
            )
            .map_err(e)?;
        }
        // What a fall stops at: the rooms, and what stands in them, as fences do.
        self.game.world.barriers = Some(std::sync::Arc::new(roomshell::RoomShell::of(&scene)));
        self.chests = chests;
        self.chest_bodies.clear();
        self.lids.clear();
        self.seam.base = scene.clone();
        self.static_scene = scene;
        self.static_version += 1;
        self.floor = Floor::load(&mut self.inst, &area).ok();
        self.doors.clear();
        self.events_seen = 0;
        self.fixed_cameras = fixed_cameras(&mut self.inst, &area);
        if self.dialogue_labels.is_empty() {
            self.dialogue_labels = self
                .inst
                .read("dialoganimations", ResType::TWODA)
                .ok()
                .and_then(|b| TwoDa::parse(&b).ok())
                .map(|t| {
                    (0..t.row_count())
                        .map(|r| t.get(r, "name").unwrap_or("").to_string())
                        .collect()
                })
                .unwrap_or_default();
        }
        if turret {
            self.effects = Default::default();
        } else {
            self.effects.load_area(&mut self.inst, &area);
        }
        self.log_seen = 0;
        self.door_seen.clear();
        let music = (|| {
            let git = Gff::parse(&self.inst.read(&area, ResType::GIT).ok()?).ok()?;
            let props = git.root.strukt("AreaProperties")?;
            Some(props.int("MusicDay")? as usize)
        })();
        match self
            .turret_music()
            .or_else(|| music.and_then(|row| self.music_file(row)))
        {
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
        self.idle_clock += dt;
        self.walkers_tick(dt);
        self.pictures_left = PICTURES_A_FRAME;
        self.take_upscaled();
        self.main_scene_tick(dt);
        self.chargen_mist_tick(dt);
        self.figure_mist_tick(dt);
        self.galaxy_tick(dt);
        self.bench_tick(dt);
        self.take_bench_request();
        self.take_party_request();
        self.take_galaxy_request();
        self.take_pazaak_request();
        self.forget_container_choices();
        self.slots_tick();
        self.notices_tick();
        if self.menu_open() && !self.container_up() {
            self.refresh_overlay();
            return std::mem::take(&mut self.cues);
        }
        if self.death_tick(dt) {
            return std::mem::take(&mut self.cues);
        }
        if self.pause_tick() {
            self.game.world.roll_request = false;
            self.update_camera(dt);
            if self.camera_arm.is_some() {
                self.arm_tick(dt);
            }
            self.refresh_overlay();
            return std::mem::take(&mut self.cues);
        }
        self.turret_tick(dt);
        self.thrower_tick();
        self.race_input();
        let (ahead, right, walk) = self.steer;
        let steer_yaw = self.keys_yaw(dt);
        let (cos, sin) = (steer_yaw.cos(), steer_yaw.sin());
        let dir = [cos * ahead + sin * right, sin * ahead - cos * right];
        let len = dir[0].hypot(dir[1]);
        let held = !self.settings().combat_movement && self.party_blocked();
        self.game.world.steer = (len > 0.01 && !held && self.game.world.minigame.is_none())
            .then(|| ([dir[0] / len, dir[1] / len], !walk));
        // A scripted player keeps the walk its timings were measured on.
        self.game.world.movement = if self.settings().movement_modern && self.pilot.is_none() {
            MovementMode::Modern
        } else {
            MovementMode::Classic
        };
        let speed = self.speed;
        // A scripted player keeps to the game's own scenes, so that its route and digests do not move.
        self.game.world.extended_story = self.settings().extended_story && self.pilot.is_none();
        if let Some((who, clip)) = self.game.world.story.cue.take() {
            // The owner's own clip, once he has made it. Till then the game's stock attack has played.
            if kotor_render::custom_clip::get(clip).is_some() {
                self.force_clip(who, Some(clip));
            }
        }
        if let Some(mut p) = self.pilot.take() {
            // A scripted player opens no screens: levels and forced party members come straight away.
            self.game.world.hold_levels = false;
            self.game.world.party_screen = false;
            p.by_clicks = true;
            let n = self.game.steps_due(dt, speed);
            let step = self.game.dt;
            for _ in 0..n {
                p.step(&mut self.game, step);
                if let Some(reply) = p.wanted_reply.take() {
                    self.pilot_clicks_a_line(&mut p, reply);
                }
                p.end_step(&mut self.game);
            }
            self.pilot = Some(p);
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
        self.seamless_tick();
        self.scan_music_events();
        self.play_world_sounds(dt_game);
        self.fire_on_frames(dt_game);
        self.play_effects_at();
        for id in self.game.world.open_sources() {
            self.lids.entry(id).or_insert(0.0);
        }
        for t in self.lids.values_mut() {
            *t += dt_game;
        }
        let eye = self.camera().eye;
        self.effects.update(dt_game, eye);
        self.update_line(dt_game);
        self.movement_tick(dt);
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

    /// What has been put down at a place: a grenade's burst.
    fn play_effects_at(&mut self) {
        for (handle, at) in std::mem::take(&mut self.game.world.effects_at) {
            let rec = handle
                .checked_sub(1)
                .and_then(|i| self.game.world.effects.get(i as usize));
            let row = rec
                .filter(|r| r.name == "EffectVisualEffect")
                .and_then(|r| r.nums.first())
                .map(|r| *r as i32);
            // What the blast sounds like: the row's `soundimpact` in `visualeffects.2da` (`cb_gr_fragment` for a frag grenade).
            if let Some(sound) = row.and_then(|r| self.visual_sound(r)) {
                self.effect_from(&sound, at);
            }
            let Some((_, Some((model, up)))) = row.and_then(|r| self.visual_row(r)) else {
                continue;
            };
            if let Some(m) = self.model(&model) {
                self.effects.play_model(&m, [at[0], at[1], at[2] + up]);
            }
        }
    }

    /// The sound a visual effect makes where it lands: `soundimpact` of its row in `visualeffects.2da`.
    fn visual_sound(&mut self, row: i32) -> Option<String> {
        let t = TwoDa::parse(&self.inst.read("visualeffects", ResType::TWODA).ok()?).ok()?;
        let i = t.labels.iter().position(|l| l == &row.to_string())?;
        t.get(i, "soundimpact")
            .filter(|s| !s.is_empty() && *s != "****")
            .map(str::to_string)
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
            match caster
                .and_then(|id| self.game.world.object(id))
                .map(|o| o.position)
            {
                Some(at) => self.effect_from(&s, at),
                None => self.effect(&s),
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
            self.beat_time = 0.0;
            self.voice_energy = self
                .sound_file(&vo)
                .map(|p| voice_energy(&p))
                .unwrap_or_default();
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
            // The pause a node's script makes already lasts as long as its `hold`, and runs from the node's start, so the
            // node lasts its voice or its camera, or the pause if that is longer (bug 32: the duel ran the holds twice).
            let seconds = self.beat.as_ref().map_or(1.5, |b| b.length);
            // A node whose only replies say nothing is a cutscene's: it moves on by itself, when its voice-over ends.
            self.line_left = (self.settings().auto_advance
                && replies.iter().all(|r| r.trim().is_empty()))
            .then_some(seconds);
        } else {
            self.beat_time += dt;
        }
        if self.pilot.is_some() {
            return;
        }
        let paused = self.game.world.conversation_paused();
        if let Some(left) = self.line_left.as_mut() {
            *left -= dt;
            if *left <= 0.0 && !paused {
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

    /// Hold creature `id` in the clip `name` (or let go, with `None`): for looking at a stance or a gesture, as the viewer's drivers do.
    pub fn force_clip(&mut self, id: ObjId, name: Option<&str>) {
        self.forced_clip = name.map(|n| (id, &*Box::leak(n.to_string().into_boxed_str())));
        self.forced_at = self.idle_clock;
    }

    /// How far the mouth of creature `id` is open while it says the line on now, from 0 to 1, with the clock of its talking
    /// loop: `None` when it is not the speaker, or the line is said. It follows the voice's loudness, and a line with no voice
    /// is spoken at an even pace for its length.
    pub fn mouth_of(&self, id: ObjId) -> Option<(f32, f32)> {
        let w = &self.game.world;
        let (speaker, _, _, _) = w.dialogue_view()?;
        let o = w.object(id)?;
        // A node's speaker is a tag, or empty, or a name that no object has (the opening's `Trask`): then the owner speaks.
        let says = if speaker.is_empty() || w.by_tag(&speaker).is_empty() {
            Some(id) == w.dialogue_owner() && id != w.pc
        } else {
            o.tag.eq_ignore_ascii_case(&speaker)
        };
        let said = self.beat.as_ref().map_or(0.0, |b| b.length);
        if !says || self.beat_time >= said {
            return None;
        }
        let open = if self.voice_energy.is_empty() {
            0.85
        } else {
            let at = (self.beat_time * VOICE_RATE) as usize;
            // A little ahead and behind, so that the mouth does not flutter at 50 Hz.
            let around = |k: usize| {
                (at.saturating_sub(k)..=at + k)
                    .map(|i| self.voice_energy.get(i).copied().unwrap_or(0.0))
                    .sum::<f32>()
                    / (2 * k + 1) as f32
            };
            around(2)
        };
        // The mouth shuts at the line's very start and end.
        let edge = (self.beat_time / 0.12)
            .min((said - self.beat_time) / 0.12)
            .clamp(0.0, 1.0);
        Some((open * edge, self.beat_time))
    }

    /// Whether a fixed camera stands closer to whoever owns the conversation than `TOO_CLOSE_METRES`. Carth's on the
    /// Endar Spire (camera 2 of `end_m01ab`, 1.03 m away, 0.3 m from the console he stands behind) shows a ceiling and
    /// the top of his head: the shot is replaced by `speaker_camera`.
    ///
    /// **Assumed:** that the original does not use such a tight shot, or shows the console differently (its near plane
    /// is not known). The video has no frame of this talk to compare.
    fn too_close_to_speaker(&self, c: &Camera) -> bool {
        let w = &self.game.world;
        w.dialogue_owner()
            .filter(|o| *o != w.pc)
            .and_then(|o| w.object(o))
            .is_some_and(|o| {
                o.kind == Kind::Creature
                    && (o.position[0] - c.eye[0]).hypot(o.position[1] - c.eye[1]) < TOO_CLOSE_METRES
            })
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

    /// A body standing and breathing on a screen, `nth` of those shown, each at its own point of the idle clip.
    pub(crate) fn idling(&self, nth: usize) -> Option<Clip> {
        Some(Clip {
            name: "pause1",
            time: self.idle_clock + nth as f32 * 1.7,
            fade: None,
        })
    }

    /// The clip a creature plays in a conversation: what its node's `AnimList` names (`dialoganimations.2da`, ids from
    /// 10000), and else the owner talking and the player listening. `None` outside a conversation, and for a body
    /// that a cutscene's stunt animates.
    ///
    /// **Assumed:** the clip each `dialoganimations.2da` label stands for (`dialogue_clip_name`), and that everyone else
    /// stands still.
    pub fn dialogue_clip(&self, id: ObjId) -> Option<Clip> {
        let w = &self.game.world;
        if !w.in_dialogue() {
            return None;
        }
        let o = w.object(id)?;
        let named = self.beat.as_ref().and_then(|b| {
            b.step.animations.iter().find_map(|(who, anim)| {
                let mine = (id == w.pc && who.eq_ignore_ascii_case("PLAYER"))
                    || who.eq_ignore_ascii_case(&o.tag);
                let row = anim.checked_sub(DIALOGUE_ANIMATION_BASE)?;
                (mine && *anim >= DIALOGUE_ANIMATION_BASE)
                    .then(|| self.dialogue_label(row as usize))
                    .flatten()
            })
        });
        let name = named.or_else(|| {
            if Some(id) == w.dialogue_owner() && id != w.pc && !self.answering() {
                Some("tlknorm")
            } else if id == w.pc {
                Some("listen")
            } else {
                None
            }
        })?;
        Some(Clip {
            name,
            time: self.beat_time,
            fade: None,
        })
    }

    /// The clip of row `row` of `dialoganimations.2da`, by its label.
    fn dialogue_label(&self, row: usize) -> Option<&'static str> {
        let label = self.dialogue_labels.get(row)?;
        dialogue_clip_name(label)
    }

    /// The face of a creature in a cutscene's stunt: its mouth speaks the line on now, and its lids blink as it stands.
    fn stunt_face(&self, id: ObjId, rig: &Rig) -> Option<kotor_model::Pose> {
        let mouth = self.mouth_of(id);
        let name = if mouth.is_some() { "tlknorm" } else { "pause1" };
        let len = rig.animation_length(name).filter(|l| *l > 0.0)?;
        let t = self.beat_time % len;
        match mouth {
            Some((open, clock)) => rig.speaking_face(name, t, open, clock),
            None => rig.sample_face(name, t),
        }
    }

    /// The pose a creature's cutscene stunt puts it in this beat.
    fn stunt_pose(&self, id: ObjId, rig: &Rig) -> Option<kotor_model::Pose> {
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
        let raw = a.sample(stunt, self.beat_time.min(a.length));
        Some(rig.retarget(stunt, raw))
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
            // Not while the mouse is looking: it decides where the camera goes. In Modern the keys go by the camera,
            // so a camera that followed them would turn a strafe into a circle.
            _ if self.looking || (w.movement == MovementMode::Modern && w.steer.is_some()) => None,
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

    /// How far the camera may stand from the player at `p` along `back`, as the walkmesh and the doors have it.
    pub(super) fn camera_room(&self, p: [f32; 3], back: [f32; 3]) -> f32 {
        let target = [p[0], p[1], p[2] + CAMERA_LOOKS_AT];
        let mut d = self.distance;
        let modern = self.game.world.movement == MovementMode::Modern;
        if let Some(f) = &self.floor {
            // Walls in the walkmesh, and where the floor ends.
            if let Some(hit) = f.ray_distance(target, back, self.distance) {
                d = d.min(hit - 0.25);
            }
            // In Modern a floor above is a floor, as a player at the foot of a cliff has one behind them, and the gap that a
            // cliff leaves in the walkmesh is not the end of it.
            let top = p[2]
                + if modern {
                    moving::CAMERA_FLOOR_ABOVE
                } else {
                    1.0
                };
            let there = |t: f32| {
                let q = [p[0] + back[0] * t, p[1] + back[1] * t];
                f.height(q[0], q[1], top).is_some()
            };
            let mut t = 0.8;
            while t < d {
                let beyond = modern
                    && (1..=moving::CAMERA_GAP_LOOKS).any(|k| there(t + 0.3 + k as f32 * 0.3));
                if !there(t + 0.3) && !beyond {
                    d = t;
                    break;
                }
                t += 0.1;
            }
        }
        // Closed doors are not in the walkmesh, and a player beside one may have less room than usual.
        let d = d.max(0.8);
        self.door_limit(target, back)
            .map_or(d, |limit| d.min(limit))
    }

    /// How far the camera may stand from `target` along `back` for a closed door, across the door's whole width.
    pub(super) fn door_limit(&self, target: [f32; 3], back: [f32; 3]) -> Option<f32> {
        let mut nearest: Option<f32> = None;
        for o in self.game.world.objects.values() {
            if o.kind != Kind::Door || o.open {
                continue;
            }
            let [ax, ay] = o.door_axis;
            // Where the line of sight meets the door's plane, and how far along the door that is.
            let (ox, oy) = (o.position[0] - target[0], o.position[1] - target[1]);
            let (nx, ny) = (ay, -ax);
            let towards = back[0] * nx + back[1] * ny;
            if towards.abs() < 1e-4 {
                continue;
            }
            let t = (ox * nx + oy * ny) / towards;
            let along = (back[0] * t - ox) * ax + (back[1] * t - oy) * ay;
            let high = target[2] + back[2] * t - o.position[2];
            if t > 0.0
                && along.abs() <= o.door_half_width
                && high < DOOR_CAMERA_HEIGHT
                && nearest.is_none_or(|n| t < n)
            {
                nearest = Some(t);
            }
        }
        nearest.map(|t| (t - CAMERA_OFF_SURFACE).max(0.1))
    }

    /// The camera behind the player, drawn in where the floor ends so that it stays on this side of the walls.
    pub fn camera(&self) -> Camera {
        if let Some(c) = self
            .beat_camera()
            .filter(|c| !self.too_close_to_speaker(c))
            .or_else(|| self.speaker_camera())
            .or_else(|| self.turret_camera())
        {
            return c;
        }
        // Where it is drawn, or the player shakes against the room.
        let p = self
            .game
            .drawn_position(self.game.world.pc)
            .unwrap_or([0.0; 3]);
        let p = self.lagged(p);
        let (dx, dy) = (self.yaw.cos(), self.yaw.sin());
        let p = [
            p[0] + dy * self.camera_side,
            p[1] - dx * self.camera_side,
            p[2],
        ];
        let target = [p[0], p[1], p[2] + CAMERA_LOOKS_AT];
        let (flat, up) = (self.pitch + self.camera_rise).sin_cos();
        let (flat, up) = (up, flat);
        let back = [-dx * flat, -dy * flat, up];
        // In Modern the arm eases, and a fall does not snatch it in: see `arm_tick`, which also keeps it inside the room.
        let d = self.camera_arm.unwrap_or_else(|| {
            let d = self.camera_room(p, back);
            // Inside the room, however it is tilted. It keeps a little off the surface, and comes in to the player if it must.
            match self.shell.first_hit(target, back, d) {
                Some(hit) => (hit - CAMERA_OFF_SURFACE).max(0.1),
                None => d,
            }
        });
        Camera {
            eye: [
                p[0] - dx * flat * d,
                p[1] - dy * flat * d,
                p[2] + CAMERA_LOOKS_AT + up * d,
            ],
            target,
            fov: CAMERA_FOV + self.fov_extra,
        }
    }

    /// How far the camera points down, from -1 straight up to 1 straight down, for a test driver.
    pub fn looks_down(&self) -> f32 {
        let c = self.camera();
        let d = [
            c.target[0] - c.eye[0],
            c.target[1] - c.eye[1],
            c.target[2] - c.eye[2],
        ];
        -d[2] / (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-3)
    }

    /// Drag with the right button: turn the camera round the player and tilt it. The mouse going down looks down and going up
    /// looks up, since `pitch` lifts the camera above the player. The Invert Camera box is the other way round.
    pub fn look(&mut self, dx: f32, dy: f32) {
        // A half on the Mouse Settings slider is the speed this always had.
        let speed = self.settings().mouse_sensitivity * 2.0;
        self.yaw -= dx * 0.005 * speed;
        let flip = if self.settings().invert_y { -1.0 } else { 1.0 };
        self.pitch = (self.pitch + dy * flip * 0.004 * speed).clamp(-0.2, 1.2);
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

    /// How wide the 480-high layout is.
    pub fn ui_layout_width(&self) -> u32 {
        self.ui_width
    }

    /// Where in layout units a click takes this reply. A line with nothing to answer goes on with a click anywhere.
    pub fn line_point(&mut self, reply: usize) -> Option<(f32, f32)> {
        let view = self.choice_view()?;
        if reply == 0 && view.replies.iter().all(|r| r.trim().is_empty()) {
            return Some((self.ui_width as f32 / 2.0, 240.0));
        }
        let rows = self.ui.reply_rows(&view)?;
        let r = rows.get(reply)?;
        Some(((r[0] + r[2] / 2) as f32, (r[1] + r[3] / 2) as f32))
    }

    /// The pilot takes a reply as a player does: the cursor goes there, and it clicks. A line the mouse cannot move on is
    /// written down and moved on all the same, so that a run does not stall on it.
    fn pilot_clicks_a_line(&mut self, p: &mut kotor_game::Autopilot, reply: usize) {
        let size = (self.ui_width, 480);
        let before = self.game.world.dialogue_serial();
        if let Some(at) = self.line_point(reply) {
            self.hover(at, size);
            self.click(at, size);
        }
        if self.game.world.dialogue_serial() != before {
            p.lines_clicked += 1;
            if p.lines_clicked.is_multiple_of(CLICKS_TO_REPORT) {
                let line = format!(
                    "{} lines moved on by a click, {} not",
                    p.lines_clicked,
                    p.lines_not_clicked.len()
                );
                p.log.push(line);
            }
            return;
        }
        let name = self
            .game
            .world
            .dialogue_conversation()
            .map_or(String::new(), |c| c.name);
        p.log
            .push(format!("a click did not move on a line of {name}"));
        p.lines_not_clicked.push(name);
        self.game.world.dialogue_choose(reply);
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
        let under = self.plate_at(at.0, at.1).or(self.game.world.pick(o, d));
        self.cursor_on(under, at);
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
            if tag == "BTN_ACTION0" {
                let action = self.target_view_action();
                match self.target().filter(|_| !menus::slot_is_pickup(action)) {
                    Some(id) => self.use_thing(id),
                    None => self.pickup(),
                }
            } else if let Some(slot) = menus::target_slot(&tag) {
                if self.use_slot(slot) {
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
        if let Some(id) = self.plate_at(x, y).or(self.game.world.pick(o, d)) {
            self.plates.kept = None;
            self.hover_target = Some(id);
            self.use_thing(id);
            return;
        }
        self.drop_kept_target();
        if let Some(p) = self
            .floor_under_ray(o, d)
            .filter(|_| self.settings().click_to_walk)
        {
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

    /// A number key, by what is up: a step on a numbered panel, a reply, a line going on, or a slot under an enemy, as the
    /// video's "Activate Left Action : 1" has it.
    pub fn number_pressed(&mut self, n: usize) {
        if self.menu_open() && !self.container_up() {
            return self.menu_number(n);
        }
        let w = &self.game.world;
        if w.in_dialogue() {
            let replies = w.dialogue_view().map_or(0, |v| v.3.len());
            if replies == 0 && n == 1 {
                return self.continue_line();
            }
        } else if w.opened.is_none() && (1..=3).contains(&n) {
            let hostile = self.target_view().is_some_and(|t| t.3 == "Attack");
            if hostile && self.use_slot(n - 1) {
                if let Some(id) = self.target() {
                    self.use_thing(id);
                }
            }
            return;
        }
        self.number_key(n);
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
        // A cutscene's node has a reply that says nothing: it goes on just the same.
        let waiting = self
            .game
            .world
            .dialogue_view()
            .is_some_and(|(_, _, _, r)| r.iter().all(|x| x.trim().is_empty()));
        if waiting {
            self.line_left = None;
            self.game.world.dialogue_choose(0);
        }
    }

    pub fn static_scene(&self) -> &Scene {
        &self.static_scene
    }

    /// A named texture, decoded once.
    /// Change the underwear style. Takes effect as textures load, so with the next module or start.
    pub fn set_conventional_underwear(&mut self, on: bool) {
        if self.conventional_underwear != on {
            self.conventional_underwear = on;
            self.textures.retain(|name, _| {
                let name = kotor_render::underwear::split_variant(name).0;
                kotor_render::underwear::body_of_texture(name).is_none()
                    && !kotor_render::underwear::is_hero_head(name)
            });
            self.neck_plans.clear();
            self.rigs.clear();
            self.static_version += 1;
        }
    }

    /// What to add to the names of a figure's body and head textures so it wears an underwear variant: nothing for the
    /// standard paint, else `@` and the variant's id. The character creation screen's figure uses it for its preview.
    pub fn figure_underwear_suffix(id: &str) -> String {
        if id.is_empty() {
            String::new()
        } else {
            format!("@{id}")
        }
    }

    /// The underwear the character creation screen's figures wear while a choice is being looked at: a variant's id, or empty.
    pub fn set_preview_underwear(&mut self, id: &str) {
        self.preview_underwear = id.to_string();
    }

    pub fn texture(&mut self, name: &str) -> Option<Image> {
        let (plain, variant) = kotor_render::underwear::split_variant(name);
        let (plain, variant) = (plain.to_string(), variant.to_string());
        let inst = &mut self.inst;
        let hd = self.hd3d.clone();
        let conventional = self.conventional_underwear;
        let plans = &mut self.underwear_plans;
        let necks = &mut self.neck_plans;
        let folder = self.texture_folder.clone();
        let problems = &mut self.texture_problems;
        let mut problem_found = false;
        let got = self
            .textures
            .entry(name.to_string())
            .or_insert_with(|| {
                let name = plain.as_str();
                let stock = hd
                    .and_then(|d| upscaled(&d, name))
                    .or_else(|| kotor_render::load_texture(inst, name));
                // The owner's own picture of the texture stands in for all of this: it is what he painted.
                if let Some(folder) = &folder {
                    match crate::texture_override::load(
                        folder,
                        name,
                        stock.as_ref().map(|s| (s.width, s.height)),
                    ) {
                        Ok(Some(own)) => return Some(own),
                        Ok(None) => {}
                        Err(why) => {
                            problems.push(why);
                            problem_found = true;
                        }
                    }
                }
                if let Some(inside) = kotor_render::custom_mesh::embedded_texture(name) {
                    if stock.is_none() {
                        return Some(inside);
                    }
                }
                let mut img = stock?;
                let body = kotor_render::underwear::body_of_texture(name).filter(|_| conventional);
                if let Some(body) = body {
                    let plan = plans
                        .entry(body.clone())
                        .or_insert_with(|| kotor_render::underwear::plan_for(inst, &body));
                    if let Some(plan) = plan {
                        plan.apply(&mut img);
                        // A player's chosen underwear: our bra and briefs in the blues of its folder's sheet.
                        if let Some(palette) = kotor_render::underwear::variant_palette(&variant) {
                            plan.tint(&mut img, &palette);
                        }
                    }
                }
                if conventional && kotor_render::underwear::is_hero_head(name) {
                    let plan = necks
                        .entry(name.to_ascii_lowercase())
                        .or_insert_with(|| kotor_render::underwear::neck_plan_for(inst, name));
                    if let Some(plan) = plan {
                        plan.apply(&mut img);
                    }
                }
                Some(img)
            })
            .clone();
        if problem_found {
            self.write_texture_problems();
        }
        got
    }

    /// `texture_problems.txt` in the owner's folder: which of his pictures the game is not using, and why.
    fn write_texture_problems(&self) {
        let Some(folder) = &self.texture_folder else {
            return;
        };
        let mut text =
            String::from("The game is using its own texture for each picture named here:\n");
        for p in &self.texture_problems {
            text.push_str(p);
            text.push('\n');
        }
        // Only a help to the reader: the game goes on without it.
        let _ = std::fs::write(folder.join("texture_problems.txt"), text);
    }

    /// F8: look at the owner's textures again. Pictures already drawn are asked for afresh.
    pub fn reload_textures(&mut self) {
        self.texture_folder = crate::texture_override::folder().filter(|d| d.is_dir());
        self.textures.clear();
        self.texture_problems.clear();
        if let Some(folder) = &self.texture_folder {
            let _ = std::fs::write(
                folder.join("texture_problems.txt"),
                "No problems found so far: the game is using your pictures.\n",
            );
        }
    }

    /// The owner's animation folder: `KOTOR_RS_ANIMS`, else `%LOCALAPPDATA%\kotor-rs\anims`.
    fn anim_folder() -> Option<std::path::PathBuf> {
        if let Some(d) = std::env::var_os("KOTOR_RS_ANIMS") {
            return Some(std::path::PathBuf::from(d));
        }
        let base = std::env::var_os("LOCALAPPDATA")?;
        Some(
            std::path::PathBuf::from(base)
                .join("kotor-rs")
                .join("anims"),
        )
    }

    /// F8: read the owner's `*.anim.txt` files again. A clip named like a stock one stands in for it; any other name is a clip
    /// of its own, which F9 plays on the player. What is wrong goes to `anim_problems.txt` in the folder.
    pub fn reload_custom_clips(&mut self) {
        self.anim_problems.clear();
        self.shown_clip = None;
        let Some(folder) = Self::anim_folder().filter(|d| d.is_dir()) else {
            kotor_render::custom_clip::set_all(Vec::new());
            return;
        };
        // The player's own body names the bones a clip may key.
        let pc = self.game.world.pc;
        let bones: Vec<String> = self
            .rig_of(pc)
            .map(|r| {
                let mut v = Vec::new();
                r.body_model()
                    .root
                    .walk(&mut |n| v.push(n.name.to_ascii_lowercase()));
                v
            })
            .unwrap_or_default();
        let (clips, problems) = kotor_render::custom_clip::load_folder(&folder, &bones);
        let mut text = if problems.is_empty() {
            format!(
                "No problems: the game is using {} animation file(s).\n",
                clips.len()
            )
        } else {
            String::from("Problems with your animation files (a file named here is not used, or part of it):\n")
        };
        for p in &problems {
            text.push_str(p);
            text.push('\n');
        }
        // Only a help to the reader: the game goes on without it.
        let _ = std::fs::write(folder.join("anim_problems.txt"), text);
        self.anim_problems = problems;
        kotor_render::custom_clip::set_all(clips);
    }

    /// The owner's mesh folder: `KOTOR_RS_MESHES`, else `%LOCALAPPDATA%\kotor-rs\meshes`.
    fn mesh_folder() -> Option<std::path::PathBuf> {
        if let Some(d) = std::env::var_os("KOTOR_RS_MESHES") {
            return Some(std::path::PathBuf::from(d));
        }
        let base = std::env::var_os("LOCALAPPDATA")?;
        Some(
            std::path::PathBuf::from(base)
                .join("kotor-rs")
                .join("meshes"),
        )
    }

    /// F8: read the owner's garments again: every `.glb` (or `.gltf` with its `.bin`) in the folder, with what each covers of
    /// the stock body from `items.txt` (`<file name without the extension> covers=body,arms`). What is wrong goes to
    /// `mesh_problems.txt` in the folder, and a file with a mistake is left out.
    pub fn reload_custom_meshes(&mut self) {
        self.mesh_problems.clear();
        self.garments.clear();
        self.dressed.clear();
        self.textures.clear();
        kotor_render::custom_mesh::clear_embedded();
        let Some(folder) = Self::mesh_folder().filter(|d| d.is_dir()) else {
            self.worn_garment = None;
            return;
        };
        let pc = self.game.world.pc;
        let bones: Vec<String> = self
            .rig_of(pc)
            .map(|r| {
                let mut v = Vec::new();
                r.body_model()
                    .root
                    .walk(&mut |n| v.push(n.name.to_ascii_lowercase()));
                v
            })
            .unwrap_or_default();
        let items = std::fs::read_to_string(folder.join("items.txt")).unwrap_or_default();
        let covers_of = |name: &str| -> Vec<String> {
            for line in items.lines() {
                let line = line.split('#').next().unwrap_or("").trim();
                let mut words = line.split_whitespace();
                if words.next().is_some_and(|w| w.eq_ignore_ascii_case(name)) {
                    return words
                        .filter_map(|w| w.strip_prefix("covers="))
                        .flat_map(|c| c.split(','))
                        .map(|c| c.to_ascii_lowercase())
                        .collect();
                }
            }
            Vec::new()
        };
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&folder)
            .map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).collect())
            .unwrap_or_default();
        files.sort();
        for path in files {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase)
                .unwrap_or_default();
            if ext != "glb" && ext != "gltf" {
                continue;
            }
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("garment")
                .to_string();
            let shown = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("?")
                .to_string();
            let Ok(bytes) = std::fs::read(&path) else {
                self.mesh_problems.push(format!("{shown}: cannot be read"));
                continue;
            };
            let bin = std::fs::read(path.with_extension("bin")).ok();
            match kotor_render::custom_mesh::parse(&stem, &bytes, bin.as_deref(), &bones) {
                Ok(mut g) => {
                    g.covers = covers_of(&stem);
                    self.garments.push(g);
                }
                Err(errs) => {
                    for e in errs {
                        self.mesh_problems
                            .push(format!("{shown}: {e}; the game is not using this file"));
                    }
                }
            }
        }
        let mut text = if self.mesh_problems.is_empty() {
            format!(
                "No problems: the game is using {} garment file(s).\n",
                self.garments.len()
            )
        } else {
            String::from("Problems with your mesh files (a file named here is not used):\n")
        };
        for p in &self.mesh_problems {
            text.push_str(p);
            text.push('\n');
        }
        // Only a help to the reader: the game goes on without it.
        let _ = std::fs::write(folder.join("mesh_problems.txt"), text);
        if self.worn_garment.is_some_and(|g| g >= self.garments.len()) {
            self.worn_garment = None;
        }
    }

    /// What was wrong with the owner's mesh files when last read.
    pub fn mesh_problems(&self) -> &[String] {
        &self.mesh_problems
    }

    /// F10: the player puts on the next of the owner's garments, and after the last, takes them off.
    pub fn cycle_garment(&mut self) {
        let next = self.worn_garment.map_or(0, |g| g + 1);
        if next < self.garments.len() {
            self.worn_garment = Some(next);
            let name = self.garments[next].name.clone();
            self.say_over_player(format!("Wearing: {name}"));
        } else {
            self.worn_garment = None;
            self.say_over_player(if self.garments.is_empty() {
                "No garment files of yours were found".to_string()
            } else {
                "Back to the game's own clothes".to_string()
            });
        }
    }

    /// What was wrong with the owner's animation files when last read.
    pub fn anim_problems(&self) -> &[String] {
        &self.anim_problems
    }

    /// F9: the player plays the next of the owner's clips, and after the last, goes back to what the game has.
    pub fn cycle_custom_clip(&mut self) {
        let names = kotor_render::custom_clip::names();
        let next = match self.shown_clip {
            None => 0,
            Some(i) => i + 1,
        };
        let pc = self.game.world.pc;
        if let Some(name) = names.get(next) {
            self.shown_clip = Some(next);
            self.force_clip(pc, Some(name));
            self.say_over_player(format!("Your clip: {name}"));
        } else {
            self.shown_clip = None;
            self.force_clip(pc, None);
            self.say_over_player(if names.is_empty() {
                "No animation files of yours were found".to_string()
            } else {
                "Back to the game's own".to_string()
            });
        }
    }

    /// What was wrong with the owner's textures when last read.
    pub fn texture_problems(&self) -> &[String] {
        &self.texture_problems
    }

    /// A picture for the frame, without making it wait. An upscaled one is read on a thread and the stock one stands in; a few
    /// stock ones are read in a frame, and past that one with no pixels says to try again. The paint on a hero's body and
    /// head is not put off: they come with a load or a change of clothes, behind a screen, and the wrong paint must not show.
    pub fn texture_soon(&mut self, name: &str) -> Option<Image> {
        let (plain, variant) = kotor_render::underwear::split_variant(name);
        let painted = !variant.is_empty()
            || kotor_render::underwear::body_of_texture(plain).is_some()
            || kotor_render::underwear::is_hero_head(plain);
        if painted || self.textures.contains_key(name) {
            return self.texture(name);
        }
        let later = Image {
            width: 0,
            height: 0,
            rgba: Vec::new(),
        };
        let upscaled_here = self.hd3d.as_ref().is_some_and(|d| {
            d.join(format!("{}.png", plain.to_ascii_lowercase()))
                .is_file()
        });
        if self.pictures_left == 0 {
            return Some(later);
        }
        self.pictures_left -= 1;
        if !upscaled_here {
            return self.texture(name);
        }
        self.read_upscaled(name);
        match kotor_render::load_texture(&mut self.inst, plain) {
            Some(stock) => {
                self.stand_ins.insert(name.to_string());
                Some(stock)
            }
            None => Some(later),
        }
    }

    fn read_upscaled(&mut self, name: &str) {
        let Some(dir) = self.hd3d.clone() else {
            return;
        };
        let reader = self.hd_reader.get_or_insert_with(|| {
            let (ask, asked) = std::sync::mpsc::channel::<String>();
            let (give, given) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                for name in asked {
                    let img = upscaled(&dir, &name);
                    if give.send((name, img)).is_err() {
                        break;
                    }
                }
            });
            HdReader {
                ask,
                given,
                asked: HashSet::new(),
            }
        });
        if reader.asked.insert(name.to_string()) {
            let _ = reader.ask.send(name.to_string());
        }
    }

    /// What the thread has read since last asked, one a frame: each has to be sent to the card.
    fn take_upscaled(&mut self) {
        let Some(reader) = self.hd_reader.as_ref() else {
            return;
        };
        if let Ok((name, Some(img))) = reader.given.try_recv() {
            self.stand_ins.remove(&name);
            self.textures.insert(name.clone(), Some(img));
            self.hd_come.push(name);
        }
    }

    /// The pictures the renderer should ask for again. With `everything`, those standing in as well: what is drawn from now
    /// on for good must not keep a stand-in.
    pub fn textures_changed(&mut self, everything: bool) -> Vec<String> {
        let mut names = std::mem::take(&mut self.hd_come);
        if everything {
            names.extend(self.stand_ins.drain());
        }
        names
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

    /// A sound from a place: the farther from the player, the less of it.
    fn effect_from(&mut self, name: &str, at: [f32; 3]) {
        let gain = heard_share(dist(self.pc_position(), at));
        if gain <= 0.0 {
            return;
        }
        if let Ok(bytes) = self.inst.read(name, ResType::WAV) {
            self.cues.push(if gain < 1.0 {
                Cue::FaintEffect(bytes, gain)
            } else {
                Cue::Effect(bytes)
            });
        }
    }

    /// Sounds and bolts of what happened since the last frame: attacks, doors and the player's steps. Sound names come
    /// from `ammunitiontypes.2da`, `weaponsounds.2da`, `placeableobjsnds.2da` and `footstepsounds.2da`.
    ///
    /// **Assumed:** that the floor sounds like metal, and the pace of steps.
    fn play_world_sounds(&mut self, dt: f32) {
        let n = self.game.world.combat.log.len();
        let new: Vec<_> = self.game.world.combat.log[self.log_seen.min(n)..].to_vec();
        self.log_seen = n;
        for l in new {
            self.attack_effects(l.attacker, l.target, l.outcome != kotor_game::Outcome::Miss);
        }
        // A thrown creature comes down: against a wall first (a hard blunt blow on metal), then the floor.
        for t in std::mem::take(&mut self.game.world.thuds) {
            if let Some(at) = self.game.world.object(t.who).map(|o| o.position) {
                if t.wall {
                    self.effect_from("cb_ht_bllrgmetal", at);
                }
                self.effect_from("cb_ht_bllrgleth1", at);
            }
        }
        // What the hurt and the dying say, from their soundsets.
        for (id, sound) in std::mem::take(&mut self.game.world.sound_cues) {
            if let Some(at) = self.game.world.object(id).map(|o| o.position) {
                self.effect_from(&sound, at);
            }
        }
        let doors: Vec<(ObjId, bool, String, [f32; 3])> = self
            .game
            .world
            .objects
            .values()
            .filter(|o| o.kind == Kind::Door)
            .map(|o| (o.id, o.open, o.template.clone(), o.position))
            .collect();
        for (id, open, template, at) in doors {
            let was = self.door_seen.insert(id, open);
            if was.is_some_and(|w| w != open) {
                let column = if open { "opened" } else { "closed" };
                if let Some(sound) = self.door_sound(&template, column) {
                    self.effect_from(&sound, at);
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

    /// Fire the shots of every ranged attack clip whose moment has come since the last frame.
    fn fire_on_frames(&mut self, dt: f32) {
        let ids: Vec<ObjId> = self.volleys.keys().copied().collect();
        for id in ids {
            let clip = self.game.clip(id).map(|c| (c.name, c.time));
            let Some(mut v) = self.volleys.remove(&id) else {
                continue;
            };
            v.age += dt;
            let ranged = clip.filter(|(n, _)| kotor_game::is_ranged_attack(n));
            let Some((name, time)) = ranged else {
                // The clip has come and gone, or has not come in four seconds: no more shots.
                if !(v.seen || v.age > 4.0) {
                    self.volleys.insert(id, v);
                }
                continue;
            };
            if !v.seen || time < v.last {
                v.seen = true;
                v.last = -1.0;
            }
            for t in kotor_game::fire_times(name) {
                if v.last < *t && *t <= time {
                    let w = &self.game.world;
                    let (Some(a), Some(g)) = (w.object(id), w.object(v.target)) else {
                        continue;
                    };
                    let (from, to) = (a.position, g.position);
                    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
                    let len = dx.hypot(dy).max(0.01);
                    let muzzle = [
                        from[0] + dx / len * 0.6,
                        from[1] + dy / len * 0.6,
                        from[2] + 1.15,
                    ];
                    self.effects
                        .fire(muzzle, [to[0], to[1], to[2] + 1.15], &v.bolt, v.color);
                    if let Some(s) = v.shot.clone() {
                        self.effect_from(&s, from);
                    }
                }
            }
            v.last = time;
            self.volleys.insert(id, v);
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

    /// `name` if the install has a sound of that name, else `fallback`.
    fn sound_that_exists(
        &mut self,
        name: Option<String>,
        fallback: Option<String>,
    ) -> Option<String> {
        match name {
            Some(n) if self.inst.read(&n, ResType::WAV).is_ok() => Some(n),
            _ => fallback,
        }
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
            // The sonic weapons' impact, `cb_ht_sonblclth1`, is in no archive of the game: a sonic hit was silent. Whatever a
            // row names that is not there falls back to the plain blaster's (row 1) sound of the same kind.
            let plain = |c: &str| {
                table
                    .get(1, c)
                    .filter(|v| !v.is_empty())
                    .map(str::to_string)
            };
            let (plain_shot, plain_impact) = (plain("shotsound0"), plain("impactsound0"));
            let (shot, impact) = (
                self.sound_that_exists(shot, plain_shot),
                self.sound_that_exists(impact, plain_impact),
            );
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
            let _ = muzzle;
            // The shots leave at their moments of the attack clip, each with its flash and its sound.
            self.volleys.insert(
                attacker,
                Volley {
                    target,
                    bolt,
                    color,
                    shot,
                    seen: false,
                    last: -1.0,
                    age: 0.0,
                },
            );
            if hit {
                if let Some(s) = impact {
                    self.effect_from(&s, to);
                }
            }
            return;
        }
        // The row of `weaponsounds.2da` is the item's own `weaponmattype` in `baseitems.2da`: 0 unarmed, 1 lightsaber, 2 blade,
        // 3 large blade. A stun baton, a staff or a quarterstaff once got a blade's sound by a guess from the label.
        let row = base
            .and_then(|b| self.baseitem(b, "weaponmattype"))
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
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
            self.effect_from(&s, from);
        }
        if hit {
            if let Some(s) = blow {
                self.effect_from(&s, to);
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
        let rig = self.rig_for(appearance, &template, armour)?;
        // The player in a garment of the owner's: the body with it on, kept by body and garment.
        if let (true, Some(g)) = (id == self.game.world.pc, self.worn_garment) {
            if let Some(garment) = self.garments.get(g) {
                let key = (appearance, g);
                let dressed = self
                    .dressed
                    .entry(key)
                    .or_insert_with(|| Rc::new(rig.dressed(garment)))
                    .clone();
                return Some(dressed);
            }
        }
        Some(rig)
    }

    /// A body in the clothes a character starts with, as the character's screens show it: the second set of `appearance.2da`.
    fn clothed_rig(&mut self, appearance: i32) -> Option<Rc<Rig>> {
        let key = (appearance, 1, Some(CLOTHES_LETTER));
        let inst = &mut self.inst;
        let conventional = self.conventional_underwear;
        self.rigs
            .entry(key)
            .or_insert_with(|| {
                kotor_render::rig_for_outfit(inst, appearance as usize, 1, Some(CLOTHES_LETTER))
                    .ok()
                    .map(|rig| styled(rig, conventional))
            })
            .clone()
    }

    /// How far out to fade a body, 0 to 1, for the camera at `eye`. A party member following the leader can stand where the
    /// camera is, and in a tight room the camera comes in to the player itself, and the view is then the inside of a body. Head,
    /// armour and what is in the hands all go together. Not in a conversation or a cutscene, where the shot is chosen and may be
    /// of them.
    fn eye_fade(&self, id: ObjId, at: [f32; 3], eye: [f32; 3]) -> f32 {
        let w = &self.game.world;
        let ours = id == w.pc || w.object(id).is_some_and(|o| o.party_member);
        if !ours || w.in_dialogue() || self.beat_camera().is_some() {
            return 0.0;
        }
        let across = (at[0] - eye[0]).hypot(at[1] - eye[1]);
        let up = (eye[2] - at[2]).clamp(0.0, BODY_TALL);
        let off = across.hypot(at[2] + up - eye[2]);
        ((FADE_FROM - off) / (FADE_FROM - GONE_WITHIN)).clamp(0.0, 1.0)
    }

    /// How far out the player is faded now, for a test.
    pub fn player_fade(&self) -> f32 {
        let w = &self.game.world;
        self.eye_fade(w.pc, self.pc_position(), self.camera().eye)
    }

    /// A body with nothing over its underwear, for the character screen.
    fn bare_rig(&mut self, appearance: i32) -> Option<Rc<Rig>> {
        let key = (appearance, 1, None);
        let inst = &mut self.inst;
        let conventional = self.conventional_underwear;
        self.rigs
            .entry(key)
            .or_insert_with(|| {
                kotor_render::rig_for_outfit(inst, appearance as usize, 1, None)
                    .ok()
                    .map(|rig| styled(rig, conventional))
            })
            .clone()
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
        // A body is tens of milliseconds to read. The rest stand out of the picture for a frame or two.
        if !self.rigs.contains_key(&key) {
            match self.rigs_left.as_mut() {
                Some(0) => return None,
                Some(left) => *left -= 1,
                None => {}
            }
        }
        let inst = &mut self.inst;
        let conventional = self.conventional_underwear;
        self.rigs
            .entry(key)
            .or_insert_with(|| {
                kotor_render::rig_for_outfit(inst, appearance as usize, var as u32, letter)
                    .ok()
                    .map(|rig| styled(rig, conventional))
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
        let model = self.model_of_item(base, &template);
        self.held.insert(item, model.clone());
        model
    }

    /// The model of an item by its base item and template.
    fn model_of_item(&mut self, base: usize, template: &str) -> Option<Rc<Model>> {
        let variation = self
            .inst
            .read(template, ResType(2025))
            .ok()
            .and_then(|b| Gff::parse(&b).ok())
            .and_then(|g| g.root.int("ModelVariation"))
            .unwrap_or(1);
        let class = self.baseitem(base, "itemclass").unwrap_or_default();
        let named = format!("{class}_{variation:03}");
        self.model(&named).or_else(|| {
            let d = self.baseitem(base, "defaultmodel")?;
            self.model(&d)
        })
    }

    /// A grenade in the air: its model by the item's template.
    fn flying_model(&mut self, template: &str) -> Option<Rc<Model>> {
        let base = self
            .inst
            .read(template, ResType(2025))
            .ok()
            .and_then(|b| Gff::parse(&b).ok())
            .and_then(|g| g.root.int("BaseItem"))?;
        self.model_of_item(base as usize, template)
    }

    /// Whether the clip a creature is playing hides what it holds.
    fn hides_equipment(&mut self, id: ObjId) -> bool {
        let Some(name) = self.game.clip(id).map(|c| c.name) else {
            return false;
        };
        if self.hide_equipment.is_none() {
            let set = self
                .inst
                .read("animations", ResType::TWODA)
                .ok()
                .and_then(|b| TwoDa::parse(&b).ok())
                .map(|t| {
                    (0..t.row_count())
                        .filter(|r| t.get(*r, "hideequippeditems") == Some("1"))
                        .filter_map(|r| t.get(r, "name").map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            self.hide_equipment = Some(set);
        }
        self.hide_equipment
            .as_ref()
            .is_some_and(|s| s.contains(name))
    }

    /// What a creature holds: hand node and model.
    fn hands(&mut self, id: ObjId) -> Vec<(&'static str, Rc<Model>)> {
        if self.hides_equipment(id) {
            return Vec::new();
        }
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
        let mut held: Vec<(&'static str, Rc<Model>)> = items
            .into_iter()
            .filter_map(|(node, item)| Some((node, self.item_model(item)?)))
            .collect();
        // The grenade being thrown is in the right hand until it is let go.
        if let Some(template) = self.game.world.grenade_in_hand(id).map(str::to_string) {
            if let Some(m) = self.flying_model(&template) {
                held.retain(|(node, _)| *node != "rhand");
                held.push(("rhand", m));
            }
        }
        held
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
        self.door_model(&template, pos, bearing)
    }

    /// The model of a door of this template, standing at `pos` turned `bearing`.
    fn door_model(&mut self, template: &str, pos: [f32; 3], bearing: f32) -> Option<DoorBody> {
        let utd = Gff::parse(&self.inst.read(template, ResType(2042)).ok()?).ok()?;
        let table = TwoDa::parse(&self.inst.read("genericdoors", ResType::TWODA).ok()?).ok()?;
        let name = table
            .get(utd.root.int("GenericType")? as usize, "modelname")?
            .to_string();
        Some(DoorBody {
            model: load_model(&mut self.inst, &name)?,
            xf: placement(pos, bearing),
        })
    }

    /// A door's bearing in an area's GIT, at the door's position.
    fn door_bearing(&mut self, area: &str, pos: [f32; 3]) -> f32 {
        let Some(git) = self
            .inst
            .read(area, ResType::GIT)
            .ok()
            .and_then(|b| Gff::parse(&b).ok())
        else {
            return 0.0;
        };
        let at = |s: &kotor_gff::Struct, l: &str| s.float(l).unwrap_or(0.0) as f32;
        git.root
            .list("Door List")
            .and_then(|l| {
                l.iter()
                    .find(|d| (at(d, "X") - pos[0]).abs() + (at(d, "Y") - pos[1]).abs() < 0.01)
            })
            .map_or(0.0, |d| at(d, "Bearing"))
    }

    /// Everything that moves, posed for this frame: creatures near the player and every door.
    pub fn dynamic_scene(&mut self) -> Scene {
        self.rigs_left = self.pace_loading.then_some(RIGS_A_FRAME);
        let scene = self.dynamic_scene_now();
        self.rigs_left = None;
        scene
    }

    fn dynamic_scene_now(&mut self) -> Scene {
        let mut scene = Scene::default();
        if !self.started {
            return scene;
        }
        if let Some(turret) = self.turret_scene() {
            return turret;
        }
        let here = self.pc_position();
        // A shot of someone far from the player (Carth on the comm) is a view from beside them.
        let eye = self.camera().eye;
        let speaker = self.game.world.dialogue_owner();
        for pose in self.game.poses() {
            match pose.kind {
                Kind::Creature => {
                    let from = |at: [f32; 3]| {
                        let (dx, dy) = (pose.position[0] - at[0], pose.position[1] - at[1]);
                        (dx * dx + dy * dy).sqrt()
                    };
                    if from(here) > DRAW_RANGE && from(eye) > DRAW_RANGE && speaker != Some(pose.id)
                    {
                        continue;
                    }
                    let fade = self.eye_fade(pose.id, pose.position, eye);
                    if fade >= 1.0 {
                        continue;
                    }
                    let Some(rig) = self.rig_of(pose.id) else {
                        continue;
                    };
                    let hands = self.hands(pose.id);
                    let underwear = self
                        .game
                        .world
                        .object(pose.id)
                        .map(|o| o.underwear.clone())
                        .unwrap_or_default();
                    if let Some(p) = self.stunt_pose(pose.id, &rig) {
                        // A stunt is in world space, like the camera beside it.
                        let face = self.stunt_face(pose.id, &rig);
                        if let Ok(mut body) = rig.posed_with_face(Some(&p), face.as_ref()) {
                            for (node, item) in &hands {
                                rig.attach(&mut body, Some(&p), node, item);
                            }
                            wear_underwear(&mut body, 0, &underwear);
                            scene.append_transformed(&body, &placement([0.0; 3], 0.0));
                        }
                        continue;
                    }
                    let clip = match self.forced_clip {
                        Some((id, name)) if id == pose.id => Some(Clip {
                            name,
                            time: (self.idle_clock - self.forced_at).max(0.0),
                            fade: None,
                        }),
                        _ => self.dialogue_clip(pose.id).or_else(|| pose.clip.clone()),
                    };
                    let start = scene.meshes.len();
                    if self.add_bent_body(
                        &mut scene,
                        pose.id,
                        &rig,
                        &hands,
                        &clip,
                        pose.position,
                        pose.heading,
                    ) {
                        wear_underwear(&mut scene, start, &underwear);
                        scene.meshes[start..].iter_mut().for_each(|m| m.fade = fade);
                        continue;
                    }
                    self.add_speaking_creature(
                        &mut scene,
                        &rig,
                        &hands,
                        &clip,
                        pose.position,
                        pose.heading,
                        &underwear,
                        self.mouth_of(pose.id),
                    );
                    scene.meshes[start..].iter_mut().for_each(|m| m.fade = fade);
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
        // A grenade in the air, spinning as it goes.
        for g in self.game.world.grenades_in_flight() {
            if let Some(m) = self.flying_model(&g.template) {
                let angle = self.game.world.time * 9.0;
                scene.add_model_with(&m, None, &placement(g.position, angle), &[], None, None);
            }
        }
        self.neighbour_figures(&mut scene, false);
        scene
    }

    #[allow(clippy::too_many_arguments)]
    fn add_creature(
        &self,
        scene: &mut Scene,
        rig: &Rig,
        hands: &[(&'static str, Rc<Model>)],
        clip: &Option<Clip>,
        at: [f32; 3],
        heading: f32,
        underwear: &str,
    ) {
        self.add_speaking_creature(scene, rig, hands, clip, at, heading, underwear, None);
    }

    /// Like `add_creature`, with a mouth that speaks: how far open, and the talking loop's clock (`Stage::mouth_of`).
    #[allow(clippy::too_many_arguments)]
    fn add_speaking_creature(
        &self,
        scene: &mut Scene,
        rig: &Rig,
        hands: &[(&'static str, Rc<Model>)],
        clip: &Option<Clip>,
        at: [f32; 3],
        heading: f32,
        underwear: &str,
        mouth: Option<(f32, f32)>,
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
        let mut pose = len.and_then(|_| rig.sample(name, t));
        // The clip this one took over from still shows through, fading out.
        if let (Some(f), Some(now)) = (clip.as_ref().and_then(|c| c.fade), pose.as_ref()) {
            if let Some(l) = rig.animation_length(f.name).filter(|l| *l > 0.0) {
                if let Some(old) = rig.sample(f.name, f.time % l) {
                    pose = Some(kotor_model::blend(&old, now, 1.0 - f.weight));
                }
            }
        }
        // The face moves with the clip when the head has a clip of that name: breathing, talking, listening.
        let face = len.and_then(|_| match mouth {
            Some((open, clock)) => rig.speaking_face(name, t, open, clock),
            None => rig.sample_face(name, t),
        });
        let Ok(mut body) = rig.posed_with_face(pose.as_ref(), face.as_ref()) else {
            return;
        };
        for (node, item) in hands {
            rig.attach(&mut body, pose.as_ref(), node, item);
        }
        // A player's choice of underwear: the bare body and the head's neck go by another name, which `texture` knows.
        wear_underwear(&mut body, 0, underwear);
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
        let began = std::time::Instant::now();
        self.refresh_overlay_now();
        self.overlay_ms = began.elapsed().as_secs_f32() * 1000.0;
    }

    fn refresh_overlay_now(&mut self) {
        let s = self.settings();
        if self.ui.set_text_style(s.noto_font, s.glow_boxes) {
            self.overlay_key.clear();
        }
        if kotor_ui::set_australian(s.australian_english) {
            self.overlay_key.clear();
        }
        if let Some(view) = self.menu_view() {
            self.plates_view_none();
            let key = format!("{view:?}{}", self.ui_width);
            if key != self.overlay_key {
                if self.menu_in_part(&view) {
                    self.overlay_key = key;
                    return;
                }
                // A pop-up over the game leaves the HUD under it, as the video has it at 3:36.
                let hud = (view.over_scene && view.notice.is_some()).then(|| {
                    self.overlay_key.clear();
                    self.refresh_game_overlay();
                    self.overlay.take()
                });
                self.overlay_key = key;
                self.hud_is_up = false;
                self.overlay = self.ui.menu_overlay(&view);
                self.apply_brightness();
                if let (Some(Some(mut under)), Some(over)) = (hud, self.overlay.as_ref()) {
                    crate::composite(&mut under, over);
                    self.overlay = Some(under);
                }
                self.overlay_version += 1;
                self.menu_drawn = Some((self.overlay_version, view));
            }
            return;
        }
        // A race has its clock and meter on its camera's model.
        if self.game.world.in_race() {
            if self.overlay.take().is_some() {
                self.overlay_key.clear();
                self.overlay_version += 1;
            }
            return;
        }
        self.refresh_game_overlay();
    }

    /// Draw again only what has changed on the screen that is up, if that is all that has to be. Returns whether it did.
    fn menu_in_part(&mut self, view: &MenuView) -> bool {
        let Some((seen, was)) = self.menu_drawn.take() else {
            return false;
        };
        let usable = seen == self.overlay_version
            && !self.overlay_key.is_empty()
            && self.settings().brightness == 0.5;
        let picture = self.overlay.take_if(|_| usable);
        let Some(picture) = picture else { return false };
        let Some((picture, changed)) = self.ui.menu_part(picture, &was, view) else {
            return false;
        };
        self.overlay = Some(picture);
        self.overlay_changed_in(changed);
        self.menu_drawn = Some((self.overlay_version, view.clone()));
        true
    }

    /// The overlay has a new version, and all that changed is inside this box. Boxes since the last full change go together.
    fn overlay_changed_in(&mut self, changed: [u32; 4]) {
        let chained = self.plates_moved.filter(|m| m.1 == self.overlay_version);
        let (from, all) = match chained {
            Some((from, _, all)) => (from, union_box(Some(all), Some(changed)).unwrap_or(changed)),
            None => (self.overlay_version, changed),
        };
        self.overlay_version += 1;
        self.plates_moved = Some((from, self.overlay_version, all));
    }

    /// The HUD, or the conversation's box.
    fn refresh_game_overlay(&mut self) {
        if self.turret_on() {
            // No HUD: the cockpit's own dials, as the video has it.
            if self.overlay.is_some() {
                self.overlay = None;
                self.overlay_key.clear();
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
        let minimap = self.minimap_view();
        let combat = self.combat_view(target.as_ref().map(|t| t.3));
        let slots_percent = self.slots_percent();
        self.ui.set_slots_percent(slots_percent);
        self.plates.slots_up = combat.attack.is_some();
        let target_at = self.target_on_screen(combat.attack.is_some() || combat.locked);
        self.ui.set_target_at(target_at);
        self.ui.set_slots_above_bar(self.settings().nameplates);
        // Only over the plain HUD: not in a conversation or a cutscene.
        let plates = if view.is_none() && self.beat_camera().is_none() {
            self.plates_view(target_at)
        } else {
            self.plates_view_none()
        };
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
                "{}h{:?}{target:?}{target_at:?}{slots_percent}{cut}{combat:?}{notices:?}{pause:?}{minimap:?}{floats:?}",
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
                    .collect::<Vec<_>>(),
            ),
        };
        // What is under the plates depends on whether one is the target's, which takes the place of its bar.
        let key = format!("{key}{}", plates.iter().any(|p| p.framed));
        let plate_key = format!("{plates:?}");
        let map_seen_more = std::mem::take(&mut self.map_seen_more) && view.is_none();
        let redraw = key != self.overlay_key || map_seen_more;
        // Something has asked for all of it, by forgetting what was drawn.
        let part = view.is_none() && self.hud_is_up && !self.overlay_key.is_empty();
        self.hud_drawn = 0;
        if !redraw && plate_key == self.plate_key {
            return;
        }
        self.hud_drawn = 3;
        self.overlay_key = key;
        self.plate_key = plate_key;
        self.ui.set_plates(plates);
        if !redraw {
            self.hud_drawn = 1;
            self.plates_on_hud(true, None);
            return;
        }
        self.ui.set_minimap(minimap);
        self.ui.set_notices(notices);
        self.ui.set_floats(floats);
        self.ui.set_solo(self.game.world.solo_mode);
        self.ui.set_pause(pause.0, pause.1);
        self.ui.set_combat(combat);
        self.hud_is_up = view.is_none();
        if part {
            let t = target.as_ref();
            let action = t.map_or("", |t| t.3);
            let drawn = self.hud_under_plates.take().and_then(|picture| {
                self.ui.hud_part(
                    picture,
                    &party,
                    t.map(|t| (t.0.as_str(), t.1, t.2)),
                    action,
                    action_icon(action),
                )
            });
            if let Some((picture, changed)) = drawn {
                self.hud_under_plates = Some(picture);
                self.hud_drawn = 2;
                self.plates_on_hud(true, Some(changed));
                return;
            }
            self.hud_drawn = 5;
        }
        if view.is_some() {
            self.hud_drawn = 4;
        }
        self.hud_under_plates = match view {
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
        self.plates_on_hud(false, None);
    }

    /// What goes on screen: the HUD as last drawn, and the plates on it. With `same_hud`, only the plates have moved, and what
    /// has changed on the HUD inside the box `hud_box`.
    fn plates_on_hud(&mut self, same_hud: bool, hud_box: Option<[u32; 4]>) {
        // A conversation's box has no plates to move and is not drawn on again, so it need not be kept apart.
        if !self.settings().nameplates || !self.hud_is_up {
            self.overlay = self.hud_under_plates.take();
            self.plate_box = None;
            self.apply_brightness();
            self.overlay_version += 1;
            return;
        }
        let new_box = self.ui.plates_box();
        let plain = self.settings().brightness == 0.5;
        let quick = same_hud && plain && self.overlay.is_some();
        let dirty = union_box(union_box(self.plate_box, new_box), hud_box);
        self.plate_box = new_box;
        if let (true, Some(hud), Some(picture)) =
            (quick, self.hud_under_plates.as_ref(), self.overlay.as_ref())
        {
            if picture.width == hud.width && picture.height == hud.height {
                let Some(dirty) = dirty else { return };
                let picture = self.overlay.take().expect("just seen");
                let hud = self.hud_under_plates.as_ref().expect("just seen");
                self.overlay = Some(self.ui.move_plates(hud, picture, dirty));
                self.overlay_changed_in(dirty);
                return;
            }
        }
        let hud = self.hud_under_plates.as_ref();
        let ui = &mut self.ui;
        self.overlay = hud.map(|h| ui.plates_over(h));
        self.apply_brightness();
        self.overlay_version += 1;
    }

    /// If all that has changed since version `seen` of the overlay is inside a box, that box: left, top, width, height.
    pub fn overlay_change_since(&self, seen: u64) -> Option<[u32; 4]> {
        self.plates_moved
            .filter(|m| m.0 <= seen && m.1 == self.overlay_version)
            .map(|m| m.2)
    }

    /// The thing under the cursor, else the nearest in reach.
    fn target(&self) -> Option<ObjId> {
        let w = &self.game.world;
        self.hover_target
            .filter(|id| w.object(*id).is_some_and(|o| !o.dead) || w.lootable(*id))
            // The one being fought stays the target when the cursor leaves it, as at 6:05, so its slots can be reached.
            .or_else(|| {
                let fought = w.combat.target.get(&w.pc).copied();
                fought.filter(|id| w.object(*id).is_some_and(|o| !o.dead))
            })
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
        let left_slot = at(self.hud_pixel("BTN_TARGET0", layout));
        let left_down = at(self.hud_pixel("BTN_TARGETDOWN0", layout));
        let middle_slot = at(self.hud_pixel("BTN_TARGET1", layout));
        let right_slot = at(self.hud_pixel("BTN_TARGET2", layout));
        let disengage = at(self.hud_pixel("BTN_CLEARALL", layout));
        let down2 = at(self.hud_pixel("BTN_TARGETDOWN2", layout));
        let thrown = self.right_slot_name();
        let beside = at(self.hud_pixel("BTN_CHAR3", layout));
        let w = &self.game.world;
        let align = w.object(w.pc).map_or(-1, |o| o.alignment);
        // The OK of a message, on a screen's 640 by 480.
        let ok = at(self.menu_pixel("BTN_OK", None, (640, 480)));
        // The rows of a screen's list, the same way.
        let row0 = at(self.menu_pixel("LB_ITEMS", Some(0), (640, 480)));
        let row1 = at(self.menu_pixel("LB_ITEMS", Some(1), (640, 480)));
        let pc = self.game.world.pc;
        let solo = self.game.world.solo_mode;
        // What the one armed by a test driver has left.
        let grenades = self
            .armed
            .and_then(|i| self.game.world.object(i))
            .map_or("-".to_string(), |o| o.stack.to_string());
        // Whether a driver has armed anyone yet: "-" for the grenades also means all thrown.
        let armed = self.armed.is_some() as u8;
        // The one armed by a driver: its distance from the player, whether it fights, has noticed the player, and its
        // hit points. For a driver that finds a thrower holding its grenades.
        let thrower = {
            let w = &self.game.world;
            let holder = self.armed.and_then(|i| {
                w.objects
                    .values()
                    .find(|o| o.kind == Kind::Creature && w.carried(o.id).contains(&i))
            });
            holder.map_or("-".to_string(), |o| {
                let at = w.object(w.pc).map_or([0.0; 3], |p| p.position);
                format!(
                    "{:.1}/{}/{}/{}/{}",
                    (o.position[0] - at[0]).hypot(o.position[1] - at[1]),
                    w.combat.in_combat(o.id) as u8,
                    w.combat.perceived.contains(&(o.id, w.pc)) as u8,
                    o.hp,
                    o.template
                )
            })
        };
        // The turret's hit points, how many fighters are flying, and how far round it is.
        let turret = self
            .game
            .world
            .minigame
            .as_ref()
            .map_or("-".to_string(), |g| {
                format!("{}/{}/{:.0}", g.player.hit_points, g.alive(), g.offset[2])
            });
        let attack = self.game.world.attack_bonus_of(pc, 4);
        let defense = self.game.world.defense_of(pc);
        let (paused, note) = self.pause_view();
        let floats = self.floats_shown().len();
        let foe = self
            .seen_enemy()
            .and_then(|id| self.game.world.object(id))
            .and_then(|o| self.on_screen(o.position, 1.0))
            .map_or("-".to_string(), |p| format!("{},{}", p.0, p.1));
        let shown = self.plates_shown();
        let plates = shown.len();
        let plate0 = shown
            .first()
            .map_or("-".to_string(), |(_, p)| format!("{},{}", p.0, p.1));
        // Who stands nearest the camera, and how far its eye is from the top of them, in metres.
        let eye = self.camera().eye;
        let near_camera = self
            .game
            .world
            .objects
            .values()
            .filter(|o| o.kind == Kind::Creature && !o.dead && o.id != self.game.world.pc)
            .map(|o| {
                let head = [o.position[0], o.position[1], o.position[2] + 1.7];
                let d = (0..3)
                    .map(|k| (head[k] - eye[k]).powi(2))
                    .sum::<f32>()
                    .sqrt();
                (d, o.tag.replace(' ', "_"))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map_or("-".to_string(), |(d, tag)| format!("{tag},{d:.2}"));
        let enemy_plate = self
            .enemy_plate()
            .map_or("-".to_string(), |p| format!("{},{}", p.0, p.1));
        let chosen = self.container_chosen();
        let slotsize = self.ui.slots_percent();
        // Where a click takes the first reply or moves the line on, for a driver.
        let line_at = at(self.line_point(0));
        let w = &self.game.world;
        // What the open container holds: each row's template and how many.
        let loot = w
            .container_items()
            .iter()
            .filter_map(|(i, _)| w.object(*i))
            .map(|o| format!("{}*{}", o.template, w.container_count(o.id)))
            .collect::<Vec<_>>()
            .join(",");
        let here = self.pc_position();
        let view = w.dialogue_view();
        let race = w.race.as_ref().map_or("-".to_string(), |g| {
            let at = g.position();
            let gear = w.global_numbers.get("MIN_RACE_GEAR").copied().unwrap_or(-6);
            format!("{:.1},{gear},{:.2},{:.1}", g.speed, at[0], at[1])
        });
        format!(
            "state menu={} saves={} gold={} level={} xp={} hp={} worn={} module={} pc={:.2},{:.2} clip={} dlg={} replies={} said={} opened={} items={} inv={} pilot={} engaged={} paused={paused} note={} attacks={} queue={} floats={floats} width={} slot0={left_slot} down0={left_down} slot1={middle_slot} slot2={right_slot} disengage={disengage} ok={ok} solo={solo} turret={turret} grenades={grenades} armed={armed} thrower={thrower} align={align} attack={attack} defense={defense} row0={row0} row1={row1} beside={beside} leader={pc} down2={down2} thrown={thrown} foe={foe} race={race} pcz={:.3} speed={:.2} air={} roll={} plates={plates} plate0={plate0} enemyplate={enemy_plate} nearcam={near_camera} fov={:.1} pitch={:.4} yaw={:.3} heading={:.3} lookdown={:.4} steps={} loot={loot} chosen={chosen} slotsize={slotsize} lineat={line_at} target={}",
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
            w.pc_mover.pos[2],
            w.pc_body.speed(),
            !w.pc_body.grounded,
            w.pc_body.rolling(),
            self.camera().fov,
            self.pitch,
            self.yaw,
            w.pc_mover.heading,
            self.looks_down(),
            self.game.steps,
            self.target_view().map_or("-".to_string(), |t| t.0.replace(' ', "_")),
        )
    }

    /// Where the target's name bar goes, in layout units: over the target's head with its slots, as the video has it at 7:50.
    /// So it covers neither the target nor what is next to it, and the mouse can still reach them.
    fn target_on_screen(&self, slots: bool) -> Option<(i32, i32)> {
        let w = &self.game.world;
        let o = w.object(self.target()?)?;
        // A plate stays where it was when it becomes the target. Its slots go over it, if there is room.
        if self.settings().nameplates {
            let up = self.plate_up(o);
            let (x, y) = self.on_screen(o.position, up)?;
            let room = if slots {
                SLOTS_AND_GAP * self.slots_percent() / 100
            } else {
                0
            };
            return Some((x, (y - kotor_ui::TARGET_BAR_HIGH).max(room)));
        }
        let high = if slots {
            kotor_ui::TARGET_BAR_HIGH + SLOTS_AND_GAP * self.slots_percent() / 100
        } else {
            kotor_ui::TARGET_BAR_HIGH
        };
        let up = match o.kind {
            Kind::Door => 2.6,
            Kind::Creature if !o.dead => 2.1,
            _ => 1.1,
        };
        let (x, y) = self.on_screen(o.position, up)?;
        Some((x, (y - high - TARGET_BAR_CLEAR).max(0)))
    }

    /// Where a point of the world, `up` above the floor, is in the layout, kept on the screen.
    fn on_screen(&self, at: [f32; 3], up: f32) -> Option<(i32, i32)> {
        let wide = self.ui_width as i32;
        let (x, y) = self.project(at, up)?;
        Some((x.clamp(80, wide - 80), y.clamp(2, 440)))
    }

    /// Where a point of the world, `up` above the floor, is in the layout, on the screen or not.
    fn project(&self, at: [f32; 3], up: f32) -> Option<(i32, i32)> {
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
        Some((x, y))
    }

    /// The thing under the cursor, else the nearest in reach: name, hit points, and what using it does.
    fn target_view_action(&self) -> Option<&'static str> {
        self.target_view().map(|t| t.3)
    }

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
                // Only a player with no character made has no picture of their own.
                portrait: if o.portrait.is_empty() && o.id == w.pc {
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
        self.stand_ins.clear();
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

/// A creature in a player's chosen underwear: from mesh `from` on, the bare body and the head's neck go by another name, which
/// `Stage::texture` knows. The standard paint keeps its names.
fn wear_underwear(scene: &mut Scene, from: usize, underwear: &str) {
    if underwear.is_empty() {
        return;
    }
    for m in scene.meshes.iter_mut().skip(from) {
        if let Some(t) = &m.texture {
            if kotor_render::underwear::body_of_texture(t).is_some()
                || kotor_render::underwear::is_hero_head(t)
            {
                m.texture = Some(kotor_render::underwear::variant_texture_name(t, underwear));
            }
        }
    }
}
