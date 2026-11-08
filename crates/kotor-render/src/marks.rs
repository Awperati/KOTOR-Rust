//! Scars, tattoos and make-up on a player's head, from the marks of character creation. The game has none to choose, and
//! adding pictures would be new art, so each mark is a shape given in metres on the face at rest, by the game's own points of
//! the face (the `f_*_g` nodes), and painted into the head's texture as it loads. It fits all 30 heads and any resolution.
//!
//! A mark's strength goes from 0, none, to `MARK_MOST`, full. A look keeps marks by name with its sliders, and a texture's
//! name carries the head's marks after `#`, so `Stage::texture` paints each set once.
//!
//! The heads' textures are mirrored: one half of the picture paints both sides of the face. So every mark is the same on
//! both sides, or on the face's middle line. A scar on one cheek cannot be made this way.
//!
//! **Assumed:** the marks' shapes, places and colours, by eye from frames of one head, `PFHA01`. The game's points of the
//! face are not all where the features show: the jaw's is its hinge, and the mouth's middle is above the lips. So the marks
//! go by the brows, the points under the eyes and the mouth's corners.

#![allow(clippy::needless_range_loop)]

use crate::face::{bump, ramp, Face};
use kotor_image::Image;
use kotor_model::{Model, Node};

/// The strongest a mark goes, and the step of the page's arrows.
pub const MARK_MOST: i32 = 100;
pub const MARK_STEP: i32 = 10;

/// The marks: the name a look keeps each by, and its words on the page. In the order of the page.
pub const MARKS: [(&str, &str); 9] = [
    ("scar_nose", "Scar: nose"),
    ("scar_lip", "Scar: lip"),
    ("scar_chin", "Scar: chin"),
    ("tattoo_cheeks", "Tattoo: cheeks"),
    ("tattoo_brow", "Tattoo: brow"),
    ("tattoo_chin", "Tattoo: chin"),
    ("paint_eyes", "Make-up: eyes"),
    ("paint_lips", "Make-up: lips"),
    ("paint_cheeks", "Make-up: cheeks"),
];

/// The hair's colour, kept in a look by this name as a place in `HAIR_COLOURS`: 0 is the head's own.
pub const HAIR_KEY: &str = "hair_colour";
/// The hair's colours: words for the page, and the colour of hair of middling light.
pub const HAIR_COLOURS: [(&str, [f32; 3]); 10] = [
    ("The head's own", [0.0; 3]),
    ("Black", [30.0, 26.0, 26.0]),
    ("Dark brown", [66.0, 44.0, 30.0]),
    ("Brown", [108.0, 72.0, 44.0]),
    ("Auburn", [124.0, 54.0, 34.0]),
    ("Red", [160.0, 56.0, 30.0]),
    ("Blonde", [206.0, 170.0, 104.0]),
    ("Platinum", [226.0, 218.0, 196.0]),
    ("Grey", [136.0, 134.0, 132.0]),
    ("White", [236.0, 236.0, 232.0]),
];
/// Hair is told from skin by which it is nearer: the cheeks' colour or the crown's, the top of the head. Hue counts this many
/// times more than light. A texel is some hair from the first share of the way to the crown's colour, and all hair from
/// the second.
const HAIR_HUE: f32 = 4.0;
const HAIR_APART: (f32, f32) = (0.45, 0.62);
/// How far under the top of the head the crown is sampled, in metres.
const HAIR_CROWN: f32 = 0.03;
/// How far under the brow's point hair goes on the face: the brows are hair, the eyes' lashes are not.
const HAIR_BELOW_BROW: f32 = 0.012;
/// How dark and how light a texel of new hair goes, as a share of the colour, so that strands keep their shading.
const HAIR_SHADE: (f32, f32) = (0.25, 1.8);

/// A body's skin in a race head's tone: the keys of the two heads, and how near in hue a texel must be to the body's own
/// skin to be skin, from all to none, and how light, against that skin, from none to all.
const TONE_FROM: &str = "tone_from";
const TONE_TO: &str = "tone_to";
const SKIN_HUE: (f32, f32) = (0.07, 0.1);
const SKIN_LIGHT: (f32, f32) = (0.12, 0.25);

/// What goes between a texture's name and its marks.
const MARKS_MARK: char = '#';

/// A scar's middle, paler than the skin, and its edge, a little darker: half-widths in metres.
const SCAR_CORE: f32 = 0.0007;
const SCAR_EDGE: f32 = 0.0015;
const SCAR_PALE: [f32; 3] = [226.0, 170.0, 160.0];
const SCAR_DARKEN: f32 = 0.86;
/// Ink: its colour, how much of the skin it covers at full strength, and a line's half-width.
const INK: [f32; 3] = [30.0, 34.0, 56.0];
const INK_COVER: f32 = 0.85;
const INK_LINE: f32 = 0.0011;
/// Make-up's colours and how much they cover at full strength.
const EYE_PAINT: [f32; 3] = [72.0, 40.0, 78.0];
const EYE_COVER: f32 = 0.55;
const LIP_PAINT: [f32; 3] = [150.0, 32.0, 52.0];
const LIP_COVER: f32 = 0.6;
const BLUSH: [f32; 3] = [208.0, 96.0, 96.0];
const BLUSH_COVER: f32 = 0.32;

/// A head's marks for its texture's name: empty for none, else `#` and each mark's name and strength, the hair's colour
/// first.
pub fn marks_suffix(get: impl Fn(&str) -> i32) -> String {
    let hair = get(HAIR_KEY);
    let hair = (1..HAIR_COLOURS.len() as i32)
        .contains(&hair)
        .then(|| format!("{HAIR_KEY}={hair}"));
    let marks: Vec<String> = hair
        .into_iter()
        .chain(
            MARKS
                .iter()
                .map(|(name, _)| (name, get(name).clamp(0, MARK_MOST)))
                .filter(|(_, v)| *v > 0)
                .map(|(name, v)| format!("{name}={v}")),
        )
        .collect();
    if marks.is_empty() {
        String::new()
    } else {
        format!("{MARKS_MARK}{}", marks.join(","))
    }
}

/// A texture's name without its marks, and the marks. A `#` that a mark's name does not follow is part of the name, as a
/// folder of underwear may have one.
pub fn split_marks(name: &str) -> (&str, &str) {
    name.match_indices(MARKS_MARK)
        .map(|(at, _)| at)
        .find(|at| {
            let rest = &name[at + 1..];
            rest.starts_with(HAIR_KEY)
                || rest.starts_with(TONE_FROM)
                || MARKS.iter().any(|(mark, _)| rest.starts_with(mark))
        })
        .map_or((name, ""), |at| (&name[..at], &name[at + 1..]))
}

/// The strength of each mark in `MARKS` from a texture name's marks, each from 0 to 1.
fn strengths(marks: &str) -> [f32; 9] {
    let mut out = [0.0; 9];
    for part in marks.split(',') {
        let Some((name, v)) = part.split_once('=') else {
            continue;
        };
        if let (Some(n), Ok(v)) = (MARKS.iter().position(|m| m.0 == name), v.parse::<i32>()) {
            out[n] = v.clamp(0, MARK_MOST) as f32 / MARK_MOST as f32;
        }
    }
    out
}

/// The hair's colour from a texture name's marks: a place in `HAIR_COLOURS`, 0 for the head's own.
fn hair_choice(marks: &str) -> usize {
    marks
        .split(',')
        .filter_map(|part| part.split_once('='))
        .find(|(name, _)| *name == HAIR_KEY)
        .and_then(|(_, v)| v.parse::<usize>().ok())
        .filter(|v| *v < HAIR_COLOURS.len())
        .unwrap_or(0)
}

fn light(c: [f32; 3]) -> f32 {
    0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
}

/// The hair in a head's texture in another colour. Hair is what is nearer in colour to the crown of the head than to the
/// cheeks, on the top and back of the head and on the brows, so the lips, the eyes and the skin round them keep theirs.
/// Each texel keeps how light it is against the hair's mean, so the strands keep their shading.
fn recolour_hair(img: &mut Image, points: &[Option<[f32; 3]>], at: &Places, colour: [f32; 3]) {
    let texel = |img: &Image, n: usize| [0, 1, 2].map(|k| img.rgba[n * 4 + k] as f32);
    let mean_of = |img: &Image, keep: &dyn Fn([f32; 3]) -> bool| {
        let mut sum = [0.0f32; 3];
        let mut count = 0.0;
        for (n, p) in points.iter().enumerate() {
            if p.is_some_and(keep) {
                let c = texel(img, n);
                for k in 0..3 {
                    sum[k] += c[k];
                }
                count += 1.0;
            }
        }
        (count > 0.0).then(|| sum.map(|v| v / count))
    };
    // The skin: the mean of the cheeks. The hair: the mean of the crown.
    let on_cheek = |p: [f32; 3]| at.on_cheek(p);
    let top = points
        .iter()
        .flatten()
        .fold(f32::MIN, |top, p| top.max(p[2]));
    let (Some(skin), Some(crown)) = (
        mean_of(img, &on_cheek),
        mean_of(img, &|p: [f32; 3]| p[2] > top - HAIR_CROWN),
    ) else {
        return;
    };
    // Hue, so many times over, and light.
    let feature = |c: [f32; 3]| {
        let sum = (c[0] + c[1] + c[2]).max(1.0);
        [
            c[0] / sum * HAIR_HUE,
            c[1] / sum * HAIR_HUE,
            c[2] / sum * HAIR_HUE,
            light(c) / 255.0,
        ]
    };
    let apart =
        |a: [f32; 4], b: [f32; 4]| (0..4).map(|k| (a[k] - b[k]).powi(2)).sum::<f32>().sqrt();
    let (skin, crown) = (feature(skin), feature(crown));
    let hairness: Vec<f32> = points
        .iter()
        .enumerate()
        .map(|(n, p)| {
            let Some(p) = p else { return 0.0 };
            let up = p[2] > at.brow - HAIR_BELOW_BROW;
            let behind = p[1] < at.front;
            if !(up || behind) {
                return 0.0;
            }
            let f = feature(texel(img, n));
            let (to_skin, to_hair) = (apart(f, skin), apart(f, crown));
            let share = to_skin / (to_skin + to_hair).max(1e-4);
            ramp(share, HAIR_APART.0, HAIR_APART.1 - HAIR_APART.0)
        })
        .collect();
    let (mut sum, mut weight) = (0.0, 0.0);
    for (n, h) in hairness.iter().enumerate() {
        if *h > 0.0 {
            sum += light(texel(img, n)) * h;
            weight += h;
        }
    }
    if weight == 0.0 {
        return;
    }
    let mean = (sum / weight).max(1.0);
    for (n, h) in hairness.iter().enumerate() {
        if *h <= 0.0 {
            continue;
        }
        let c = texel(img, n);
        let shade = (light(c) / mean).clamp(HAIR_SHADE.0, HAIR_SHADE.1);
        for k in 0..3 {
            let new = (colour[k] * shade).min(255.0);
            img.rgba[n * 4 + k] = (c[k] + (new - c[k]) * h).round().clamp(0.0, 255.0) as u8;
        }
    }
}

/// How far a point is from a line between two points, across and up only: the face seen from the front.
fn line_distance(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
    let long = dx * dx + dz * dz;
    let t = if long > 0.0 {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dz) / long).clamp(0.0, 1.0)
    } else {
        0.0
    };
    ((p[0] - a[0] - t * dx).powi(2) + (p[1] - a[1] - t * dz).powi(2)).sqrt()
}

/// 1 inside an ellipse's middle, falling smoothly to 0 at its edge.
fn blob(p: [f32; 2], middle: [f32; 2], radius: [f32; 2]) -> f32 {
    let d = ((p[0] - middle[0]) / radius[0]).powi(2) + ((p[1] - middle[1]) / radius[1]).powi(2);
    bump(d.sqrt(), 1.0)
}

/// A line of ink `half` wide, with a soft edge.
fn ink_line(p: [f32; 2], a: [f32; 2], b: [f32; 2], half: f32) -> f32 {
    1.0 - ramp(line_distance(p, a, b), half, half * 0.8)
}

/// The face's places that the marks go by, across and up.
struct Places {
    middle: f32,
    brow: f32,
    /// Under each eye: the one of the lower x first.
    eyes: [[f32; 2]; 2],
    /// The mouth's corners: the one of the lower x first.
    corners: [[f32; 2]; 2],
    /// The line between the lips.
    mouth: f32,
    front: f32,
}

impl Places {
    fn of(f: &Face) -> Places {
        let two = |p: [f32; 3]| [p[0], p[2]];
        let mut eyes = f.under_eyes.map(two);
        eyes.sort_by(|a, b| a[0].total_cmp(&b[0]));
        let mut corners = f.corners.map(two);
        corners.sort_by(|a, b| a[0].total_cmp(&b[0]));
        Places {
            middle: f.middle,
            brow: f.brow[2],
            eyes,
            corners,
            mouth: (corners[0][1] + corners[1][1]) / 2.0,
            front: f.front,
        }
    }

    /// Whether a point is on a cheek, a little out from and under an eye: skin on every head.
    fn on_cheek(&self, p: [f32; 3]) -> bool {
        p[1] > self.front
            && (0..2).any(|e| {
                (p[0] - self.out_to(e, 1.2)).abs() < 0.012
                    && (p[2] - (self.eyes[e][1] - 0.02)).abs() < 0.012
            })
    }

    /// A point `share` of the way out from the face's middle to an eye, across.
    fn out_to(&self, eye: usize, share: f32) -> f32 {
        self.middle + (self.eyes[eye][0] - self.middle) * share
    }
}

/// One texel's colour with the marks on it, at point `p` of the head at rest.
fn marked(base: [f32; 3], p: [f32; 3], s: &[f32; 9], at: &Places) -> [f32; 3] {
    let face = ramp(p[1], at.front, 0.03);
    if face <= 0.0 {
        return base;
    }
    let q = [p[0], p[2]];
    let mut c = base;
    let over = |c: &mut [f32; 3], colour: [f32; 3], a: f32| {
        let a = (a * face).clamp(0.0, 1.0);
        for k in 0..3 {
            c[k] += (colour[k] - c[k]) * a;
        }
    };
    let m = at.middle;
    let eye_line = (at.eyes[0][1] + at.eyes[1][1]) / 2.0;
    // Make-up first, under the scars and ink.
    if s[6] > 0.0 {
        for eye in at.eyes {
            let a = blob(q, [eye[0], eye[1] + 0.021], [0.017, 0.007]);
            over(&mut c, EYE_PAINT, a * EYE_COVER * s[6]);
        }
    }
    if s[8] > 0.0 {
        for n in 0..2 {
            let a = blob(
                q,
                [at.out_to(n, 1.35), at.eyes[n][1] - 0.022],
                [0.018, 0.016],
            );
            over(&mut c, BLUSH, a * BLUSH_COVER * s[8]);
        }
    }
    if s[7] > 0.0 {
        let (l, r) = (at.corners[0], at.corners[1]);
        let a = blob(q, [m, at.mouth], [(r[0] - l[0]) / 2.0 * 1.05, 0.0065]);
        over(&mut c, LIP_PAINT, a * LIP_COVER * s[7]);
    }
    // Ink: three bars under each eye, a line up the forehead with a dot, and three down the chin.
    if s[3] > 0.0 {
        for n in 0..2 {
            for k in 0..3 {
                let x = at.out_to(n, 0.85 + 0.22 * k as f32);
                let lean = (at.eyes[n][0] - m).signum() * 0.003;
                let (top, low) = (at.eyes[n][1] - 0.003, at.eyes[n][1] - 0.024);
                let a = ink_line(q, [x, top], [x + lean, low], INK_LINE);
                over(&mut c, INK, a * INK_COVER * s[3]);
            }
        }
    }
    if s[4] > 0.0 {
        let line = ink_line(q, [m, at.brow + 0.006], [m, at.brow + 0.026], INK_LINE);
        let from_dot = ((q[0] - m).powi(2) + (q[1] - at.brow - 0.032).powi(2)).sqrt();
        let dot = 1.0 - ramp(from_dot, 0.0025, 0.0015);
        over(&mut c, INK, line.max(dot) * INK_COVER * s[4]);
    }
    if s[5] > 0.0 {
        let (top, low) = (at.mouth - 0.014, at.mouth - 0.034);
        let mut a = ink_line(q, [m, top], [m, low], INK_LINE * 1.4);
        for side in [-1.0, 1.0] {
            let (a1, b1) = (
                [m + side * 0.008, top - 0.002],
                [m + side * 0.007, low + 0.004],
            );
            a = a.max(ink_line(q, a1, b1, INK_LINE));
        }
        over(&mut c, INK, a * INK_COVER * s[5]);
    }
    // Scars last: a pale middle and a darker edge.
    let scar = |c: &mut [f32; 3], a: [f32; 2], b: [f32; 2], strength: f32| {
        if strength <= 0.0 {
            return;
        }
        let d = line_distance(q, a, b);
        let edge =
            (1.0 - ramp(d, SCAR_EDGE, SCAR_EDGE * 0.6)) * (1.0 - SCAR_DARKEN) * strength * face;
        for k in 0..3 {
            c[k] *= 1.0 - edge;
        }
        let core = (1.0 - ramp(d, SCAR_CORE, SCAR_CORE)) * strength * face;
        for k in 0..3 {
            let pale = (c[k] + SCAR_PALE[k]) / 2.0;
            c[k] += (pale - c[k]) * core;
        }
    };
    // Across the bridge of the nose, down through the lips, and across the chin.
    let bridge = eye_line + 0.006;
    scar(&mut c, [m - 0.011, bridge], [m + 0.011, bridge], s[0]);
    scar(&mut c, [m, at.mouth + 0.012], [m, at.mouth - 0.011], s[1]);
    let chin = at.mouth - 0.024;
    scar(&mut c, [m - 0.009, chin], [m + 0.009, chin], s[2]);
    c
}

/// Paint a head's marks into its texture, `head` being the head model and `texture` the texture's own name. A head without
/// the game's points of the face is left as it is.
pub fn paint_marks(head: &Model, texture: &str, img: &mut Image, marks: &str) {
    let s = strengths(marks);
    let hair = hair_choice(marks);
    if s.iter().all(|v| *v <= 0.0) && hair == 0 {
        return;
    }
    let Some(face) = Face::of(head) else {
        return;
    };
    let places = Places::of(&face);
    let Some(point) = texel_points(head, texture, img) else {
        return;
    };
    if hair > 0 {
        recolour_hair(img, &point, &places, HAIR_COLOURS[hair].1);
    }
    if s.iter().all(|v| *v <= 0.0) {
        return;
    }
    for (n, p) in point.into_iter().enumerate() {
        let Some(p) = p else { continue };
        let at = n * 4;
        let base = [0, 1, 2].map(|k| img.rgba[at + k] as f32);
        let c = marked(base, p, &s, &places);
        for k in 0..3 {
            img.rgba[at + k] = c[k].round().clamp(0.0, 255.0) as u8;
        }
    }
}

/// The skin of a head's cheeks: the mean colour of its texture there.
pub fn cheek_tone(head: &Model, texture: &str, img: &Image) -> Option<[f32; 3]> {
    let places = Places::of(&Face::of(head)?);
    let points = texel_points(head, texture, img)?;
    let mut sum = [0.0f32; 3];
    let mut count = 0.0;
    for (n, p) in points.iter().enumerate() {
        let Some(p) = p else { continue };
        if places.on_cheek(*p) {
            for k in 0..3 {
                sum[k] += img.rgba[n * 4 + k] as f32;
            }
            count += 1.0;
        }
    }
    (count > 0.0).then(|| sum.map(|v| v / count))
}

/// The names of the two heads whose tones a body's skin goes between, from a texture name's marks: `tone_from` is the
/// head the body was painted for, `tone_to` the race head it now wears.
pub fn tone_names(marks: &str) -> Option<(&str, &str)> {
    let get = |key: &str| {
        marks
            .split(',')
            .filter_map(|part| part.split_once('='))
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v)
    };
    Some((get(TONE_FROM)?, get(TONE_TO)?))
}

/// The marks of a body's texture name for a race head: from the player's own head's texture to the race head's.
pub fn tone_suffix(from: &str, to: &str) -> String {
    format!("{TONE_FROM}={from},{TONE_TO}={to}")
}

/// A body's skin moved from one tone to another. Skin is what has the hue of `from` and is not much darker: cloth and
/// boots keep theirs. Each texel keeps how light it is against `from`, so the body's shading stays.
pub fn tint_skin(img: &mut Image, from: [f32; 3], to: [f32; 3]) {
    let hue = |c: [f32; 3]| {
        let sum = (c[0] + c[1] + c[2]).max(1.0);
        c.map(|v| v / sum)
    };
    let (from_hue, from_light) = (hue(from), light(from).max(1.0));
    let pixels = img.rgba.len() / 4;
    for n in 0..pixels {
        let c = [0, 1, 2].map(|k| img.rgba[n * 4 + k] as f32);
        let h = hue(c);
        let apart = ((0..3).map(|k| (h[k] - from_hue[k]).powi(2)).sum::<f32>()).sqrt();
        let ratio = light(c) / from_light;
        let skin = (1.0 - ramp(apart, SKIN_HUE.0, SKIN_HUE.1 - SKIN_HUE.0))
            * ramp(ratio, SKIN_LIGHT.0, SKIN_LIGHT.1 - SKIN_LIGHT.0);
        if skin <= 0.0 {
            continue;
        }
        for k in 0..3 {
            let new = (to[k] * ratio).min(255.0);
            img.rgba[n * 4 + k] = (c[k] + (new - c[k]) * skin).round().clamp(0.0, 255.0) as u8;
        }
    }
}

/// For each texel of a texture, the point of the model at rest that covers it, across, forward and up, from the meshes
/// with that texture. Where the picture is shared, the most forward point wins, as the face is what is marked.
fn texel_points(head: &Model, texture: &str, img: &Image) -> Option<Vec<Option<[f32; 3]>>> {
    let world = head.world_matrices(None);
    let (w, h) = (img.width as usize, img.height as usize);
    if w == 0 || h == 0 || img.rgba.len() < w * h * 4 {
        return None;
    }
    let mut point: Vec<Option<[f32; 3]>> = vec![None; w * h];
    let mut depth = vec![f32::MIN; w * h];
    fn each(n: &Node, f: &mut dyn FnMut(&Node)) {
        f(n);
        for c in &n.children {
            each(c, f);
        }
    }
    each(&head.root, &mut |n| {
        let Some(mesh) = &n.mesh else { return };
        if !mesh
            .texture
            .as_deref()
            .is_some_and(|t| t.eq_ignore_ascii_case(texture))
        {
            return;
        }
        // The eyeballs have their own part of the picture, and no mark goes on them. Their lids do take make-up.
        let lower = n.name.to_ascii_lowercase();
        if lower.starts_with("eye") && !lower.contains("lid") {
            return;
        }
        let Some(m) = world
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(&n.name))
            .map(|x| x.1)
        else {
            return;
        };
        let at = |p: [f32; 3]| {
            [0, 1, 2].map(|r| m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2] + m[r][3])
        };
        for f in &mesh.faces {
            let i = f.indices.map(|v| v as usize);
            let (Some(a), Some(b), Some(c)) =
                (mesh.uvs.get(i[0]), mesh.uvs.get(i[1]), mesh.uvs.get(i[2]))
            else {
                continue;
            };
            let (Some(pa), Some(pb), Some(pc)) = (
                mesh.positions.get(i[0]),
                mesh.positions.get(i[1]),
                mesh.positions.get(i[2]),
            ) else {
                continue;
            };
            let (pa, pb, pc) = (at(*pa), at(*pb), at(*pc));
            let to_texel = |uv: &[f32; 2]| (uv[0] * w as f32, (1.0 - uv[1]) * h as f32);
            let ((ax, ay), (bx, by), (cx, cy)) = (to_texel(a), to_texel(b), to_texel(c));
            let d = (by - cy) * (ax - cx) + (cx - bx) * (ay - cy);
            if d.abs() < 1e-6 {
                continue;
            }
            let x0 = ax.min(bx).min(cx).floor().max(0.0) as usize;
            let x1 = (ax.max(bx).max(cx).ceil().max(0.0) as usize).min(w - 1);
            let y0 = ay.min(by).min(cy).floor().max(0.0) as usize;
            let y1 = (ay.max(by).max(cy).ceil().max(0.0) as usize).min(h - 1);
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let u = ((by - cy) * (px - cx) + (cx - bx) * (py - cy)) / d;
                    let v = ((cy - ay) * (px - cx) + (ax - cx) * (py - cy)) / d;
                    let t = 1.0 - u - v;
                    // A little past the edge, so that no seam shows.
                    if u < -0.02 || v < -0.02 || t < -0.02 {
                        continue;
                    }
                    let p = [0, 1, 2].map(|k| u * pa[k] + v * pb[k] + t * pc[k]);
                    let n = y * w + x;
                    // The most forward point wins where the texture is shared, as the face is what is marked.
                    if p[1] > depth[n] {
                        depth[n] = p[1];
                        point[n] = Some(p);
                    }
                }
            }
        }
    });
    Some(point)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_go_in_a_texture_name_and_come_back_in_range() {
        let suffix = marks_suffix(|n| match n {
            "scar_nose" => 60,
            "paint_lips" => 500,
            "tattoo_chin" => -20,
            _ => 0,
        });
        assert_eq!(suffix, "#scar_nose=60,paint_lips=100");
        assert_eq!(marks_suffix(|_| 0), "");
        let name = format!("pfha01@blue{suffix}");
        assert_eq!(
            split_marks(&name),
            ("pfha01@blue", "scar_nose=60,paint_lips=100")
        );
        assert_eq!(split_marks("pfha01"), ("pfha01", ""));
        assert_eq!(split_marks("pack:A#B"), ("pack:A#B", ""));
        assert_eq!(
            split_marks("pack:A#B#scar_lip=10"),
            ("pack:A#B", "scar_lip=10")
        );
        let s = strengths("scar_nose=60,paint_lips=100,nonsense=5");
        assert_eq!((s[0], s[7], s[1]), (0.6, 1.0, 0.0));
        // The hair's colour goes first, and alone it is marks too.
        let hair = marks_suffix(|n| match n {
            HAIR_KEY => 6,
            "scar_lip" => 20,
            _ => 0,
        });
        assert_eq!(hair, "#hair_colour=6,scar_lip=20");
        assert_eq!(
            split_marks(&format!("pfha01{hair}")).1,
            "hair_colour=6,scar_lip=20"
        );
        assert_eq!(hair_choice("hair_colour=6,scar_lip=20"), 6);
        assert_eq!(hair_choice("hair_colour=99"), 0);
        assert_eq!(marks_suffix(|n| if n == HAIR_KEY { 99 } else { 0 }), "");
    }

    #[test]
    fn a_line_is_near_along_it_and_far_beside_it() {
        assert!(line_distance([0.5, 0.0], [0.0, 0.0], [1.0, 0.0]) < 1e-6);
        assert!((line_distance([2.0, 0.0], [0.0, 0.0], [1.0, 0.0]) - 1.0).abs() < 1e-6);
        assert_eq!(blob([0.0, 0.0], [0.0, 0.0], [1.0, 1.0]), 1.0);
        assert_eq!(blob([2.0, 0.0], [0.0, 0.0], [1.0, 1.0]), 0.0);
    }
}
