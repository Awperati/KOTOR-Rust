//! Noto Sans, read from its TrueType file and drawn at the canvas's own pixels, so that it stays sharp at any size.
//!
//! Only what that file needs: `cmap` format 4, `glyf` simple and composite outlines, `hmtx` and the header tables. No
//! hinting and no kerning, since the file keeps its kerning in `GPOS`. A glyph's coverage is the exact area each outline
//! covers of a pixel, accumulated along its edges.

use crate::canvas::Canvas;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

/// Under the SIL Open Font License, which is beside it.
const BUNDLED: &[u8] = include_bytes!("../../../assets/fonts/NotoSans-Regular.ttf");
/// Segments for each curve of an outline: more than the eye needs at any size here.
const CURVE_STEPS: usize = 8;
const ON_CURVE: u8 = 1;
const X_SHORT: u8 = 2;
const Y_SHORT: u8 = 4;
const REPEAT: u8 = 8;
const X_SAME: u8 = 16;
const Y_SAME: u8 = 32;
const ARGS_ARE_WORDS: u16 = 1;
const ARGS_ARE_XY: u16 = 2;
const HAS_SCALE: u16 = 8;
const MORE_COMPONENTS: u16 = 32;
const HAS_XY_SCALE: u16 = 64;
const HAS_2X2: u16 = 128;
/// How deep composite glyphs may nest.
const MOST_NESTING: usize = 5;
/// Thin letters on dark go faint when coverage is taken straight.
const WEIGHT: f32 = 0.8;

/// One glyph's coverage at one size, with where it sits from the pen and the baseline.
pub struct Bitmap {
    width: usize,
    height: usize,
    left: i32,
    top: i32,
    coverage: Vec<u8>,
}

type Contour = Vec<(f32, f32, bool)>;
/// A straight piece of an outline, from one point to another.
type Edge = ((f32, f32), (f32, f32));

pub struct Face {
    data: Vec<u8>,
    tables: HashMap<[u8; 4], (usize, usize)>,
    units: f32,
    long_loca: bool,
    metrics: usize,
    pub ascent: f32,
    pub descent: f32,
    /// Layout units across for each one down, on the canvas text is measured for.
    across: AtomicU32,
    cache: Mutex<HashMap<(u32, char), Arc<Bitmap>>>,
}

impl std::fmt::Debug for Face {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Face")
    }
}

impl Face {
    /// The font that comes with the game.
    pub fn bundled() -> Option<Face> {
        Face::parse(BUNDLED.to_vec())
    }

    pub fn parse(data: Vec<u8>) -> Option<Face> {
        let u16_at = |at: usize| Some(u16::from_be_bytes([*data.get(at)?, *data.get(at + 1)?]));
        let u32_at = |at: usize| Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?));
        let count = u16_at(4)? as usize;
        let mut tables = HashMap::new();
        for n in 0..count {
            let at = 12 + n * 16;
            let tag: [u8; 4] = data.get(at..at + 4)?.try_into().ok()?;
            tables.insert(tag, (u32_at(at + 8)? as usize, u32_at(at + 12)? as usize));
        }
        let head = tables.get(b"head")?.0;
        let hhea = tables.get(b"hhea")?.0;
        let units = u16_at(head + 18)? as f32;
        let long_loca = u16_at(head + 50)? != 0;
        let ascent = u16_at(hhea + 4)? as i16 as f32 / units;
        let descent = -(u16_at(hhea + 6)? as i16 as f32) / units;
        let metrics = u16_at(hhea + 34)? as usize;
        for need in [b"cmap", b"glyf", b"loca", b"hmtx"] {
            tables.get(need)?;
        }
        Some(Face {
            tables,
            units,
            long_loca,
            metrics,
            ascent,
            descent,
            across: AtomicU32::new(1f32.to_bits()),
            cache: Mutex::new(HashMap::new()),
            data,
        })
    }

    /// How many layout units across one is down on the canvas text is being measured for.
    pub fn set_across(&self, ratio: f32) {
        self.across.store(ratio.to_bits(), Ordering::Relaxed);
    }

    fn across(&self) -> f32 {
        f32::from_bits(self.across.load(Ordering::Relaxed))
    }

    fn u16_at(&self, at: usize) -> Option<u16> {
        Some(u16::from_be_bytes([
            *self.data.get(at)?,
            *self.data.get(at + 1)?,
        ]))
    }

    fn i16_at(&self, at: usize) -> Option<i16> {
        self.u16_at(at).map(|v| v as i16)
    }

    /// The glyph for a character, from the Windows Unicode list of `cmap`, format 4. Unknown ones are glyph 0.
    fn glyph_index(&self, c: char) -> u16 {
        self.find_glyph(c as u32).unwrap_or(0)
    }

    fn find_glyph(&self, code: u32) -> Option<u16> {
        if code > 0xFFFF {
            return None;
        }
        let cmap = self.tables.get(b"cmap")?.0;
        let lists = self.u16_at(cmap + 2)? as usize;
        let list = (0..lists).find_map(|n| {
            let at = cmap + 4 + n * 8;
            let (platform, encoding) = (self.u16_at(at)?, self.u16_at(at + 2)?);
            let offset = u32::from_be_bytes(self.data.get(at + 4..at + 8)?.try_into().ok()?);
            let wanted = (platform == 3 && encoding == 1) || platform == 0;
            let start = cmap + offset as usize;
            (wanted && self.u16_at(start)? == 4).then_some(start)
        })?;
        let segments = self.u16_at(list + 6)? as usize / 2;
        let ends = list + 14;
        let starts = ends + segments * 2 + 2;
        let deltas = starts + segments * 2;
        let ranges = deltas + segments * 2;
        for s in 0..segments {
            let end = self.u16_at(ends + s * 2)? as u32;
            if code > end {
                continue;
            }
            let start = self.u16_at(starts + s * 2)? as u32;
            if code < start {
                return None;
            }
            let delta = self.u16_at(deltas + s * 2)?;
            let range = self.u16_at(ranges + s * 2)? as usize;
            if range == 0 {
                return Some((code as u16).wrapping_add(delta));
            }
            let at = ranges + s * 2 + range + (code - start) as usize * 2;
            let glyph = self.u16_at(at)?;
            return (glyph != 0).then(|| glyph.wrapping_add(delta));
        }
        None
    }

    /// How far a character moves the pen, in ems.
    pub fn advance(&self, c: char) -> f32 {
        let glyph = self.glyph_index(c) as usize;
        let Some(&(hmtx, _)) = self.tables.get(b"hmtx") else {
            return 0.0;
        };
        let row = glyph.min(self.metrics.saturating_sub(1));
        self.u16_at(hmtx + row * 4).unwrap_or(0) as f32 / self.units
    }

    fn glyph_range(&self, glyph: u16) -> Option<(usize, usize)> {
        let loca = self.tables.get(b"loca")?.0;
        let glyf = self.tables.get(b"glyf")?.0;
        let n = glyph as usize;
        let (a, b) = if self.long_loca {
            let at = |k: usize| -> Option<usize> {
                Some(u32::from_be_bytes(
                    self.data
                        .get(loca + k * 4..loca + k * 4 + 4)?
                        .try_into()
                        .ok()?,
                ) as usize)
            };
            (at(n)?, at(n + 1)?)
        } else {
            (
                self.u16_at(loca + n * 2)? as usize * 2,
                self.u16_at(loca + n * 2 + 2)? as usize * 2,
            )
        };
        Some((glyf + a, glyf + b))
    }

    /// The outline of a glyph, in font units.
    fn contours(&self, glyph: u16, depth: usize) -> Option<Vec<Contour>> {
        let (start, end) = self.glyph_range(glyph)?;
        if start >= end || depth > MOST_NESTING {
            return Some(Vec::new());
        }
        let count = self.i16_at(start)?;
        if count < 0 {
            return self.composite(start + 10, depth);
        }
        let count = count as usize;
        let mut at = start + 10;
        let mut ends = Vec::with_capacity(count);
        for _ in 0..count {
            ends.push(self.u16_at(at)? as usize);
            at += 2;
        }
        let points = ends.last().map_or(0, |e| e + 1);
        at += 2 + self.u16_at(at)? as usize;
        let mut flags = Vec::with_capacity(points);
        while flags.len() < points {
            let f = *self.data.get(at)?;
            at += 1;
            flags.push(f);
            if f & REPEAT != 0 {
                let times = *self.data.get(at)?;
                at += 1;
                for _ in 0..times {
                    flags.push(f);
                }
            }
        }
        let mut coordinates = |short: u8, same: u8| -> Option<Vec<f32>> {
            let mut out = Vec::with_capacity(points);
            let mut value = 0i32;
            for f in flags.iter().take(points) {
                if f & short != 0 {
                    let d = *self.data.get(at)? as i32;
                    at += 1;
                    value += if f & same != 0 { d } else { -d };
                } else if f & same == 0 {
                    value += self.i16_at(at)? as i32;
                    at += 2;
                }
                out.push(value as f32);
            }
            Some(out)
        };
        let xs = coordinates(X_SHORT, X_SAME)?;
        let ys = coordinates(Y_SHORT, Y_SAME)?;
        let mut out = Vec::with_capacity(count);
        let mut first = 0;
        for e in ends {
            out.push(
                (first..=e)
                    .map(|i| (xs[i], ys[i], flags[i] & ON_CURVE != 0))
                    .collect(),
            );
            first = e + 1;
        }
        Some(out)
    }

    fn composite(&self, mut at: usize, depth: usize) -> Option<Vec<Contour>> {
        let mut out = Vec::new();
        loop {
            let flags = self.u16_at(at)?;
            let part = self.u16_at(at + 2)?;
            at += 4;
            let (dx, dy) = if flags & ARGS_ARE_WORDS != 0 {
                let v = (self.i16_at(at)? as f32, self.i16_at(at + 2)? as f32);
                at += 4;
                v
            } else {
                let v = (
                    *self.data.get(at)? as i8 as f32,
                    *self.data.get(at + 1)? as i8 as f32,
                );
                at += 2;
                v
            };
            let (dx, dy) = if flags & ARGS_ARE_XY != 0 {
                (dx, dy)
            } else {
                (0.0, 0.0)
            };
            let number = |at: usize| Some(self.i16_at(at)? as f32 / 16384.0);
            let mut m = [1.0, 0.0, 0.0, 1.0];
            if flags & HAS_SCALE != 0 {
                m[0] = number(at)?;
                m[3] = m[0];
                at += 2;
            } else if flags & HAS_XY_SCALE != 0 {
                m[0] = number(at)?;
                m[3] = number(at + 2)?;
                at += 4;
            } else if flags & HAS_2X2 != 0 {
                m = [
                    number(at)?,
                    number(at + 2)?,
                    number(at + 4)?,
                    number(at + 6)?,
                ];
                at += 8;
            }
            for contour in self.contours(part, depth + 1)? {
                out.push(
                    contour
                        .into_iter()
                        .map(|(x, y, on)| (x * m[0] + y * m[2] + dx, x * m[1] + y * m[3] + dy, on))
                        .collect(),
                );
            }
            if flags & MORE_COMPONENTS == 0 {
                return Some(out);
            }
        }
    }

    /// A character's coverage at this many pixels to the em.
    fn bitmap(&self, c: char, px: f32) -> Arc<Bitmap> {
        let key = ((px * 8.0) as u32, c);
        if let Ok(cache) = self.cache.lock() {
            if let Some(b) = cache.get(&key) {
                return b.clone();
            }
        }
        let bitmap = Arc::new(self.rasterise(c, px));
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(key, bitmap.clone());
        }
        bitmap
    }

    fn rasterise(&self, c: char, px: f32) -> Bitmap {
        let scale = px / self.units;
        let contours = self.contours(self.glyph_index(c), 0).unwrap_or_default();
        let mut edges: Vec<Edge> = Vec::new();
        for contour in &contours {
            flatten(contour, scale, &mut edges);
        }
        let (mut left, mut right, mut low, mut high) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for (a, b) in &edges {
            for p in [a, b] {
                left = left.min(p.0);
                right = right.max(p.0);
                low = low.min(p.1);
                high = high.max(p.1);
            }
        }
        if edges.is_empty() {
            return Bitmap {
                width: 0,
                height: 0,
                left: 0,
                top: 0,
                coverage: Vec::new(),
            };
        }
        let (x0, y0) = (left.floor() as i32, high.ceil() as i32);
        let width = (right.ceil() as i32 - x0).max(1) as usize + 1;
        let height = (y0 - low.floor() as i32).max(1) as usize;
        let mut area = vec![0f32; width * height + 1];
        for (a, b) in &edges {
            // Down the picture, from its top.
            add_edge(
                &mut area,
                width,
                height,
                (a.0 - x0 as f32, y0 as f32 - a.1),
                (b.0 - x0 as f32, y0 as f32 - b.1),
            );
        }
        let mut coverage = vec![0u8; width * height];
        for row in 0..height {
            let mut sum = 0.0;
            for col in 0..width {
                sum += area[row * width + col];
                let c = sum.abs().min(1.0).powf(WEIGHT);
                coverage[row * width + col] = (c * 255.0) as u8;
            }
        }
        Bitmap {
            width,
            height,
            left: x0,
            top: y0,
            coverage,
        }
    }
}

/// A contour's straight pieces in pixels, with its curves cut up. Two off-curve points in a row have one on-curve between.
fn flatten(contour: &Contour, scale: f32, edges: &mut Vec<Edge>) {
    let n = contour.len();
    if n < 2 {
        return;
    }
    let point = |i: usize| {
        let p = contour[i % n];
        (p.0 * scale, p.1 * scale, p.2)
    };
    let middle = |a: (f32, f32), b: (f32, f32)| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let first_on = (0..n).find(|i| contour[*i].2);
    let (start, begin) = match first_on {
        Some(i) => {
            let p = point(i);
            ((p.0, p.1), i)
        }
        None => {
            let (a, b) = (point(0), point(1));
            (middle((a.0, a.1), (b.0, b.1)), 0)
        }
    };
    let mut pen = start;
    let mut control: Option<(f32, f32)> = None;
    for k in 1..=n {
        let p = point(begin + k);
        let here = (p.0, p.1);
        match (p.2, control) {
            (true, None) => {
                edges.push((pen, here));
                pen = here;
            }
            (true, Some(c)) => {
                curve(pen, c, here, edges);
                pen = here;
                control = None;
            }
            (false, None) => control = Some(here),
            (false, Some(c)) => {
                let on = middle(c, here);
                curve(pen, c, on, edges);
                pen = on;
                control = Some(here);
            }
        }
    }
    match control {
        Some(c) => curve(pen, c, start, edges),
        None if pen != start => edges.push((pen, start)),
        None => {}
    }
}

fn curve(a: (f32, f32), c: (f32, f32), b: (f32, f32), edges: &mut Vec<Edge>) {
    let mut last = a;
    for s in 1..=CURVE_STEPS {
        let t = s as f32 / CURVE_STEPS as f32;
        let u = 1.0 - t;
        let p = (
            u * u * a.0 + 2.0 * u * t * c.0 + t * t * b.0,
            u * u * a.1 + 2.0 * u * t * c.1 + t * t * b.1,
        );
        edges.push((last, p));
        last = p;
    }
}

/// Add what one edge does to each pixel's covered area; a sum along each row then gives the coverage.
fn add_edge(area: &mut [f32], width: usize, height: usize, a: (f32, f32), b: (f32, f32)) {
    if a.1 == b.1 {
        return;
    }
    let (sign, top, bottom) = if a.1 < b.1 { (1.0, a, b) } else { (-1.0, b, a) };
    let slope = (bottom.0 - top.0) / (bottom.1 - top.1);
    let mut x = top.0 + slope * (top.1.max(0.0) - top.1);
    let first = top.1.max(0.0) as usize;
    let last = (bottom.1.ceil().max(0.0) as usize).min(height);
    for y in first..last {
        let row = y * width;
        let dy = ((y + 1) as f32).min(bottom.1) - (y as f32).max(top.1);
        let next = x + slope * dy;
        let d = dy * sign;
        let (x0, x1) = if x < next { (x, next) } else { (next, x) };
        let (fx0, cx1) = (x0.floor(), x1.ceil());
        let i0 = fx0 as i32;
        let i1 = cx1 as i32;
        let put = |area: &mut [f32], col: i32, v: f32| {
            let col = col.clamp(0, width as i32 - 1) as usize;
            area[row + col] += v;
        };
        if i1 <= i0 + 1 {
            let mid = 0.5 * (x + next) - fx0;
            put(area, i0, d - d * mid);
            put(area, i0 + 1, d * mid);
        } else {
            let s = 1.0 / (x1 - x0);
            let x0f = x0 - fx0;
            let a0 = 0.5 * s * (1.0 - x0f) * (1.0 - x0f);
            let x1f = x1 - cx1 + 1.0;
            let am = 0.5 * s * x1f * x1f;
            put(area, i0, d * a0);
            if i1 == i0 + 2 {
                put(area, i0 + 1, d * (1.0 - a0 - am));
            } else {
                let a1 = s * (1.5 - x0f);
                put(area, i0 + 1, d * (a1 - a0));
                for xi in i0 + 2..i1 - 1 {
                    put(area, xi, d * s);
                }
                let a2 = a1 + (i1 - i0 - 3) as f32 * s;
                put(area, i1 - 1, d * (1.0 - a2 - am));
            }
            put(area, i1, d * am);
        }
        x = next;
    }
}

/// One of the game's fonts drawn in this face: the size comes from the line height of the font it stands for.
#[derive(Clone, Debug)]
pub struct Sized {
    pub face: Arc<Face>,
    /// Layout units down to the em.
    pub em: f32,
    pub line_height: i32,
}

impl Sized {
    /// Width in layout units, on the canvas text is being measured for.
    pub fn measure(&self, s: &str) -> i32 {
        let ems: f32 = s.chars().map(|c| self.face.advance(c)).sum();
        (ems * self.em * self.face.across()).ceil() as i32
    }

    /// Draw one line with its top-left at `(x, y)` in layout units; returns the width in layout units.
    pub fn draw_line(&self, canvas: &mut Canvas, x: i32, y: i32, s: &str, color: [f32; 3]) -> i32 {
        let px = self.em * canvas.scale;
        let f = &self.face;
        let line = self.line_height as f32 * canvas.scale;
        let ink = (f.ascent + f.descent) * px;
        let baseline =
            (y as f32 * canvas.scale + (line - ink) / 2.0 + f.ascent * px).round() as i32;
        let start = x as f32 * canvas.scale_x;
        let mut pen = start;
        let rgb = color.map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8);
        for c in s.chars() {
            let b = f.bitmap(c, px);
            let (ox, oy) = (pen.round() as i32 + b.left, baseline - b.top);
            for row in 0..b.height {
                for col in 0..b.width {
                    let a = b.coverage[row * b.width + col];
                    if a > 0 {
                        canvas.blend(
                            ox + col as i32,
                            oy + row as i32,
                            [rgb[0], rgb[1], rgb[2], a],
                        );
                    }
                }
            }
            pen += f.advance(c) * px;
        }
        ((pen - start) / canvas.scale_x).ceil() as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sized(em: f32) -> Sized {
        Sized {
            face: Arc::new(Face::bundled().unwrap()),
            em,
            line_height: em as i32 + 4,
        }
    }

    #[test]
    fn the_font_in_the_program_reads_and_measures_as_a_proportional_font() {
        let s = sized(16.0);
        assert!(s.measure("iiii") < s.measure("WWWW"));
        // Its own numbers: "W" is wider than half an em, "i" is narrower.
        assert!(s.face.advance('W') > 0.5 && s.face.advance('i') < 0.5);
        assert_eq!(s.measure(""), 0);
    }

    #[test]
    fn a_letter_has_ink_where_the_outline_is_and_composites_are_whole() {
        let face = Face::bundled().unwrap();
        for c in ['A', 'g', '\u{e9}'] {
            let b = face.rasterise(c, 32.0);
            let ink = b.coverage.iter().filter(|&&v| v > 128).count();
            assert!(ink > 20, "{c} has {ink} pixels of ink");
            assert!(b.coverage.contains(&0), "{c} has holes round it");
        }
        // The accent is on top of the "e": more ink, and higher.
        let (e, acute) = (face.rasterise('e', 32.0), face.rasterise('\u{e9}', 32.0));
        assert!(acute.top > e.top && acute.height > e.height);
    }

    #[test]
    fn a_character_the_font_lacks_and_a_broken_file_do_no_harm() {
        let s = sized(16.0);
        assert!(s.measure("\u{10ffff}") >= 0);
        assert!(Face::parse(vec![0; 40]).is_none());
        assert!(Face::parse(BUNDLED[..64].to_vec()).is_none());
    }

    #[test]
    fn letters_get_narrower_in_layout_units_when_the_canvas_is_wider_than_the_layout() {
        let s = sized(16.0);
        let full = s.measure("Knights of the Old Republic");
        s.face.set_across(0.75);
        let narrow = s.measure("Knights of the Old Republic");
        s.face.set_across(1.0);
        assert!(
            (narrow as f32 - full as f32 * 0.75).abs() <= 2.0,
            "{full} {narrow}"
        );
    }
}
