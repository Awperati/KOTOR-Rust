//! Bitmap fonts: a texture plus a TXI listing each character's cell.
//!
//! The TXI gives `upperleftcoords N` and `lowerrightcoords N`, each followed by N lines of `u v 0` (texture
//! coordinates with v = 1 at the top). A glyph's width is its cell width; the line is as tall as the tallest cell.
//! **Assumption:** spacing keys (`spacingR`, `spacingB`) are ignored because the fonts used here set them to 0.

use crate::canvas::{Canvas, Orient};
use kotor_archive::Install;
use kotor_image::{parse_tpc, Image};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    /// Cell in the texture, pixels: x, y (from the top), width, height.
    pub rect: [i32; 4],
}

#[derive(Clone, Debug)]
pub struct Font {
    pub name: String,
    pub image: Image,
    pub glyphs: Vec<Glyph>,
    pub line_height: i32,
    /// Drawn in this instead of its own picture, at the same line height.
    pub drawn_in: Option<crate::ttf::Sized>,
}

/// The glyph cells named by a font TXI, as `(upper_left, lower_right)` texture coordinates.
pub fn parse_font_txi(txi: &str) -> Vec<([f32; 2], [f32; 2])> {
    let lines: Vec<&str> = txi.lines().collect();
    let block = |key: &str| -> Vec<[f32; 2]> {
        let Some(at) = lines
            .iter()
            .position(|l| l.trim().to_ascii_lowercase().starts_with(key))
        else {
            return Vec::new();
        };
        let n: usize = lines[at]
            .split_whitespace()
            .nth(1)
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        lines[at + 1..]
            .iter()
            .take(n)
            .filter_map(|l| {
                let mut it = l.split_whitespace().map(|v| v.parse::<f32>());
                Some([it.next()?.ok()?, it.next()?.ok()?])
            })
            .collect()
    };
    block("upperleftcoords")
        .into_iter()
        .zip(block("lowerrightcoords"))
        .collect()
}

/// TPC rows are stored bottom-up; return the image top-down.
pub fn flip_rows(img: Image) -> Image {
    let row = img.width as usize * 4;
    let rgba = img
        .rgba
        .chunks_exact(row)
        .rev()
        .flatten()
        .copied()
        .collect();
    Image { rgba, ..img }
}

impl Font {
    pub fn from_parts(name: &str, image: Image, txi: &str) -> Font {
        let (w, h) = (image.width as f32, image.height as f32);
        let glyphs: Vec<Glyph> = parse_font_txi(txi)
            .into_iter()
            .map(|(ul, lr)| {
                // Texture coordinates have v = 0 at the bottom of the (top-down) image.
                let x = (ul[0] * w).round() as i32;
                let y = ((1.0 - ul[1]) * h).round() as i32;
                Glyph {
                    rect: [
                        x,
                        y,
                        ((lr[0] * w).round() as i32 - x).max(0),
                        (((1.0 - lr[1]) * h).round() as i32 - y).max(0),
                    ],
                }
            })
            .collect();
        let line_height = glyphs.iter().map(|g| g.rect[3]).max().unwrap_or(0);
        Font {
            name: name.to_string(),
            image,
            glyphs,
            line_height,
            drawn_in: None,
        }
    }

    /// Load a font by resource name (for example `fnt_d16x16`). Needs `Install::load_texture_packs`.
    pub fn load(inst: &mut Install, name: &str) -> Option<Font> {
        let t = parse_tpc(&inst.read(name, kotor_core::ResType::TPC).ok()?).ok()?;
        let f = Font::from_parts(name, flip_rows(t.image), &t.txi);
        (f.has_ink_for("Ae0")).then_some(f)
    }

    /// Whether every character of `s` has a glyph cell with some opaque pixel in it. A few font textures ship
    /// with a TXI that does not describe their picture (`fnt_d16x16`, `fnt_d10x10b`); those fail this.
    pub fn has_ink_for(&self, s: &str) -> bool {
        s.chars().all(|c| {
            self.glyph(c).is_some_and(|g| {
                let [x, y, w, h] = g.rect;
                (y..y + h).any(|yy| {
                    (x..x + w).any(|xx| {
                        let i = ((yy as u32 * self.image.width + xx as u32) * 4 + 3) as usize;
                        yy >= 0
                            && xx >= 0
                            && xx < self.image.width as i32
                            && i < self.image.rgba.len()
                            && self.image.rgba[i] > 100
                    })
                })
            })
        })
    }

    fn glyph(&self, c: char) -> Option<&Glyph> {
        let code = if (c as u32) < 256 {
            c as usize
        } else {
            b'?' as usize
        };
        self.glyphs.get(code)
    }

    /// Width in pixels of a string on one line.
    pub fn measure(&self, s: &str) -> i32 {
        if let Some(t) = &self.drawn_in {
            return t.measure(s);
        }
        s.chars()
            .filter_map(|c| self.glyph(c))
            .map(|g| g.rect[2])
            .sum()
    }

    /// Break `s` into lines no wider than `width` pixels, at spaces (a word wider than the line gets its own).
    pub fn wrap(&self, s: &str, width: i32) -> Vec<String> {
        // All that is drawn comes through here, so the spelling is done here.
        let s = crate::spelling::shown(s);
        let mut out = Vec::new();
        for para in s.split('\n') {
            let mut line = String::new();
            for word in para.split_whitespace() {
                let trial = if line.is_empty() {
                    word.to_string()
                } else {
                    format!("{line} {word}")
                };
                if line.is_empty() || self.measure(&trial) <= width {
                    line = trial;
                } else {
                    out.push(std::mem::take(&mut line));
                    line = word.to_string();
                }
            }
            out.push(line);
        }
        out
    }

    /// Draw one line with its top-left at `(x, y)`; returns the width drawn.
    pub fn draw_line(&self, canvas: &mut Canvas, x: i32, y: i32, s: &str, color: [f32; 3]) -> i32 {
        if let Some(t) = &self.drawn_in {
            return t.draw_line(canvas, x, y, s, color);
        }
        let mut pen = x;
        // Thin stock letters enlarged smoothly go faint; upscaled pictures keep their weight.
        let strength = if canvas.scale != 1.0 { 1.6 } else { 1.0 };
        for c in s.chars() {
            let Some(g) = self.glyph(c) else { continue };
            canvas.draw_image(
                &self.image,
                g.rect,
                [pen, y, g.rect[2], g.rect[3]],
                color,
                strength,
                Orient::default(),
            );
            pen += g.rect[2];
        }
        pen - x
    }
}
