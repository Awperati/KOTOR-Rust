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
//! painted, so it borrows the muscles of the one body whose belly is bare, as light and dark only, whatever the skin.
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
const MARGIN: usize = 3;
/// Passes of the fill, and how far each overshoots to arrive sooner.
const FILL_PASSES: usize = 300;
const OVERSHOOT: f32 = 1.85;
/// Rounds of softening the edge of what is painted, a texel each.
const FEATHER: usize = 2;
/// How near two edges of the body are to be one seam, in metres.
const SEAM_METRES: f32 = 0.015;
/// Patches of grain, in texels, and how far a texel is from the smooth skin it counts as grain against.
const GRAIN_BLOCK: usize = 24;
const GRAIN_BLUR: usize = 3;
/// The bare belly that is borrowed, as cells across the body and up it, and how far a cell's own light is measured against
/// what is round it. Near kept skin it fades out over `RELIEF_FADE` texels, so that the fill meets it with no step.
const BARE_BELLY: &str = "pfbal";
const RELIEF_WIDE: usize = 64;
const RELIEF_TALL: usize = 48;
const RELIEF_ACROSS: i32 = 8;
const RELIEF_UP: i32 = 15;
const RELIEF_TRIM: usize = 2;
const RELIEF_FADE: f32 = 6.0;
/// Not a part of any piece of the sheet.
const NO_ISLAND: u32 = u32::MAX;
/// How close two vertices are to be one point of the body, and rounds of evening out its shading.
const WELD_METRES: f32 = 0.015;
const SOFTEN_ROUNDS: usize = 6;
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
    let mut weight: Vec<f32> = Vec::new();
    let mut soft: Vec<bool> = Vec::new();
    let mut point_of: Vec<Vec<usize>> = Vec::new();
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
                weight.push(0.0);
                soft.push(name != "Torso" || at[2] / top >= WAIST);
            }
            let n = mesh.normals.get(i).copied().unwrap_or([0.0, 0.0, 1.0]);
            let w = [0, 1, 2].map(|k| dot3(turn[k], n));
            for k in 0..3 {
                normal[id][k] += w[k];
            }
            weight[id] += 1.0;
            here.push(id);
        }
        for face in &mesh.faces {
            let f = face.indices.map(|v| here[v as usize]);
            joins.extend([(f[0], f[1]), (f[1], f[2]), (f[2], f[0])]);
        }
        point_of.push(here);
    }
    for _ in 0..SOFTEN_ROUNDS {
        let last = normal.clone();
        for (a, b) in &joins {
            for (to, from) in [(*a, *b), (*b, *a)] {
                if soft[to] {
                    for k in 0..3 {
                        normal[to][k] += last[from][k] / weight[from].max(1.0);
                    }
                }
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
    // Two straps, front and back, from the top of the cups over the shoulders.
    if (CUP_TOP - 0.005..STRAP_TOP).contains(&h) && (side - STRAP_AT).abs() < STRAP_HALF {
        return Part::Strap;
    }
    if !front && (BAND_HIGH..STRAP_TOP).contains(&h) && (side - STRAP_AT).abs() < STRAP_HALF {
        return Part::Strap;
    }
    Part::None
}

const BAND_LOW: f32 = 0.833;
const BAND_HIGH: f32 = 0.849;
const CUP_TOP: f32 = 0.897;
const STRAP_TOP: f32 = 1.0;
const CUP_INNER: f32 = 0.03;
const CUP_OUTER: f32 = 0.57;
const STRAP_AT: f32 = 0.33;
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
    /// How much each texel takes of the fill and of painted cloth.
    fill_alpha: Vec<f32>,
    paint_alpha: Vec<f32>,
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

/// A texel of the sheet's grid.
fn texel(uv: [f32; 2]) -> usize {
    let s = WORK as f32;
    let x = ((uv[0] * s) as i32).clamp(0, WORK as i32 - 1) as usize;
    let y = (((1.0 - uv[1]) * s) as i32).clamp(0, WORK as i32 - 1) as usize;
    y * WORK + x
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

/// An edge of a mesh that only one face has: an edge of a piece of the sheet, in place on the body.
struct OpenEdge {
    at: [[f32; 3]; 2],
    uv: [[f32; 2]; 2],
}

fn open_edges(mesh: &Mesh, at: impl Fn(&[f32; 3]) -> [f32; 3], out: &mut Vec<OpenEdge>) {
    let mut seen = std::collections::BTreeMap::new();
    for face in &mesh.faces {
        let i = face.indices.map(|v| v as usize);
        for (a, b) in [(i[0], i[1]), (i[1], i[2]), (i[2], i[0])] {
            *seen.entry((a.min(b), a.max(b))).or_insert(0) += 1;
        }
    }
    for ((a, b), times) in seen {
        if times == 1 && a < mesh.uvs.len() && b < mesh.uvs.len() {
            out.push(OpenEdge {
                at: [at(&mesh.positions[a]), at(&mesh.positions[b])],
                uv: [mesh.uvs[a], mesh.uvs[b]],
            });
        }
    }
}

/// Texels that are one place on the body across a seam of the sheet. An edge may meet several shorter ones, so each point
/// along one looks for the nearest point of any other.
fn seam_links(edges: &[OpenEdge]) -> Vec<(u32, u32)> {
    let mut links = Vec::new();
    let sub = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    for (i, e) in edges.iter().enumerate() {
        let long = (e.uv[0][0] - e.uv[1][0]).hypot(e.uv[0][1] - e.uv[1][1]) * WORK as f32;
        let steps = (long.ceil() as usize + 1) * 2;
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let p = [0, 1, 2].map(|k| e.at[0][k] + t * (e.at[1][k] - e.at[0][k]));
            let mut best = (SEAM_METRES, None);
            for (j, f) in edges.iter().enumerate() {
                if i == j {
                    continue;
                }
                let along = sub(f.at[1], f.at[0]);
                let share =
                    (dot3(sub(p, f.at[0]), along) / dot3(along, along).max(1e-9)).clamp(0.0, 1.0);
                let q = [0, 1, 2].map(|k| f.at[0][k] + share * along[k]);
                let off = sub(p, q);
                let away = dot3(off, off).sqrt();
                if away < best.0 {
                    best = (away, Some((j, share)));
                }
            }
            let Some((j, share)) = best.1 else {
                continue;
            };
            let mix = |e: &OpenEdge, t: f32| {
                [
                    e.uv[0][0] + t * (e.uv[1][0] - e.uv[0][0]),
                    e.uv[0][1] + t * (e.uv[1][1] - e.uv[0][1]),
                ]
            };
            let (a, b) = (texel(mix(e, t)), texel(mix(&edges[j], share)));
            if a != b {
                links.push((a.min(b) as u32, a.max(b) as u32));
            }
        }
    }
    links.sort_unstable();
    links.dedup();
    links
}

/// Soften a mask over a few rounds, staying inside a piece of the sheet.
fn feather(mask: &[f32], islands: &[u32]) -> Vec<f32> {
    let mut out = mask.to_vec();
    for _ in 0..FEATHER {
        let last = out.clone();
        for n in 0..WORK * WORK {
            if islands[n] == NO_ISLAND {
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
                    if islands[m] == islands[n] {
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
    if !(WAIST..BAND_LOW).contains(&h) {
        return None;
    }
    let across = ((x / half * 0.5 + 0.5) * RELIEF_WIDE as f32) as usize;
    let up = ((h - WAIST) / (BAND_LOW - WAIST) * RELIEF_TALL as f32) as usize;
    let side = usize::from(y > 0.0);
    (across < RELIEF_WIDE && up < RELIEF_TALL)
        .then_some((side * RELIEF_TALL + up) * RELIEF_WIDE + across)
}

/// The bare belly's light and dark at a place on another body at rest, between cells.
fn relief_at(belly: &[f32], x: f32, y: f32, h: f32, half: f32) -> f32 {
    if !(WAIST..BAND_LOW).contains(&h) {
        return 1.0;
    }
    let across =
        ((x / half * 0.5 + 0.5) * RELIEF_WIDE as f32 - 0.5).clamp(0.0, RELIEF_WIDE as f32 - 1.0);
    let up = ((h - WAIST) / (BAND_LOW - WAIST) * RELIEF_TALL as f32 - 0.5)
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
    let mut marks = vec![NOTHING; WORK * WORK];
    let mut islands = vec![NO_ISLAND; WORK * WORK];
    let mut straps = vec![false; WORK * WORK];
    let torso_islands = face_islands(mesh, 0);
    texel_pass(mesh, at, |n, p, face| {
        islands[n] = torso_islands[face];
        let h = p[2] / top;
        if let Some(belly) = &belly {
            relief[n] = relief_at(belly, p[0], p[1], h, half);
        }
        let cloth = is_cloth(pixels[n], skin);
        if h < WAIST {
            marks[n] = if cloth { KEEP } else { SKIN };
            return;
        }
        let part = bra_part(p[0], p[1], h, half);
        straps[n] = part == Part::Strap;
        marks[n] = match (cloth, part) {
            (true, Part::None) => CLEAR,
            (false, Part::Band | Part::Strap) => PAINT,
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
    let mut first = mesh.positions.len() as u32;
    let mut edges = Vec::new();
    open_edges(mesh, at, &mut edges);
    for (name, arm) in &arms {
        // Arms are bare in this style, so a sleeve goes as well. They share the sheet with the torso.
        let arm_islands = face_islands(arm, first);
        first += arm.positions.len() as u32;
        texel_pass(
            arm,
            |p| placed(&world, name, p),
            |n, _, face| {
                islands[n] = arm_islands[face];
                marks[n] = if is_cloth(pixels[n], skin) {
                    CLEAR
                } else {
                    SKIN
                };
            },
        );
        open_edges(arm, |p| placed(&world, name, p), &mut edges);
    }
    // Skin beside what goes is tinted by it.
    for _ in 0..MARGIN {
        let last = marks.clone();
        for n in 0..WORK * WORK {
            if last[n] == SKIN && beside(n).any(|m| last[m] == CLEAR && islands[m] == islands[n]) {
                marks[n] = CLEAR;
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
    Some(Plan::new(marks, islands, seam_links(&edges), relief))
}

impl Plan {
    fn new(
        marks: Vec<u8>,
        islands: Vec<u32>,
        links: Vec<(u32, u32)>,
        mut relief: Vec<f32>,
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
        let mut front: Vec<usize> = (0..WORK * WORK).filter(|n| marks[*n] == SKIN).collect();
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
        let mask = |want: u8| -> Vec<f32> { marks.iter().map(|m| f32::from(*m == want)).collect() };
        let fill_alpha = feather(&mask(CLEAR), &islands);
        let paint_alpha = feather(&mask(PAINT), &islands);
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
            paint_alpha,
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

    /// The smooth skin at every texel: what stays as it is, and in what goes the mean of what is round it.
    fn solve(&self, small: &[[f32; 3]]) -> Vec<[f32; 3]> {
        let mut known: Vec<bool> = self.marks.iter().map(|m| *m == SKIN).collect();
        let mut field: Vec<[f32; 3]> = small.to_vec();
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
        let mut left: Vec<usize> = (0..self.fill.len()).collect();
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
                if near.is_empty() {
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
        let mut out = img.rgba.clone();
        for y in 0..h {
            for x in 0..w {
                let u = (x as f32 + 0.5) * WORK as f32 / w as f32 - 0.5;
                let v = (y as f32 + 0.5) * WORK as f32 / h as f32 - 0.5;
                let a = bilinear1(&self.fill_alpha, u, v);
                let b = bilinear1(&self.paint_alpha, u, v);
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
                    let grain = grain_at(u, v);
                    let shade = bilinear1(&self.relief, u, v);
                    for k in 0..3 {
                        c[k] += a * (low[k] * shade + grain[k] - c[k]);
                    }
                }
                for k in 0..3 {
                    c[k] += b * (cloth[k] - c[k]);
                    out[p + k] = (c[k] + 0.5).clamp(0.0, 255.0) as u8;
                }
            }
        }
        img.rgba = out;
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
        SeamReport {
            colour: colour / patches.max(1.0),
            worst_colour: worst,
            edge: edge / edges.max(1.0),
            wrap: wrap / wraps.max(1.0),
            cloth_left: left / all.max(1.0),
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
