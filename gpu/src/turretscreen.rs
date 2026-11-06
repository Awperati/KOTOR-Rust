//! The Ebon Hawk's turret on the screen, as the reference video has it at 19,530 s: no HUD, the cockpit turning with the view,
//! the fighters and their shots, and on the cockpit's own models a radar and a health dial. The rules are
//! `kotor_game::minigame`. The mouse or the movement keys turn it, the left button or Space fires, as `keymap.2da`'s `MGshoot`.
//! The sky is the area's room, drawn each frame in the `scriptloop` its script has set going, which turns it and shows the
//! planet of the place. **Assumed:** how far a pixel of the mouse turns it, by feel; the room's own name, `<area>_01a`, until a
//! script names it. **Not built:** the room's emitters, which would not turn with it, and the lights in the models.

use super::Stage;
use kotor_model::{Model, Pose};
use kotor_render::{Camera, Scene};
use std::collections::HashSet;
use std::rc::Rc;

/// Degrees a pixel of the mouse turns the turret at the middle of the sensitivity slider.
const TURN_PER_PIXEL: f32 = 0.2;
/// What the slider's middle is.
const SLIDER_MIDDLE: f32 = 0.5;
/// The controller of a mesh's alpha, as `Mesh07` of `mgf_hud02` has it.
const ALPHA_CONTROLLER: u32 = 132;
const GONE: f32 = 0.01;
const ROOM_ANIMATION: &str = "scriptloop";
const ROOM_SUFFIX: &str = "_01a";
const FIRST_ROOM_ANIMATION: i32 = 4;
/// How much light the ship gets, by eye from the video at 19,533 s: the cockpit is far darker than the textures.
const COCKPIT_SHADE: f32 = 0.42;
/// The models that are lit like that, and not the dials on them.
const SHADED: [&str; 2] = ["mgf_turret", "mgf_ebonhawk"];

type Mat = [[f32; 4]; 4];

fn multiply(a: &Mat, b: &Mat) -> Mat {
    let mut out = [[0.0; 4]; 4];
    for (r, row) in out.iter_mut().enumerate() {
        for (c, v) in row.iter_mut().enumerate() {
            *v = (0..4).map(|k| a[r][k] * b[k][c]).sum();
        }
    }
    out
}

fn at(p: [f32; 3]) -> Mat {
    [
        [1.0, 0.0, 0.0, p[0]],
        [0.0, 1.0, 0.0, p[1]],
        [0.0, 0.0, 1.0, p[2]],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// A shot's matrix: its +y along the way it goes.
fn flying(p: [f32; 3], way: [f32; 3]) -> Mat {
    let len = (way[0] * way[0] + way[1] * way[1] + way[2] * way[2])
        .sqrt()
        .max(1e-6);
    let y = [way[0] / len, way[1] / len, way[2] / len];
    let flat = (y[0] * y[0] + y[1] * y[1]).sqrt().max(1e-6);
    let x = [y[1] / flat, -y[0] / flat, 0.0];
    let z = [
        x[1] * y[2] - x[2] * y[1],
        x[2] * y[0] - x[0] * y[2],
        x[0] * y[1] - x[1] * y[0],
    ];
    [
        [x[0], y[0], z[0], p[0]],
        [x[1], y[1], z[1], p[1]],
        [x[2], y[2], z[2], p[2]],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// A value of a controller at a time: the last key at or before it.
fn key_at(c: &kotor_model::Controller, t: f32) -> Option<f32> {
    let n = c.times.iter().rposition(|k| *k <= t).unwrap_or(0);
    c.values.get(n * c.columns.max(1) as usize).copied()
}

/// How an animation stands at a time.
fn played_time(a: &kotor_game::PlayedAnimation, length: f32) -> f32 {
    if a.looping && length > 0.0 {
        a.time % length
    } else {
        a.time.min(length)
    }
}

/// The pose the animations give a model, and the meshes they, or the model, leave unseen.
fn posed(model: &Model, animations: &[kotor_game::PlayedAnimation]) -> (Pose, Vec<String>) {
    let mut pose = Pose::new();
    let mut seen = std::collections::HashMap::new();
    model.root.walk(&mut |n| {
        if let Some(c) = n.controllers.iter().find(|c| c.kind == ALPHA_CONTROLLER) {
            seen.insert(
                n.name.to_ascii_lowercase(),
                c.values.first().copied().unwrap_or(1.0),
            );
        }
    });
    for played in animations {
        let Some(a) = model.animation(&played.name) else {
            continue;
        };
        let t = played_time(played, a.length);
        pose.extend(a.sample(model, t));
        a.root.walk(&mut |n| {
            if let Some(v) = n
                .controllers
                .iter()
                .find(|c| c.kind == ALPHA_CONTROLLER)
                .and_then(|c| key_at(c, t))
            {
                seen.insert(n.name.to_ascii_lowercase(), v);
            }
        });
    }
    let hidden = seen
        .into_iter()
        .filter(|(_, v)| *v < GONE)
        .map(|(n, _)| n)
        .collect();
    (pose, hidden)
}

impl Stage {
    /// Whether the turret has the input.
    pub fn turret_on(&self) -> bool {
        self.game.world.minigame.is_some() && !self.menu_open() && self.choice_view().is_none()
    }

    /// The mouse has moved by this many pixels.
    pub fn turret_look(&mut self, dx: f32, dy: f32) {
        let flip = if self.settings().invert_camera {
            -1.0
        } else {
            1.0
        };
        let k = TURN_PER_PIXEL * self.settings().mouse_sensitivity / SLIDER_MIDDLE;
        if let Some(g) = self.game.world.minigame.as_mut() {
            g.turn_by(-dy * k * flip, -dx * k);
        }
    }

    /// The fire button, down or up. A tap still fires.
    pub fn turret_fire(&mut self, on: bool) {
        if let Some(g) = self.game.world.minigame.as_mut() {
            g.firing = on;
            g.fire_once |= on;
        }
    }

    /// The camera, while the game is on.
    pub(in crate::stage) fn turret_camera(&self) -> Option<Camera> {
        let g = self.game.world.minigame.as_ref()?;
        let (eye, target) = g.camera();
        Some(Camera {
            eye,
            target,
            fov: g.view_angle,
        })
    }

    /// The keys turn it, and what has happened sounds and bursts.
    pub(in crate::stage) fn turret_tick(&mut self, dt: f32) {
        let (ahead, right, _) = self.steer;
        let assist = self.turret_assist;
        let Some(g) = self.game.world.minigame.as_mut() else {
            return;
        };
        let turn = g.turn_per_second * dt;
        g.turn_by(ahead * turn, -right * turn);
        if assist {
            let first = g
                .enemies
                .iter()
                .find(|e| e.dying.is_none() && !e.gone)
                .map(|e| e.at);
            if let Some(first) = first {
                g.point_at(first);
            }
        }
        let sounds = std::mem::take(&mut g.sounds);
        let deaths = std::mem::take(&mut g.deaths);
        let dying: Vec<([f32; 3], String)> = deaths
            .iter()
            .filter_map(|id| {
                let f = g.follower(*id)?;
                Some((f.at, f.models.first()?.0.clone()))
            })
            .collect();
        self.game.world.steer = None;
        for s in sounds {
            self.effect(&s);
        }
        for (place, name) in dying {
            if let Some(m) = self.model(&name) {
                self.effects.play_model(&m, place);
            }
        }
    }

    /// What the module's game plays, if it has its own music.
    pub(in crate::stage) fn turret_music(&self) -> Option<std::path::PathBuf> {
        let g = self.game.world.minigame.as_ref()?;
        let p = self
            .dir
            .join("streammusic")
            .join(format!("{}.wav", g.music));
        p.is_file().then_some(p)
    }

    fn add_turret_model(
        scene: &mut Scene,
        model: &Rc<Model>,
        animations: &[kotor_game::PlayedAnimation],
        xf: &Mat,
    ) {
        let (pose, hidden) = posed(model, animations);
        let skip: Vec<&str> = hidden.iter().map(String::as_str).collect();
        let before = scene.meshes.len();
        scene.add_model_with(model, Some(&pose), xf, &skip, None, None);
        if SHADED.iter().any(|s| model.name.eq_ignore_ascii_case(s)) {
            for m in &mut scene.meshes[before..] {
                m.shade = COCKPIT_SHADE;
            }
        }
    }

    /// Everything that moves, in place of the people of a module.
    pub(in crate::stage) fn turret_scene(&mut self) -> Option<Scene> {
        let g = self.game.world.minigame.as_ref()?;
        let turning = g.turning_matrix();
        let still = at(g.player.at);
        let mut wanted: Vec<(String, Vec<kotor_game::PlayedAnimation>, Mat)> = Vec::new();
        for (name, turns) in &g.player.models {
            let xf = if *turns { turning } else { still };
            wanted.push((name.clone(), g.player.animations.clone(), xf));
        }
        for (n, bank) in g.player.guns.iter().enumerate() {
            let place = at(g.gun_place(n));
            wanted.push((
                bank.gun_model.clone(),
                Vec::new(),
                multiply(&turning, &place),
            ));
        }
        for e in g.enemies.iter().filter(|e| !e.gone) {
            for (name, _) in &e.models {
                wanted.push((name.clone(), e.animations.clone(), e.placed));
            }
        }
        for b in &g.bullets {
            wanted.push((b.model.clone(), Vec::new(), flying(b.at, b.moving)));
        }
        // The sky goes on while anything is going on.
        let number = self
            .game
            .world
            .room_animation
            .as_ref()
            .map_or(FIRST_ROOM_ANIMATION, |r| r.1);
        let room = self
            .game
            .world
            .room_animation
            .as_ref()
            .map_or(format!("{}{ROOM_SUFFIX}", self.area_name()), |r| {
                r.0.clone()
            });
        let sky = kotor_game::PlayedAnimation {
            name: format!("{ROOM_ANIMATION}{number:02}"),
            time: self.game.world.time,
            looping: true,
            overlay: false,
        };
        wanted.push((room, vec![sky], at([0.0; 3])));
        let mut scene = Scene::default();
        let mut missing = HashSet::new();
        for (name, animations, xf) in wanted {
            if missing.contains(&name) {
                continue;
            }
            match self.model(&name) {
                Some(m) => Self::add_turret_model(&mut scene, &m, &animations, &xf),
                None => {
                    missing.insert(name);
                }
            }
        }
        Some(scene)
    }
}
