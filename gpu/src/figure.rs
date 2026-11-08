//! The 3D figure on the character sheet and the level-up panel, as the reference video has it at 1:23:50: the screen
//! leaves a hole in its picture, and behind it the renderer draws the leader alone, with a camera that puts them in the
//! hole, measured once per body so that all of it stands inside. **Not built:** turning the figure. The party screen has one too, as at
//! 3:12:00, of whoever is chosen: with nothing in their hands, as the video has them, and from their creature file if they
//! are not here.
//!
//! The mist behind them is the emitters of `charmain_light.mdl`, as making a character has `cgmain_light.mdl`. The install
//! has no dark model, so a dark figure's mist is the same mist with its blue and red swapped: the owner's words are blue for
//! the light side, red for the dark, and "The middle is grey". The video has Carth, 75 in his file, in blue at 50:12 and
//! Canderous, 30, in red at 7:17:02. **Assumed:** where it changes, `MIST_BLUE_FROM` and `MIST_RED_FROM`, and that the owner's
//! grey is this mist: I found no frame of someone at 50.

use super::{Screen, Stage};
use crate::effects::Effects;
use kotor_core::ResType;
use kotor_game::{Clip, SLOT_BODY};
use kotor_gff::Gff;
use kotor_render::{Camera, Rig, Scene};
use std::rc::Rc;

/// Where the holes are, from `character.gui`'s `LBL_3DCHAR` and `maincg.gui`'s `MODEL_LBL`: left, top, width, height.
const CHARACTER_HOLE: [f32; 4] = [55.0, 110.0, 259.0, 295.0];
const LEVEL_HOLE: [f32; 4] = [155.0, 89.0, 162.0, 186.0];
const PARTY_HOLE: [f32; 4] = [326.0, 109.0, 258.0, 213.0];
const CREATURE_FILE: ResType = ResType(2027);
const ITEM_FILE: ResType = ResType(2025);
/// A body no shorter than this is sized to fill the hole; a shorter one, such as T3-M4, stands at this scale and is not
/// blown up to a Wookiee's height. **Assumed:** the video has no droid on these screens that I could measure.
const FIGURE_HEIGHT: f32 = 1.75;
/// Air on each side, of the hole's height and width. An idle body moves a little, and the owner asked for at least 3%. The
/// video's figure fills about 92% of the hole at 22:17, and this leaves it 90%.
const FIGURE_MARGIN: f32 = 0.05;
/// How much of the idle is measured.
const IDLE_SAMPLES: usize = 12;
const IDLE_SAMPLE_SECONDS: f32 = 0.5;
/// Steps to settle size and place: each moves the picture a little for the next.
const FIT_STEPS: usize = 8;
/// `charmain_light.mdl`'s `camerahook` stands this far in front of the figure, and looks level.
const CAMERA_AWAY: f32 = 4.87;
/// How far over the feet the mist's model goes. **Assumed:** by eye and by measure from the video at 22:17, which is bright
/// at the foot and a little less at the top.
const MIST_UP: f32 = 0.25;
const SHEET_MIST: &str = "charmain_light";
const LEVEL_MIST: &str = "cgmain_light";
/// Seconds of mist made at once when a screen opens, so that it is there already.
const MIST_START: f32 = 6.0;
const MIST_STEP: f32 = 1.0 / 30.0;
const MIST_BLUE_FROM: i32 = 60;
const MIST_RED_FROM: i32 = 40;
const ALIGNMENT_NEUTRAL: i32 = 50;

/// The player's portrait made from their face, chargen's stage 8: what its name starts with, and its size, square.
const FACE_PORTRAIT_PREFIX: &str = "kotorrs_face_";
const FACE_PORTRAIT_SIZE: usize = 128;
/// Its camera: the view, how much of the head it shows top to bottom in metres, and how far under the top of the head its
/// middle is. The head is turned a little, as the game's own portraits are. **Assumed:** all four, by eye.
const FACE_PORTRAIT_FOV: f32 = 20.0;
const FACE_PORTRAIT_SPAN: f32 = 0.36;
const FACE_PORTRAIT_BELOW_TOP: f32 = 0.14;
const FACE_PORTRAIT_HEADING: f32 = -1.3;

/// How a camera puts a body in its hole, measured once a body so that its idling does not make the picture breathe.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::stage) struct FigureFit {
    /// The screen, the body and the window's shape it was measured for.
    key: (Screen, usize, u32),
    place: FigurePlace,
}

/// The tangent of half the view; how far the look is turned from the figure, at its depth; and the eye's height over the feet.
#[derive(Clone, Copy, Debug, PartialEq)]
struct FigurePlace {
    tan: f32,
    across: f32,
    down: f32,
    middle: f32,
}

fn norm(a: [f32; 3]) -> [f32; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-6);
    [a[0] / l, a[1] / l, a[2] / l]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Where a point falls on a window, from -1 to 1 each way.
fn on_window(camera: &Camera, aspect: f32, p: [f32; 3]) -> [f32; 2] {
    let (e, t) = (camera.eye, camera.target);
    let f = norm([t[0] - e[0], t[1] - e[1], t[2] - e[2]]);
    let r = norm([f[1], -f[0], 0.0]);
    let u = [
        r[1] * f[2] - r[2] * f[1],
        r[2] * f[0] - r[0] * f[2],
        r[0] * f[1] - r[1] * f[0],
    ];
    let d = [p[0] - e[0], p[1] - e[1], p[2] - e[2]];
    let tan = (camera.fov.to_radians() / 2.0).tan();
    let z = dot(f, d).max(1e-3);
    [dot(r, d) / z / (tan * aspect), dot(u, d) / z / tan]
}

/// The camera for a figure standing at `at`, facing `heading`.
fn camera_at(at: [f32; 3], heading: f32, place: FigurePlace) -> Camera {
    let (sin, cos) = heading.sin_cos();
    let middle = at[2] + place.middle;
    // The camera looks back along the way they face, so its right is the figure's left.
    let right = [-sin, cos];
    Camera {
        eye: [at[0] + cos * CAMERA_AWAY, at[1] + sin * CAMERA_AWAY, middle],
        target: [
            at[0] - right[0] * place.across,
            at[1] - right[1] * place.across,
            middle - place.down,
        ],
        fov: 2.0 * place.tan.atan().to_degrees(),
    }
}

/// A hole on the window, from -1 to 1 each way: left, right, bottom, top.
fn hole_on_window([left, top, width, height]: [f32; 4]) -> [f32; 4] {
    [
        left / 320.0 - 1.0,
        (left + width) / 320.0 - 1.0,
        1.0 - (top + height) / 240.0,
        1.0 - top / 240.0,
    ]
}

/// The square in the middle of a picture, as tall as it, made `size` across: each pixel the mean of those it covers.
fn middle_square(img: &kotor_image::Image, size: usize) -> Option<kotor_image::Image> {
    let (w, h) = (img.width as usize, img.height as usize);
    let side = w.min(h);
    if side == 0 || img.rgba.len() < w * h * 4 || size == 0 {
        return None;
    }
    let (left, top) = ((w - side) / 2, (h - side) / 2);
    let mut rgba = vec![0u8; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            let (x0, y0) = (x * side / size, y * side / size);
            let x1 = ((x + 1) * side / size).max(x0 + 1);
            let y1 = ((y + 1) * side / size).max(y0 + 1);
            let mut sum = [0u32; 3];
            for py in y0..y1 {
                for px in x0..x1 {
                    let at = ((top + py) * w + left + px) * 4;
                    for (s, v) in sum.iter_mut().zip(&img.rgba[at..at + 3]) {
                        *s += *v as u32;
                    }
                }
            }
            let count = ((x1 - x0) * (y1 - y0)) as u32;
            let at = (y * size + x) * 4;
            for k in 0..3 {
                rgba[at + k] = (sum[k] / count) as u8;
            }
            // A portrait is solid: the figure's background shows as it was drawn.
            rgba[at + 3] = 255;
        }
    }
    Some(kotor_image::Image {
        width: size as u32,
        height: size as u32,
        rgba,
    })
}

impl Stage {
    fn figure_hole(&self) -> Option<[f32; 4]> {
        match self.top()? {
            Screen::Character => Some(CHARACTER_HOLE),
            Screen::LevelUp => Some(LEVEL_HOLE),
            Screen::Party => self.party_figure_slot().map(|_| PARTY_HOLE),
            _ => None,
        }
    }

    /// The camera for a screen that shows a figure.
    pub fn figure_camera(&self) -> Option<Camera> {
        let hole = self.figure_hole()?;
        let w = &self.game.world;
        let at = w.object(w.pc)?.position;
        let top = self.top()?;
        let fitted = self
            .menus
            .figure_fit
            .filter(|f| f.key.0 == top && f.key.2 == self.ui_width);
        let place = fitted.map_or_else(|| self.standard_place(hole), |f| f.place);
        Some(camera_at(at, w.pc_mover.heading, place))
    }

    /// Where a point falls on the window, seen by a figure's camera, from -1 to 1 each way.
    pub fn figure_on_window(&self, camera: &Camera, p: [f32; 3]) -> [f32; 2] {
        on_window(camera, self.ui_width as f32 / 480.0, p)
    }

    /// Where a body of the usual height goes in a hole.
    fn standard_place(&self, hole: [f32; 4]) -> FigurePlace {
        let [left, right, bottom, top] = hole_on_window(hole);
        let fill = 1.0 - 2.0 * FIGURE_MARGIN;
        let tan = FIGURE_HEIGHT / (CAMERA_AWAY * (top - bottom) * fill);
        let aspect = self.ui_width as f32 / 480.0;
        FigurePlace {
            tan,
            // How far the middle of the hole is from the middle of the window, at that depth.
            across: (left + right) / 2.0 * CAMERA_AWAY * tan * aspect,
            down: (bottom + top) / 2.0 * CAMERA_AWAY * tan,
            middle: FIGURE_HEIGHT / 2.0,
        }
    }

    /// Measure a body that a figure screen has not had before, through the whole of its idle: all of it stands inside the
    /// hole, with air on each side.
    fn fit_figure(&mut self, rig: &Rc<Rig>, wear: &str) {
        let (Some(top), Some(hole)) = (self.top(), self.figure_hole()) else {
            return;
        };
        let key = (top, Rc::as_ptr(rig) as usize, self.ui_width);
        if self.menus.figure_fit.is_some_and(|f| f.key == key) {
            return;
        }
        let w = &self.game.world;
        let Some(at) = w.object(w.pc).map(|o| o.position) else {
            return;
        };
        let heading = w.pc_mover.heading;
        let mut scene = Scene::default();
        let idle = self.idling(0);
        for k in 0..IDLE_SAMPLES {
            let clip = idle.as_ref().map(|c| Clip {
                time: k as f32 * IDLE_SAMPLE_SECONDS,
                ..c.clone()
            });
            self.add_creature(&mut scene, rig, &[], &clip, at, heading, wear);
        }
        let points = || {
            scene
                .meshes
                .iter()
                .flat_map(|m| m.positions.iter().copied())
        };
        if points().next().is_none() {
            return;
        }
        let aspect = self.ui_width as f32 / 480.0;
        let [left, right, bottom, up] = hole_on_window(hole);
        let (wide, high) = (
            (right - left) * (1.0 - 2.0 * FIGURE_MARGIN),
            (up - bottom) * (1.0 - 2.0 * FIGURE_MARGIN),
        );
        let usual = self.standard_place(hole);
        let (low, tall) =
            points().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[2]), b.max(p[2])));
        let mut place = FigurePlace {
            middle: (low + tall) / 2.0 - at[2],
            ..usual
        };
        for _ in 0..FIT_STEPS {
            let camera = camera_at(at, heading, place);
            let mut seen = [f32::MAX, f32::MIN, f32::MAX, f32::MIN];
            for p in points() {
                let [x, y] = on_window(&camera, aspect, p);
                seen = [
                    seen[0].min(x),
                    seen[1].max(x),
                    seen[2].min(y),
                    seen[3].max(y),
                ];
            }
            // Larger or smaller, but never past the usual scale for a short body.
            let grow = ((seen[1] - seen[0]) / wide).max((seen[3] - seen[2]) / high);
            let tan = (place.tan * grow).max(usual.tan);
            let shrunk = place.tan / tan;
            let off = [
                (left + right) / 2.0 - (seen[0] + seen[1]) / 2.0 * shrunk,
                (bottom + up) / 2.0 - (seen[2] + seen[3]) / 2.0 * shrunk,
            ];
            place.across = place.across * tan / place.tan + off[0] * CAMERA_AWAY * tan * aspect;
            place.down = place.down * tan / place.tan + off[1] * CAMERA_AWAY * tan;
            place.tan = tan;
        }
        self.menus.figure_fit = Some(FigureFit { key, place });
    }

    /// The alignment of whoever a figure screen shows.
    fn figure_alignment(&mut self) -> i32 {
        let w = &self.game.world;
        let pc = w.object(w.pc).map_or(ALIGNMENT_NEUTRAL, |o| o.alignment);
        let slot = match self.top() {
            Some(Screen::Party) => self.party_figure_slot(),
            _ => None,
        };
        let Some(slot) = slot else { return pc };
        let w = &self.game.world;
        let here = w.npc_slots.get(&slot).and_then(|id| w.object(*id));
        if let Some(o) = here {
            return o.alignment;
        }
        if let Some(b) = w.bench.get(&slot) {
            return b.alignment;
        }
        let Some(template) = w.available.get(&slot).cloned() else {
            return ALIGNMENT_NEUTRAL;
        };
        self.inst
            .read(&template, CREATURE_FILE)
            .ok()
            .and_then(|bytes| Gff::parse(&bytes).ok())
            .and_then(|file| file.root.int("GoodEvil"))
            .map_or(ALIGNMENT_NEUTRAL, |n| n as i32)
    }

    /// The mist goes on while a figure is up, and starts over for another screen or alignment.
    pub(in crate::stage) fn figure_mist_tick(&mut self, dt: f32) {
        let (Some(top), Some(camera)) = (self.top(), self.figure_camera()) else {
            self.menus.figure_mist = None;
            return;
        };
        let alignment = self.figure_alignment();
        let same = |m: &(Screen, i32, Effects)| m.0 == top && m.1 == alignment;
        if !self.menus.figure_mist.as_ref().is_some_and(same) {
            let name = if top == Screen::LevelUp {
                LEVEL_MIST
            } else {
                SHEET_MIST
            };
            let mut mist = Effects::default();
            let w = &self.game.world;
            let at = w.object(w.pc).map_or([0.0; 3], |o| o.position);
            let (sin, cos) = w.pc_mover.heading.sin_cos();
            if let Some(model) = self.model(name) {
                // The model's camera looks along -x, and this one looks back along the way the figure faces.
                mist.load_model(
                    &model,
                    &[
                        [cos, -sin, 0.0, at[0]],
                        [sin, cos, 0.0, at[1]],
                        [0.0, 0.0, 1.0, at[2] + MIST_UP],
                        [0.0, 0.0, 0.0, 1.0],
                    ],
                );
            }
            let span = (MIST_BLUE_FROM - MIST_RED_FROM) as f32;
            mist.redden(((MIST_BLUE_FROM - alignment) as f32 / span).clamp(0.0, 1.0));
            for _ in 0..(MIST_START / MIST_STEP) as usize {
                mist.update(MIST_STEP, camera.eye);
            }
            self.menus.figure_mist = Some((top, alignment, mist));
        }
        if let Some((_, _, mist)) = self.menus.figure_mist.as_mut() {
            mist.update(dt, camera.eye);
        }
    }

    /// Who a party slot offers, from the creature file: what they wear is what they were left with, or else the file's.
    fn benched_rig(&mut self, slot: i32) -> Option<Rc<Rig>> {
        let w = &self.game.world;
        let template = w.available.get(&slot)?.clone();
        let kept = w.bench.get(&slot).map(|b| {
            let worn = b.worn.iter().find(|(s, _, _)| *s == SLOT_BODY);
            worn.map(|(_, t, _)| t.clone())
        });
        let file = Gff::parse(&self.inst.read(&template, CREATURE_FILE).ok()?).ok()?;
        let appearance = file.root.int("Appearance_Type")? as i32;
        let from_file = || {
            let items = file.root.list("Equip_ItemList").unwrap_or_default();
            let armour = items.iter().find(|e| e.id == 1 << SLOT_BODY)?;
            armour.str("EquippedRes").map(str::to_string)
        };
        let armour = kept.unwrap_or_else(from_file).and_then(|item| {
            let file = Gff::parse(&self.inst.read(&item, ITEM_FILE).ok()?).ok()?;
            Some((file.root.int("BaseItem")? as i32, item))
        });
        self.rig_for(appearance, &template, armour)
    }

    /// Whoever the party screen has chosen, standing where the camera looks.
    fn party_figure(&mut self, slot: i32) -> Scene {
        let mut scene = Scene::default();
        let w = &self.game.world;
        let Some(at) = w.object(w.pc).map(|o| o.position) else {
            return scene;
        };
        let heading = w.pc_mover.heading;
        let here = w
            .npc_slots
            .get(&slot)
            .copied()
            .filter(|id| w.object(*id).is_some());
        let rig = match here {
            Some(id) => self.rig_of(id),
            None => self.benched_rig(slot),
        };
        if let Some(rig) = rig {
            let clip = self.idling(0);
            let wear = here.map(|id| self.dress_of(id)).unwrap_or_default();
            self.add_creature(&mut scene, &rig, &[], &clip, at, heading, &wear);
            self.fit_figure(&rig, &wear);
        }
        scene
    }

    /// The name of a portrait made from the player's face, when the face is not the game's own: its sliders or marks
    /// moved. The name changes with the face, so a changed face is drawn again.
    fn face_portrait_name(&self) -> Option<String> {
        use std::hash::{Hash, Hasher};
        let w = &self.game.world;
        let o = w.object(w.pc)?;
        let keys = kotor_render::face::FACE_SLIDERS
            .iter()
            .copied()
            .chain(kotor_render::marks::MARKS.iter().map(|m| m.0))
            .chain([kotor_render::marks::HAIR_KEY]);
        let shaped: Vec<(&str, i32)> = keys
            .filter_map(|k| o.look.sliders.get(k).map(|v| (k, *v)))
            .filter(|(_, v)| *v != 0)
            .collect();
        if shaped.is_empty() {
            return None;
        }
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (o.appearance, &o.portrait, &shaped).hash(&mut h);
        Some(format!("{FACE_PORTRAIT_PREFIX}{:016x}", h.finish()))
    }

    /// The player's picture: the one made from their face once there is one, else the game's.
    pub fn pc_portrait(&self) -> String {
        if let Some(name) = self
            .face_portrait_name()
            .filter(|n| self.face_portraits.contains(n))
        {
            return name;
        }
        let w = &self.game.world;
        w.object(w.pc)
            .map(|o| o.portrait.clone())
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| "po_pfha2".to_string())
    }

    /// For the window: the player's head to draw for a portrait made from their face, the name it will go by, and the
    /// camera. Each face is asked for once: None when it is made or asked for already, or the face is the game's own.
    /// The window draws it with only the figure, and gives the picture to `set_face_portrait`.
    pub fn face_portrait_wanted(&mut self) -> Option<(String, Scene, Camera)> {
        let name = self.face_portrait_name()?;
        if self.face_portraits.contains(&name) || self.face_portrait_asked.as_deref() == Some(&name)
        {
            return None;
        }
        let pc = self.game.world.pc;
        let rig = self.rig_of(pc)?;
        self.face_portrait_asked = Some(name.clone());
        let mut scene = Scene::default();
        let clip = self.idling(0);
        let wear = self.dress_of(pc);
        self.add_creature(
            &mut scene,
            &rig,
            &[],
            &clip,
            [0.0; 3],
            FACE_PORTRAIT_HEADING,
            &wear,
        );
        let points = || scene.meshes.iter().flat_map(|m| m.positions.iter());
        let top = points().fold(f32::MIN, |top, p| top.max(p[2]));
        // The head's middle across, from what is near the top: a body may not stand on its own middle.
        let head = points().filter(|p| p[2] > top - FACE_PORTRAIT_SPAN / 2.0);
        let (low_x, high_x) =
            head.fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[0]), b.max(p[0])));
        if low_x > high_x {
            return None;
        }
        let across = (low_x + high_x) / 2.0;
        let middle = top - FACE_PORTRAIT_BELOW_TOP;
        let depth = FACE_PORTRAIT_SPAN / 2.0 / (FACE_PORTRAIT_FOV.to_radians() / 2.0).tan();
        let camera = Camera {
            eye: [across, -depth, middle],
            target: [across, 0.0, middle],
            fov: FACE_PORTRAIT_FOV,
        };
        Some((name, scene, camera))
    }

    /// The window drew the head `face_portrait_wanted` asked for: the middle square of its picture is the portrait.
    pub fn set_face_portrait(&mut self, name: &str, drawn: &kotor_image::Image) {
        let Some(square) = middle_square(drawn, FACE_PORTRAIT_SIZE) else {
            return;
        };
        self.ui.set_texture(name, square);
        self.face_portraits.insert(name.to_string());
        self.overlay_key.clear();
        self.refresh_overlay();
    }

    /// The leader alone, or whoever the party screen has chosen. They stand idle with empty hands, as the video has it at
    /// 1:23:50 with a sword in the slot, whatever they were doing when the screen opened.
    pub fn figure_scene(&mut self) -> Scene {
        if self.top() == Some(Screen::Party) {
            if let Some(slot) = self.party_figure_slot() {
                return self.party_figure(slot);
            }
        }
        let mut scene = Scene::default();
        let pc = self.game.world.pc;
        let Some(pose) = self.game.poses().into_iter().find(|p| p.id == pc) else {
            return scene;
        };
        let Some(rig) = self.rig_of(pc) else {
            return scene;
        };
        let clip = self.idling(0);
        let wear = self.dress_of(pc);
        self.add_creature(
            &mut scene,
            &rig,
            &[],
            &clip,
            pose.position,
            pose.heading,
            &wear,
        );
        self.fit_figure(&rig, &wear);
        scene
    }

    /// The particles that go with `frame_scene`.
    pub fn frame_particles(&mut self) -> Vec<crate::effects::ParticleBatch> {
        if let Some(fog) = self.main_scene_particles() {
            return fog;
        }
        if let Some(mist) = self.chargen_particles() {
            return mist;
        }
        if let Some(galaxy) = self.galaxy_particles() {
            return galaxy;
        }
        if let Some(camera) = self.figure_camera() {
            return self
                .menus
                .figure_mist
                .as_ref()
                .map_or(Vec::new(), |m| m.2.batches(&camera));
        }
        if !self.in_game() {
            return Vec::new();
        }
        self.particles()
    }

    /// What a window should draw now, and with what camera; the last says that only this is drawn, for a figure.
    pub fn frame_scene(&mut self) -> (Scene, Camera, bool) {
        if let Some((scene, camera)) = self.main_scene_now() {
            return (scene, camera, true);
        }
        if let Some((scene, camera)) = self.race_scene() {
            return (scene, camera, false);
        }
        if let Some((scene, camera)) = self
            .chargen_scene()
            .or_else(|| self.bench_scene())
            .or_else(|| self.galaxy_scene())
        {
            return (scene, camera, true);
        }
        if self.figure_hole().is_some() {
            // The scene first: the camera is made to fit what is in it.
            let scene = self.figure_scene();
            if let Some(camera) = self.figure_camera() {
                return (scene, camera, true);
            }
        }
        (self.dynamic_scene(), self.camera(), false)
    }
}
