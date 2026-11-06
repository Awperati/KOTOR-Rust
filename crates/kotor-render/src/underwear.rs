//! "Underwear style: Conventional", the owner's own taste, off the game's look. The player's bare bodies wear a sports top
//! painted on one closed skin: there is no mesh of cloth to cut, and taking triangles away would open holes. So the rule
//! runs on the texture as it loads, in memory: cloth outside a cup, a band and two straps becomes the skin round it.
//!
//! The shape is given in metres on the body at rest, so it needs no drawing per texture and fits any skin and resolution.
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

/// The body a texture is painted for: `pfbam01`, `pfbamb03`, `pfbasa02`, but no armour's.
pub fn body_of_texture(name: &str) -> Option<String> {
    let n = name.to_ascii_lowercase();
    let rest = n.strip_prefix("pfba")?;
    let size = rest.chars().next().filter(|c| "sml".contains(*c))?;
    let rest = &rest[1..];
    let rest = rest.strip_prefix(['a', 'b']).unwrap_or(rest);
    (rest.len() == 2 && rest.bytes().all(|b| b.is_ascii_digit())).then(|| format!("pfba{size}"))
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
}

fn is_cloth(p: &[u8], skin: [f32; 3]) -> bool {
    let (r, g, b) = (p[0] as f32, p[1] as f32, p[2] as f32);
    // Skin keeps its hue however it is lit. Anything darker or of another hue is cloth: dark leather, gold trim, grey and
    // white, red. The three bodies wear different things.
    let bright = r + g + b;
    if bright < 0.72 * (skin[0] + skin[1] + skin[2]) {
        return true;
    }
    (b / r.max(1.0) - skin[2] / skin[0].max(1.0)).abs() > 0.09
        || (g / r.max(1.0) - skin[1] / skin[0].max(1.0)).abs() > 0.09
}

fn pixels_are_skin(p: &[u8; 3], skin: [f32; 3]) -> bool {
    !is_cloth(p, skin)
}

fn torso(m: &Model) -> Option<&kotor_model::Node> {
    m.root.find("Torso").filter(|n| n.mesh.is_some())
}

fn texel_pass(mesh: &Mesh, at: impl Fn(&[f32; 3]) -> [f32; 3], mut f: impl FnMut(usize, [f32; 3])) {
    for face in &mesh.faces {
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
                f(y * WORK + x, p);
            }
        }
    }
}

/// Sample an image at the work size.
fn work_size(img: &Image) -> Vec<[u8; 3]> {
    let (w, h) = (img.width as usize, img.height as usize);
    (0..WORK * WORK)
        .map(|n| {
            let (x, y) = (n % WORK, n / WORK);
            let p = ((y * h / WORK) * w + x * w / WORK) * 4;
            [img.rgba[p], img.rgba[p + 1], img.rgba[p + 2]]
        })
        .collect()
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
    let w = world
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(&node.name))?
        .1;
    let at = |p: &[f32; 3]| {
        [0, 1, 2].map(|r| w[r][0] * p[0] + w[r][1] * p[1] + w[r][2] * p[2] + w[r][3])
    };
    let (mut top, mut half) = (0.0f32, 0.0f32);
    for p in &mesh.positions {
        let q = at(p);
        top = top.max(q[2]);
        half = half.max(q[0].abs());
    }
    // The skin: the forearms, which no body covers.
    let mut sum = [0.0f32; 3];
    let mut count = 0.0f32;
    if let Some(arm) = m.root.find("RArm").and_then(|n| n.mesh.as_ref()) {
        texel_pass(
            arm,
            |p| *p,
            |n, p| {
                if p[2] < 1.05 {
                    for k in 0..3 {
                        sum[k] += pixels[n][k] as f32;
                    }
                    count += 1.0;
                }
            },
        );
    }
    let skin = sum.map(|v| v / count.max(1.0));
    let mut marks = vec![NOTHING; WORK * WORK];
    texel_pass(mesh, at, |n, p| {
        let h = p[2] / top;
        let cloth = is_cloth(&pixels[n], skin);
        if h < WAIST {
            marks[n] = if cloth { KEEP } else { SKIN };
            return;
        }
        let part = bra_part(p[0], p[1], h, half);
        marks[n] = match (cloth, part) {
            (true, Part::None) => CLEAR,
            (false, Part::Band | Part::Strap) => PAINT,
            (false, _) => SKIN,
            (true, Part::Cup) => CUP,
            _ => KEEP,
        };
    });
    for name in ["RArm", "LArm"] {
        let Some(mesh) = m.root.find(name).and_then(|n| n.mesh.as_ref()) else {
            continue;
        };
        // Arms are bare in this style, so a sleeve goes as well. They share the sheet with the torso.
        texel_pass(
            mesh,
            |p| *p,
            |n, _| {
                marks[n] = if pixels_are_skin(&pixels[n], skin) {
                    SKIN
                } else {
                    CLEAR
                };
            },
        );
    }
    // A texture is read a little past a triangle's edge, so what lies just outside cleared cloth goes too.
    for _ in 0..3 {
        let last = marks.clone();
        for n in 0..WORK * WORK {
            if last[n] != NOTHING {
                continue;
            }
            let (x, y) = (n % WORK, n / WORK);
            let near = [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .any(|(dx, dy)| {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    nx >= 0
                        && ny >= 0
                        && nx < WORK as i32
                        && ny < WORK as i32
                        && last[ny as usize * WORK + nx as usize] == CLEAR
                });
            if near {
                marks[n] = CLEAR;
            }
        }
    }
    Some(Plan { marks })
}

impl Plan {
    /// How many texels of the work size it takes cloth from, and paints.
    pub fn counts(&self) -> (usize, usize) {
        let n = |m: u8| self.marks.iter().filter(|v| **v == m).count();
        (n(CLEAR), n(PAINT))
    }

    /// Apply to a texture of the body, at its own size: `img` may be the game's or an upscaled one.
    pub fn apply(&self, img: &mut Image) {
        let (w, h) = (img.width as usize, img.height as usize);
        let small = work_size(img);
        // Skin grows over what is cleared, a ring at a time, from skin that stays.
        let mut fill: Vec<Option<[f32; 3]>> = (0..WORK * WORK)
            .map(|n| (self.marks[n] == SKIN).then(|| small[n].map(f32::from)))
            .collect();
        for _ in 0..WORK {
            let mut any = false;
            let last = fill.clone();
            for n in 0..WORK * WORK {
                if last[n].is_some() || self.marks[n] != CLEAR {
                    continue;
                }
                let (x, y) = (n % WORK, n / WORK);
                let mut sum = [0.0f32; 3];
                let mut c = 0.0;
                for (dx, dy) in [
                    (1i32, 0i32),
                    (-1, 0),
                    (0, 1),
                    (0, -1),
                    (1, 1),
                    (-1, -1),
                    (1, -1),
                    (-1, 1),
                ] {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx < 0 || ny < 0 || nx >= WORK as i32 || ny >= WORK as i32 {
                        continue;
                    }
                    if let Some(v) = last[ny as usize * WORK + nx as usize] {
                        for k in 0..3 {
                            sum[k] += v[k];
                        }
                        c += 1.0;
                    }
                }
                if c > 0.0 {
                    fill[n] = Some(sum.map(|v| v / c));
                    any = true;
                }
            }
            if !any {
                break;
            }
        }
        // What grew out in rings shows them: soften it, leaving the skin it grew from alone.
        for _ in 0..4 {
            let last = fill.clone();
            for n in 0..WORK * WORK {
                if self.marks[n] != CLEAR {
                    continue;
                }
                let (x, y) = (n % WORK, n / WORK);
                let mut sum = [0.0f32; 3];
                let mut c = 0.0;
                for dy in -2i32..=2 {
                    for dx in -2i32..=2 {
                        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                        if nx < 0 || ny < 0 || nx >= WORK as i32 || ny >= WORK as i32 {
                            continue;
                        }
                        if let Some(v) = last[ny as usize * WORK + nx as usize] {
                            for k in 0..3 {
                                sum[k] += v[k];
                            }
                            c += 1.0;
                        }
                    }
                }
                if c > 0.0 {
                    fill[n] = Some(sum.map(|v| v / c));
                }
            }
        }
        let average = {
            let (mut s, mut c) = ([0.0f32; 3], 0.0f32);
            for v in fill.iter().flatten() {
                for k in 0..3 {
                    s[k] += v[k];
                }
                c += 1.0;
            }
            s.map(|v| v / c.max(1.0))
        };
        // A band or strap on bare skin takes the colour of the cups in this very texture.
        let cloth = {
            let (mut s, mut c) = ([0.0f32; 3], 0.0f32);
            for n in 0..WORK * WORK {
                if self.marks[n] == CUP {
                    for k in 0..3 {
                        s[k] += small[n][k] as f32;
                    }
                    c += 1.0;
                }
            }
            s.map(|v| v / c.max(1.0))
        };
        for y in 0..h {
            for x in 0..w {
                let n = (y * WORK / h) * WORK + x * WORK / w;
                let p = (y * w + x) * 4;
                match self.marks[n] {
                    CLEAR => {
                        let v = fill[n].unwrap_or(average);
                        for k in 0..3 {
                            img.rgba[p + k] = v[k] as u8;
                        }
                    }
                    PAINT => {
                        for k in 0..3 {
                            img.rgba[p + k] = cloth[k] as u8;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
