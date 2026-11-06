//! The 3D figure on the character sheet and the level-up panel, as the reference video has it at 1:23:50: the screen
//! leaves a hole in its picture, and behind it the renderer draws the leader alone, with a camera that puts them in the
//! hole. **Assumed:** a figure about 1.9 m high. **Not built:** turning the figure. The party screen has one too, as at
//! 3:12:00, of whoever is chosen: with nothing in their hands, as the video has them, and from their creature file if they
//! are not here.
//!
//! The mist behind them is the emitters of `charmain_light.mdl`, as making a character has `cgmain_light.mdl`. The install
//! has no dark model, so a dark figure's mist is the same mist with its blue and red swapped: the owner's words are blue for
//! the light side and red for the dark. **Assumed:** where that starts, `MIST_BLUE_FROM` and `MIST_RED_FROM`: the video has
//! clean blue at about 65, and a dark player is not in the frames I looked at.

use super::{Screen, Stage};
use crate::effects::Effects;
use kotor_core::ResType;
use kotor_game::SLOT_BODY;
use kotor_gff::Gff;
use kotor_render::{Camera, Rig, Scene};
use std::rc::Rc;

/// Where the holes are, from `character.gui`'s `LBL_3DCHAR` and `maincg.gui`'s `MODEL_LBL`: left, top, width, height.
const CHARACTER_HOLE: [f32; 4] = [55.0, 110.0, 259.0, 295.0];
const LEVEL_HOLE: [f32; 4] = [155.0, 89.0, 162.0, 186.0];
const PARTY_HOLE: [f32; 4] = [326.0, 109.0, 258.0, 213.0];
const CREATURE_FILE: ResType = ResType(2027);
const ITEM_FILE: ResType = ResType(2025);
const FIGURE_HEIGHT: f32 = 1.9;
/// A little air over the head and under the feet.
const FIGURE_MARGIN: f32 = 1.15;
const FIGURE_FOV: f32 = 30.0;
const SHEET_MIST: &str = "charmain_light";
const LEVEL_MIST: &str = "cgmain_light";
/// Seconds of mist made at once when a screen opens, so that it is there already.
const MIST_START: f32 = 6.0;
const MIST_STEP: f32 = 1.0 / 30.0;
const MIST_BLUE_FROM: i32 = 60;
const MIST_RED_FROM: i32 = 40;
const ALIGNMENT_NEUTRAL: i32 = 50;

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
                        [0.0, 0.0, 1.0, at[2]],
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
            self.add_creature(&mut scene, &rig, &[], &None, at, heading);
        }
        scene
    }

    /// The leader alone, or whoever the party screen has chosen.
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

    /// The particles that go with `frame_scene`.
    pub fn frame_particles(&mut self) -> Vec<crate::effects::ParticleBatch> {
        if let Some(fog) = self.main_scene_particles() {
            return fog;
        }
        if let Some(mist) = self.chargen_particles() {
            return mist;
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
        if let Some((scene, camera)) = self.chargen_scene().or_else(|| self.bench_scene()) {
            return (scene, camera, true);
        }
        match self.figure_camera() {
            Some(camera) => (self.figure_scene(), camera, true),
            None => (self.dynamic_scene(), self.camera(), false),
        }
    }
}
