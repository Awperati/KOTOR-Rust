//! The galaxy map: `galaxymap.gui`, as the reference video has it at 8:29:45. The picture of the galaxy is the emitters of
//! `galaxy.mdl`, seen from its `camerahook`, and the planet beside the words is the row's model, which zooms in and then
//! turns. The screen leaves a hole for each, and both are drawn with one camera, which looks past them level.
//!
//! **Measured from the video:** the stars of `galaxy.mdl` lie 533 layout units from the middle for each unit of distance
//! they are off the line of sight, so that is the camera's focal length. **Assumed:** a planet's model has the same.
//! The rules are not built, so `open_galaxy` is told what to show and `take_galaxy_choice` says what was chosen.

use super::{Screen, Stage};
use crate::effects::{Effects, ParticleBatch};
use kotor_model::Model;
use kotor_render::{Camera, Rig, Scene};
use kotor_ui::MenuView;
use std::rc::Rc;

const GUI: &str = "galaxymap";
const TABLE: &str = "planetary";
const TABLE_FILE: kotor_core::ResType = kotor_core::ResType(2017);
const GALAXY_MODEL: &str = "galaxy";
const GALAXY_HOLE: &str = "3D_PlanetDisplay";
const PLANET_HOLE: &str = "3D_PlanetModel";
const CAMERA_NODE: &str = "camerahook";
const ZOOM: &str = "zoomin";
const TURN: &str = "rotate";
/// The middles of the holes, from the file, in layout units.
const GALAXY_AT: (f32, f32) = (320.0, 149.0);
const PLANET_AT: (f32, f32) = (491.5, 333.5);
/// The foot of the galaxy's hole: nothing of the galaxy is wanted below it, where the planet's hole is.
const GALAXY_FOOT: f32 = 256.0;
const FOCAL: f32 = 533.0;
/// How far the sun of `galaxy.mdl` lies from its origin toward its camera.
const SUN_AT: f32 = 11.2;
/// How near a planet's camera comes: a radius of about 107 layout units, as the video has it, which runs past its hole.
const NEAREST: f32 = 2.7;
/// The top of the planet's hole. The other edges are the screen's picture, but this one opens onto the galaxy's hole.
const PLANET_TOP: f32 = 261.0;
/// Corners of a particle's two triangles.
const QUAD_VERTICES: usize = 6;
/// Seconds of the galaxy made at once when the screen opens, so that it is there already.
const START_SECONDS: f32 = 6.0;
const STEP_SECONDS: f32 = 1.0 / 30.0;
const NOT_A_PLANET: [&str; 2] = ["", "****"];

/// A row of `planetary.2da`, and what the story allows of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanetLine {
    pub row: i32,
    /// It is on the map.
    pub available: bool,
    /// It can be travelled to.
    pub selectable: bool,
}

/// A row as the table has it.
#[derive(Clone)]
struct PlanetFile {
    tag: String,
    picture: String,
    model: String,
    name: u32,
    words: u32,
}

#[derive(Default)]
pub(super) struct Galaxy {
    current: i32,
    shown: i32,
    lines: Vec<PlanetLine>,
    files: Vec<PlanetFile>,
    effects: Effects,
    camera: Option<Camera>,
    planet: Option<(Rc<Rig>, Rc<Model>)>,
    /// How far along its model the planet on show is, in seconds.
    time: f32,
    /// How far off the line of sight a planet's model has its own camera, and how much farther than the galaxy's.
    planet_scale: f32,
    chosen: Option<Option<i32>>,
}

fn table_int(t: &kotor_tables::TwoDa, row: usize, column: &str) -> u32 {
    t.int(row, column).unwrap_or(0).max(0) as u32
}

impl Stage {
    fn camera_away(&mut self, model: &str) -> Option<f32> {
        let m = self.model(model)?;
        let mats = m.world_matrices(None);
        let (_, at) = mats
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(CAMERA_NODE))?;
        Some(at[0][3])
    }

    /// The map comes up, on `current`. What each planet may do comes from the story.
    pub fn open_galaxy(&mut self, current: i32, lines: Vec<PlanetLine>) {
        let mut g = Galaxy {
            current,
            shown: current,
            lines,
            ..Galaxy::default()
        };
        let table = self
            .inst
            .read(TABLE, TABLE_FILE)
            .ok()
            .and_then(|bytes| kotor_tables::TwoDa::parse(&bytes).ok());
        if let Some(t) = &table {
            g.files = (0..t.row_count())
                .map(|row| PlanetFile {
                    tag: t.get(row, "guitag").unwrap_or("").to_string(),
                    picture: t.get(row, "icon").unwrap_or("").to_ascii_lowercase(),
                    model: t.get(row, "model").unwrap_or("").to_ascii_lowercase(),
                    name: table_int(t, row, "name"),
                    words: table_int(t, row, "description"),
                })
                .collect();
        }
        if let (Some(away), Some(model)) =
            (self.camera_away(GALAXY_MODEL), self.model(GALAXY_MODEL))
        {
            g.effects.load_model(
                &model,
                &[
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ],
            );
            g.effects.lay_flat();
            // Looking level from below the middle of the window leaves the sun in the middle of its hole.
            let down = (240.0 - GALAXY_AT.1) / FOCAL * (away - SUN_AT);
            g.camera = Some(Camera {
                eye: [away, 0.0, -down],
                target: [0.0, 0.0, -down],
                fov: 2.0 * (240.0 / FOCAL).atan().to_degrees(),
            });
            for _ in 0..(START_SECONDS / STEP_SECONDS) as usize {
                g.effects.update(STEP_SECONDS, [away, 0.0, 0.0]);
            }
        }
        self.menus.galaxy = Some(g);
        self.menus.hover = None;
        self.menus.stack.push(Screen::Galaxy);
        self.show_planet(current);
    }

    fn show_planet(&mut self, row: i32) {
        let name = self.menus.galaxy.as_ref().and_then(|g| {
            let file = g.files.get(row.max(0) as usize)?;
            (!NOT_A_PLANET.contains(&file.model.as_str())).then(|| file.model.clone())
        });
        let planet = name.as_ref().and_then(|n| {
            let model = self.model(n)?;
            let rig = Rig::load(&mut self.inst, n, None, None).ok()?;
            Some((Rc::new(rig), model))
        });
        let away = name.and_then(|n| self.camera_away(&n));
        let galaxy_away = self
            .menus
            .galaxy
            .as_ref()
            .and_then(|g| g.camera)
            .map(|c| c.eye[0]);
        if let Some(g) = self.menus.galaxy.as_mut() {
            g.shown = row;
            g.planet = planet;
            g.time = 0.0;
            g.planet_scale = match (galaxy_away, away) {
                (Some(a), Some(b)) if b > 0.0 => a / b,
                _ => 1.0,
            };
        }
    }

    /// What the map's user chose, once: a planet, or nothing for Cancel. `None` while nothing has been said.
    pub fn take_galaxy_choice(&mut self) -> Option<Option<i32>> {
        self.menus.galaxy.as_mut()?.chosen.take()
    }

    /// The planet on show.
    pub fn galaxy_shown(&self) -> Option<i32> {
        self.menus.galaxy.as_ref().map(|g| g.shown)
    }

    pub(super) fn galaxy_view(&mut self) -> MenuView {
        let mut v = MenuView::new(GUI);
        v.holes.push(GALAXY_HOLE.to_string());
        v.holes.push(PLANET_HOLE.to_string());
        v.hole_shades.push(PLANET_HOLE.to_string());
        let Some(g) = self.menus.galaxy.as_ref() else {
            return v;
        };
        let (shown, current) = (g.shown, g.current);
        let files = g.files.clone();
        let lines = g.lines.clone();
        for file in files
            .iter()
            .filter(|f| !f.tag.is_empty() && f.tag != "****")
        {
            v.hide(&[file.tag.as_str()]);
        }
        for line in &lines {
            let Some(file) = files.get(line.row.max(0) as usize) else {
                continue;
            };
            if !line.available || file.tag.is_empty() {
                continue;
            }
            let patch = v.patches.entry(file.tag.clone()).or_default();
            patch.hidden = false;
            patch.fill = Some(file.picture.clone());
        }
        let file = files.get(shown.max(0) as usize);
        let name = file.map_or(String::new(), |f| self.ui.strref(f.name));
        let words = file.map_or(String::new(), |f| self.ui.strref(f.words));
        v.text("LBL_PLANETNAME", name);
        v.text("LBL_DESC", words);
        let can_go = lines.iter().any(|l| l.row == shown && l.selectable) && shown != current;
        if !can_go {
            v.inert(&["BTN_ACCEPT"]);
        }
        v
    }

    fn close_galaxy(&mut self, chosen: Option<i32>) {
        if let Some(g) = self.menus.galaxy.as_mut() {
            g.chosen = Some(chosen);
        }
        self.menus.stack.pop();
        self.menus.hover = None;
    }

    pub(super) fn galaxy_press(&mut self, tag: &str) {
        match tag {
            "BTN_BACK" => self.close_galaxy(None),
            "BTN_ACCEPT" => {
                let chosen = self.menus.galaxy.as_ref().and_then(|g| {
                    let line = g.lines.iter().find(|l| l.row == g.shown)?;
                    (line.selectable && g.shown != g.current).then_some(g.shown)
                });
                if chosen.is_some() {
                    self.close_galaxy(chosen);
                }
            }
            _ => {
                let row = self.menus.galaxy.as_ref().and_then(|g| {
                    let row = g.files.iter().position(|f| f.tag == tag)? as i32;
                    let line = g.lines.iter().find(|l| l.row == row)?;
                    line.available.then_some(row)
                });
                if let Some(row) = row {
                    self.show_planet(row);
                }
            }
        }
    }

    /// Esc is Cancel.
    pub(super) fn galaxy_escape(&mut self) {
        self.close_galaxy(None);
    }

    /// Time goes on while it is up.
    pub(in crate::stage) fn galaxy_tick(&mut self, dt: f32) {
        if self.top() != Some(Screen::Galaxy) {
            return;
        }
        if let Some(g) = self.menus.galaxy.as_mut() {
            g.time += dt;
            if let Some(eye) = g.camera.map(|c| c.eye) {
                g.effects.update(dt, eye);
            }
        }
    }

    /// The planet on show, placed in its hole, and the camera.
    pub(super) fn galaxy_scene(&mut self) -> Option<(Scene, Camera)> {
        if self.top() != Some(Screen::Galaxy) {
            return None;
        }
        let across = self.ui_width as f32 / 640.0;
        let g = self.menus.galaxy.as_ref()?;
        let camera = g.camera?;
        let mut scene = Scene::default();
        if let Some((rig, model)) = &g.planet {
            // It zooms in and stays there, and turns.
            let zoom = model.animation(ZOOM).map(|a| a.sample(model, g.time));
            let turn = model
                .animation(TURN)
                .map(|a| a.sample(model, g.time % a.length.max(0.001)));
            let pose = match (zoom, turn) {
                (Some(zoom), Some(mut turn)) => {
                    for (node, place) in zoom {
                        turn.entry(node)
                            .and_modify(|p| p.position = place.position)
                            .or_insert(place);
                    }
                    Some(turn)
                }
                (zoom, turn) => zoom.or(turn),
            };
            if let Ok(body) = rig.posed(pose.as_ref()) {
                // The zoom is its camera coming in, so the planet grows as that nears: the scale keeps the view's shape.
                let hook = model
                    .animation(ZOOM)
                    .map(|a| a.sample(model, g.time))
                    .and_then(|p| p.get(CAMERA_NODE).map(|n| n.position[0]))
                    .map(|x| x.max(NEAREST));
                let s = hook.map_or(g.planet_scale, |x| camera.eye[0] / x);
                let away = camera.eye[0];
                let y = (PLANET_AT.0 - 320.0) * across / FOCAL * away;
                let z = camera.eye[2] - (PLANET_AT.1 - 240.0) / FOCAL * away;
                scene.append_transformed(
                    &body,
                    &[
                        [s, 0.0, 0.0, 0.0],
                        [0.0, s, 0.0, y],
                        [0.0, 0.0, s, z],
                        [0.0, 0.0, 0.0, 1.0],
                    ],
                );
                // One camera draws both holes, so what would rise past its own is laid along that edge.
                for mesh in &mut scene.meshes {
                    for p in &mut mesh.positions {
                        let top =
                            camera.eye[2] - (PLANET_TOP - 240.0) / FOCAL * (camera.eye[0] - p[0]);
                        p[2] = p[2].min(top);
                    }
                }
            }
        }
        Some((scene, camera))
    }

    /// The galaxy, when the map is up.
    pub(super) fn galaxy_particles(&mut self) -> Option<Vec<ParticleBatch>> {
        if self.top() != Some(Screen::Galaxy) {
            return None;
        }
        let g = self.menus.galaxy.as_ref()?;
        let camera = g.camera?;
        let mut batches = g.effects.batches(&camera);
        // What would lie below its hole would show in the planet's.
        for b in &mut batches {
            let quads: Vec<_> = b
                .vertices
                .chunks(QUAD_VERTICES)
                .filter(|q| {
                    let middle = [0, 2].map(|k| (q[0].pos[k] + q[2].pos[k]) / 2.0);
                    let away = camera.eye[0] - middle[0];
                    middle[1] > camera.eye[2] - (GALAXY_FOOT - 240.0) / FOCAL * away
                })
                .flatten()
                .copied()
                .collect();
            b.vertices = quads;
        }
        Some(batches)
    }
}
