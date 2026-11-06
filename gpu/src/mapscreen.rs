//! `map.gui`, as the reference video has it at 3:50:35: the area's picture, the party and the notes on it. The pictures
//! are 512 wide and only the left 440 are the map: the video shows a black strip for the rest, and the Bridge note of
//! `m01aa` only lands on the bridge like that. The video draws it smaller, pixel for pixel. What has not been seen shows
//! a grid. Return to Hideout is greyed.

use super::{Screen, Stage};
use kotor_game::{AreaMap, SEEN_COLS, SEEN_ROWS};
use kotor_ui::{Mark, MenuView, MiniMap};

const STR_MAP_NOTE: u32 = 349;
/// The file's place for the picture.
const PICTURE: [f32; 4] = [95.0, 118.0, 440.0, 256.0];
const PICTURE_PART: [f32; 2] = [440.0 / 512.0, 1.0];
const NOTE_PICTURE: &str = "lbl_mapcircle";
const NOTE_COLOR: [f32; 3] = [0.0, 0.65, 0.97];
const CHOSEN_COLOR: [f32; 3] = [1.0, 1.0, 0.3];
const PLAYER_PICTURE: &str = "mm_barrow";
const PLAYER_SIDE: i32 = 24;
/// The map as far as it has been seen, made afresh when more has.
const SEEN_PICTURE: &str = "kotor_rs_seen_map";

/// The map on show, and the note chosen.
pub(super) struct Shown {
    area: u32,
    map: Option<AreaMap>,
    note: usize,
    /// How many cells the picture was last made with.
    composed: usize,
}

fn mark(at: [f32; 2], side: i32, tint: [f32; 3]) -> Mark {
    Mark {
        at: (
            PICTURE[0] + at[0] * PICTURE[2],
            PICTURE[1] + at[1] * PICTURE[3],
        ),
        picture: NOTE_PICTURE.to_string(),
        size: (side, side),
        part: [1.0, 1.0],
        tint,
        angle: None,
    }
}

impl Stage {
    /// Read the map if the party has moved to another area since.
    fn current_map(&mut self) {
        let area = self.game.world.area;
        let fresh = self.menus.map.as_ref().is_some_and(|s| s.area == area);
        if !fresh {
            self.menus.map = Some(Shown {
                area,
                map: self.game.world.area_map(),
                note: 0,
                composed: 0,
            });
        }
    }

    /// What is round the player has been seen. A picture that shows it is made if that has changed.
    fn see_here(&mut self) {
        self.current_map();
        let w = &mut self.game.world;
        let Some(p) = w.object(w.pc).map(|o| o.position) else {
            return;
        };
        let Some(s) = self.menus.map.as_mut() else {
            return;
        };
        let Some(map) = s.map.as_ref() else {
            return;
        };
        let count = w.reveal(map, p[0], p[1]);
        if count == s.composed && self.ui.has_picture(SEEN_PICTURE) {
            return;
        }
        s.composed = count;
        if let Some(cells) = w.explored.get(&map.picture) {
            let picture = map.picture.clone();
            let cells = cells.clone();
            self.ui
                .compose_seen_map(SEEN_PICTURE, &picture, &cells, SEEN_COLS, SEEN_ROWS);
            self.overlay_key.clear();
        }
    }

    pub(super) fn map_view(&mut self) -> MenuView {
        self.see_here();
        let mut v = MenuView::new("map");
        let w = &self.game.world;
        let area = w.object(w.area).map_or(String::new(), |a| a.name.clone());
        let player = w.object(w.pc).map(|o| o.position);
        let heading = w.pc_mover.heading;
        v.text("LBL_Area", area);
        v.inert(&["BTN_RETURN"]);
        if self.party_blocked() {
            v.inert(&["BTN_PRTYSLCT"]);
        }
        let label = self.ui.strref(STR_MAP_NOTE);
        let Some((map, note)) = self
            .menus
            .map
            .as_ref()
            .and_then(|s| Some((s.map.as_ref()?.clone(), s.note)))
        else {
            v.hide(&["LBL_MapNote", "BTN_UP", "BTN_DOWN"]);
            return v;
        };
        v.marks.push(Mark {
            at: (PICTURE[0] + PICTURE[2] / 2.0, PICTURE[1] + PICTURE[3] / 2.0),
            picture: SEEN_PICTURE.to_string(),
            size: (PICTURE[2] as i32, PICTURE[3] as i32),
            part: PICTURE_PART,
            tint: [1.0; 3],
            angle: None,
        });
        for (n, m) in map.notes.iter().enumerate() {
            if n != note {
                v.marks.push(mark(m.at, 12, NOTE_COLOR));
            }
        }
        match map.notes.get(note) {
            Some(m) => {
                v.marks.push(mark(m.at, 14, CHOSEN_COLOR));
                v.text("LBL_MapNote", format!("{label}: {}", m.name));
            }
            None => v.hide(&["LBL_MapNote", "BTN_UP", "BTN_DOWN"]),
        }
        if let Some(p) = player {
            let mut arrow = mark(map.place(p[0], p[1]), PLAYER_SIDE, [1.0; 3]);
            arrow.picture = PLAYER_PICTURE.to_string();
            arrow.angle = Some(map.facing(p[0], p[1], heading));
            v.marks.push(arrow);
        }
        v
    }

    /// The small map on the HUD. What moves it is rounded, so the HUD is drawn again only when it shows.
    pub(in crate::stage) fn minimap_view(&mut self) -> Option<MiniMap> {
        self.see_here();
        let off = !self.menus.settings.minimap;
        self.ui.set_minimap_off(off);
        if off {
            return None;
        }
        let map = self.menus.map.as_ref()?.map.as_ref()?;
        let w = &self.game.world;
        let p = w.object(w.pc)?.position;
        let at = map.pixel(p[0], p[1]);
        let facing = map.facing(p[0], p[1], w.pc_mover.heading);
        const FACING_STEP: f32 = 0.05;
        Some(MiniMap {
            picture: SEEN_PICTURE.to_string(),
            at: [at[0].round(), at[1].round()],
            facing: (facing / FACING_STEP).round() * FACING_STEP,
        })
    }

    /// The party cannot change in a fight.
    pub(in crate::stage) fn party_blocked(&self) -> bool {
        let w = &self.game.world;
        let party = |id: &u32| *id == w.pc || w.party.contains(id);
        w.combat.target.iter().any(|(a, t)| party(a) || party(t))
    }

    fn step_note(&mut self, by: i64) {
        if let Some(s) = self.menus.map.as_mut() {
            let count = s.map.as_ref().map_or(0, |m| m.notes.len()) as i64;
            if count > 0 {
                s.note = (s.note as i64 + by).rem_euclid(count) as usize;
            }
        }
    }

    pub(super) fn map_press(&mut self, tag: &str) {
        match tag {
            "BTN_EXIT" => {
                self.menus.stack.pop();
            }
            "BTN_UP" => self.step_note(-1),
            "BTN_DOWN" => self.step_note(1),
            "BTN_PRTYSLCT" if !self.party_blocked() => self.open_party(),
            _ => {}
        }
    }

    /// The M key: the map, or back to the game if it is up.
    pub fn open_map(&mut self) {
        self.open_items(Screen::Map);
    }
}
