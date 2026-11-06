//! The swoop race on screen: the bike and the camera that rides with it, which holds the race's clock and meter as well,
//! and the pads and buoys near it. The rules are `kotor_game::minigame`.

use super::Stage;
use kotor_model::{Model, Pose};
use kotor_render::{Camera, Scene};

/// How far ahead pads and buoys are drawn, and how far behind (layout units). The renderer's far plane is 2,000.
const DRAW_AHEAD: f32 = 1500.0;
const DRAW_BEHIND: f32 = 30.0;
const CAMERA_NODE: &str = "camera";

fn at(p: [f32; 3]) -> [[f32; 4]; 4] {
    [
        [1.0, 0.0, 0.0, p[0]],
        [0.0, 1.0, 0.0, p[1]],
        [0.0, 0.0, 1.0, p[2]],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// The bike's models take what they hold of everything playing. Later clips win a node.
fn posed(model: &Model, playing: &[(String, f32)]) -> Pose {
    let mut pose = Pose::new();
    for (name, t) in playing {
        if let Some(a) = model.animation(name) {
            pose.extend(a.sample(model, t.min(a.length)));
        }
    }
    pose
}

impl Stage {
    /// Which way the keys steer the bike.
    pub(super) fn race_input(&mut self) {
        // The arrow keys turn the camera everywhere else: positive is to the left.
        let right = (self.steer.1 - self.turn.clamp(-1.0, 1.0)).clamp(-1.0, 1.0);
        if let Some(game) = self.game.world.race.as_mut() {
            game.steer = right;
        }
    }

    /// The fire key, in a race: it shifts the gears. Whether one was on.
    pub(super) fn race_fire(&mut self) -> bool {
        match self.game.world.race.as_mut() {
            Some(game) => {
                game.fire = true;
                true
            }
            None => false,
        }
    }

    /// What a race draws over the rooms, and from where.
    pub(super) fn race_scene(&mut self) -> Option<(Scene, Camera)> {
        let game = self.game.world.race.clone()?;
        let here = game.position();
        let playing = game.animations_now();
        let mut scene = Scene::default();
        let mut camera = None;
        for name in &game.file.player.models {
            let Some(model) = self.model(name) else {
                continue;
            };
            let pose = posed(&model, &playing);
            scene.add_model_with(&model, Some(&pose), &at(here), &[], None, None);
            if let Some((_, m)) = model
                .world_matrices(Some(&pose))
                .into_iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(CAMERA_NODE))
            {
                // A camera looks along its node's -z.
                let eye = [0, 1, 2].map(|i| m[i][3] + here[i]);
                camera = Some(Camera {
                    eye,
                    target: [0, 1, 2].map(|i| eye[i] - m[i][2]),
                    fov: game.file.view_angle,
                });
            }
        }
        let near = |y: f32| y > here[1] - DRAW_BEHIND && y < here[1] + DRAW_AHEAD;
        for r in game.riders.iter().filter(|r| near(r.at[1])) {
            for name in &r.models {
                let Some(model) = self.model(name) else {
                    continue;
                };
                let (clip, t) = match r.fell_at {
                    Some(t) => ("die", game.time - t),
                    None => ("ready_01", 0.0),
                };
                let pose = model
                    .animation(clip)
                    .map(|a| a.sample(&model, t.min(a.length)));
                scene.add_model_with(&model, pose.as_ref(), &at(r.at), &[], None, None);
            }
        }
        for o in game.obstacles.iter().filter(|o| near(o.at[1])) {
            let Some(model) = self.model(&o.model) else {
                continue;
            };
            let pose = model
                .animation("ready")
                .map(|a| a.sample(&model, game.time % a.length.max(0.01)));
            scene.add_model_with(&model, pose.as_ref(), &at(o.at), &[], None, None);
        }
        Some((scene, camera?))
    }
}
