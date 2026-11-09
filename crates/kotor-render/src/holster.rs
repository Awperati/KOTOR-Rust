//! Where a weapon is when it is not in the hand: a place on the body (a node, and an offset in its frame) by the weapon's class
//! and hand, from `docs/holsters.tsv`, and the plain holster shape drawn there. The design is `docs/HOLSTERS.md`.
use crate::scene::{Mat, SceneMesh};
use std::sync::OnceLock;

/// One row of the table: a weapon class, in a hand, sits at `local` of the body node `node`.
#[derive(Clone, Debug)]
pub struct Holster {
    /// `class:hand`, what `Stage::hands` gives in place of a hand node and `Rig::attach` takes.
    pub key: String,
    pub class: String,
    pub right_hand: bool,
    pub node: String,
    pub local: Mat,
    pub shape: String,
}

static TABLE: OnceLock<Vec<Holster>> = OnceLock::new();

fn mul(a: &Mat, b: &Mat) -> Mat {
    let mut o = [[0.0; 4]; 4];
    for (i, row) in o.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (0..4).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    o
}

/// Offset, then turns of roll about x, pitch about y and yaw about z, in degrees: `T * Rz * Ry * Rx`.
fn local_matrix(p: [f32; 3], roll: f32, pitch: f32, yaw: f32) -> Mat {
    let rot = |axis: usize, deg: f32| {
        let (s, c) = deg.to_radians().sin_cos();
        let mut m = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let (a, b) = match axis {
            0 => (1, 2),
            1 => (2, 0),
            _ => (0, 1),
        };
        m[a][a] = c;
        m[a][b] = -s;
        m[b][a] = s;
        m[b][b] = c;
        m
    };
    let mut m = mul(&rot(2, yaw), &mul(&rot(1, pitch), &rot(0, roll)));
    for (i, v) in p.iter().enumerate() {
        m[i][3] = *v;
    }
    m
}

/// A place made `k` times the size, about its own origin.
fn scaled(mut m: Mat, k: f32) -> Mat {
    for row in m.iter_mut().take(3) {
        for v in row.iter_mut().take(3) {
            *v *= k;
        }
    }
    m
}

fn parse(text: &str) -> Vec<Holster> {
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .skip(1)
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            let num = |i: usize| f.get(i)?.trim().parse::<f32>().ok();
            let right_hand = *f.get(1)? == "right";
            Some(Holster {
                key: format!("holster:{}:{}", f.first()?, f.get(1)?),
                class: f.first()?.to_string(),
                right_hand,
                node: f.get(2)?.to_string(),
                local: scaled(
                    local_matrix([num(3)?, num(4)?, num(5)?], num(6)?, num(7)?, num(8)?),
                    num(10).unwrap_or(1.0),
                ),
                shape: f.get(9)?.trim().to_string(),
            })
        })
        .collect()
}

/// Every place in the table.
pub fn table() -> &'static [Holster] {
    TABLE.get_or_init(|| parse(include_str!("../../../docs/holsters.tsv")))
}

/// The weapon class of a `baseitems.2da` `weaponwield`: `None` for what is not carried on the body (not a weapon).
pub fn class_of(wield: i32) -> Option<&'static str> {
    Some(match wield {
        1 => "baton",
        2 => "blade",
        3 => "twohand",
        4 => "pistol",
        5 => "rifle",
        6 => "heavy",
        _ => return None,
    })
}

/// The key of the place for a weapon class in a hand, which `Rig::attach` takes as a node name.
pub fn key_for(class: &str, right_hand: bool) -> Option<&'static str> {
    table()
        .iter()
        .find(|h| h.class == class && h.right_hand == right_hand)
        .map(|h| h.key.as_str())
}

/// The place a key names.
pub fn by_key(key: &str) -> Option<&'static Holster> {
    key.starts_with("holster:")
        .then(|| table().iter().find(|h| h.key == key))
        .flatten()
}

/// The plain holster of a place: a box or two, in the frame of the place (the weapon's own origin is the hilt or the butt).
pub fn shape_mesh(shape: &str, xf: &Mat) -> Option<SceneMesh> {
    // (centre, half sizes) of each box, in the frame of the place. A shape named `_z` runs along z, for the blades.
    let (name, along_z) = shape
        .strip_suffix("_z")
        .map_or((shape, false), |n| (n, true));
    let boxes: Vec<([f32; 3], [f32; 3])> = match name {
        // The pistol's holster: the pouch round the gun, its flap, a loop up to the belt, and a strap round the thigh with a buckle.
        "hip_holster" => vec![
            ([0.0, 0.13, 0.0], [0.035, 0.11, 0.045]),
            ([0.0, 0.0, 0.0], [0.04, 0.02, 0.05]),
            ([0.0, -0.05, 0.0], [0.012, 0.045, 0.062]),
            ([0.0, 0.2, 0.0], [0.045, 0.008, 0.055]),
            ([0.0, 0.2, 0.05], [0.014, 0.012, 0.012]),
        ],
        // The rifle's sling: the strap down the back, two loops round the weapon, a buckle at the top and the butt's pad.
        "sling" => vec![
            ([0.0, 0.3, 0.0], [0.03, 0.03, 0.012]),
            ([0.0, 0.12, 0.0], [0.04, 0.012, 0.03]),
            ([0.0, 0.46, 0.0], [0.04, 0.012, 0.03]),
            ([0.0, 0.02, 0.0], [0.016, 0.016, 0.034]),
            ([0.0, 0.58, 0.0], [0.034, 0.02, 0.026]),
        ],
        // A sword's scabbard along the back, with its two bands and the belt it hangs from.
        "sheath" => vec![
            ([0.0, 0.45, 0.0], [0.025, 0.4, 0.02]),
            ([0.0, 0.1, 0.0], [0.034, 0.015, 0.028]),
            ([0.0, 0.7, 0.0], [0.034, 0.015, 0.028]),
            ([0.0, 0.0, 0.0], [0.02, 0.02, 0.05]),
        ],
        // A double blade is carried bare on the back in a harness: two bands and a strap across the chest.
        "staff" => vec![
            ([0.0, 0.1, 0.0], [0.03, 0.04, 0.03]),
            ([0.0, 0.7, 0.0], [0.034, 0.018, 0.034]),
            ([0.0, -0.5, 0.0], [0.034, 0.018, 0.034]),
        ],
        _ => return None,
    };
    let boxes: Vec<([f32; 3], [f32; 3])> = if along_z {
        // The same pieces turned so that their long side is the item's z, not its y.
        boxes
            .into_iter()
            .map(|(c, h)| ([c[0], c[2], c[1]], [h[0], h[2], h[1]]))
            .collect()
    } else {
        boxes
    };
    let mut mesh = SceneMesh {
        name: format!("holster {name}"),
        positions: Vec::new(),
        normals: Vec::new(),
        uvs: Vec::new(),
        uvs2: Vec::new(),
        triangles: Vec::new(),
        // The pistol's own gun-metal picture: a plain dull colour.
        texture: Some("w_blstrpstl_001".to_string()),
        lightmap: None,
        shade: 0.0,
        fade: 0.0,
    };
    let at = |p: [f32; 3]| {
        [
            xf[0][0] * p[0] + xf[0][1] * p[1] + xf[0][2] * p[2] + xf[0][3],
            xf[1][0] * p[0] + xf[1][1] * p[1] + xf[1][2] * p[2] + xf[1][3],
            xf[2][0] * p[0] + xf[2][1] * p[1] + xf[2][2] * p[2] + xf[2][3],
        ]
    };
    for (c, h) in &boxes {
        let base = mesh.positions.len() as u32;
        for k in 0..8u32 {
            let p = [
                c[0] + if k & 1 == 0 { -h[0] } else { h[0] },
                c[1] + if k & 2 == 0 { -h[1] } else { h[1] },
                c[2] + if k & 4 == 0 { -h[2] } else { h[2] },
            ];
            mesh.positions.push(at(p));
            mesh.normals.push([0.0, 0.0, 1.0]);
            mesh.uvs.push([0.5, 0.5]);
        }
        for q in [
            [0, 1, 3, 2],
            [4, 6, 7, 5],
            [0, 4, 5, 1],
            [2, 3, 7, 6],
            [0, 2, 6, 4],
            [1, 5, 7, 3],
        ] {
            mesh.triangles.push([base + q[0], base + q[1], base + q[2]]);
            mesh.triangles.push([base + q[0], base + q[2], base + q[3]]);
        }
    }
    Some(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_has_a_place_for_each_class_in_each_hand() {
        for class in ["baton", "blade", "twohand", "pistol", "rifle", "heavy"] {
            for right in [true, false] {
                assert!(key_for(class, right).is_some(), "{class} {right}");
            }
        }
        assert!(by_key("holster:pistol:right").is_some());
        assert!(by_key("rhand").is_none());
    }

    #[test]
    fn the_places_are_on_the_sides_the_owner_named() {
        let at = |class: &str, right: bool| by_key(key_for(class, right).unwrap()).unwrap().local;
        // A right-handed pistol on the left hip (x positive is the creature's right), a left-handed one on the right.
        assert!(at("pistol", true)[0][3] < -0.1);
        assert!(at("pistol", false)[0][3] > 0.1);
        // Blades on the back, behind the body's middle.
        assert!(at("blade", true)[1][3] < 0.0);
    }
    /// Swords, sabers and batons are modelled along z, guns along y (`item_axes`: a vibroblade z -0.24 to 1.05, a rifle y -0.12 to
    /// 0.58). A sword on the back lies along the back, not across the shoulders as it did when it was turned like a gun.
    #[test]
    fn a_blade_lies_along_the_back_and_a_baton_hangs_down() {
        let world_z = |class: &str, axis: usize| {
            let m = by_key(key_for(class, true).unwrap()).unwrap().local;
            // How much of the item's own axis points up in the node's frame.
            m[2][axis]
        };
        // The blade's z: a long way up or down, not forward.
        for class in ["blade", "twohand"] {
            let m = by_key(key_for(class, true).unwrap()).unwrap().local;
            let along = [m[0][2], m[1][2], m[2][2]];
            let length: f32 = along.iter().map(|v| v * v).sum::<f32>().sqrt();
            assert!(
                along[2].abs() / length > 0.7,
                "{class} lies flat: {along:?}"
            );
            assert!(
                along[1].abs() / length < 0.2,
                "{class} points forward: {along:?}"
            );
        }
        assert!(world_z("baton", 2) < -0.9, "a baton's z hangs down");
        // A gun's y: a rifle stands up on the back.
        assert!(world_z("rifle", 1) > 0.8);
    }
}
