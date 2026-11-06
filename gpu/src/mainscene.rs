//! The main menu's scene, as the reference video has it at 0:21: `mainmenu.mdl` holds Malak, the fog, the back wall with
//! `loadscreen3` on it, and a `camerahook`. The screen leaves a hole for it. **Assumed:** the camera looks at the middle of
//! that wall and just fits it. In a wider window the video stretches the wall with the screen but not Malak or the fog, and
//! has him further left than a centred picture would, so `WIDE_SHIFT` moves them. Its lights are not used, since the
//! renderer has its own, and the video's scene is far darker, so `VEIL` dims it: a guess by eye.

use super::{Screen, Stage};
use crate::effects::{Effects, ParticleBatch};
use kotor_model::Model;
use kotor_render::{Camera, Rig, Scene};
use std::rc::Rc;

const MODEL: &str = "mainmenu";
const CAMERA_NODE: &str = "camerahook";
const WALL_NODE: &str = "Plane01";
const WALL_PICTURE: &str = "loadscreen3";
const ANIMATION: &str = "default";
/// Half of the wall's height, from its mesh.
const WALL_HALF_HEIGHT: f32 = 2.4;
/// The shape the scene was made for, as wide as the layout it lies behind.
const MADE_WIDTH: f32 = 640.0;
/// How far Malak and the fog move to the left in a wider window, in metres for each time as wide. From the video at 0:21.
const WIDE_SHIFT: f32 = 0.52;
/// How much black lies over the scene.
pub(super) const VEIL: u8 = 120;

pub(super) struct MainScene {
    rig: Rc<Rig>,
    model: Rc<Model>,
    time: f32,
    effects: Effects,
    camera: Camera,
    /// The window's width against what the scene was made for.
    wide: f32,
}

/// Across only, about the middle of the view.
fn stretched(wide: f32) -> [[f32; 4]; 4] {
    placed(wide, 0.0)
}

fn placed(across: f32, shift: f32) -> [[f32; 4]; 4] {
    [
        [across, 0.0, 0.0, shift],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// Malak and the fog keep their shape and move over.
fn in_front(wide: f32) -> [[f32; 4]; 4] {
    placed(1.0, -WIDE_SHIFT * (wide - 1.0))
}

impl Stage {
    fn load_main_scene(&mut self) -> Option<MainScene> {
        let model = self.model(MODEL)?;
        let rig = Rig::load(&mut self.inst, MODEL, None, None).ok()?;
        let mats = model.world_matrices(None);
        let place = |name: &str| {
            let (_, m) = mats.iter().find(|(n, _)| n.eq_ignore_ascii_case(name))?;
            Some([m[0][3], m[1][3], m[2][3]])
        };
        let (eye, target) = (place(CAMERA_NODE)?, place(WALL_NODE)?);
        let away = (0..3)
            .map(|k| (target[k] - eye[k]).powi(2))
            .sum::<f32>()
            .sqrt();
        let camera = Camera {
            eye,
            target,
            fov: 2.0 * (WALL_HALF_HEIGHT / away).atan().to_degrees(),
        };
        Some(MainScene {
            rig: Rc::new(rig),
            model,
            time: 0.0,
            effects: Effects::default(),
            camera,
            wide: 0.0,
        })
    }

    /// The scene is there to be shown: it is loaded the first time.
    pub(super) fn main_scene_ready(&mut self) -> bool {
        if self.menus.main_scene.is_none() && !self.menus.main_scene_failed {
            self.menus.main_scene = self.load_main_scene();
            self.menus.main_scene_failed = self.menus.main_scene.is_none();
        }
        self.menus.main_scene.is_some()
    }

    /// Time goes on while it is up.
    pub(in crate::stage) fn main_scene_tick(&mut self, dt: f32) {
        if self.top() != Some(Screen::Main) || !self.main_scene_ready() {
            return;
        }
        let wide = self.ui_width as f32 / MADE_WIDTH;
        let Some(s) = self.menus.main_scene.as_mut() else {
            return;
        };
        if s.wide != wide {
            s.wide = wide;
            s.effects = Effects::default();
            s.effects.load_model(&s.model, &in_front(wide));
        }
        s.time += dt;
        s.effects.update(dt, s.camera.eye);
    }

    /// Malak and the room at this moment, and the camera, when the main menu is up.
    pub(super) fn main_scene_now(&mut self) -> Option<(Scene, Camera)> {
        if self.top() != Some(Screen::Main) || !self.main_scene_ready() {
            return None;
        }
        let wide = self.ui_width as f32 / MADE_WIDTH;
        let s = self.menus.main_scene.as_ref()?;
        let pose = s.model.animation(ANIMATION).map(|a| {
            let t = if a.length > 0.0 {
                s.time % a.length
            } else {
                0.0
            };
            a.sample(&s.model, t)
        });
        let body = s.rig.posed(pose.as_ref()).ok()?;
        // The wall is the one mesh that names its picture so: everything else is in front of it.
        let (mut wall, mut front) = (Scene::default(), Scene::default());
        for mesh in body.meshes {
            let on_wall = mesh
                .texture
                .as_deref()
                .is_some_and(|t| t.eq_ignore_ascii_case(WALL_PICTURE));
            let part = if on_wall { &mut wall } else { &mut front };
            part.meshes.push(mesh);
        }
        let mut scene = Scene::default();
        scene.append_transformed(&wall, &stretched(wide));
        scene.append_transformed(&front, &in_front(wide));
        let camera = s.camera;
        Some((scene, camera))
    }

    /// The fog, when the main menu is up.
    pub(super) fn main_scene_particles(&mut self) -> Option<Vec<ParticleBatch>> {
        let (_, camera) = self.main_scene_now()?;
        let s = self.menus.main_scene.as_ref()?;
        Some(s.effects.batches(&camera))
    }
}
