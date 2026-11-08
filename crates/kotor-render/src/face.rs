//! A player's head reshaped by the sliders of character creation: brow, nose, jaw, cheeks, eyes and mouth. The owner: "I want the
//! character creation to be complex and detailed like the skyrim one". The heads have no morph targets, but each carries the
//! game's own points of its face, the `f_*_g` nodes its mouth and brows move by: the brows, the nostrils, the mouth's middle and
//! corners, the jaw and under each eye. Each slider moves the vertices near its points, at rest and once when the head is loaded,
//! so every pose and every word spoken shows it. The eyes and their lids are meshes of their own and move with the eye slider.
//!
//! **Assumed:** how far each slider goes at its ends and how far round its points it reaches, by eye from frames, kept to a few
//! millimetres: a head has about 600 to 1,000 vertices, so the shapes are broad, and the painted face stretches with them.

use crate::shape::SLIDER_MOST;
use kotor_model::{Model, Node};

/// How far each slider goes at its end, in metres or as a share of the face's width.
const BROW_FORWARD: f32 = 0.006;
const BROW_UP: f32 = 0.003;
const NOSE_FORWARD: f32 = 0.007;
const NOSE_WIDE: f32 = 0.12;
const JAW_WIDE: f32 = 0.12;
const CHIN_FORWARD: f32 = 0.006;
const CHEEK_OUT: f32 = 0.005;
const CHEEK_FORWARD: f32 = 0.003;
const EYES_APART: f32 = 0.003;
const MOUTH_WIDE: f32 = 0.15;
const LIPS_FORWARD: f32 = 0.002;
/// How far round its points each slider reaches, in metres.
const BROW_REACH: (f32, f32) = (0.05, 0.025);
const NOSE_REACH: f32 = 0.025;
const MOUTH_REACH: f32 = 0.03;
const JAW_REACH: f32 = 0.03;
const CHEEK_REACH: f32 = 0.03;
const EYE_REACH: f32 = 0.025;

/// The sliders, each from -1 to 1. Zero is the game's own head.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FaceShape {
    pub brow: f32,
    pub nose: f32,
    pub jaw: f32,
    pub cheeks: f32,
    pub eyes: f32,
    pub mouth: f32,
}

/// The sliders' names, as a look keeps them, in the order of the page.
pub const FACE_SLIDERS: [&str; 6] = ["brow", "nose", "jaw", "cheeks", "eyes", "mouth"];

impl FaceShape {
    /// From a look's numbers by name, each from `-SLIDER_MOST` to `SLIDER_MOST`.
    pub fn from_sliders(get: impl Fn(&str) -> i32) -> FaceShape {
        let v = |name: &str| get(name).clamp(-SLIDER_MOST, SLIDER_MOST) as f32 / SLIDER_MOST as f32;
        FaceShape {
            brow: v("brow"),
            nose: v("nose"),
            jaw: v("jaw"),
            cheeks: v("cheeks"),
            eyes: v("eyes"),
            mouth: v("mouth"),
        }
    }

    /// Whether the head is the game's own.
    pub fn is_plain(&self) -> bool {
        *self == FaceShape::default()
    }
}

/// A smooth hump: 1 at 0, 0 from `reach` away.
pub(crate) fn bump(d: f32, reach: f32) -> f32 {
    let t = (d.abs() / reach.max(1e-4)).min(1.0);
    0.5 + 0.5 * (t * std::f32::consts::PI).cos()
}

/// 0 below `from`, 1 from `from + over`, smooth between.
pub(crate) fn ramp(x: f32, from: f32, over: f32) -> f32 {
    let t = ((x - from) / over.max(1e-4)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn mid(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        (a[0] + b[0]) / 2.0,
        (a[1] + b[1]) / 2.0,
        (a[2] + b[2]) / 2.0,
    ]
}

/// The face's points at rest, from the head's own nodes. Across is x, forward is y and up is z.
pub(crate) struct Face {
    /// The middle of the face across: the heads are not all centred on 0.
    pub(crate) middle: f32,
    pub(crate) brow: [f32; 3],
    pub(crate) nose: [f32; 3],
    pub(crate) mouth: [f32; 3],
    /// The mouth's corners, left and then right as the game names them.
    pub(crate) corners: [[f32; 3]; 2],
    pub(crate) jaw: [f32; 3],
    /// Under each eye, left and then right as the game names them.
    pub(crate) under_eyes: [[f32; 3]; 2],
    /// From this far forward a vertex is of the face, not the back of the head.
    pub(crate) front: f32,
}

impl Face {
    pub(crate) fn of(model: &Model) -> Option<Face> {
        let points: Vec<(String, [f32; 3])> = model
            .world_matrices(None)
            .into_iter()
            .map(|(n, m)| (n.to_ascii_lowercase(), [m[0][3], m[1][3], m[2][3]]))
            .collect();
        let at = |name: &str| points.iter().find(|(n, _)| n == name).map(|p| p.1);
        let brow = at("f_mdbrw_g")?;
        let nose = mid(at("f_lns_g")?, at("f_rns_g")?);
        let corners = [at("f_lmc_g")?, at("f_rmc_g")?];
        let under_eyes = [at("f_llweye_g")?, at("f_rlweye_g")?];
        Some(Face {
            middle: brow[0],
            brow,
            nose,
            mouth: at("f_um_g")?,
            corners,
            jaw: at("f_jaw_g")?,
            under_eyes,
            front: nose[1] - 0.08,
        })
    }
}

/// How far the sliders move a point of the head at rest. `eye_part`: a point of an eye or its lid, which moves with it whole.
fn face_move(q: [f32; 3], s: &FaceShape, f: &Face, eye_part: bool) -> [f32; 3] {
    let mut out = [0.0; 3];
    let across = q[0] - f.middle;
    let side = across.signum();
    let face = ramp(q[1], f.front, 0.03);
    // The eyes: wider apart, or closer, with what is round them.
    let eye = f.under_eyes[if side < 0.0 { 0 } else { 1 }];
    let eye_middle = [eye[0], eye[1] - 0.01, eye[2] + 0.02];
    let near_eye = if eye_part {
        1.0
    } else {
        bump(dist(q, eye_middle), EYE_REACH) * face
    };
    out[0] += side * s.eyes * EYES_APART * near_eye;
    if eye_part {
        return out;
    }
    // The brow: forward and up along its ridge.
    let brow = bump(q[2] - f.brow[2], BROW_REACH.1) * bump(across, BROW_REACH.0) * face;
    out[1] += s.brow * BROW_FORWARD * brow;
    out[2] += s.brow * BROW_UP * brow;
    // The nose: from the nostrils up the bridge, out at the front, and wider.
    let up_nose = ramp(q[2], f.nose[2] - 0.012, 0.01) * (1.0 - ramp(q[2], f.brow[2] - 0.03, 0.015));
    let tip = ramp(q[1], f.nose[1] - 0.025, 0.015);
    let nose = bump(across, NOSE_REACH) * up_nose * tip;
    out[1] += s.nose * NOSE_FORWARD * nose;
    out[0] += across * s.nose * NOSE_WIDE * nose;
    // The mouth: wider at the corners, the lips a little forward.
    let lips = mid(f.corners[0], f.corners[1]);
    let mouth = bump(
        dist(q, [lips[0], lips[1], (lips[2] + f.mouth[2]) / 2.0]),
        MOUTH_REACH,
    ) * face;
    out[0] += across * s.mouth * MOUTH_WIDE * mouth;
    out[1] += s.mouth * LIPS_FORWARD * mouth;
    // The jaw: below the mouth, wider, and the chin forward. Under the chin it fades out, so that the neck still meets the body.
    let below = ramp(f.mouth[2] - q[2], 0.0, JAW_REACH) * (1.0 - ramp(f.jaw[2] - q[2], 0.0, 0.02));
    out[0] += across * s.jaw * JAW_WIDE * below;
    out[1] += s.jaw * CHIN_FORWARD * below * bump(q[0] - f.jaw[0], 0.03) * face;
    // The cheeks: out at the sides under the eyes.
    let cheek = [
        f.middle + (eye[0] - f.middle) * 1.4,
        eye[1] - 0.01,
        eye[2] - 0.02,
    ];
    let cheeks = bump(dist(q, cheek), CHEEK_REACH);
    out[0] += side * s.cheeks * CHEEK_OUT * cheeks;
    out[1] += s.cheeks * CHEEK_FORWARD * cheeks;
    out
}

/// Reshape a head at rest by the sliders. A head without the game's face points is left as it is.
pub fn shape_face(model: &mut Model, s: &FaceShape) {
    if s.is_plain() {
        return;
    }
    let Some(face) = Face::of(model) else {
        return;
    };
    let world = model.world_matrices(None);
    fn each_mesh(n: &mut Node, f: &mut dyn FnMut(&str, &mut kotor_model::Mesh)) {
        if let Some(m) = n.mesh.as_mut() {
            f(&n.name, m);
        }
        for c in &mut n.children {
            each_mesh(c, f);
        }
    }
    each_mesh(&mut model.root, &mut |name, mesh| {
        let lower = name.to_ascii_lowercase();
        // The game's own points of the face, and the neck, stay where they are.
        if lower.starts_with("f_") || lower.starts_with("neck") || lower == "head_g" {
            return;
        }
        let eye_part = lower.starts_with("eye");
        let Some(w) = world
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|x| x.1)
        else {
            return;
        };
        let turn = [0, 1, 2].map(|r| [w[r][0], w[r][1], w[r][2]]);
        let at = |p: [f32; 3]| {
            [0, 1, 2].map(|r| turn[r][0] * p[0] + turn[r][1] * p[1] + turn[r][2] * p[2] + w[r][3])
        };
        for p in &mut mesh.positions {
            let delta = face_move(at(*p), s, &face, eye_part);
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
    fn the_face_sliders_come_from_a_looks_numbers_and_stay_in_range() {
        let s = FaceShape::from_sliders(|n| match n {
            "nose" => 30,
            "jaw" => -500,
            _ => 0,
        });
        assert_eq!((s.nose, s.jaw, s.brow), (0.3, -1.0, 0.0));
        assert!(!s.is_plain());
        assert!(FaceShape::default().is_plain());
        assert_eq!(bump(0.0, 0.1), 1.0);
        assert_eq!(bump(0.2, 0.1), 0.0);
        assert_eq!(ramp(1.0, 0.0, 0.5), 1.0);
    }
}
