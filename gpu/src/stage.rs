//! The playable scene: a [`Game`] with everything a window needs from it each frame. The camera, the posed bodies
//! of creatures and doors, the dialogue box and HUD, and what to play. Nothing here touches the GPU or a window,
//! so it runs in a headless test.

use kotor_archive::Install;
use kotor_core::ResType;
use kotor_game::{pixel_ray, Clip, Game, Kind, ObjId, World};
use kotor_gff::Gff;
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
/// Reading speed for a line with no voice-over, characters per second, as in `kotor_game::cutscene`.
const READING_SPEED: f32 = 15.0;
/// The follow camera: metres behind the player and above its feet, and the height it looks at. As `render_walk`.
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
    /// Degrees of `Q`/`E` turning asked for this frame, per second.
    pub turn: f32,
    overlay: Option<Image>,
    overlay_key: String,
    /// Goes up whenever the overlay changes.
    pub overlay_version: u64,
    cues: Vec<Cue>,
    events_seen: usize,
    line_key: Option<usize>,
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
            overlay: None,
            overlay_key: String::new(),
            overlay_version: 0,
            cues: Vec::new(),
            events_seen: 0,
            line_key: None,
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

    /// A new spoken line starts its voice and its timer; a line with no replies moves on when the timer ends.
    fn update_line(&mut self, dt: f32) {
        let Some((_, text, vo, replies)) = self.game.world.dialogue_view() else {
            if self.line_key.take().is_some() {
                self.cues.push(Cue::StopVoice);
            }
            self.line_left = None;
            return;
        };
        let key = self.game.world.transcript.len();
        if self.line_key != Some(key) {
            self.line_key = Some(key);
            self.lines_shown += 1;
            let voice = self.sound_file(&vo);
            let seconds = voice
                .as_ref()
                .and_then(|p| std::fs::read(p).ok())
                .and_then(|d| kotor_audio::probe(&d).ok())
                .map(|i| i.duration_secs as f32);
            self.cues.push(match voice {
                Some(p) => Cue::Voice(p),
                None => Cue::StopVoice,
            });
            self.line_left = replies.is_empty().then(|| {
                seconds
                    .unwrap_or(text.chars().count() as f32 / READING_SPEED)
                    .max(1.5)
                    + 0.3
            });
        }
        if let Some(left) = self.line_left.as_mut() {
            *left -= dt;
            if *left <= 0.0 {
                self.line_left = None;
                self.game.world.dialogue_choose(0);
            }
        }
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
        let p = self.pc_position();
        let target = [p[0], p[1], p[2] + CAMERA_LOOKS_AT];
        let (dx, dy) = (self.yaw.cos(), self.yaw.sin());
        let mut d = 0.8;
        if let Some(f) = &self.floor {
            while d < self.distance
                && f.height(p[0] - dx * (d + 0.3), p[1] - dy * (d + 0.3), p[2] + 1.0)
                    .is_some()
            {
                d += 0.1;
            }
        } else {
            d = self.distance;
        }
        Camera {
            eye: [
                p[0] - dx * d,
                p[1] - dy * d,
                p[2] + CAMERA_LOOKS_AT + (CAMERA_ABOVE - CAMERA_LOOKS_AT) * d / CAMERA_BEHIND,
            ],
            target,
            fov: 55.0,
        }
    }

    /// A left click at `pixel` of a window of `size`: acknowledge a line, or use what is under the cursor, or walk.
    pub fn click(&mut self, pixel: (f32, f32), size: (u32, u32)) {
        if self.game.world.in_dialogue() {
            self.continue_line();
            return;
        }
        let cam = self.camera();
        let (o, d) = pixel_ray(cam.eye, cam.target, cam.fov, size, pixel);
        let pc = self.game.world.pc;
        if let Some(id) = self.game.world.pick(o, d) {
            let w = &mut self.game.world;
            let enemy = w.object(id).is_some_and(|t| t.kind == Kind::Creature)
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
        // **Assumption:** a body whose supermodel has no `dead` clip is laid flat instead.
        let flat = name == "dead" && len.is_none();
        let (name, len) = if len.is_some() {
            (name, len)
        } else {
            ("pause1", rig.animation_length("pause1"))
        };
        let t = clip.as_ref().map_or(0.0, |c| c.time);
        let t = len
            .filter(|l| *l > 0.0)
            .map_or(0.0, |l| if name == "dead" { t.min(l) } else { t % l });
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
        let view = self.game.world.dialogue_view();
        let party = self.party_view();
        let key = match &view {
            Some(v) => format!("{}d{v:?}", self.module),
            None => format!(
                "{}h{:?}",
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
            Some((speaker, subtitle, _, replies)) => self.ui.dialogue_overlay(&DialogueView {
                speaker,
                subtitle,
                replies,
                selected: usize::MAX,
            }),
            None => self.ui.hud_overlay(&party, None),
        };
        let label = self.label();
        self.overlay = self.overlay.take().and_then(|o| self.ui.caption(o, &label));
        self.overlay_version += 1;
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
            "kotor-rs | {} | {} | area {} | camera on the player",
            self.module,
            if installed {
                "installed .mod"
            } else {
                "original files"
            },
            self.area_name()
        )
    }

    /// The window's shape: the interface is laid out 480 units high and as wide as that makes it.
    pub fn set_view(&mut self, size: (u32, u32)) {
        if let Some(width) = (480 * size.0).checked_div(size.1) {
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
