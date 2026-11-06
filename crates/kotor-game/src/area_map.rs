//! The area's map: the picture's name, and where the world lies on it, from the area's `.are` and `.git` files.
//!
//! **Assumed, not compared with the game's code:** the picture is `lbl_map` and the area's name, as the file names in the
//! GUI texture pack suggest; a `NorthAxis` of 2 or 3 has the picture's across run along the world's y, since the numbers
//! in `m01aa` only give the same scale both ways like that; `MapPt` is a fraction of the part of the picture that shows,
//! from its top left; what has been seen is a circle round the player of `SEEN_RADIUS`, measured from the width of the
//! one short walk at 1:25:00 of the reference video.

use crate::world::{ObjId, World};
use kotor_core::ResType;
use kotor_gff::{Gff, Struct, Value};

/// The part of a 512 by 256 picture that is the map: the left 440, as the video's black strip at 3:50:35 shows, and only
/// like that does the Bridge note of `m01aa` land on the bridge.
const SHOWN_WIDTH: f32 = 440.0;
const SHOWN_HEIGHT: f32 = 256.0;

/// What has been seen of a map is kept in cells of this many of the picture's pixels.
const CELL: f32 = 4.0;
pub const SEEN_COLS: usize = (SHOWN_WIDTH / CELL) as usize;
pub const SEEN_ROWS: usize = (SHOWN_HEIGHT / CELL) as usize;
const SEEN_RADIUS: f32 = 14.0;

/// A waypoint with a note on the map.
#[derive(Clone, Debug, PartialEq)]
pub struct MapNote {
    pub name: String,
    /// Where it is, as a fraction of the part of the picture that shows.
    pub at: [f32; 2],
}

/// An area's map.
#[derive(Clone, Debug, PartialEq)]
pub struct AreaMap {
    pub area: ObjId,
    pub picture: String,
    pub notes: Vec<MapNote>,
    swapped: bool,
    world: [[f32; 2]; 2],
    map: [[f32; 2]; 2],
}

impl AreaMap {
    /// Where a point of the world is, as a fraction of the part of the picture that shows.
    pub fn place(&self, x: f32, y: f32) -> [f32; 2] {
        let along = |value: f32, world: [f32; 2], map: [f32; 2]| {
            let span = world[1] - world[0];
            if span == 0.0 {
                map[0]
            } else {
                map[0] + (value - world[0]) * (map[1] - map[0]) / span
            }
        };
        let [wx, wy] = self.world;
        let [mx, my] = self.map;
        if self.swapped {
            [along(y, wy, mx), along(x, wx, my)]
        } else {
            [along(x, wx, mx), along(y, wy, my)]
        }
    }
}

impl AreaMap {
    /// Which way a heading in the world's radians points on the picture: radians clockwise from up.
    pub fn facing(&self, x: f32, y: f32, heading: f32) -> f32 {
        let (a, b) = (
            self.place(x, y),
            self.place(x + heading.cos(), y + heading.sin()),
        );
        ((b[0] - a[0]) * SHOWN_WIDTH).atan2(-(b[1] - a[1]) * SHOWN_HEIGHT)
    }

    /// Where a point of the world is in the picture's own pixels, for a picture 512 by 256.
    pub fn pixel(&self, x: f32, y: f32) -> [f32; 2] {
        let [u, v] = self.place(x, y);
        [u * SHOWN_WIDTH, v * SHOWN_HEIGHT]
    }
}

impl World {
    /// The player has been here: what is round them has been seen. Returns how many cells of this map are seen.
    pub fn reveal(&mut self, map: &AreaMap, x: f32, y: f32) -> usize {
        let cells = self
            .explored
            .entry(map.picture.clone())
            .or_insert_with(|| vec![false; SEEN_COLS * SEEN_ROWS]);
        let [px, py] = map.pixel(x, y);
        let reach = (SEEN_RADIUS / CELL).ceil() as i32 + 1;
        let (cx, cy) = ((px / CELL) as i32, (py / CELL) as i32);
        for row in cy - reach..=cy + reach {
            for col in cx - reach..=cx + reach {
                let inside =
                    (0..SEEN_COLS as i32).contains(&col) && (0..SEEN_ROWS as i32).contains(&row);
                let (dx, dy) = (
                    (col as f32 + 0.5) * CELL - px,
                    (row as f32 + 0.5) * CELL - py,
                );
                if inside && dx * dx + dy * dy <= SEEN_RADIUS * SEEN_RADIUS {
                    cells[row as usize * SEEN_COLS + col as usize] = true;
                }
            }
        }
        cells.iter().filter(|c| **c).count()
    }

    /// The map of the area the party is in, if it has one.
    pub fn area_map(&mut self) -> Option<AreaMap> {
        let area = self.area;
        let name = self.object(area)?.tag.clone();
        let are = Gff::parse(&self.inst.read(&name, ResType::ARE).ok()?).ok()?;
        let Some(Value::Struct(m)) = are.root.get("Map") else {
            return None;
        };
        let m: &Struct = m;
        let f = |l: &str| m.float(l).unwrap_or(0.0) as f32;
        let mut out = AreaMap {
            area,
            picture: format!("lbl_map{}", name.to_ascii_lowercase()),
            notes: Vec::new(),
            swapped: m.int("NorthAxis").unwrap_or(0) >= 2,
            world: [
                [f("WorldPt1X"), f("WorldPt2X")],
                [f("WorldPt1Y"), f("WorldPt2Y")],
            ],
            map: [[f("MapPt1X"), f("MapPt2X")], [f("MapPt1Y"), f("MapPt2Y")]],
        };
        let git = Gff::parse(&self.inst.read(&name, ResType::GIT).ok()?).ok()?;
        for w in git.root.list("WaypointList").unwrap_or_default() {
            let shown = |l: &str| w.int(l).unwrap_or(0) != 0;
            if shown("HasMapNote") && shown("MapNoteEnabled") {
                let x = w.float("XPosition").unwrap_or(0.0) as f32;
                let y = w.float("YPosition").unwrap_or(0.0) as f32;
                out.notes.push(MapNote {
                    name: w.loc("MapNote").map_or(String::new(), |l| self.text(l)),
                    at: out.place(x, y),
                });
            }
        }
        Some(out)
    }
}
