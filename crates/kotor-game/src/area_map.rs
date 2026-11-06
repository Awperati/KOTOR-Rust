//! The area's map: the picture's name, and where the world lies on it, from the area's `.are` and `.git` files.
//!
//! **Assumed, not compared with the game's code:** the picture is `lbl_map` and the area's name, as the file names in the
//! GUI texture pack suggest; a `NorthAxis` of 2 or 3 has the picture's across run along the world's y, since the numbers
//! in `m01aa` only give the same scale both ways like that; `MapPt` is a fraction of the part of the picture that shows,
//! from its top left.

use crate::world::{ObjId, World};
use kotor_core::ResType;
use kotor_gff::{Gff, Struct, Value};

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

impl World {
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
