//! "Underwear style: Conventional", the owner's own taste, off the game's look. The player's bare bodies wear a sports top
//! painted on one closed skin: there is no mesh of cloth to cut, and taking triangles away would open holes. So the rule
//! runs on the texture as it loads, in memory: cloth outside a cup, a band and two straps becomes the skin round it.
//!
//! The shape is given in metres on the body at rest, so it needs no drawing per texture and fits any skin and resolution.
//!
//! The skin must not show where it was painted. Skin close to cloth is tinted by it, so that goes too, and what stays is
//! the edge of a smooth fill: each new texel is the mean of its neighbours, the neighbours of an edge of the sheet being
//! those it meets on the body. So the fill arrives at the skin round it with no step, also across the sheet's seams, and
//! carries the light and shade of that skin. Grain comes from clean skin of the same picture. A belly under cloth was never
//! painted, so it borrows the muscles of the one body whose belly is bare, as light and dark only, whatever the skin. What
//! was under the top elsewhere takes its light and dark from the body's own shape. The edge of the bra stays crisp.
//!
//! A head's neck was always under a collar, and its tone is the head's own, so heads of one skin differ a lot. Its lowest
//! part now fades to the tone of the body's skin, and the body's fill starts from that same tone at the neck.
//! **Adults only:** this names the player's three female bodies, `PFBAS`, `PFBAM` and `PFBAL`, and nothing else.
//! Nothing is written to the install or the repository.

#![allow(clippy::needless_range_loop)]

use crate::load_texture;
use kotor_archive::Install;
use kotor_core::ResType;
use kotor_image::Image;
use kotor_model::{Mesh, Model};

/// Where the fill and the masks are worked out, whatever the texture's own size.
const WORK: usize = 512;
const NOTHING: u8 = 0;
/// A texel of the torso that is left as it is.
const KEEP: u8 = 1;
/// Cloth that stays as a cup.
const CUP: u8 = 5;
/// Cloth to take away.
const CLEAR: u8 = 2;
/// Skin to paint cloth on, for a band or strap where there is none.
const PAINT: u8 = 3;
/// A texel of skin, which the fill grows from.
const SKIN: u8 = 4;

/// Above this share of the torso's height the rule applies. What is lower stays as it is.
const WAIST: f32 = 0.726;
/// Texels of skin beside cloth that are repainted, as the cloth tints them: more than a soft edge reaches.
const MARGIN: usize = 5;
/// Passes of the fill, and how far each overshoots to arrive sooner.
const FILL_PASSES: usize = 300;
const OVERSHOOT: f32 = 1.85;
/// Rounds of softening, a texel each: where the fill meets skin, wide, as nothing should show there; and the bra's edge,
/// which rounds off the texels' steps, then is sharpened to `CLOTH_RAMP` times as steep.
const FEATHER: usize = 4;
const CLOTH_SOFTEN: usize = 2;
const CLOTH_RAMP: f32 = 5.0;
/// Share of the torso's height from which it is the neck's ring, and how strongly the shape of the body shades the fill.
const NECK_RING: f32 = 0.985;
/// From this share of the torso's height up, any cloth left on the sheet is the old top's collar, not the bra.
const NECK_CLOTH: f32 = 0.96;
const SHAPE_SHADE: f32 = 0.12;
const SHAPE_MOST: f32 = 0.5;
/// How near two edges of the body are to be one seam, in metres.
const SEAM_METRES: f32 = 0.03;
/// Patches of grain, in texels, and how far a texel is from the smooth skin it counts as grain against.
const GRAIN_BLOCK: usize = 24;
const GRAIN_BLUR: usize = 3;
/// The bare belly that is borrowed, as cells across the body and up it, and how far a cell's own light is measured against
/// what is round it. Near kept skin it fades out over `RELIEF_FADE` texels, so that the fill meets it with no step.
const BARE_BELLY: &str = "pfbal";
const MIDDLE_BODY: &str = "pfbam";
/// How far across the front of a belly a line down the middle is looked for, as a share of the torso's half width, and in
/// how many strips.
const BELLY_ACROSS: f32 = 0.45;
const BELLY_STRIPS: usize = 16;
/// Where on the upper arms the skin's tone is read, in metres up from the feet.
const TONE_LOW: f32 = 1.0;
const TONE_HIGH: f32 = 1.2;
/// How far up a head's neck the change reaches, and how much of its lowest part counts as what meets the body, in metres.
const NECK_FADE_METRES: f32 = 0.07;
/// The back of the neck, seen from behind in a conversation, holds the body's tone this far up before it fades, where the front's chin
/// keeps its own skin; it takes over starting behind `NECK_FRONT_METRES` towards the face and full `NECK_BACK_OUT_METRES` further back (the head model faces +y).
const NECK_BACK_HOLD_METRES: f32 = 0.08;
const NECK_BACK_FADE_METRES: f32 = 0.05;
const NECK_FRONT_METRES: f32 = 0.07;
const NECK_BACK_OUT_METRES: f32 = 0.03;
const NECK_RING_METRES: f32 = 0.012;
/// How far down the neck reaches, and how much of it stretches to do that.
const NECK_REACH_METRES: f32 = 0.04;
const NECK_STRETCH_METRES: f32 = 0.02;
/// How much stronger than measured the borrowed belly is laid on the others.
const BELLY_GAIN: f32 = 1.8;
/// How far up the bare body's light and dark is borrowed: the belly, and the upper back and chest above it, to the shoulders'
/// roots. The skin there was a smooth fill beside arms and legs with their painted shading.
const RELIEF_TOP: f32 = 0.95;
const RELIEF_WIDE: usize = 64;
const RELIEF_TALL: usize = 96;
const RELIEF_ACROSS: i32 = 8;
const RELIEF_UP: i32 = 30;
const RELIEF_TRIM: usize = 2;
const RELIEF_FADE: f32 = 16.0;
/// Not a part of any piece of the sheet.
const NO_ISLAND: u32 = u32::MAX;
/// How close two vertices are to be one point of the body, and rounds of evening out its shading.
const WELD_METRES: f32 = 0.015;
const SOFTEN_ROUNDS: usize = 12;
/// How near two points that the mesh does not join shade alike: the old top stands off the skin by about this much.
const STEP_METRES: f32 = 0.035;
/// Over how much of the torso's height a belly's shading blends into the rest.
const BELLY_BLEND: f32 = 0.02;
/// How far from every colour of the cups, summed over red, green and blue, a strap's cloth is another garment's trim.
const TRIM_LEVELS: f32 = 90.0;

/// The body a texture is painted for: `pfbam01`, `pfbamb03`, `pfbasa02`, but no armour's.
pub fn body_of_texture(name: &str) -> Option<String> {
    let n = name.to_ascii_lowercase();
    let rest = n.strip_prefix("pfba")?;
    let size = rest.chars().next().filter(|c| "sml".contains(*c))?;
    let rest = &rest[1..];
    let rest = rest.strip_prefix(['a', 'b']).unwrap_or(rest);
    (rest.len() == 2 && rest.bytes().all(|b| b.is_ascii_digit())).then(|| format!("pfba{size}"))
}

/// How far the painted bra's cups stand out from a flat chest, as a share of the torso's half width, and where they are: their
/// centres' height and distance from the middle, and how far each reaches, all as shares of the torso's height and half width.
const CHEST_OUT: f32 = 0.11;
/// How far the cups stand out from the chest under them, in metres, on every body: the small body's, which looked best in profile.
const CHEST_PROJECTION: f32 = 0.034;
const CHEST_HEIGHT: f32 = 0.873;
const CHEST_HEIGHT_REACH: f32 = 0.085;
const CHEST_ACROSS: f32 = 0.30;
const CHEST_ACROSS_REACH: f32 = 0.32;

/// The base mesh of a woman's torso is flat where the cups are painted, so the bra lay on a board and, in profile, stood off
/// the skin. The chest vertices are pushed forward and a little out to the sides by a smooth bump under each cup, at load.
/// Nothing of the game's files changes. Call before [`soften_shading`], which takes the normals from the shape.
pub fn shape_chest(m: &mut Model) {
    let world = m.world_matrices(None);
    let Some(w) = world
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("Torso"))
        .map(|x| x.1)
    else {
        return;
    };
    let turn = [0, 1, 2].map(|r| [w[r][0], w[r][1], w[r][2]]);
    let at = |p: &[f32; 3]| [0, 1, 2].map(|k| dot3(turn[k], *p) + w[k][3]);
    fn find_mut<'a>(n: &'a mut kotor_model::Node, name: &str) -> Option<&'a mut kotor_model::Node> {
        if n.name.eq_ignore_ascii_case(name) {
            return Some(n);
        }
        n.children.iter_mut().find_map(|c| find_mut(c, name))
    }
    let Some(mesh) = find_mut(&mut m.root, "Torso").and_then(|n| n.mesh.as_mut()) else {
        return;
    };
    let (top, half) = torso_size(mesh, at);
    if top <= 0.0 || half <= 0.0 {
        return;
    }
    let bump = |x: f32, centre: f32, reach: f32| {
        let t = ((x - centre).abs() / reach).min(1.0);
        0.5 + 0.5 * (t * std::f32::consts::PI).cos()
    };
    // The bodies stand out by different amounts under the cups to begin with (1.6 to 2.0 cm); each gets what brings it to the
    // same projection, a bump raising the front by four fifths of its own height.
    let (mut band, mut cup) = (f32::MIN, f32::MIN);
    for p in &mesh.positions {
        let q = at(p);
        let (h, side) = (q[2] / top, q[0].abs() / half);
        if (0.80..0.83).contains(&h) && side < 0.5 {
            band = band.max(q[1]);
        }
        if (0.86..0.89).contains(&h) && (0.1..0.5).contains(&side) {
            cup = cup.max(q[1]);
        }
    }
    let wanted = if band > f32::MIN && cup > f32::MIN {
        ((CHEST_PROJECTION - (cup - band)) / 0.8).clamp(0.0, 0.04)
    } else {
        CHEST_OUT * half
    };
    for p in &mut mesh.positions {
        let q = at(p);
        let (h, side) = (q[2] / top, q[0].abs() / half);
        let front = (q[1] / 0.02).clamp(0.0, 1.0);
        let weight = bump(h, CHEST_HEIGHT, CHEST_HEIGHT_REACH)
            * bump(side, CHEST_ACROSS, CHEST_ACROSS_REACH)
            * front;
        if weight <= 0.0 {
            continue;
        }
        let out = wanted * weight;
        // Forward, and a little away from the middle, in the world; back into the mesh's own frame, a pure turn.
        let delta = [q[0].signum() * 0.25 * out, out, 0.0];
        for k in 0..3 {
            p[k] += dot3([turn[0][k], turn[1][k], turn[2][k]], delta);
        }
    }
}

/// The sports top is modelled as well as painted: its hems and collar are ridges of the skin's shading, which would show
/// as lines on bare skin. Even out the shading of the arms and the torso above the waist. The shape stays as it is.
pub fn soften_shading(m: &mut Model) {
    let world = m.world_matrices(None);
    let turn_of = |name: &str| {
        world
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map_or(IDENTITY_TURN, |(_, w)| {
                [0, 1, 2].map(|r| [w[r][0], w[r][1], w[r][2]])
            })
    };
    let shift_of = |name: &str| {
        world
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map_or([0.0; 3], |(_, w)| [w[0][3], w[1][3], w[2][3]])
    };
    let names = ["Torso", "RArm", "LArm"];
    let top = m
        .root
        .find("Torso")
        .and_then(|n| n.mesh.as_ref())
        .map_or(1.0, |mesh| {
            let (turn, shift) = (turn_of("Torso"), shift_of("Torso"));
            let high = |p: &[f32; 3]| dot3(turn[2], *p) + shift[2];
            mesh.positions.iter().map(high).fold(0.0, f32::max)
        });
    // Each vertex of the three, as one point of the body, with the normal there.
    let mut points: std::collections::BTreeMap<[i32; 3], usize> = Default::default();
    let mut normal: Vec<[f32; 3]> = Vec::new();
    let mut soft: Vec<bool> = Vec::new();
    let mut point_of: Vec<Vec<usize>> = Vec::new();
    let mut point_at: Vec<[f32; 3]> = Vec::new();
    let mut from_torso: Vec<bool> = Vec::new();
    let mut joins: Vec<(usize, usize)> = Vec::new();
    for name in names {
        let (turn, shift) = (turn_of(name), shift_of(name));
        let Some(mesh) = m.root.find(name).and_then(|n| n.mesh.as_ref()) else {
            point_of.push(Vec::new());
            continue;
        };
        let mut here = Vec::with_capacity(mesh.positions.len());
        for (i, p) in mesh.positions.iter().enumerate() {
            let at = [0, 1, 2].map(|k| dot3(turn[k], *p) + shift[k]);
            let key = at.map(|v| (v / WELD_METRES).round() as i32);
            let next = normal.len();
            let id = *points.entry(key).or_insert(next);
            if id == next {
                normal.push([0.0; 3]);
                point_at.push(at);
                from_torso.push(name == "Torso");
                soft.push(if name == "Torso" {
                    at[2] / top >= WAIST
                } else {
                    at[2] >= TONE_LOW
                });
            }
            let n = mesh.normals.get(i).copied().unwrap_or([0.0, 0.0, 1.0]);
            let w = [0, 1, 2].map(|k| dot3(turn[k], n));
            for k in 0..3 {
                normal[id][k] += w[k];
            }
            here.push(id);
        }
        for face in &mesh.faces {
            let f = face.indices.map(|v| here[v as usize]);
            joins.extend([(f[0], f[1]), (f[1], f[2]), (f[2], f[0])]);
        }
        point_of.push(here);
    }
    // What the top's own edges keep apart: its hems are a step in the mesh, with no faces across it.
    let places: Vec<([i32; 3], usize)> = points.iter().map(|(k, id)| (*k, *id)).collect();
    let cell = STEP_METRES / WELD_METRES;
    let mut close = Vec::new();
    for (i, (a, ida)) in places.iter().enumerate() {
        if !soft[*ida] {
            continue;
        }
        for (b, idb) in &places[i + 1..] {
            if !soft[*idb] {
                continue;
            }
            let d = (0..3)
                .map(|k| (a[k] - b[k]) as f32)
                .map(|v| v * v)
                .sum::<f32>();
            if d <= cell * cell {
                joins.push((*ida, *idb));
                close.push((*ida, *idb));
            }
        }
    }
    let unit = |v: [f32; 3]| {
        let long = dot3(v, v).sqrt().max(1e-6);
        v.map(|c| c / long)
    };
    let mut normal: Vec<[f32; 3]> = normal.into_iter().map(unit).collect();
    for _ in 0..SOFTEN_ROUNDS {
        let last = normal.clone();
        for (a, b) in &joins {
            for (to, from) in [(*a, *b), (*b, *a)] {
                if soft[to] {
                    for k in 0..3 {
                        normal[to][k] += last[from][k];
                    }
                }
            }
        }
        for n in &mut normal {
            *n = unit(*n);
        }
    }
    // Across such a step they must end up the same, or it shows as a line.
    for _ in 0..SOFTEN_ROUNDS {
        for (a, b) in &close {
            let mean = unit([0, 1, 2].map(|k| normal[*a][k] + normal[*b][k]));
            normal[*a] = mean;
            normal[*b] = mean;
        }
    }
    // The middle body's centre bar is one long edge down the belly against shorter ones beside it, and shading runs straight
    // along a long edge, so anything that changes on the way up shows as a line. On a belly the shading now changes only
    // round the body, as on a smooth barrel, which is the same all the way up an upright edge.
    let band = |h: f32| {
        let up = ((h - WAIST) / BELLY_BLEND).clamp(0.0, 1.0);
        let down = ((BAND_HIGH - h) / BELLY_BLEND).clamp(0.0, 1.0);
        up.min(down)
    };
    let (mut wide, mut deep) = (0.0f32, 0.0f32);
    for (v, p) in point_at.iter().enumerate() {
        if soft[v] && from_torso[v] && band(p[2] / top) >= 1.0 {
            wide = wide.max(p[0].abs());
            deep = deep.max(p[1].abs());
        }
    }
    if wide > 0.0 && deep > 0.0 {
        for (v, p) in point_at.iter().enumerate() {
            let w = band(p[2] / top);
            if soft[v] && from_torso[v] && w > 0.0 {
                let barrel = unit([p[0] / (wide * wide), p[1] / (deep * deep), 0.0]);
                normal[v] = unit([0, 1, 2].map(|k| normal[v][k] + w * (barrel[k] - normal[v][k])));
            }
        }
    }
    fn walk(n: &mut kotor_model::Node, f: &mut impl FnMut(&mut kotor_model::Node)) {
        f(n);
        for c in &mut n.children {
            walk(c, f);
        }
    }
    walk(&mut m.root, &mut |node| {
        let Some(which) = names.iter().position(|n| node.name.eq_ignore_ascii_case(n)) else {
            return;
        };
        let turn = turn_of(names[which]);
        let Some(mesh) = node.mesh.as_mut() else {
            return;
        };
        for (i, id) in point_of[which].iter().enumerate() {
            if !soft[*id] || i >= mesh.normals.len() {
                continue;
            }
            let w = normal[*id];
            let long = dot3(w, w).sqrt().max(1e-6);
            // Back into the mesh's own frame, the turn being a pure rotation.
            mesh.normals[i] = [0, 1, 2]
                .map(|c| (turn[0][c] * w[0] + turn[1][c] * w[1] + turn[2][c] * w[2]) / long);
        }
    });
}

const IDENTITY_TURN: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Whether a place on a body at rest is under a conventional bra: `h` is how far up the torso from 0 at the feet to 1 at
/// the neck, `x` how far from the middle and `front` the side of the chest, all as shares of the body's own size.
fn bra_part(x: f32, y: f32, h: f32, half_width: f32) -> Part {
    let side = x.abs() / half_width;
    let front = y > 0.0;
    // A band under the bust, all the way round.
    if (BAND_LOW..BAND_HIGH).contains(&h) {
        return Part::Band;
    }
    if front && (BAND_HIGH..CUP_TOP).contains(&h) {
        // Two cups, each highest towards the strap, and meeting low in the middle.
        let reach = (h - BAND_HIGH) / (CUP_TOP - BAND_HIGH);
        let from = CUP_INNER + reach * (STRAP_AT - CUP_INNER);
        let to = CUP_OUTER - reach * (CUP_OUTER - STRAP_AT);
        if side >= from - STRAP_HALF && side <= to + STRAP_HALF {
            return Part::Cup;
        }
    }
    // Two straps, front and back, from the top of the cups over the collarbones and the shoulders.
    if front
        && (CUP_TOP - 0.005..STRAP_TOP + STRAP_OVER).contains(&h)
        && (side - strap_at(h, true)).abs() < STRAP_HALF
    {
        return Part::Strap;
    }
    if !front
        && (BAND_HIGH..STRAP_TOP + STRAP_OVER).contains(&h)
        && (side - strap_at(h, false)).abs() < STRAP_HALF
    {
        return Part::Strap;
    }
    Part::None
}

/// Where a strap runs, as a share of the half width: up from the cup (or the band, at the back) it leans out, and by the top
/// of the torso it is over the shoulder, the same place from the front and the back, so that the two meet.
fn strap_at(h: f32, front: bool) -> f32 {
    let from = if front { CUP_TOP } else { BAND_HIGH };
    let t = ((h - from) / (STRAP_TOP - from)).clamp(0.0, 1.0);
    STRAP_AT + STRAP_OUT * t * t * (3.0 - 2.0 * t)
}

const BAND_LOW: f32 = 0.833;
const BAND_HIGH: f32 = 0.849;
const CUP_TOP: f32 = 0.897;
const STRAP_TOP: f32 = 1.0;
/// How far out the straps lean by the top, as a share of the half width, and how far past the neck's own height they go: the
/// shoulder's top is lower than the neck's, but the arm's cap reaches round it.
const STRAP_OUT: f32 = 0.15;
const STRAP_OVER: f32 = 0.04;
const CUP_INNER: f32 = 0.03;
const CUP_OUTER: f32 = 0.57;
const STRAP_AT: f32 = 0.29;
const STRAP_HALF: f32 = 0.045;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    None,
    Band,
    Cup,
    Strap,
}

/// What to do at each texel for one body, worked out once.
pub struct Plan {
    marks: Vec<u8>,
    /// The texels to fill, and for each where its neighbours are in `near`.
    fill: Vec<u32>,
    near_at: Vec<u32>,
    near: Vec<u32>,
    /// Texels that are one place on the body, on either side of a seam of the sheet.
    links: Vec<(u32, u32)>,
    /// Light and dark to multiply the fill by.
    relief: Vec<f32>,
    /// How much each texel takes of the fill, and how much of it is cloth or painted cloth before sharpening.
    fill_alpha: Vec<f32>,
    cloth_soft: Vec<f32>,
    paint_soft: Vec<f32>,
    /// How the shape of the body shades each texel (1 where it neither stands out nor lies in a hollow), and how much of each
    /// texel is skin the style left as it was, which is what the form shading goes on.
    form: Vec<f32>,
    skin_soft: Vec<f32>,
    /// How much of the paint is a strap at the back, which takes the band's colour, not the cups'.
    back_soft: Vec<f32>,
    /// The texels of the band that are cloth: the colour the back straps take.
    band_from: Vec<u32>,
    /// The ring of the neck, which starts from the skin's tone, and the texels this body's tone is read from. A head goes
    /// with all three bodies and matches the middle one, so each skin of this body has its tone here and the middle body's.
    neck: Vec<bool>,
    tone_from: Vec<u32>,
    tones: Vec<([f32; 3], [f32; 3])>,
    /// Texels of the front of the belly, with how far across it they are, in strips.
    belly: Vec<(u32, u8)>,
    /// Corners of square patches of clean skin.
    donors: Vec<(u32, u32)>,
}

/// How well a painted texture hides it, in levels of 255.
#[derive(Clone, Copy, Debug)]
pub struct SeamReport {
    /// The difference in mean colour between painted skin and the skin beside it, over patches of the sheet: the mean, and
    /// the patch where it is largest.
    pub colour: f32,
    pub worst_colour: f32,
    /// The mean step between a painted texel and the kept skin next to it.
    pub edge: f32,
    /// The mean step across seams of the sheet where one side is painted.
    pub wrap: f32,
    /// The share of painted texels of another hue than the skin beside them, as grey, orange, red and leather are.
    pub cloth_left: f32,
    /// The grain of the fill and of the skin beside it: how far each stands from its own smooth self.
    pub grain: f32,
    pub skin_grain: f32,
    /// The largest step between strips side by side down the front of the belly.
    pub midline: f32,
}

/// Skin keeps its hue however it is lit. Anything darker or of another hue is cloth: dark leather, gold trim, grey and
/// white, red. The three bodies wear different things.
fn is_cloth(p: [f32; 3], skin: [f32; 3]) -> bool {
    let [r, g, b] = p;
    let bright = r + g + b;
    let usual = skin[0] + skin[1] + skin[2];
    if bright < 0.72 * usual || bright > 1.4 * usual {
        return true;
    }
    (b / r.max(1.0) - skin[2] / skin[0].max(1.0)).abs() > 0.07
        || (g / r.max(1.0) - skin[1] / skin[0].max(1.0)).abs() > 0.07
}

/// The texels beside one, on the sheet.
fn beside(n: usize) -> impl Iterator<Item = usize> {
    let (x, y) = ((n % WORK) as i32, (n / WORK) as i32);
    [(1, 0), (-1, 0), (0, 1), (0, -1)]
        .into_iter()
        .map(move |(dx, dy)| (x + dx, y + dy))
        .filter(|(x, y)| *x >= 0 && *y >= 0 && *x < WORK as i32 && *y < WORK as i32)
        .map(|(x, y)| y as usize * WORK + x as usize)
}

/// The piece of the sheet each face of a mesh is in: faces sharing a vertex share a place on it.
fn face_islands(mesh: &Mesh, first: u32) -> Vec<u32> {
    let mut parent: Vec<usize> = (0..mesh.positions.len().max(mesh.uvs.len())).collect();
    fn root(parent: &mut [usize], mut v: usize) -> usize {
        while parent[v] != v {
            parent[v] = parent[parent[v]];
            v = parent[v];
        }
        v
    }
    for face in &mesh.faces {
        let a = root(&mut parent, face.indices[0] as usize);
        for v in &face.indices[1..] {
            let b = root(&mut parent, *v as usize);
            parent[b] = a;
        }
    }
    mesh.faces
        .iter()
        .map(|f| first + root(&mut parent, f.indices[0] as usize) as u32)
        .collect()
}

/// Texels at the rim of a piece of the sheet, where they are on the body.
fn rim_of(
    mesh: &Mesh,
    at: impl Fn(&[f32; 3]) -> [f32; 3],
    island_of_face: &[u32],
    out: &mut Vec<(u32, u32, [f32; 3])>,
) {
    let mut here = vec![NO_ISLAND; WORK * WORK];
    let mut place = vec![[0.0f32; 3]; WORK * WORK];
    texel_pass(mesh, at, |n, p, face| {
        here[n] = island_of_face[face];
        place[n] = p;
    });
    for n in 0..WORK * WORK {
        if here[n] != NO_ISLAND && beside(n).any(|m| here[m] != here[n]) {
            out.push((n as u32, here[n], place[n]));
        }
    }
}

/// Texels that are one place on the body across a seam of the sheet, found by where they are on the body. Pieces need not
/// share vertices: the old top's hems stand off the skin, and one long edge may face several short ones.
fn seam_links(rim: &[(u32, u32, [f32; 3])]) -> Vec<(u32, u32)> {
    let cell = |p: &[f32; 3]| p.map(|v| (v / SEAM_METRES).floor() as i32);
    let mut grid: std::collections::HashMap<[i32; 3], Vec<usize>> = Default::default();
    for (i, entry) in rim.iter().enumerate() {
        grid.entry(cell(&entry.2)).or_default().push(i);
    }
    let mut links = Vec::new();
    for (n, island, p) in rim {
        let c = cell(p);
        let mut best = (SEAM_METRES, None);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let Some(list) = grid.get(&[c[0] + dx, c[1] + dy, c[2] + dz]) else {
                        continue;
                    };
                    for i in list {
                        let (m, other, q) = &rim[*i];
                        if other == island {
                            continue;
                        }
                        let d = [p[0] - q[0], p[1] - q[1], p[2] - q[2]];
                        let away = dot3(d, d).sqrt();
                        if away < best.0 {
                            best = (away, Some(*m));
                        }
                    }
                }
            }
        }
        if let Some(m) = best.1 {
            if m != *n {
                links.push((m.min(*n), m.max(*n)));
            }
        }
    }
    links.sort_unstable();
    links.dedup();
    links
}

/// Soften a mask over rounds, staying inside a piece of the sheet and among the texels `among` allows.
fn soften(mask: &[f32], islands: &[u32], rounds: usize, among: impl Fn(usize) -> bool) -> Vec<f32> {
    let mut out = mask.to_vec();
    for _ in 0..rounds {
        let last = out.clone();
        for n in 0..WORK * WORK {
            if islands[n] == NO_ISLAND || !among(n) {
                continue;
            }
            let (mut sum, mut count) = (last[n], 1.0);
            let (x, y) = ((n % WORK) as i32, (n / WORK) as i32);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (nx, ny) = (x + dx, y + dy);
                    if (dx, dy) == (0, 0)
                        || nx < 0
                        || ny < 0
                        || nx >= WORK as i32
                        || ny >= WORK as i32
                    {
                        continue;
                    }
                    let m = ny as usize * WORK + nx as usize;
                    if islands[m] == islands[n] && among(m) {
                        sum += last[m];
                        count += 1.0;
                    }
                }
            }
            out[n] = sum / count;
        }
    }
    out
}

/// The bra's coverage at a pixel from its softened mask: steep, so that its edge is a texel or two wide.
fn sharpened(soft: f32) -> f32 {
    ((soft - 0.5) * CLOTH_RAMP + 0.5).clamp(0.0, 1.0)
}

/// How much the shape of a mesh shades each vertex: above 0 on what stands out, below on what lies in a hollow. Vertices at
/// one place on the body count as one. The body's own normals are not used, as the old top's hems are in them.
fn shape_shade(mesh: &Mesh) -> Vec<f32> {
    let key = |p: &[f32; 3]| p.map(|v| (v / WELD_METRES).round() as i32);
    let mut ids = std::collections::BTreeMap::new();
    let mut id_of = Vec::with_capacity(mesh.positions.len());
    let mut at: Vec<[f32; 3]> = Vec::new();
    for p in &mesh.positions {
        let next = at.len();
        let id = *ids.entry(key(p)).or_insert(next);
        if id == next {
            at.push(*p);
        }
        id_of.push(id);
    }
    let sub = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let mut normal = vec![[0.0f32; 3]; at.len()];
    let mut near: Vec<std::collections::BTreeSet<usize>> =
        vec![std::collections::BTreeSet::new(); at.len()];
    for face in &mesh.faces {
        let v = face.indices.map(|i| id_of[i as usize]);
        let (e, f) = (sub(at[v[1]], at[v[0]]), sub(at[v[2]], at[v[0]]));
        let cross = [
            e[1] * f[2] - e[2] * f[1],
            e[2] * f[0] - e[0] * f[2],
            e[0] * f[1] - e[1] * f[0],
        ];
        for i in 0..3 {
            for k in 0..3 {
                normal[v[i]][k] += cross[k];
            }
            near[v[i]].insert(v[(i + 1) % 3]);
            near[v[i]].insert(v[(i + 2) % 3]);
        }
    }
    let shade: Vec<f32> = (0..at.len())
        .map(|v| {
            let long = dot3(normal[v], normal[v]).sqrt().max(1e-9);
            let n = normal[v].map(|c| c / long);
            let sum: f32 = near[v]
                .iter()
                .map(|u| {
                    let d = sub(at[v], at[*u]);
                    dot3(d, n) / dot3(d, d).sqrt().max(1e-6)
                })
                .sum();
            sum / near[v].len().max(1) as f32
        })
        .collect();
    // A body stands out nearly everywhere: only what differs from the rest of it shades.
    let mean = shade.iter().sum::<f32>() / shade.len().max(1) as f32;
    id_of
        .iter()
        .map(|id| (shade[*id] - mean).clamp(-SHAPE_MOST, SHAPE_MOST))
        .collect()
}

fn torso(m: &Model) -> Option<&kotor_model::Node> {
    m.root.find("Torso").filter(|n| n.mesh.is_some())
}

fn texel_pass(
    mesh: &Mesh,
    at: impl Fn(&[f32; 3]) -> [f32; 3],
    mut f: impl FnMut(usize, [f32; 3], usize),
) {
    for (which, face) in mesh.faces.iter().enumerate() {
        let i = face.indices.map(|v| v as usize);
        let (Some(a), Some(b), Some(c)) =
            (mesh.uvs.get(i[0]), mesh.uvs.get(i[1]), mesh.uvs.get(i[2]))
        else {
            continue;
        };
        let (pa, pb, pc) = (
            at(&mesh.positions[i[0]]),
            at(&mesh.positions[i[1]]),
            at(&mesh.positions[i[2]]),
        );
        let s = WORK as f32;
        let (ax, ay, bx, by, cx, cy) = (
            a[0] * s,
            (1.0 - a[1]) * s,
            b[0] * s,
            (1.0 - b[1]) * s,
            c[0] * s,
            (1.0 - c[1]) * s,
        );
        let d = (by - cy) * (ax - cx) + (cx - bx) * (ay - cy);
        if d.abs() < 1e-6 {
            continue;
        }
        let x0 = ax.min(bx).min(cx).floor().max(0.0) as usize;
        let x1 = (ax.max(bx).max(cx).ceil() as usize).min(WORK - 1);
        let y0 = ay.min(by).min(cy).floor().max(0.0) as usize;
        let y1 = (ay.max(by).max(cy).ceil() as usize).min(WORK - 1);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let u = ((by - cy) * (px - cx) + (cx - bx) * (py - cy)) / d;
                let v = ((cy - ay) * (px - cx) + (ax - cx) * (py - cy)) / d;
                let w = 1.0 - u - v;
                // A little past the edge, so that no seam shows the old cloth.
                if u < -0.02 || v < -0.02 || w < -0.02 {
                    continue;
                }
                let p = [0, 1, 2].map(|k| u * pa[k] + v * pb[k] + w * pc[k]);
                f(y * WORK + x, p, which);
            }
        }
    }
}

/// An image at the work size. Each texel is the mean of the pixels it covers, so that the grain of a large picture does
/// not end up in the fill.
fn work_size(img: &Image) -> Vec<[f32; 3]> {
    let (w, h) = (img.width as usize, img.height as usize);
    (0..WORK * WORK)
        .map(|n| {
            let (x, y) = (n % WORK, n / WORK);
            let (x0, y0) = (x * w / WORK, y * h / WORK);
            let (x1, y1) = (
                ((x + 1) * w / WORK).max(x0 + 1),
                ((y + 1) * h / WORK).max(y0 + 1),
            );
            let mut sum = [0.0f32; 3];
            for py in y0..y1 {
                for px in x0..x1 {
                    let p = (py * w + px) * 4;
                    for k in 0..3 {
                        sum[k] += img.rgba[p + k] as f32;
                    }
                }
            }
            sum.map(|v| v / ((x1 - x0) * (y1 - y0)) as f32)
        })
        .collect()
}

/// Where a mesh's point is on the body at rest.
fn placed(world: &[(String, [[f32; 4]; 4])], name: &str, p: &[f32; 3]) -> [f32; 3] {
    let w = world.iter().find(|(k, _)| k.eq_ignore_ascii_case(name));
    w.map_or(*p, |(_, w)| {
        [0, 1, 2].map(|r| w[r][0] * p[0] + w[r][1] * p[1] + w[r][2] * p[2] + w[r][3])
    })
}

/// The height of the top of a torso, and how far it reaches to the side.
fn torso_size(mesh: &Mesh, at: impl Fn(&[f32; 3]) -> [f32; 3]) -> (f32, f32) {
    let (mut top, mut half) = (0.0f32, 0.0f32);
    for p in &mesh.positions {
        let q = at(p);
        top = top.max(q[2]);
        half = half.max(q[0].abs());
    }
    (top, half)
}

/// The skin of a sheet: the forearms, which no body covers.
fn forearm_skin(m: &Model, world: &[(String, [[f32; 4]; 4])], pixels: &[[f32; 3]]) -> [f32; 3] {
    let mut sum = [0.0f32; 3];
    let mut count = 0.0f32;
    for name in ["RArm", "LArm"] {
        let Some(arm) = m.root.find(name).and_then(|n| n.mesh.as_ref()) else {
            continue;
        };
        texel_pass(
            arm,
            |p| placed(world, name, p),
            |n, p, _| {
                if p[2] < 1.05 {
                    for k in 0..3 {
                        sum[k] += pixels[n][k];
                    }
                    count += 1.0;
                }
            },
        );
    }
    sum.map(|v| v / count.max(1.0))
}

/// The cell of a place on a body at rest, if it is on a belly.
fn relief_cell(x: f32, y: f32, h: f32, half: f32) -> Option<usize> {
    if !(WAIST..RELIEF_TOP).contains(&h) {
        return None;
    }
    let across = ((x / half * 0.5 + 0.5) * RELIEF_WIDE as f32) as usize;
    let up = ((h - WAIST) / (RELIEF_TOP - WAIST) * RELIEF_TALL as f32) as usize;
    let side = usize::from(y > 0.0);
    (across < RELIEF_WIDE && up < RELIEF_TALL)
        .then_some((side * RELIEF_TALL + up) * RELIEF_WIDE + across)
}

/// The bare belly's light and dark at a place on another body at rest, between cells.
fn relief_at(belly: &[f32], x: f32, y: f32, h: f32, half: f32) -> f32 {
    if !(WAIST..RELIEF_TOP).contains(&h) {
        return 1.0;
    }
    let across =
        ((x / half * 0.5 + 0.5) * RELIEF_WIDE as f32 - 0.5).clamp(0.0, RELIEF_WIDE as f32 - 1.0);
    let up = ((h - WAIST) / (RELIEF_TOP - WAIST) * RELIEF_TALL as f32 - 0.5)
        .clamp(0.0, RELIEF_TALL as f32 - 1.0);
    let side = usize::from(y > 0.0);
    let (a0, u0) = (across as usize, up as usize);
    let (a1, u1) = ((a0 + 1).min(RELIEF_WIDE - 1), (u0 + 1).min(RELIEF_TALL - 1));
    let (fa, fu) = (across - a0 as f32, up - u0 as f32);
    let cell = |u: usize, a: usize| belly[(side * RELIEF_TALL + u) * RELIEF_WIDE + a];
    (cell(u0, a0) * (1.0 - fa) + cell(u0, a1) * fa) * (1.0 - fu)
        + (cell(u1, a0) * (1.0 - fa) + cell(u1, a1) * fa) * fu
}

/// The light and dark of the bare belly, front and back, as a share of what is round each cell. 1 where there is none.
fn belly_relief(inst: &mut Install) -> Option<Vec<f32>> {
    let mdl = inst.read(BARE_BELLY, ResType::MDL).ok()?;
    let mdx = inst.read(BARE_BELLY, ResType::MDX).unwrap_or_default();
    let m = kotor_model::parse_model(&mdl, &mdx).ok()?;
    let pixels = work_size(&load_texture(inst, &format!("{BARE_BELLY}01"))?);
    let node = torso(&m)?;
    let mesh = node.mesh.as_ref()?;
    let world = m.world_matrices(None);
    let at = |p: &[f32; 3]| placed(&world, &node.name, p);
    let (top, half) = torso_size(mesh, at);
    let skin = forearm_skin(&m, &world, &pixels);
    let cells = 2 * RELIEF_TALL * RELIEF_WIDE;
    let (mut light, mut count) = (vec![0.0f32; cells], vec![0.0f32; cells]);
    texel_pass(mesh, at, |n, p, _| {
        if let Some(c) = relief_cell(p[0], p[1], p[2] / top, half) {
            if !is_cloth(pixels[n], skin) {
                light[c] += pixels[n][0] + pixels[n][1] + pixels[n][2];
                count[c] += 1.0;
            }
        }
    });
    // Cells beside what is not skin are tinted by it.
    let mut good: Vec<bool> = count.iter().map(|c| *c > 0.0).collect();
    for _ in 0..RELIEF_TRIM {
        let last = good.clone();
        for c in RELIEF_TALL * RELIEF_WIDE..cells {
            let up = c.checked_sub(RELIEF_WIDE).is_some_and(|m| last[m]);
            let down = c + RELIEF_WIDE < cells && last[c + RELIEF_WIDE];
            good[c] = last[c] && up && down;
        }
    }
    let value = |c: usize| {
        if good[c] {
            Some(light[c] / count[c])
        } else {
            None
        }
    };
    let mut out = vec![1.0f32; cells];
    for side in 0..2i32 {
        for up in 0..RELIEF_TALL as i32 {
            for across in 0..RELIEF_WIDE as i32 {
                let cell = |up: i32, across: i32| {
                    ((side as usize * RELIEF_TALL + up as usize) * RELIEF_WIDE) + across as usize
                };
                let Some(own) = value(cell(up, across)) else {
                    continue;
                };
                let (mut sum, mut n) = (0.0f32, 0.0f32);
                for dy in -RELIEF_UP..=RELIEF_UP {
                    for dx in -RELIEF_ACROSS..=RELIEF_ACROSS {
                        let (u, a) = (up + dy, across + dx);
                        if u < 0 || a < 0 || u >= RELIEF_TALL as i32 || a >= RELIEF_WIDE as i32 {
                            continue;
                        }
                        if let Some(v) = value(cell(u, a)) {
                            sum += v;
                            n += 1.0;
                        }
                    }
                }
                out[cell(up, across)] = own / (sum / n);
            }
        }
    }
    Some(out)
}

/// Work out the plan for a body from its model and its first texture, which every skin of it shares the cloth of.
pub fn plan_for(inst: &mut Install, body: &str) -> Option<Plan> {
    let mdl = inst.read(body, ResType::MDL).ok()?;
    let mdx = inst.read(body, ResType::MDX).unwrap_or_default();
    let m = kotor_model::parse_model(&mdl, &mdx).ok()?;
    let reference = load_texture(inst, &format!("{body}01"))?;
    let pixels = work_size(&reference);
    let node = torso(&m)?;
    let mesh = node.mesh.as_ref()?;
    let world = m.world_matrices(None);
    let at = |p: &[f32; 3]| placed(&world, &node.name, p);
    let (top, half) = torso_size(mesh, at);
    let arms: Vec<(&str, &Mesh)> = ["RArm", "LArm"]
        .iter()
        .filter_map(|name| Some((*name, m.root.find(name)?.mesh.as_ref()?)))
        .collect();
    let skin = forearm_skin(&m, &world, &pixels);
    let belly = belly_relief(inst);
    let mut relief = vec![1.0f32; WORK * WORK];
    let mut neck = vec![false; WORK * WORK];
    let mut belly_strips = Vec::new();
    let mut marks = vec![NOTHING; WORK * WORK];
    let mut islands = vec![NO_ISLAND; WORK * WORK];
    let mut straps = vec![false; WORK * WORK];
    // 1 where a strap at the back is, 2 where the old cloth of the band is kept.
    let mut tags = vec![0u8; WORK * WORK];
    let torso_islands = face_islands(mesh, 0);
    texel_pass(mesh, at, |n, p, face| {
        islands[n] = torso_islands[face];
        let h = p[2] / top;
        if let Some(belly) = &belly {
            // The large body's belly, as its light and dark against what is round each cell, made stronger: on a body whose
            // belly was never painted the muscles and the navel must read as plainly as on the one whose was.
            relief[n] = 1.0 + BELLY_GAIN * (relief_at(belly, p[0], p[1], h, half) - 1.0);
        }
        neck[n] = h >= NECK_RING;
        if p[1] > 0.0 && (WAIST..BAND_LOW).contains(&h) && p[0].abs() < half * BELLY_ACROSS {
            let strip = (p[0] / half / BELLY_ACROSS * 0.5 + 0.5) * BELLY_STRIPS as f32;
            belly_strips.push((n as u32, strip as u8));
        }
        let cloth = is_cloth(pixels[n], skin);
        if h < WAIST {
            marks[n] = if cloth { KEEP } else { SKIN };
            return;
        }
        // The old top's collar ends at the neck, and nothing of the bra does: what is left of it there is cloth to go.
        if cloth && h >= NECK_CLOTH {
            marks[n] = CLEAR;
            return;
        }
        let part = bra_part(p[0], p[1], h, half);
        straps[n] = part == Part::Strap;
        let behind = p[1] <= 0.0;
        if part == Part::Strap && behind {
            tags[n] = 1;
        } else if part == Part::Band && cloth {
            tags[n] = 2;
        }
        marks[n] = match (cloth, part) {
            (true, Part::None) => CLEAR,
            (false, Part::Band | Part::Strap) => PAINT,
            // A strap at the back is all paint, so that it is one colour with the band whatever the old top was.
            (true, Part::Strap) if behind => PAINT,
            (false, _) => SKIN,
            (true, Part::Cup) => CUP,
            _ => KEEP,
        };
    });
    // A strap crossing the old top's trim would change colour there: it takes the cups' colour instead.
    let cups: Vec<[f32; 3]> = (0..WORK * WORK)
        .filter(|n| marks[*n] == CUP)
        .step_by(16)
        .map(|n| pixels[n])
        .collect();
    for n in 0..WORK * WORK {
        let far =
            |c: &[f32; 3]| (0..3).map(|k| (c[k] - pixels[n][k]).abs()).sum::<f32>() > TRIM_LEVELS;
        if straps[n] && marks[n] == KEEP && cups.iter().all(far) {
            marks[n] = PAINT;
        }
    }
    let mut form = vec![1.0f32; WORK * WORK];
    let shaded = |mesh: &Mesh, relief: &mut [f32], form: &mut [f32]| {
        let shade = shape_shade(mesh);
        let by_place: std::collections::HashMap<[u32; 3], f32> = mesh
            .positions
            .iter()
            .zip(&shade)
            .map(|(p, s)| (p.map(f32::to_bits), *s))
            .collect();
        // Once a texel, though triangles overlap a little at their edges.
        let mut each = vec![1.0f32; WORK * WORK];
        texel_pass(
            mesh,
            |p| {
                [
                    by_place.get(&p.map(f32::to_bits)).copied().unwrap_or(0.0),
                    0.0,
                    0.0,
                ]
            },
            |n, v, _| each[n] = 1.0 + SHAPE_SHADE * v[0],
        );
        for n in 0..WORK * WORK {
            relief[n] *= each[n];
            if each[n] != 1.0 {
                form[n] = each[n];
            }
        }
    };
    shaded(mesh, &mut relief, &mut form);
    let mut first = mesh.positions.len() as u32;
    let mut rim = Vec::new();
    rim_of(mesh, at, &torso_islands, &mut rim);
    for (name, arm) in &arms {
        // Arms are bare in this style, so a sleeve goes as well. They share the sheet with the torso.
        let arm_islands = face_islands(arm, first);
        first += arm.positions.len() as u32;
        texel_pass(
            arm,
            |p| placed(&world, name, p),
            |n, p, face| {
                islands[n] = arm_islands[face];
                let cloth = is_cloth(pixels[n], skin);
                // The strap carries on over the shoulder, on the cap of the arm.
                let h = p[2] / top;
                let part = bra_part(p[0], p[1], h, half);
                marks[n] = if part == Part::Strap && h > CUP_TOP {
                    if p[1] <= 0.0 {
                        tags[n] = 1;
                    }
                    straps[n] = true;
                    PAINT
                } else if cloth {
                    CLEAR
                } else {
                    SKIN
                };
            },
        );
        shaded(arm, &mut relief, &mut form);
        rim_of(arm, |p| placed(&world, name, p), &arm_islands, &mut rim);
    }
    let links = seam_links(&rim);
    // Skin beside what goes is tinted by it, on the far side of a seam of the sheet too: the shadow painted along the middle
    // body's centre bar lies on another piece of the sheet from the bar.
    for _ in 0..MARGIN {
        let last = marks.clone();
        for n in 0..WORK * WORK {
            if last[n] == SKIN && beside(n).any(|m| last[m] == CLEAR && islands[m] == islands[n]) {
                marks[n] = CLEAR;
            }
        }
        for (p, q) in &links {
            let (p, q) = (*p as usize, *q as usize);
            if last[p] == CLEAR && last[q] == SKIN {
                marks[q] = CLEAR;
            }
            if last[q] == CLEAR && last[p] == SKIN {
                marks[p] = CLEAR;
            }
        }
    }
    // A texture is read a little past a triangle's edge, so what lies just outside cleared cloth goes too.
    for _ in 0..3 {
        let last = marks.clone();
        let last_islands = islands.clone();
        for n in 0..WORK * WORK {
            if last[n] != NOTHING {
                continue;
            }
            if let Some(m) = beside(n).find(|m| last[*m] == CLEAR) {
                marks[n] = CLEAR;
                islands[n] = last_islands[m];
            }
        }
    }
    // The tone of the skin: the upper arms, nearest to the neck of what no top covers on every body. Not clean everywhere,
    // as a sleeve may reach them, so only what is marked as skin counts.
    let mut tone_from = Vec::new();
    for (name, arm) in &arms {
        texel_pass(
            arm,
            |p| placed(&world, name, p),
            |n, p, _| {
                if (TONE_LOW..TONE_HIGH).contains(&p[2]) && marks[n] == SKIN {
                    tone_from.push(n as u32);
                }
            },
        );
    }
    tone_from.sort_unstable();
    tone_from.dedup();
    let mut plan = Plan::new(marks, islands, links, relief, neck, &tags, form);
    for family in ["", "a", "b"] {
        let (Some(picture), Some(middle)) = (
            load_texture(inst, &format!("{body}{family}01")),
            woman_tone(inst, &format!("{MIDDLE_BODY}{family}01")),
        ) else {
            continue;
        };
        let small = work_size(&picture);
        let mut own = [0.0f32; 3];
        for n in &tone_from {
            for k in 0..3 {
                own[k] += small[*n as usize][k];
            }
        }
        plan.tones
            .push((own.map(|v| v / tone_from.len().max(1) as f32), middle));
    }
    plan.tone_from = tone_from;
    plan.belly = belly_strips;
    Some(plan)
}

impl Plan {
    fn new(
        marks: Vec<u8>,
        islands: Vec<u32>,
        links: Vec<(u32, u32)>,
        mut relief: Vec<f32>,
        neck: Vec<bool>,
        tags: &[u8],
        form: Vec<f32>,
    ) -> Plan {
        let takes = |m: usize| marks[m] == CLEAR || marks[m] == SKIN;
        let mut across: Vec<Vec<u32>> = vec![Vec::new(); WORK * WORK];
        for (p, q) in &links {
            across[*p as usize].push(*q);
            across[*q as usize].push(*p);
        }
        let (mut fill, mut near_at, mut near) = (Vec::new(), vec![0u32], Vec::new());
        for n in 0..WORK * WORK {
            if marks[n] != CLEAR {
                continue;
            }
            fill.push(n as u32);
            near.extend(
                beside(n)
                    .filter(|m| islands[*m] == islands[n] && takes(*m))
                    .map(|m| m as u32),
            );
            near.extend(across[n].iter().filter(|m| takes(**m as usize)));
            near_at.push(near.len() as u32);
        }
        // Borrowed light and dark only counts away from the skin the fill must meet.
        let mut rings = vec![f32::MAX; WORK * WORK];
        let mut front: Vec<usize> = (0..WORK * WORK)
            .filter(|n| marks[*n] == SKIN || (neck[*n] && marks[*n] == CLEAR))
            .collect();
        for n in &front {
            rings[*n] = 0.0;
        }
        let mut ring = 0.0;
        let place: std::collections::HashMap<u32, usize> =
            fill.iter().enumerate().map(|(i, n)| (*n, i)).collect();
        let near_of = |n: usize| {
            let i = place[&(n as u32)];
            &near[near_at[i] as usize..near_at[i + 1] as usize]
        };
        while ring < RELIEF_FADE && !front.is_empty() {
            ring += 1.0;
            let mut next = Vec::new();
            for i in &fill {
                let n = *i as usize;
                if rings[n] == f32::MAX && near_of(n).iter().any(|m| rings[*m as usize] < ring) {
                    next.push(n);
                }
            }
            for n in &next {
                rings[*n] = ring;
            }
            front = next;
        }
        for n in 0..WORK * WORK {
            let keep = if marks[n] == CLEAR {
                (rings[n] / RELIEF_FADE).min(1.0)
            } else {
                0.0
            };
            relief[n] = 1.0 + (relief[n] - 1.0) * keep;
        }
        let is_cloth_mark = |m: u8| m == CUP || m == KEEP || m == PAINT;
        // Where the fill meets skin it fades out across skin, a colour that matches. The cloth takes no part in that.
        let clear: Vec<f32> = marks.iter().map(|m| f32::from(*m == CLEAR)).collect();
        let mut fill_alpha = soften(&clear, &islands, FEATHER, |n| {
            marks[n] == CLEAR || marks[n] == SKIN
        });
        // Under the bra's edge the fill carries on, so that the edge alone decides what shows there.
        for _ in 0..=CLOTH_SOFTEN {
            let last = fill_alpha.clone();
            for n in 0..WORK * WORK {
                if is_cloth_mark(marks[n]) {
                    fill_alpha[n] = beside(n)
                        .filter(|m| islands[*m] == islands[n])
                        .map(|m| last[m])
                        .fold(last[n], f32::max);
                }
            }
        }
        let cloth: Vec<f32> = marks.iter().map(|m| f32::from(is_cloth_mark(*m))).collect();
        let cloth_soft = soften(&cloth, &islands, CLOTH_SOFTEN, |_| true);
        let paint: Vec<f32> = marks.iter().map(|m| f32::from(*m == PAINT)).collect();
        let paint_soft = soften(&paint, &islands, CLOTH_SOFTEN, |_| true);
        let kept_skin: Vec<f32> = marks.iter().map(|m| f32::from(*m == SKIN)).collect();
        let skin_soft = soften(&kept_skin, &islands, CLOTH_SOFTEN, |_| true);
        let back: Vec<f32> = (0..WORK * WORK)
            .map(|n| f32::from(marks[n] == PAINT && tags[n] == 1))
            .collect();
        let back_soft = soften(&back, &islands, CLOTH_SOFTEN, |_| true);
        let band_from: Vec<u32> = (0..WORK * WORK)
            .filter(|n| tags[*n] == 2)
            .map(|n| n as u32)
            .collect();
        let neck_ring: Vec<bool> = (0..WORK * WORK)
            .map(|n| neck[n] && marks[n] == CLEAR)
            .collect();
        let mut donors = Vec::new();
        for y in (0..WORK - GRAIN_BLOCK).step_by(GRAIN_BLOCK / 2) {
            for x in (0..WORK - GRAIN_BLOCK).step_by(GRAIN_BLOCK / 2) {
                let clean = (0..GRAIN_BLOCK)
                    .all(|dy| (0..GRAIN_BLOCK).all(|dx| marks[(y + dy) * WORK + x + dx] == SKIN));
                if clean {
                    donors.push((x as u32, y as u32));
                }
            }
        }
        Plan {
            marks,
            fill,
            near_at,
            near,
            links,
            relief,
            fill_alpha,
            cloth_soft,
            paint_soft,
            form,
            skin_soft,
            back_soft,
            band_from,
            neck: neck_ring,
            tone_from: Vec::new(),
            tones: Vec::new(),
            belly: Vec::new(),
            donors,
        }
    }

    /// How many texels of the work size it takes cloth from, and paints.
    pub fn counts(&self) -> (usize, usize) {
        let n = |m: u8| self.marks.iter().filter(|v| **v == m).count();
        (n(CLEAR), n(PAINT))
    }

    fn near_of(&self, i: usize) -> &[u32] {
        &self.near[self.near_at[i] as usize..self.near_at[i + 1] as usize]
    }

    /// The tone of a picture's skin.
    fn tone_of(&self, small: &[[f32; 3]]) -> Option<[f32; 3]> {
        if self.tone_from.is_empty() {
            return None;
        }
        let mut sum = [0.0f32; 3];
        for n in &self.tone_from {
            for k in 0..3 {
                sum[k] += small[*n as usize][k];
            }
        }
        let own = sum.map(|v| v / self.tone_from.len() as f32);
        let away = |t: &[f32; 3]| (0..3).map(|k| (t[k] - own[k]).abs()).sum::<f32>();
        self.tones
            .iter()
            .min_by(|a, b| away(&a.0).total_cmp(&away(&b.0)))
            .map(|t| t.1)
    }

    /// The mean colour of the neck's ring in a picture this plan was applied to.
    pub fn neck_colour(&self, img: &Image) -> [f32; 3] {
        let small = work_size(img);
        let mut sum = [0.0f32; 3];
        let mut count = 0.0f32;
        for n in 0..WORK * WORK {
            if self.neck[n] {
                for k in 0..3 {
                    sum[k] += small[n][k];
                }
                count += 1.0;
            }
        }
        sum.map(|v| v / count.max(1.0))
    }

    /// How wide the bra's edge is, in texels of the game's own 512 picture, seen in a picture of this size: nineteen edge
    /// pixels in twenty are no wider. Measured across the edge wherever it runs.
    pub fn bra_edge_width(&self, width: usize, height: usize) -> f32 {
        let coverage = |x: f32, y: f32| {
            let (u, v) = (
                x * WORK as f32 / width as f32 - 0.5,
                y * WORK as f32 / height as f32 - 0.5,
            );
            sharpened(bilinear1(&self.cloth_soft, u, v))
        };
        let mut widths = Vec::new();
        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let c = coverage(px, py);
                if !(0.4..0.6).contains(&c) {
                    continue;
                }
                let n = (py as usize * WORK / height) * WORK + px as usize * WORK / width;
                // Only where it meets the fill, not the rest of the sheet's cloth.
                if self.fill_alpha[n] < 0.5 {
                    continue;
                }
                let (dx, dy) = (
                    coverage(px + 1.0, py) - coverage(px - 1.0, py),
                    coverage(px, py + 1.0) - coverage(px, py - 1.0),
                );
                let long = dx.hypot(dy);
                if long <= 0.0 {
                    continue;
                }
                // From nearly no cloth to nearly all, straight across the edge, a quarter of a pixel at a time.
                let (sx, sy) = (dx / long * 0.25, dy / long * 0.25);
                let walk = |sign: f32, done: &dyn Fn(f32) -> bool| {
                    let mut steps = 0.0f32;
                    while steps < 64.0
                        && !done(coverage(px + sign * sx * steps, py + sign * sy * steps))
                    {
                        steps += 1.0;
                    }
                    steps * 0.25
                };
                let across = walk(1.0, &|c| c >= 0.9) + walk(-1.0, &|c| c <= 0.1);
                widths.push(across * WORK as f32 / width as f32);
            }
        }
        widths.sort_by(f32::total_cmp);
        widths
            .get(widths.len() * 19 / 20)
            .copied()
            .unwrap_or(f32::MAX)
    }

    /// The smooth skin at every texel: what stays as it is, and in what goes the mean of what is round it.
    fn solve(&self, small: &[[f32; 3]]) -> Vec<[f32; 3]> {
        let mut known: Vec<bool> = self.marks.iter().map(|m| *m == SKIN).collect();
        let mut field: Vec<[f32; 3]> = small.to_vec();
        if let Some(tone) = self.tone_of(small) {
            for n in 0..WORK * WORK {
                if self.neck[n] {
                    field[n] = tone;
                    known[n] = true;
                }
            }
        }
        let fixed = known.clone();
        let (mut sum, mut count) = ([0.0f32; 3], 0.0f32);
        for n in 0..WORK * WORK {
            if known[n] {
                for k in 0..3 {
                    sum[k] += small[n][k];
                }
                count += 1.0;
            }
        }
        let average = sum.map(|v| v / count.max(1.0));
        // A first guess, grown in rings: it only saves passes.
        let mut left: Vec<usize> = (0..self.fill.len())
            .filter(|i| !fixed[self.fill[*i] as usize])
            .collect();
        while !left.is_empty() {
            let last = known.clone();
            let before = left.len();
            left.retain(|i| {
                let (mut s, mut c) = ([0.0f32; 3], 0.0f32);
                for m in self.near_of(*i) {
                    if last[*m as usize] {
                        for k in 0..3 {
                            s[k] += field[*m as usize][k];
                        }
                        c += 1.0;
                    }
                }
                if c == 0.0 {
                    return true;
                }
                let n = self.fill[*i] as usize;
                field[n] = s.map(|v| v / c);
                known[n] = true;
                false
            });
            if left.len() == before {
                break;
            }
        }
        // What touches no skin at all takes the skin's mean.
        for i in left {
            field[self.fill[i] as usize] = average;
        }
        for _ in 0..FILL_PASSES {
            for i in 0..self.fill.len() {
                let near = self.near_of(i);
                if near.is_empty() || fixed[self.fill[i] as usize] {
                    continue;
                }
                let mut s = [0.0f32; 3];
                for m in near {
                    let v = field[*m as usize];
                    for k in 0..3 {
                        s[k] += v[k];
                    }
                }
                let n = self.fill[i] as usize;
                let old = field[n];
                let scale = 1.0 / near.len() as f32;
                for k in 0..3 {
                    field[n][k] = old[k] + OVERSHOOT * (s[k] * scale - old[k]);
                }
            }
        }
        // Under an edge of a cup, what is painted reaches a little way: carry the fill out so far.
        for _ in 0..=FEATHER {
            let last = field.clone();
            let last_known: Vec<bool> = self
                .marks
                .iter()
                .enumerate()
                .map(|(n, m)| *m == SKIN || *m == CLEAR || (known[n] && self.fill_alpha[n] > 0.0))
                .collect();
            for n in 0..WORK * WORK {
                if last_known[n] || self.fill_alpha[n] <= 0.0 {
                    continue;
                }
                let (mut s, mut c) = ([0.0f32; 3], 0.0f32);
                for m in beside(n) {
                    if last_known[m] {
                        for k in 0..3 {
                            s[k] += last[m][k];
                        }
                        c += 1.0;
                    }
                }
                if c > 0.0 {
                    field[n] = s.map(|v| v / c);
                    known[n] = true;
                }
            }
        }
        field
    }

    /// Apply to a texture of the body, at its own size: `img` may be the game's or an upscaled one.
    pub fn apply(&self, img: &mut Image) {
        let (w, h) = (img.width as usize, img.height as usize);
        let small = work_size(img);
        let field = self.solve(&small);
        let smooth = box_blur(&small);
        // Patches of the calmest skin, so that no fold or finger is copied.
        let mut donors: Vec<(f32, (u32, u32))> = self
            .donors
            .iter()
            .map(|d| (roughness(&small, &smooth, *d), *d))
            .collect();
        donors.sort_by(|a, b| a.0.total_cmp(&b.0));
        donors.truncate(donors.len().div_ceil(2));
        // No stronger than the grain of the skin it lies beside, which is calmer than most in some sheets.
        let hard = |n: usize| -> f32 {
            (0..3)
                .map(|k| small[n][k] - smooth[n][k])
                .map(|d| d * d)
                .sum::<f32>()
        };
        let (mut beside_skin, mut beside_count) = (0.0f32, 0.0f32);
        for n in 0..WORK * WORK {
            if self.marks[n] == SKIN && self.fill_alpha[n] > 0.0 {
                beside_skin += hard(n);
                beside_count += 1.0;
            }
        }
        let (mut in_donors, mut donor_count) = (0.0f32, 0.0f32);
        for (_, (x, y)) in &donors {
            for dy in 0..GRAIN_BLOCK {
                for dx in 0..GRAIN_BLOCK {
                    in_donors += hard((*y as usize + dy) * WORK + *x as usize + dx);
                    donor_count += 1.0;
                }
            }
        }
        let grain_strength = ((beside_skin / beside_count.max(1.0))
            / (in_donors / donor_count.max(1.0)).max(1e-6))
        .sqrt()
        .min(1.0);
        let grain_at = |u: f32, v: f32| -> [f32; 3] {
            if donors.is_empty() {
                return [0.0; 3];
            }
            let block = GRAIN_BLOCK as f32 / 2.0;
            let (gx, gy) = (u / block, v / block);
            let (cx, cy) = (gx.floor(), gy.floor());
            let (fx, fy) = (gx - cx, gy - cy);
            let mut out = [0.0f32; 3];
            for (dx, dy, weight) in [
                (0.0, 0.0, (1.0 - fx) * (1.0 - fy)),
                (1.0, 0.0, fx * (1.0 - fy)),
                (0.0, 1.0, (1.0 - fx) * fy),
                (1.0, 1.0, fx * fy),
            ] {
                let (bx, by) = ((cx + dx) as u32, (cy + dy) as u32);
                let pick = (bx.wrapping_mul(73_856_093) ^ by.wrapping_mul(19_349_663)) as usize;
                let (sx, sy) = donors[pick % donors.len()].1;
                // Where in the patch this texel falls, its centre over the block's corner.
                let px = sx as f32 + block + (u - (cx + dx) * block);
                let py = sy as f32 + block + (v - (cy + dy) * block);
                let (ix, iy) = (
                    (px * w as f32 / WORK as f32) as usize,
                    (py * h as f32 / WORK as f32) as usize,
                );
                let p = (iy.min(h - 1) * w + ix.min(w - 1)) * 4;
                let low = bilinear(&smooth, px, py);
                for k in 0..3 {
                    out[k] += weight * (img.rgba[p + k] as f32 - low[k]);
                }
            }
            out
        };
        // A band or strap on bare skin takes the colour of the cups in this very texture.
        let cloth = {
            let (mut s, mut c) = ([0.0f32; 3], 0.0f32);
            for n in 0..WORK * WORK {
                if self.marks[n] == CUP {
                    for k in 0..3 {
                        s[k] += small[n][k];
                    }
                    c += 1.0;
                }
            }
            s.map(|v| v / c.max(1.0))
        };
        // The band's own colour, which the straps at the back take so that they and the band are one garment.
        let band = if self.band_from.is_empty() {
            cloth
        } else {
            let mut s = [0.0f32; 3];
            for n in &self.band_from {
                for k in 0..3 {
                    s[k] += small[*n as usize][k];
                }
            }
            s.map(|v| v / self.band_from.len() as f32)
        };
        let mut out = img.rgba.clone();
        for y in 0..h {
            for x in 0..w {
                let u = (x as f32 + 0.5) * WORK as f32 / w as f32 - 0.5;
                let v = (y as f32 + 0.5) * WORK as f32 / h as f32 - 0.5;
                let a = bilinear1(&self.fill_alpha, u, v)
                    * (1.0 - sharpened(bilinear1(&self.cloth_soft, u, v)));
                let b = sharpened(bilinear1(&self.paint_soft, u, v));
                if a <= 0.0 && b <= 0.0 {
                    continue;
                }
                let p = (y * w + x) * 4;
                let mut c = [0.0f32; 3];
                for k in 0..3 {
                    c[k] = img.rgba[p + k] as f32;
                }
                if a > 0.0 {
                    let low = bilinear(&field, u, v);
                    let grain = grain_at(u, v).map(|g| g * grain_strength);
                    let shade = bilinear1(&self.relief, u, v);
                    for k in 0..3 {
                        c[k] += a * (low[k] * shade + grain[k] - c[k]);
                    }
                }
                let behind = if b > 0.0 {
                    (sharpened(bilinear1(&self.back_soft, u, v)) / b).min(1.0)
                } else {
                    0.0
                };
                for k in 0..3 {
                    let paint = cloth[k] + (band[k] - cloth[k]) * behind;
                    c[k] += b * (paint - c[k]);
                    out[p + k] = (c[k] + 0.5).clamp(0.0, 255.0) as u8;
                }
            }
        }
        img.rgba = out;
    }

    /// Shade the skin the style left as it was, by the body's own shape, at `strength` (0 off, 1 subtle, 2 strong): darker in the hollows
    /// (the armpit, the bend of an elbow, the groin line, under the bust), lighter on what stands out (shoulders, collarbones, knees,
    /// shins), a little redder in a hollow and a little cooler on a ridge, as skin is. What the plan filled already carries it.
    pub fn shade_skin(&self, img: &mut Image, strength: f32) {
        if strength <= 0.0 {
            return;
        }
        let (w, h) = (img.width as usize, img.height as usize);
        for y in 0..h {
            for x in 0..w {
                let u = (x as f32 + 0.5) * WORK as f32 / w as f32 - 0.5;
                let v = (y as f32 + 0.5) * WORK as f32 / h as f32 - 0.5;
                let skin = bilinear1(&self.skin_soft, u, v);
                if skin <= 0.0 {
                    continue;
                }
                // How far above or below 1 the shape puts this place, as a share of the most the shape ever does.
                let form = bilinear1(&self.form, u, v) - 1.0;
                let amount = (form / SHAPE_SHADE).clamp(-1.0, 1.0) * strength * skin;
                let p = (y * w + x) * 4;
                // Light and dark: up to 9 levels in a hundred at `strength` 1; colour: a hollow loses blue and green, a ridge red.
                let (light, warm) = (1.0 + 0.09 * amount, 0.045 * amount);
                let gains = [light + warm * 0.5, light - warm * 0.25, light - warm];
                for k in 0..3 {
                    img.rgba[p + k] =
                        (img.rgba[p + k] as f32 * gains[k] + 0.5).clamp(0.0, 255.0) as u8;
                }
            }
        }
    }

    /// How well a texture this plan was applied to hides it.
    pub fn seam_report(&self, img: &Image) -> SeamReport {
        const REACH: i32 = 6;
        const PATCH: usize = 32;
        const FEWEST: f32 = 16.0;
        // Blue over red: about 0.6 in skin, 1 in grey cloth, 0.2 in orange, red and leather.
        const CLOTH_HUE: f32 = 0.2;
        let small = work_size(img);
        let step = |a: [f32; 3], b: [f32; 3]| {
            ((a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs()) / 3.0
        };
        let filled = |n: usize| self.marks[n] == CLEAR && self.fill_alpha[n] >= 1.0;
        let kept = |n: usize| self.marks[n] == SKIN && self.fill_alpha[n] <= 0.0;
        let (mut edge, mut edges) = (0.0f32, 0.0f32);
        for n in 0..WORK * WORK {
            if self.marks[n] != CLEAR {
                continue;
            }
            for m in beside(n) {
                if self.marks[m] == SKIN {
                    edge += step(small[n], small[m]);
                    edges += 1.0;
                }
            }
        }
        let (mut wrap, mut wraps) = (0.0f32, 0.0f32);
        for (p, q) in &self.links {
            let (p, q) = (*p as usize, *q as usize);
            let bare = |n: usize| self.marks[n] == CLEAR || self.marks[n] == SKIN;
            if bare(p) && bare(q) && (self.marks[p] == CLEAR || self.marks[q] == CLEAR) {
                wrap += step(small[p], small[q]);
                wraps += 1.0;
            }
        }
        // Painted texels and kept ones near each other, a patch of the sheet at a time.
        let near_other = |n: usize, other: &dyn Fn(usize) -> bool| {
            let (x, y) = ((n % WORK) as i32, (n / WORK) as i32);
            (-REACH..=REACH).any(|dy| {
                (-REACH..=REACH).any(|dx| {
                    let (nx, ny) = (x + dx, y + dy);
                    nx >= 0
                        && ny >= 0
                        && nx < WORK as i32
                        && ny < WORK as i32
                        && other(ny as usize * WORK + nx as usize)
                })
            })
        };
        let (mut colour, mut worst, mut patches) = (0.0f32, 0.0f32, 0.0f32);
        let (mut left, mut all) = (0.0f32, 0.0f32);
        for py in (0..WORK).step_by(PATCH) {
            for px in (0..WORK).step_by(PATCH) {
                let (mut a, mut ca, mut b, mut cb) = ([0.0f32; 3], 0.0f32, [0.0f32; 3], 0.0f32);
                for y in py..py + PATCH {
                    for x in px..px + PATCH {
                        let n = y * WORK + x;
                        if self.marks[n] == CLEAR && near_other(n, &kept) {
                            for k in 0..3 {
                                a[k] += small[n][k];
                            }
                            ca += 1.0;
                        } else if kept(n) && near_other(n, &|m| self.marks[m] == CLEAR) {
                            for k in 0..3 {
                                b[k] += small[n][k];
                            }
                            cb += 1.0;
                        }
                    }
                }
                if ca >= FEWEST && cb >= FEWEST {
                    let skin = b.map(|v| v / cb);
                    let d = step(a.map(|v| v / ca), skin);
                    colour += d;
                    worst = worst.max(d);
                    patches += 1.0;
                    for y in py..py + PATCH {
                        for x in px..px + PATCH {
                            let n = y * WORK + x;
                            if filled(n) {
                                all += 1.0;
                                let hue = |c: [f32; 3]| c[2] / c[0].max(1.0);
                                if (hue(small[n]) - hue(skin)).abs() > CLOTH_HUE {
                                    left += 1.0;
                                }
                            }
                        }
                    }
                }
            }
        }
        // The grain: what stands out from the smooth skin, in the fill and in the skin beside it.
        let smooth = box_blur(&small);
        let grain_of = |n: usize| {
            let d = [0, 1, 2].map(|k| small[n][k] - smooth[n][k]);
            (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) / 3.0
        };
        let (mut in_fill, mut fill_count, mut in_skin, mut skin_count) =
            (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        // Cloth within reach of the smoothing would count as grain.
        let far_from_cloth = |n: usize| {
            let (x, y) = ((n % WORK) as i32, (n / WORK) as i32);
            let reach = GRAIN_BLUR as i32 * 2;
            (-reach..=reach).all(|dy| {
                (-reach..=reach).all(|dx| {
                    let (nx, ny) = (x + dx, y + dy);
                    nx >= 0
                        && ny >= 0
                        && nx < WORK as i32
                        && ny < WORK as i32
                        && matches!(self.marks[ny as usize * WORK + nx as usize], CLEAR | SKIN)
                })
            })
        };
        for n in 0..WORK * WORK {
            if !far_from_cloth(n) {
                continue;
            }
            if filled(n) {
                in_fill += grain_of(n);
                fill_count += 1.0;
            } else if kept(n) && near_other(n, &|m| self.marks[m] == CLEAR) {
                in_skin += grain_of(n);
                skin_count += 1.0;
            }
        }
        let grain = (in_fill / fill_count.max(1.0)).sqrt();
        let skin_grain = (in_skin / skin_count.max(1.0)).sqrt();
        // Down the middle of the belly.
        let mut strips = vec![([0.0f32; 3], 0.0f32); BELLY_STRIPS];
        for (n, strip) in &self.belly {
            let bin = &mut strips[(*strip as usize).min(BELLY_STRIPS - 1)];
            for k in 0..3 {
                bin.0[k] += small[*n as usize][k];
            }
            bin.1 += 1.0;
        }
        let means: Vec<[f32; 3]> = strips
            .iter()
            .filter(|b| b.1 > 0.0)
            .map(|b| b.0.map(|v| v / b.1))
            .collect();
        let midline = means
            .windows(2)
            .map(|w| step(w[0], w[1]))
            .fold(0.0, f32::max);
        SeamReport {
            grain,
            skin_grain,
            midline,
            colour: colour / patches.max(1.0),
            worst_colour: worst,
            edge: edge / edges.max(1.0),
            wrap: wrap / wraps.max(1.0),
            cloth_left: left / all.max(1.0),
        }
    }
}

/// A hero's head by its texture: `pfha01`, `pmhc05`. Whose body it goes with, by the family letter: what stands in the place
/// of `a` in `pfbama01`, none for `c`.
fn head_family(name: &str) -> Option<String> {
    let n = name.to_ascii_lowercase();
    let body = match n.get(..3)? {
        "pfh" => "pfbam",
        "pmh" => "pmbam",
        _ => return None,
    };
    let rest = n.get(3..)?;
    let family = rest.chars().next().filter(|c| "abc".contains(*c))?;
    (rest.len() == 3 && rest[1..].bytes().all(|b| b.is_ascii_digit())).then(|| {
        if family == 'c' {
            format!("{body}01")
        } else {
            format!("{body}{family}01")
        }
    })
}

/// Whether a texture is a hero's head, whose neck the style changes.
pub fn is_hero_head(name: &str) -> bool {
    head_family(name).is_some()
}

/// How a head's neck fades to the tone of the body's skin.
pub struct NeckPlan {
    /// How much each texel takes of the change.
    change: Vec<f32>,
    /// The lowest ring of the neck, which is what meets the body.
    ring: Vec<u32>,
    tone: [f32; 3],
}

/// The mesh of a head that is its face and neck: the largest with the head's own texture. Hair may share that, and hang lower.
fn head_mesh<'a>(m: &'a Model, head: &str) -> Option<&'a kotor_model::Node> {
    fn biggest<'a>(n: &'a kotor_model::Node, head: &str, best: &mut Option<&'a kotor_model::Node>) {
        if let Some(mesh) = &n.mesh {
            let same = mesh
                .texture
                .as_deref()
                .is_some_and(|t| t.eq_ignore_ascii_case(head));
            if same
                && best
                    .and_then(|b| b.mesh.as_ref())
                    .is_none_or(|b| mesh.faces.len() > b.faces.len())
            {
                *best = Some(n);
            }
        }
        for c in &n.children {
            biggest(c, head, best);
        }
    }
    let mut best = None;
    biggest(&m.root, head, &mut best);
    best
}

/// As a character stands in a conversation the head's neck can end above the body's, with the room showing through. A collar
/// hid that. So the lowest part of the neck reaches down inside the body. Only that part moves.
pub fn close_neck_gap(m: &mut Model, head: &str) {
    let Some(node) = head_mesh(m, head) else {
        return;
    };
    let name = node.name.clone();
    let world = m.world_matrices(None);
    let Some(w) = world
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(&name))
        .map(|x| x.1)
    else {
        return;
    };
    fn find_mut<'a>(n: &'a mut kotor_model::Node, name: &str) -> Option<&'a mut kotor_model::Node> {
        if n.name.eq_ignore_ascii_case(name) {
            return Some(n);
        }
        n.children.iter_mut().find_map(|c| find_mut(c, name))
    }
    let Some(mesh) = find_mut(&mut m.root, &name).and_then(|n| n.mesh.as_mut()) else {
        return;
    };
    let turn = [0, 1, 2].map(|r| [w[r][0], w[r][1], w[r][2]]);
    let up = |p: &[f32; 3]| dot3(turn[2], *p) + w[2][3];
    let low = mesh.positions.iter().map(up).fold(f32::MAX, f32::min);
    // Down in the world, in the mesh's own frame: the turn is a pure rotation.
    let down = [0, 1, 2].map(|c| -turn[2][c]);
    for p in &mut mesh.positions {
        let t = ((up(p) - low) / NECK_STRETCH_METRES).clamp(0.0, 1.0);
        for k in 0..3 {
            p[k] += down[k] * NECK_REACH_METRES * (1.0 - t);
        }
    }
}

/// The tone of a woman's skin in a picture of the middle body: her upper arms.
fn woman_tone(inst: &mut Install, texture: &str) -> Option<[f32; 3]> {
    let pixels = work_size(&load_texture(inst, texture)?);
    let mdl = inst.read(MIDDLE_BODY, ResType::MDL).ok()?;
    let mdx = inst.read(MIDDLE_BODY, ResType::MDX).unwrap_or_default();
    let m = kotor_model::parse_model(&mdl, &mdx).ok()?;
    let world = m.world_matrices(None);
    let skin = forearm_skin(&m, &world, &pixels);
    let mut sum = [0.0f32; 3];
    let mut count = 0.0f32;
    for name in ["RArm", "LArm"] {
        let Some(arm) = m.root.find(name).and_then(|n| n.mesh.as_ref()) else {
            continue;
        };
        texel_pass(
            arm,
            |p| placed(&world, name, p),
            |n, p, _| {
                if (TONE_LOW..TONE_HIGH).contains(&p[2]) && !is_cloth(pixels[n], skin) {
                    for k in 0..3 {
                        sum[k] += pixels[n][k];
                    }
                    count += 1.0;
                }
            },
        );
    }
    (count > 0.0).then(|| sum.map(|v| v / count))
}

/// The colour of a man's own neck, in a picture of the middle body.
fn man_neck_tone(inst: &mut Install, texture: &str) -> Option<[f32; 3]> {
    let pixels = work_size(&load_texture(inst, texture)?);
    let mdl = inst.read("pmbam", ResType::MDL).ok()?;
    let mdx = inst.read("pmbam", ResType::MDX).unwrap_or_default();
    let m = kotor_model::parse_model(&mdl, &mdx).ok()?;
    let world = m.world_matrices(None);
    let skin = forearm_skin(&m, &world, &pixels);
    let node = torso(&m)?;
    let mesh = node.mesh.as_ref()?;
    let at = |p: &[f32; 3]| placed(&world, &node.name, p);
    let (top, _) = torso_size(mesh, at);
    let mut sum = [0.0f32; 3];
    let mut count = 0.0f32;
    texel_pass(mesh, at, |n, p, _| {
        if p[2] / top >= NECK_RING && !is_cloth(pixels[n], skin) {
            for k in 0..3 {
                sum[k] += pixels[n][k];
            }
            count += 1.0;
        }
    });
    (count > 0.0).then(|| sum.map(|v| v / count))
}

/// Work out how a head's neck changes, from its model and the skin of the body it goes with.
pub fn neck_plan_for(inst: &mut Install, head: &str) -> Option<NeckPlan> {
    let body_texture = head_family(head)?;
    let mdl = inst.read(head, ResType::MDL).ok()?;
    let mdx = inst.read(head, ResType::MDX).unwrap_or_default();
    let m = kotor_model::parse_model(&mdl, &mdx).ok()?;
    let node = head_mesh(&m, head)?;
    let mesh = node.mesh.as_ref()?;
    let world = m.world_matrices(None);
    let at = |p: &[f32; 3]| placed(&world, &node.name, p);
    let low = mesh
        .positions
        .iter()
        .map(|p| at(p)[2])
        .fold(f32::MAX, f32::min);
    let mut change = vec![0.0f32; WORK * WORK];
    let mut ring = Vec::new();
    texel_pass(mesh, at, |n, p, _| {
        let behind = ((NECK_FRONT_METRES - p[1]) / NECK_BACK_OUT_METRES).clamp(0.0, 1.0);
        let height = p[2] - low;
        let smooth = |t: f32| {
            let t = t.clamp(0.0, 1.0);
            1.0 - t * t * (3.0 - 2.0 * t)
        };
        // Smooth at both ends, so that no line shows where the change stops. At the back it holds longer before it falls.
        let front = smooth(height / NECK_FADE_METRES);
        let back = smooth((height - NECK_BACK_HOLD_METRES) / NECK_BACK_FADE_METRES);
        change[n] = change[n].max(front + (back - front) * behind);
        if p[2] - low < NECK_RING_METRES {
            ring.push(n as u32);
        }
    });
    ring.sort_unstable();
    ring.dedup();
    if ring.is_empty() {
        return None;
    }
    // The same tone as the body's own fill starts from. A man's body keeps its own neck, so it is that.
    let tone = if body_texture.starts_with("pfba") {
        woman_tone(inst, &body_texture)?
    } else {
        man_neck_tone(inst, &body_texture)?
    };
    Some(NeckPlan { change, ring, tone })
}

impl NeckPlan {
    /// The mean colour where the neck meets the body.
    pub fn ring_colour(&self, img: &Image) -> [f32; 3] {
        let small = work_size(img);
        let mut sum = [0.0f32; 3];
        for n in &self.ring {
            for k in 0..3 {
                sum[k] += small[*n as usize][k];
            }
        }
        sum.map(|v| v / self.ring.len() as f32)
    }

    /// Apply to a head's picture, at its own size. Each channel is scaled, so the neck keeps its own shading and grain.
    pub fn apply(&self, img: &mut Image) {
        let at_ring = self.ring_colour(img);
        let gain = [0, 1, 2].map(|k| self.tone[k] / at_ring[k].max(1.0));
        let (w, h) = (img.width as usize, img.height as usize);
        for y in 0..h {
            for x in 0..w {
                let u = (x as f32 + 0.5) * WORK as f32 / w as f32 - 0.5;
                let v = (y as f32 + 0.5) * WORK as f32 / h as f32 - 0.5;
                let a = bilinear1(&self.change, u, v);
                if a <= 0.0 {
                    continue;
                }
                let p = (y * w + x) * 4;
                for k in 0..3 {
                    let c = img.rgba[p + k] as f32 * (1.0 + a * (gain[k] - 1.0));
                    img.rgba[p + k] = (c + 0.5).clamp(0.0, 255.0) as u8;
                }
            }
        }
    }
}

fn box_blur(small: &[[f32; 3]]) -> Vec<[f32; 3]> {
    let reach = GRAIN_BLUR as i32;
    let mut out = small.to_vec();
    for pass in 0..2 {
        let last = out.clone();
        for a in 0..WORK {
            for b in 0..WORK {
                let (mut s, mut c) = ([0.0f32; 3], 0.0f32);
                for d in -reach..=reach {
                    let along = a as i32 + d;
                    if along < 0 || along >= WORK as i32 {
                        continue;
                    }
                    let along = along as usize;
                    let m = if pass == 0 {
                        b * WORK + along
                    } else {
                        along * WORK + b
                    };
                    for k in 0..3 {
                        s[k] += last[m][k];
                    }
                    c += 1.0;
                }
                let n = if pass == 0 {
                    b * WORK + a
                } else {
                    a * WORK + b
                };
                out[n] = s.map(|v| v / c);
            }
        }
    }
    out
}

/// How far a patch of skin is from its smooth self.
fn roughness(small: &[[f32; 3]], smooth: &[[f32; 3]], corner: (u32, u32)) -> f32 {
    let mut sum = 0.0;
    for y in 0..GRAIN_BLOCK {
        for x in 0..GRAIN_BLOCK {
            let n = (corner.1 as usize + y) * WORK + corner.0 as usize + x;
            for k in 0..3 {
                sum += (small[n][k] - smooth[n][k]).abs();
            }
        }
    }
    sum
}

fn corners(u: f32, v: f32) -> ([usize; 4], [f32; 4]) {
    let top = WORK as f32 - 1.0;
    let (u, v) = (u.clamp(0.0, top), v.clamp(0.0, top));
    let (x0, y0) = (u.floor() as usize, v.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(WORK - 1), (y0 + 1).min(WORK - 1));
    let (fx, fy) = (u - x0 as f32, v - y0 as f32);
    (
        [
            y0 * WORK + x0,
            y0 * WORK + x1,
            y1 * WORK + x0,
            y1 * WORK + x1,
        ],
        [
            (1.0 - fx) * (1.0 - fy),
            fx * (1.0 - fy),
            (1.0 - fx) * fy,
            fx * fy,
        ],
    )
}

fn bilinear(field: &[[f32; 3]], u: f32, v: f32) -> [f32; 3] {
    let (at, weight) = corners(u, v);
    let mut out = [0.0f32; 3];
    for i in 0..4 {
        for k in 0..3 {
            out[k] += weight[i] * field[at[i]][k];
        }
    }
    out
}

fn bilinear1(field: &[f32], u: f32, v: f32) -> f32 {
    let (at, weight) = corners(u, v);
    (0..4).map(|i| weight[i] * field[at[i]]).sum()
}

/// How far up the head's neck takes the body's shading where they meet, in metres, and how near a body vertex counts.
const NECK_SHADE_METRES: f32 = 0.15;
const NECK_SHADE_WHOLE_METRES: f32 = 0.09;
const NECK_SHADE_NEAR_METRES: f32 = 0.16;

/// Where a head's neck meets the body, the two are lit by different normals, and the neck's lowest ring showed about a
/// fifth brighter than the chest under it. The neck's normals turn to those of the nearest body vertex, wholly at the
/// ring and not at all `NECK_SHADE_METRES` above it. `up` is the head's own up in the scene.
pub fn blend_neck_normals(scene: &mut crate::scene::Scene, head: &str, up: [f32; 3]) {
    let len = dot3(up, up).sqrt();
    if len < 1e-6 {
        return;
    }
    let up = up.map(|v| v / len);
    let is_head = |m: &crate::scene::SceneMesh| {
        m.texture
            .as_deref()
            .is_some_and(|t| t.eq_ignore_ascii_case(head))
    };
    let low = scene
        .meshes
        .iter()
        .filter(|m| is_head(m))
        .flat_map(|m| m.positions.iter())
        .map(|p| dot3(up, *p))
        .fold(f32::MAX, f32::min);
    if low == f32::MAX {
        return;
    }
    let body: Vec<([f32; 3], [f32; 3])> = scene
        .meshes
        .iter()
        .filter(|m| !is_head(m))
        .flat_map(|m| m.positions.iter().copied().zip(m.normals.iter().copied()))
        .collect();
    for m in scene.meshes.iter_mut().filter(|m| is_head(m)) {
        for (p, n) in m.positions.iter().zip(m.normals.iter_mut()) {
            let above = dot3(up, *p) - low;
            if above >= NECK_SHADE_METRES {
                continue;
            }
            let near = body
                .iter()
                .map(|(q, bn)| {
                    let d = [p[0] - q[0], p[1] - q[1], p[2] - q[2]];
                    (dot3(d, d), *bn)
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
            let Some((d2, bn)) = near else { continue };
            if d2.sqrt() > NECK_SHADE_NEAR_METRES {
                continue;
            }
            // Whole where the neck comes out of the collar, and gone `NECK_SHADE_METRES` up.
            let w = 1.0
                - ((above - NECK_SHADE_WHOLE_METRES)
                    / (NECK_SHADE_METRES - NECK_SHADE_WHOLE_METRES))
                    .clamp(0.0, 1.0);
            let mixed = [0, 1, 2].map(|k| n[k] * (1.0 - w) + bn[k] * w);
            let l = dot3(mixed, mixed).sqrt();
            if l > 1e-6 {
                *n = mixed.map(|v| v / l);
            }
        }
    }
}

/// One underwear a player may choose: the standard paint (id empty) or a modder's sheets in a folder of the owner's disk.
#[derive(Clone, Debug, PartialEq)]
pub struct UnderwearVariant {
    /// What is saved with the character: the folder's name, empty for the standard paint.
    pub id: String,
    /// What the list shows.
    pub name: String,
    /// Whether it fits this game's bodies. A sheet laid out for another body does not, and is marked "needs fix".
    pub fits: bool,
    /// Why not, or what it covers.
    pub note: String,
}

/// The folder that holds modders' folders: `KOTOR_MODS_ROOT` if set, else `Mods` beside the project. Read at run time, from the
/// owner's disk, and never kept in the repository or the build.
pub fn mods_root() -> std::path::PathBuf {
    std::env::var_os("KOTOR_MODS_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Users\User\Projects\Mods"))
}

/// The sheet of a skin family of the middle body in a folder: `a` Asian, `b` Black, none Caucasian. Names are matched without
/// case. Only files directly in the folder count: a mod with a mesh of its own keeps its sheets elsewhere.
fn sheet_file(dir: &std::path::Path, family: char) -> Option<std::path::PathBuf> {
    let want = match family {
        'a' => "pfbama01.tga",
        'b' => "pfbamb01.tga",
        _ => "pfbamc01.tga",
    };
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case(want))
        })
}

/// The choices for a player's underwear: the standard paint first, then a variant for each folder under [`mods_root`] that
/// holds a sheet of the middle body. A new folder is a new choice, with no change to the program.
pub fn list_underwear_variants() -> Vec<UnderwearVariant> {
    let mut out = vec![UnderwearVariant {
        id: String::new(),
        name: "Standard".to_string(),
        fits: true,
        note: "Our own paint, for every body and skin".to_string(),
    }];
    let Ok(read) = std::fs::read_dir(mods_root()) else {
        return out;
    };
    let mut found: Vec<UnderwearVariant> = read
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter(|e| {
            ['a', 'b', 'c']
                .iter()
                .any(|f| sheet_file(&e.path(), *f).is_some())
        })
        .filter_map(|e| e.file_name().into_string().ok())
        .map(|name| UnderwearVariant {
            id: name.clone(),
            name,
            fits: true,
            note: "Our bra and briefs in the blues of its sheet, for every body".to_string(),
        })
        .collect();
    found.sort_by_key(|a| a.name.to_lowercase());
    out.extend(found);
    out
}

/// The folder of a variant, none for the standard paint or a folder that is gone.
pub fn variant_dir(id: &str) -> Option<std::path::PathBuf> {
    if id.is_empty() || id.contains(['/', '\\']) || id.contains("..") {
        return None;
    }
    let dir = mods_root().join(id);
    dir.is_dir().then_some(dir)
}

/// The name a texture goes by when a creature wears a variant: `pfbama01@Blue Undies-3-1`. The standard paint keeps its name.
pub fn variant_texture_name(texture: &str, id: &str) -> String {
    if id.is_empty() {
        texture.to_string()
    } else {
        format!("{texture}@{id}")
    }
}

/// A texture's own name and its variant, from a name made by [`variant_texture_name`].
pub fn split_variant(name: &str) -> (&str, &str) {
    name.split_once('@').unwrap_or((name, ""))
}

/// How far in levels (summed over red, green and blue) a texel must be from the skin beside it to count as cloth to recolour.
const TINT_APART: f32 = 90.0;

/// The blues of a variant's sheets: its dark, middle and light colours.
pub type Palette = [[f32; 3]; 3];

/// A variant's colours, read from the blue texels of its sheet at run time: the means of the darkest tenth, the middle fifth and
/// the lightest tenth of them by brightness. None for the standard paint, a folder that is gone, or a sheet with no blue.
pub fn variant_palette(id: &str) -> Option<Palette> {
    let dir = variant_dir(id)?;
    let bytes = ['a', 'b', 'c']
        .iter()
        .find_map(|f| std::fs::read(sheet_file(&dir, *f)?).ok())?;
    let img = kotor_image::parse_tga(&bytes).ok()?;
    let mut blues: Vec<([f32; 3], f32)> = img
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| {
            i32::from(p[2]) > i32::from(p[0]) + 50 && i32::from(p[2]) > i32::from(p[1]) + 10
        })
        .map(|p| {
            let c = [f32::from(p[0]), f32::from(p[1]), f32::from(p[2])];
            (c, 0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2])
        })
        .collect();
    if blues.len() < 100 {
        return None;
    }
    blues.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mean = |from: f32, to: f32| {
        let n = blues.len() as f32;
        let part = &blues[(n * from) as usize..((n * to) as usize).max((n * from) as usize + 1)];
        let mut s = [0.0f32; 3];
        for (c, _) in part {
            for k in 0..3 {
                s[k] += c[k];
            }
        }
        s.map(|v| v / part.len() as f32)
    };
    Some([mean(0.0, 0.1), mean(0.4, 0.6), mean(0.9, 1.0)])
}

impl Plan {
    /// Turn the cloth that is left, the bra's cups, band and straps and the briefs, to the blues of `palette`: each texel's
    /// brightness, against the brightest cloth, picks a colour on the way from dark through middle to light. Skin is not
    /// touched. For a picture this plan was applied to, at its own size.
    pub fn tint(&self, img: &mut Image, palette: &Palette) {
        let (w, h) = (img.width as usize, img.height as usize);
        let at = |x: usize, y: usize| (y * WORK / h.max(1)) * WORK + x * WORK / w.max(1);
        // The sheet's skin, as it is now: what lies near that is skin whatever the marks say, as a shaded thigh may.
        let skin = {
            let (mut sum, mut count) = ([0.0f32; 3], 0.0f32);
            for y in (0..h).step_by(3) {
                for x in (0..w).step_by(3) {
                    if self.marks[at(x, y)] == SKIN {
                        for k in 0..3 {
                            sum[k] += f32::from(img.rgba[(y * w + x) * 4 + k]);
                        }
                        count += 1.0;
                    }
                }
            }
            sum.map(|v| v / count.max(1.0))
        };
        let far = |p: &[u8]| {
            (0..3)
                .map(|k| (f32::from(p[k]) - skin[k]).abs())
                .sum::<f32>()
                > TINT_APART
        };
        let light =
            |p: &[u8]| 0.3 * f32::from(p[0]) + 0.59 * f32::from(p[1]) + 0.11 * f32::from(p[2]);
        let cloth = |x: usize, y: usize, rgba: &[u8]| {
            matches!(self.marks[at(x, y)], KEEP | CUP | PAINT) && far(&rgba[(y * w + x) * 4..])
        };
        let mut lums = Vec::new();
        for y in 0..h {
            for x in 0..w {
                if cloth(x, y, &img.rgba) {
                    lums.push(light(&img.rgba[(y * w + x) * 4..]));
                }
            }
        }
        if lums.is_empty() {
            return;
        }
        lums.sort_by(f32::total_cmp);
        let top = lums[(lums.len() * 98 / 100).min(lums.len() - 1)].max(1.0);
        for y in 0..h {
            for x in 0..w {
                if !cloth(x, y, &img.rgba) {
                    continue;
                }
                let p = (y * w + x) * 4;
                let t = (light(&img.rgba[p..]) / top).clamp(0.0, 1.0);
                let (a, b, f) = if t < 0.5 {
                    (palette[0], palette[1], t * 2.0)
                } else {
                    (palette[1], palette[2], (t - 0.5) * 2.0)
                };
                for k in 0..3 {
                    img.rgba[p + k] = (a[k] + (b[k] - a[k]) * f + 0.5).clamp(0.0, 255.0) as u8;
                }
            }
        }
    }
}

#[cfg(test)]
mod strap_tests {
    use super::*;

    /// The straps lean out to the shoulders and meet there from the front and the back: "Bra straps still do not go over the
    /// collarbones correctly and end at the torso."
    #[test]
    fn the_straps_go_over_the_shoulders_and_meet_at_the_top() {
        let half = 0.228;
        let (front, back) = (strap_at(STRAP_TOP, true), strap_at(STRAP_TOP, false));
        assert!((front - back).abs() < 1e-4, "{front} and {back}");
        assert!(front > STRAP_AT + 0.1, "{front}");
        // On the torso, up the front and the back, at the strap's own place, it is a strap; beside it, it is not.
        for (y, h) in [(0.1, 0.95), (-0.1, 0.95), (0.1, 1.02), (-0.1, 1.02)] {
            let x = strap_at(h, y > 0.0) * half;
            assert!(bra_part(x, y, h, half) == Part::Strap, "y {y} h {h}");
            assert!(bra_part(x + 0.05, y, h, half) != Part::Strap, "y {y} h {h}");
        }
        // Low on the back it starts where the band is, at the old place.
        assert!((strap_at(BAND_HIGH, false) - STRAP_AT).abs() < 1e-4);
    }
}
