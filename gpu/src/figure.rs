//! The 3D figure on the character sheet and the level-up panel, as the reference video has it at 1:23:50: the screen
//! leaves a hole in its picture, and behind it the renderer draws the leader alone, with a camera that puts them in the
//! hole. **Assumed:** a figure about 1.9 m high. **Not built:** the glowing backdrop, for which I found no picture, so the
//! renderer's dark clear colour shows behind them, and turning the figure. The party screen keeps portraits: its figure
//! is someone who is not in the world.

use super::{Screen, Stage};
use kotor_render::{Camera, Scene};

/// Where the holes are, from `character.gui`'s `LBL_3DCHAR` and `maincg.gui`'s `MODEL_LBL`: left, top, width, height.
const CHARACTER_HOLE: [f32; 4] = [55.0, 110.0, 259.0, 295.0];
const LEVEL_HOLE: [f32; 4] = [155.0, 89.0, 162.0, 186.0];
const FIGURE_HEIGHT: f32 = 1.9;
/// A little air over the head and under the feet.
const FIGURE_MARGIN: f32 = 1.15;
const FIGURE_FOV: f32 = 30.0;

impl Stage {
    fn figure_hole(&self) -> Option<[f32; 4]> {
        match self.top()? {
            Screen::Character => Some(CHARACTER_HOLE),
            Screen::LevelUp => Some(LEVEL_HOLE),
            _ => None,
        }
    }

    /// The camera for a screen that shows a figure.
    pub fn figure_camera(&self) -> Option<Camera> {
        let [left, top, width, height] = self.figure_hole()?;
        let w = &self.game.world;
        let at = w.object(w.pc)?.position;
        let (sin, cos) = w.pc_mover.heading.sin_cos();
        let tan = (FIGURE_FOV.to_radians() / 2.0).tan();
        let aspect = self.ui_width as f32 / 480.0;
        let depth = FIGURE_MARGIN * FIGURE_HEIGHT / (2.0 * tan * (height / 480.0));
        // How far the middle of the hole is from the middle of the window, at that depth.
        let across = ((left + width / 2.0) / 320.0 - 1.0) * depth * tan * aspect;
        let down = (1.0 - (top + height / 2.0) / 240.0) * depth * tan;
        let middle = at[2] + FIGURE_HEIGHT / 2.0;
        // The camera looks back along the way they face, so its right is the figure's left.
        let right = [-sin, cos];
        Some(Camera {
            eye: [at[0] + cos * depth, at[1] + sin * depth, middle],
            target: [
                at[0] - right[0] * across,
                at[1] - right[1] * across,
                middle - down,
            ],
            fov: FIGURE_FOV,
        })
    }

    /// The leader alone.
    pub fn figure_scene(&mut self) -> Scene {
        let mut scene = Scene::default();
        let pc = self.game.world.pc;
        let Some(pose) = self.game.poses().into_iter().find(|p| p.id == pc) else {
            return scene;
        };
        let Some(rig) = self.rig_of(pc) else {
            return scene;
        };
        let hands = self.hands(pc);
        self.add_creature(
            &mut scene,
            &rig,
            &hands,
            &pose.clip,
            pose.position,
            pose.heading,
        );
        scene
    }

    /// What a window should draw now, and with what camera; the last says that only this is drawn, for a figure.
    pub fn frame_scene(&mut self) -> (Scene, Camera, bool) {
        match self.figure_camera() {
            Some(camera) => (self.figure_scene(), camera, true),
            None => (self.dynamic_scene(), self.camera(), false),
        }
    }
}
