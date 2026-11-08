//! A player's body reshaped by the sliders of character creation: shoulders, chest, waist, hips, arms and legs. The owner: "I want
//! the character creation to be complex and detailed like the skyrim one". The game's bodies have no morph targets, so each slider
//! moves the vertices of the body at rest, by the bones that carry them, once when the body is loaded: every pose then shows it, as
//! [`crate::underwear::shape_chest`] does for the chest. Height is not here: it scales the posed figure, see [`BodyShape::height_scale`].
//!
//! It works on any of the player's bodies, bare or in armour, as they share one skeleton. **Assumed:** the sizes at the ends of each
//! slider, by eye from frames, kept small: the painted texture moves with the vertices, and big moves would look wrong.

use kotor_model::{Model, Node};

/// A slider's range either side of 0, in per cent as a look keeps it, and how far one step of its arrows goes.
pub const SLIDER_MOST: i32 = 100;
pub const SLIDER_STEP: i32 = 10;

/// How far each slider goes at its end, in metres or as a share.
const HEIGHT_MOST: f32 = 0.08;
const SHOULDERS_MOST: f32 = 0.035;
const CHEST_MOST: f32 = 0.03;
const WAIST_MOST: f32 = 0.16;
const HIPS_MOST: f32 = 0.12;
const ARMS_MOST: f32 = 0.25;
const LEGS_MOST: f32 = 0.2;
/// How far up and down from its middle each band of the torso reaches, in metres.
const CHEST_REACH: f32 = 0.12;
const WAIST_REACH: f32 = 0.13;
const HIPS_REACH: f32 = 0.16;
/// Below the collar, how far down the shoulders' widening reaches, and from how far out of the middle.
const SHOULDER_DOWN: f32 = 0.14;
const SHOULDER_FROM: f32 = 0.06;

/// The sliders, each from -1 to 1. Zero is the game's own body.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BodyShape {
    pub height: f32,
    pub shoulders: f32,
    pub chest: f32,
    pub waist: f32,
    pub hips: f32,
    pub arms: f32,
    pub legs: f32,
}

/// The sliders' names, as a look keeps them, in the order of the page.
pub const BODY_SLIDERS: [&str; 7] = [
    "height",
    "shoulders",
    "chest",
    "waist",
    "hips",
    "arms",
    "legs",
];

impl BodyShape {
    /// From a look's numbers by name, each from `-SLIDER_MOST` to `SLIDER_MOST`.
    pub fn from_sliders(get: impl Fn(&str) -> i32) -> BodyShape {
        let v =
            |name: &str| (get(name).clamp(-SLIDER_MOST, SLIDER_MOST)) as f32 / SLIDER_MOST as f32;
        BodyShape {
            height: v("height"),
            shoulders: v("shoulders"),
            chest: v("chest"),
            waist: v("waist"),
            hips: v("hips"),
            arms: v("arms"),
            legs: v("legs"),
        }
    }

    /// Whether the body is the game's own, but perhaps taller or shorter.
    pub fn is_plain(&self) -> bool {
        let BodyShape {
            height: _,
            shoulders,
            chest,
            waist,
            hips,
            arms,
            legs,
        } = *self;
        [shoulders, chest, waist, hips, arms, legs]
            .iter()
            .all(|v| *v == 0.0)
    }

    /// How much bigger the whole posed figure is drawn, about its feet. Only for how it looks: doors, walking and fights keep their
    /// sizes.
    pub fn height_scale(&self) -> f32 {
        1.0 + self.height.clamp(-1.0, 1.0) * HEIGHT_MOST
    }
}

/// A smooth hump: 1 at `centre`, 0 from `reach` away.
fn bump(x: f32, centre: f32, reach: f32) -> f32 {
    let t = ((x - centre).abs() / reach.max(1e-4)).min(1.0);
    0.5 + 0.5 * (t * std::f32::consts::PI).cos()
}

/// 0 below `from`, 1 from `from + over`, smooth between.
fn ramp(x: f32, from: f32, over: f32) -> f32 {
    let t = ((x - from) / over.max(1e-4)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The bones of a limb, each with the next one down it: a vertex they carry is moved away from or towards the line between them.
const LEG_BONES: [(&str, &str); 4] = [
    ("lthigh_g", "lshin_g"),
    ("lshin_g", "lfoot_g"),
    ("rthigh_g", "rshin_g"),
    ("rshin_g", "rfoot_g"),
];
const ARM_BONES: [(&str, &str); 6] = [
    ("lbicep_g", "lbicepl_g"),
    ("lbicepl_g", "lforearm_g"),
    ("lforearm_g", "lhand_g"),
    ("rbicep_g", "rbicepl_g"),
    ("rbicepl_g", "rforearm_g"),
    ("rforearm_g", "rhand_g"),
];
/// What goes out with the shoulders: the arm and everything on it, and the collar half as far.
const SHOULDER_BONES: [&str; 4] = ["bicep", "forearm", "hand", "fngr"];
const COLLAR_BONES: [&str; 2] = ["lcollar_g", "rcollar_g"];
const TORSO_BONES: [&str; 5] = [
    "torso_g",
    "torsoupr_g",
    "pelvis_g",
    "breastbone",
    "necklwr_g",
];

/// Where the bones are at rest, by lower-case name.
struct Joints(Vec<(String, [f32; 3])>);

impl Joints {
    fn of(model: &Model) -> Joints {
        Joints(
            model
                .world_matrices(None)
                .into_iter()
                .map(|(name, m)| (name.to_ascii_lowercase(), [m[0][3], m[1][3], m[2][3]]))
                .collect(),
        )
    }

    fn at(&self, name: &str) -> Option<[f32; 3]> {
        self.0.iter().find(|(n, _)| n == name).map(|(_, p)| *p)
    }
}

/// The heights the torso's bands go round, from the skeleton.
struct Bands {
    chest: f32,
    waist: f32,
    hips: f32,
    collar: f32,
    middle_y: f32,
}

impl Bands {
    fn of(j: &Joints) -> Option<Bands> {
        let torso = j.at("torso_g")?;
        let pelvis = j.at("pelvis_g")?;
        let collar = j.at("rcollar_g").or(j.at("lcollar_g"))?;
        let chest = j
            .at("breastbone")
            .map_or((torso[2] + collar[2]) / 2.0, |b| b[2] - 0.03);
        let thigh = j
            .at("lthigh_g")
            .or(j.at("rthigh_g"))
            .map_or(pelvis[2] - 0.12, |t| t[2]);
        Some(Bands {
            chest,
            waist: torso[2] + 0.02,
            hips: (pelvis[2] + thigh) / 2.0,
            collar: collar[2],
            middle_y: torso[1],
        })
    }
}

/// The nearest point to `p` on the line from `a` to `b`, kept between them.
fn on_segment(p: [f32; 3], a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let len2 = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).max(1e-8);
    let t = (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1] + (p[2] - a[2]) * d[2]) / len2)
        .clamp(0.0, 1.0);
    [a[0] + d[0] * t, a[1] + d[1] * t, a[2] + d[2] * t]
}

/// How far one bone's rule moves a point at rest, in the model's frame.
fn bone_move(bone: &str, q: [f32; 3], s: &BodyShape, j: &Joints, bands: &Bands) -> [f32; 3] {
    let mut out = [0.0; 3];
    let limb = |pairs: &[(&str, &str)], amount: f32, out: &mut [f32; 3]| {
        if amount == 0.0 {
            return;
        }
        if let Some((a, b)) = pairs.iter().find(|(a, _)| *a == bone) {
            if let (Some(a), Some(b)) = (j.at(a), j.at(b)) {
                let c = on_segment(q, a, b);
                for k in 0..3 {
                    out[k] += (q[k] - c[k]) * amount;
                }
            }
        }
    };
    limb(&LEG_BONES, s.legs * LEGS_MOST, &mut out);
    limb(&ARM_BONES, s.arms * ARMS_MOST, &mut out);
    let side = q[0].signum();
    if SHOULDER_BONES.iter().any(|b| bone.contains(b)) {
        out[0] += side * s.shoulders * SHOULDERS_MOST;
    } else if COLLAR_BONES.contains(&bone) {
        out[0] += side * s.shoulders * SHOULDERS_MOST * 0.5;
    }
    let torso = TORSO_BONES.contains(&bone);
    let thigh = bone.ends_with("thigh_g");
    if torso {
        // The shoulders: the top of the torso out at the sides widens with them.
        let high = ramp(q[2], bands.collar - SHOULDER_DOWN, SHOULDER_DOWN);
        let out_side = ramp(q[0].abs(), SHOULDER_FROM, 0.1);
        out[0] += side * s.shoulders * SHOULDERS_MOST * 0.5 * high * out_side;
        // The chest: forward, at the front.
        let front = ramp(q[1] - bands.middle_y, 0.0, 0.04);
        out[1] += s.chest * CHEST_MOST * bump(q[2], bands.chest, CHEST_REACH) * front;
        out[0] += side * s.chest * CHEST_MOST * 0.3 * bump(q[2], bands.chest, CHEST_REACH) * front;
        // The waist: narrower or wider all round.
        let w = s.waist * WAIST_MOST * bump(q[2], bands.waist, WAIST_REACH);
        out[0] += q[0] * w;
        out[1] += (q[1] - bands.middle_y) * w;
    }
    if torso || thigh {
        // The hips: wider at the sides, a little deeper.
        let h = s.hips * HIPS_MOST * bump(q[2], bands.hips, HIPS_REACH);
        out[0] += q[0] * h;
        out[1] += (q[1] - bands.middle_y) * h * 0.5;
    }
    out
}

/// Reshape a body at rest by the sliders. A body without the player's skeleton is left as it is.
pub fn shape_body(model: &mut Model, s: &BodyShape) {
    if s.is_plain() {
        return;
    }
    let joints = Joints::of(model);
    let Some(bands) = Bands::of(&joints) else {
        return;
    };
    let world = model.world_matrices(None);
    let mut names = std::collections::HashMap::new();
    model.root.walk(&mut |n| {
        names.insert(n.index as usize, n.name.to_ascii_lowercase());
    });
    fn each_mesh(n: &mut Node, f: &mut dyn FnMut(&str, &mut kotor_model::Mesh)) {
        if let Some(m) = n.mesh.as_mut() {
            f(&n.name, m);
        }
        for c in &mut n.children {
            each_mesh(c, f);
        }
    }
    each_mesh(&mut model.root, &mut |name, mesh| {
        let Some(skin) = mesh.skin.as_ref() else {
            return;
        };
        let Some(w) = world
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|x| x.1)
        else {
            return;
        };
        // The skin's map goes from each node to its bone slot, as a number: turned round, from a slot to its bone's name.
        let mut slot_bone: Vec<Option<String>> = Vec::new();
        for (node, v) in skin.bone_map.iter().enumerate() {
            let slot = f32::from_bits(*v);
            if slot >= 0.0 {
                let slot = slot as usize;
                if slot_bone.len() <= slot {
                    slot_bone.resize(slot + 1, None);
                }
                slot_bone[slot] = names.get(&node).cloned();
            }
        }
        let turn = [0, 1, 2].map(|r| [w[r][0], w[r][1], w[r][2]]);
        let at = |p: [f32; 3]| {
            [0, 1, 2].map(|r| turn[r][0] * p[0] + turn[r][1] * p[1] + turn[r][2] * p[2] + w[r][3])
        };
        let (weights, bones) = (skin.weights.clone(), skin.bones.clone());
        for (i, p) in mesh.positions.iter_mut().enumerate() {
            let q = at(*p);
            let mut delta = [0.0; 3];
            for k in 0..4 {
                let weight = weights.get(i).map_or(0.0, |w| w[k]);
                if weight <= 0.0 {
                    continue;
                }
                let slot = bones.get(i).map_or(-1.0, |b| b[k]);
                let Some(Some(bone)) =
                    (slot >= 0.0).then(|| slot_bone.get(slot as usize).cloned().flatten())
                else {
                    continue;
                };
                let m = bone_move(&bone, q, s, &joints, &bands);
                for a in 0..3 {
                    delta[a] += m[a] * weight;
                }
            }
            // Back into the mesh's own frame: its turn is a pure rotation.
            for k in 0..3 {
                p[k] += turn[0][k] * delta[0] + turn[1][k] * delta[1] + turn[2][k] * delta[2];
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sliders_come_from_a_looks_numbers_and_stay_in_range() {
        let s = BodyShape::from_sliders(|n| match n {
            "height" => 50,
            "waist" => -300,
            _ => 0,
        });
        assert_eq!((s.height, s.waist), (0.5, -1.0));
        assert!(!s.is_plain());
        assert!((s.height_scale() - 1.04).abs() < 1e-6);
        assert!(BodyShape::default().is_plain());
        assert_eq!(BodyShape::default().height_scale(), 1.0);
    }

    #[test]
    fn a_point_is_kept_between_the_ends_of_a_bone() {
        let (a, b) = ([0.0, 0.0, 1.0], [0.0, 0.0, 0.0]);
        assert_eq!(on_segment([0.3, 0.0, 0.5], a, b), [0.0, 0.0, 0.5]);
        assert_eq!(on_segment([0.3, 0.0, 2.0], a, b), [0.0, 0.0, 1.0]);
        assert!(bump(1.0, 1.0, 0.1) == 1.0 && bump(1.2, 1.0, 0.1) == 0.0);
        assert!(ramp(0.0, 0.1, 0.1) == 0.0 && ramp(0.3, 0.1, 0.1) == 1.0);
    }
}
