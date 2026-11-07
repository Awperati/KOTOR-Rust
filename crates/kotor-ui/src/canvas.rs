//! A software canvas: RGBA pixels with alpha-blended rectangles and image blits.

use kotor_image::Image;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    /// Physical pixels per layout unit. `draw_image` and `fill_rect` take layout units; `blend` takes pixels.
    pub scale: f32,
    /// The same across, where a 640 wide screen is stretched over a wider window.
    pub scale_x: f32,
    /// RGBA, first row at the top.
    pub rgba: Vec<u8>,
    /// Only these pixels are drawn on, left, top, right and bottom, when part of a picture is drawn again.
    clip: Option<[i32; 4]>,
}

/// Pixels saved from a canvas, left, top, right and bottom.
pub struct KeptBox {
    place: [i32; 4],
    pixels: Vec<u8>,
}

/// How a texture is turned the right way up when it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Orient {
    /// Quarter turns clockwise, 0..=3.
    pub turns: u8,
    pub flip_x: bool,
    pub flip_y: bool,
}

/// A picture smoothly scaled to the pixels it covers, kept for next time. The HUD and the menus redraw whole every time
/// anything on them changes, and smooth scaling a window's worth of panels took 60 ms a time at 3840x2160.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ScaledKey {
    bytes: usize,
    width: u32,
    content: u64,
    region: [i32; 4],
    size: [i32; 2],
    tint: [u32; 3],
    alpha: u32,
    orient: (u8, bool, bool),
}

/// What is kept is let go all at once past this many pixels: a quarter of a gigabyte.
const SCALED_MOST_PIXELS: usize = 64 << 20;

/// Where a picture's pixels are, how many bytes, and how wide.
type PictureAt = (usize, usize, u32);

#[derive(Default)]
struct ScaledCache {
    pictures: HashMap<ScaledKey, Rc<Vec<[u8; 4]>>>,
    pixels: usize,
}

thread_local! {
    static SCALED: RefCell<ScaledCache> = RefCell::default();
    static CONTENTS: RefCell<HashMap<PictureAt, (u64, u64)>> = RefCell::default();
}

/// What a picture holds, as a number. By content and not by address, since a picture dropped and another loaded can have the
/// same one, as the slots' icons did. A large picture is taken in every sixteenth word, a font's once for each letter.
fn content_of(img: &Image) -> u64 {
    const WHOLE_UP_TO: usize = 256 << 10;
    /// Below this the sum is quicker than remembering it.
    const REMEMBERED_FROM: usize = 4 << 10;
    const REMEMBERED_MOST: usize = 4096;
    const TASTES: usize = 64;
    let words = img.rgba.as_chunks::<8>().0;
    let sum = |step: usize| {
        words
            .iter()
            .step_by(step.max(1))
            .fold(0xcbf2_9ce4_8422_2325, |h: u64, w| {
                (h ^ u64::from_le_bytes(*w))
                    .wrapping_mul(0x0000_0100_0000_01b3)
                    .rotate_left(23)
            })
    };
    let whole = || sum(if img.rgba.len() <= WHOLE_UP_TO { 1 } else { 16 });
    if img.rgba.len() < REMEMBERED_FROM {
        return whole();
    }
    // A font's sheet is summed for every letter on the screen, which took 15 ms of a hover at 2560x1440. So a large picture
    // is remembered by its address, as long as a few words taken across it are what they were.
    let taste = sum(words.len() / TASTES);
    let key = (img.rgba.as_ptr() as usize, img.rgba.len(), img.width);
    CONTENTS.with_borrow_mut(|known| match known.get(&key) {
        Some((t, content)) if *t == taste => *content,
        _ => {
            let content = whole();
            if known.len() >= REMEMBERED_MOST {
                known.clear();
            }
            known.insert(key, (taste, content));
            content
        }
    })
}

/// Forget what a picture held, when it is about to be changed where it is.
pub(crate) fn forget_content(img: &Image) {
    let key = (img.rgba.as_ptr() as usize, img.rgba.len(), img.width);
    CONTENTS.with_borrow_mut(|known| {
        known.remove(&key);
    });
}

/// For each pixel of a line, the whole pixel of the picture it falls on, the next, and how far it is to the next in 256ths.
fn smooth_stops(from: i32, side: i32, pixels: i32, most: i32) -> Vec<(u32, u32, u32)> {
    (0..pixels)
        .map(|n| {
            let at = ((n as f32 + 0.5) / pixels as f32 * side as f32 - 0.5)
                .clamp(0.0, (side - 1) as f32);
            let whole = at as i32;
            let next = (from + (whole + 1).min(side - 1)).clamp(0, most);
            (
                (from + whole).clamp(0, most) as u32,
                next as u32,
                ((at - whole as f32) * 256.0) as u32,
            )
        })
        .collect()
}

/// Red and blue, and green, each `t` 256ths of the way from one colour to another. Both halves of a pixel in one number each.
#[inline]
fn towards(a: [u32; 2], b: [u32; 2], t: u32) -> [u32; 2] {
    [
        ((a[0] * (256 - t) + b[0] * t) >> 8) & 0x00ff_00ff,
        ((a[1] * (256 - t) + b[1] * t) >> 8) & 0x00ff_00ff,
    ]
}

/// A smooth sample of `region` at fractions `u`, `v` of it, staying inside it so that a font's cells do not bleed.
fn bilinear(img: &Image, region: [i32; 4], u: f32, v: f32) -> [f32; 4] {
    let (fx, fy) = (
        (u * region[2] as f32 - 0.5).clamp(0.0, (region[2] - 1) as f32),
        (v * region[3] as f32 - 0.5).clamp(0.0, (region[3] - 1) as f32),
    );
    let (x0, y0) = (fx as i32, fy as i32);
    let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
    let at = |x: i32, y: i32| -> [f32; 4] {
        let (x, y) = (
            (region[0] + x.min(region[2] - 1)).clamp(0, img.width as i32 - 1),
            (region[1] + y.min(region[3] - 1)).clamp(0, img.height as i32 - 1),
        );
        let i = ((y as u32 * img.width + x as u32) * 4) as usize;
        let p = &img.rgba[i..i + 4];
        [p[0] as f32, p[1] as f32, p[2] as f32, p[3] as f32]
    };
    let (a, b, c, d) = (
        at(x0, y0),
        at(x0 + 1, y0),
        at(x0, y0 + 1),
        at(x0 + 1, y0 + 1),
    );
    let mut out = [0.0; 4];
    for k in 0..4 {
        let top = a[k] + (b[k] - a[k]) * tx;
        let bottom = c[k] + (d[k] - c[k]) * tx;
        out[k] = top + (bottom - top) * ty;
    }
    out
}

/// `region` of `img` smoothly scaled to `size`, as blendable pixels, from what is kept if it can be.
fn scaled_picture(
    img: &Image,
    region: [i32; 4],
    size: [i32; 2],
    tint: [f32; 3],
    alpha: f32,
    orient: Orient,
) -> Rc<Vec<[u8; 4]>> {
    let key = ScaledKey {
        bytes: img.rgba.len(),
        width: img.width,
        content: content_of(img),
        region,
        size,
        tint: tint.map(f32::to_bits),
        alpha: alpha.to_bits(),
        orient: (orient.turns, orient.flip_x, orient.flip_y),
    };
    SCALED.with_borrow_mut(|cache| {
        if let Some(found) = cache.pictures.get(&key) {
            return found.clone();
        }
        let made = Rc::new(scale_picture(img, region, size, tint, alpha, orient));
        if cache.pixels + made.len() > SCALED_MOST_PIXELS {
            *cache = ScaledCache::default();
        }
        cache.pixels += made.len();
        cache.pictures.insert(key, made.clone());
        made
    })
}

/// From this many pixels a picture is scaled on all the cores.
const SCALED_ON_ONE_THREAD: i32 = 200_000;

fn scale_picture(
    img: &Image,
    region: [i32; 4],
    [dw, dh]: [i32; 2],
    tint: [f32; 3],
    alpha: f32,
    orient: Orient,
) -> Vec<[u8; 4]> {
    // Shrinking a large picture: average a few samples per pixel.
    let samples = if dw * 4 < region[2] * 3 || dh * 4 < region[3] * 3 {
        2
    } else {
        1
    };
    let row = |oy: i32, out: &mut [[u8; 4]]| {
        for (ox, pixel) in (0..dw).zip(out.iter_mut()) {
            let mut acc = [0.0f32; 4];
            for sy in 0..samples {
                for sx in 0..samples {
                    let (mut u, mut v) = (
                        (ox as f32 + (sx as f32 + 0.5) / samples as f32) / dw as f32,
                        (oy as f32 + (sy as f32 + 0.5) / samples as f32) / dh as f32,
                    );
                    if orient.flip_x {
                        u = 1.0 - u;
                    }
                    if orient.flip_y {
                        v = 1.0 - v;
                    }
                    for _ in 0..orient.turns % 4 {
                        (u, v) = (v, 1.0 - u);
                    }
                    let p = bilinear(img, region, u, v);
                    let a = p[3] / 255.0;
                    acc[0] += p[0] * a;
                    acc[1] += p[1] * a;
                    acc[2] += p[2] * a;
                    acc[3] += a;
                }
            }
            if acc[3] > 0.0 {
                let n = (samples * samples) as f32;
                *pixel = [
                    (acc[0] / acc[3] * tint[0]).min(255.0) as u8,
                    (acc[1] / acc[3] * tint[1]).min(255.0) as u8,
                    (acc[2] / acc[3] * tint[2]).min(255.0) as u8,
                    (acc[3] / n * 255.0 * alpha).min(255.0) as u8,
                ];
            }
        }
    };
    let mut out = vec![[0u8; 4]; (dw * dh) as usize];
    if dw <= 0 || dh <= 0 {
        return out;
    }
    let row = &row;
    // A panel's background is half a second of one core at 3840 by 2160, and the owner sees it as the panel being slow to open.
    let threads = if dw * dh < SCALED_ON_ONE_THREAD {
        1
    } else {
        std::thread::available_parallelism().map_or(1, |n| n.get())
    };
    let rows_each = (dh as usize).div_ceil(threads);
    std::thread::scope(|scope| {
        for (n, rows) in out.chunks_mut(rows_each * dw as usize).enumerate() {
            let mut work = move || {
                for (y, line) in rows.chunks_mut(dw as usize).enumerate() {
                    row((n * rows_each + y) as i32, line);
                }
            };
            if threads == 1 {
                work();
            } else {
                scope.spawn(work);
            }
        }
    });
    out
}

impl Canvas {
    pub fn new(width: u32, height: u32) -> Canvas {
        Canvas {
            width,
            height,
            scale: 1.0,
            scale_x: 1.0,
            rgba: vec![0; (width * height * 4) as usize],
            clip: None,
        }
    }

    /// A canvas of exactly `pixels`, with `scale` pixels to a layout unit.
    pub fn scaled_layout(pixels: (u32, u32), scale: f32) -> Canvas {
        let mut c = Canvas::new(pixels.0, pixels.1);
        c.scale = scale;
        c.scale_x = scale;
        c
    }

    /// More drawing on a picture, with `scale` pixels to a layout unit.
    pub fn over(image: Image, scale: f32) -> Canvas {
        Canvas {
            width: image.width,
            height: image.height,
            scale,
            scale_x: scale,
            rgba: image.rgba,
            clip: None,
        }
    }

    /// Empty this box of pixels (left, top, width, height), and draw on nothing else until the next.
    pub fn redraw_only(&mut self, [left, top, wide, high]: [u32; 4]) {
        let (right, bottom) = ((left + wide).min(self.width), (top + high).min(self.height));
        let row = self.width as usize * 4;
        for y in top..bottom {
            let start = y as usize * row + left as usize * 4;
            self.rgba[start..y as usize * row + right as usize * 4].fill(0);
        }
        self.clip = Some([left as i32, top as i32, right as i32, bottom as i32]);
    }

    /// The part of this box of pixels (left, top, right, bottom) that may be drawn on.
    fn in_clip(&self, [left, top, right, bottom]: [i32; 4]) -> [i32; 4] {
        let [l, t, r, b] = self
            .clip
            .unwrap_or([0, 0, self.width as i32, self.height as i32]);
        [left.max(l), top.max(t), right.min(r), bottom.min(b)]
    }

    fn to_pixels(&self, x: i32, y: i32, w: i32, h: i32) -> [i32; 4] {
        let across = |v: i32| (v as f32 * self.scale_x).round() as i32;
        let down = |v: i32| (v as f32 * self.scale).round() as i32;
        [
            across(x),
            down(y),
            across(x + w) - across(x),
            down(y + h) - down(y),
        ]
    }

    /// The pixels of a box of layout units as they are now, to fade what is drawn over them with [`Canvas::fade_to`].
    pub fn keep_box(&self, x: i32, y: i32, w: i32, h: i32) -> KeptBox {
        let [x, y, w, h] = self.to_pixels(x, y, w, h);
        let [left, top, right, bottom] = self.in_clip([x, y, x + w, y + h]);
        let place = [
            left.max(0),
            top.max(0),
            right.min(self.width as i32),
            bottom.min(self.height as i32),
        ];
        let mut pixels = Vec::new();
        for row in place[1]..place[3] {
            let start = (row as usize * self.width as usize + place[0] as usize) * 4;
            let wide = (place[2] - place[0]).max(0) as usize * 4;
            pixels.extend_from_slice(&self.rgba[start..start + wide]);
        }
        KeptBox { place, pixels }
    }

    /// What has been drawn in a kept box since shows at this share of its strength.
    pub fn fade_to(&mut self, kept: &KeptBox, opacity: f32) {
        let [left, top, right, _] = kept.place;
        let wide = (right - left).max(0) as usize;
        if wide == 0 {
            return;
        }
        for (n, was) in kept.pixels.as_chunks::<4>().0.iter().enumerate() {
            let (x, y) = (left as usize + n % wide, top as usize + n / wide);
            let i = (y * self.width as usize + x) * 4;
            let now = &mut self.rgba[i..i + 4];
            if now == was {
                continue;
            }
            let (from, to) = (was[3] as f32 * (1.0 - opacity), now[3] as f32 * opacity);
            let alpha = from + to;
            for k in 0..3 {
                let c = (was[k] as f32 * from + now[k] as f32 * to) / alpha.max(1.0);
                now[k] = c.round().min(255.0) as u8;
            }
            now[3] = alpha.round().min(255.0) as u8;
        }
    }

    pub fn clear(&mut self, c: [u8; 4]) {
        for p in self.rgba.as_chunks_mut::<4>().0 {
            *p = c;
        }
    }

    pub fn into_image(self) -> Image {
        Image {
            width: self.width,
            height: self.height,
            rgba: self.rgba,
        }
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }

    /// Blend `src` (straight alpha, 0..=255) over the pixel at `(x, y)`; outside the canvas is ignored.
    pub fn blend(&mut self, x: i32, y: i32, src: [u8; 4]) {
        let [l, t, r, b] = self.in_clip([x, y, x + 1, y + 1]);
        if l >= r || t >= b || src[3] == 0 {
            return;
        }
        let i = ((y as u32 * self.width + x as u32) * 4) as usize;
        let a = src[3] as u32;
        for (d, s) in self.rgba[i..i + 3].iter_mut().zip(src) {
            *d = ((s as u32 * a + *d as u32 * (255 - a)) / 255) as u8;
        }
        let da = self.rgba[i + 3] as u32;
        self.rgba[i + 3] = (a + da * (255 - a) / 255) as u8;
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: [u8; 4]) {
        let [x, y, w, h] = self.to_pixels(x, y, w, h);
        let [left, top, right, bottom] = self.in_clip([x, y, x + w, y + h]);
        let [left, top] = [left.max(0), top.max(0)];
        let [right, bottom] = [right.min(self.width as i32), bottom.min(self.height as i32)];
        if left >= right || c[3] == 0 {
            return;
        }
        // A conversation's black bars are millions of pixels at 3840x2160.
        let a = c[3] as u32;
        for yy in top..bottom {
            let at = yy as usize * self.width as usize + left as usize;
            let (row, _) =
                self.rgba[at * 4..(at + (right - left) as usize) * 4].as_chunks_mut::<4>();
            if a == 255 {
                row.fill(c);
                continue;
            }
            for d in row {
                for (d, s) in d.iter_mut().zip(c).take(3) {
                    *d = ((s as u32 * a + *d as u32 * (255 - a)) / 255) as u8;
                }
                d[3] = (a + d[3] as u32 * (255 - a) / 255) as u8;
            }
        }
    }

    /// Leave only a veil of black, of this opacity, in a rectangle of layout units, for what lies behind the picture to show.
    pub fn clear_rect(&mut self, x: i32, y: i32, w: i32, h: i32, veil: u8) {
        let [x, y, w, h] = self.to_pixels(x, y, w, h);
        let [left, top, right, bottom] = self.in_clip([x, y, x + w, y + h]);
        for yy in top..bottom {
            for xx in left..right {
                let i = ((yy as u32 * self.width + xx as u32) * 4) as usize;
                self.rgba[i..i + 4].copy_from_slice(&[0, 0, 0, veil]);
            }
        }
    }

    /// Draw `region` (x, y, w, h in source pixels) of `img` into the rectangle `(x, y, w, h)` of layout units, scaling
    /// with nearest sampling at scale 1 and smoothly at any other. RGB is multiplied by `tint` (0..=1 per channel) and alpha by `alpha`.
    pub fn draw_image(
        &mut self,
        img: &Image,
        region: [i32; 4],
        dst: [i32; 4],
        tint: [f32; 3],
        alpha: f32,
        orient: Orient,
    ) {
        let [dx, dy, dw, dh] = self.to_pixels(dst[0], dst[1], dst[2], dst[3]);
        if dw <= 0 || dh <= 0 || region[2] <= 0 || region[3] <= 0 {
            return;
        }
        let [left, top, right, bottom] = self.in_clip([dx, dy, dx + dw, dy + dh]);
        if left >= right || top >= bottom {
            return;
        }
        if self.scale != 1.0 || self.scale_x != 1.0 {
            let scaled = scaled_picture(img, region, [dw, dh], tint, alpha, orient);
            // As `blend` has it, a row at a time: most of a window's pixels are all or nothing.
            let across = (right - left) as usize;
            for y in top..bottom {
                let from = ((y - dy) * dw + left - dx) as usize;
                let at = ((y as u32 * self.width + left as u32) * 4) as usize;
                let (row, _) = self.rgba[at..at + across * 4].as_chunks_mut::<4>();
                for (d, s) in row.iter_mut().zip(&scaled[from..from + across]) {
                    let a = s[3] as u32;
                    match a {
                        0 => {}
                        255 => *d = *s,
                        _ => {
                            for (d, s) in d.iter_mut().zip(s).take(3) {
                                *d = ((*s as u32 * a + *d as u32 * (255 - a)) / 255) as u8;
                            }
                            d[3] = (a + d[3] as u32 * (255 - a) / 255) as u8;
                        }
                    }
                }
            }
            return;
        }
        for oy in 0..dh {
            for ox in 0..dw {
                // Position inside the destination rectangle as a fraction, then undo the orientation.
                let (mut u, mut v) = ((ox as f32 + 0.5) / dw as f32, (oy as f32 + 0.5) / dh as f32);
                if orient.flip_x {
                    u = 1.0 - u;
                }
                if orient.flip_y {
                    v = 1.0 - v;
                }
                for _ in 0..orient.turns % 4 {
                    // One clockwise quarter turn of the picture: sample from the left of the destination.
                    (u, v) = (v, 1.0 - u);
                }
                let sx = region[0] + ((u * region[2] as f32) as i32).clamp(0, region[2] - 1);
                let sy = region[1] + ((v * region[3] as f32) as i32).clamp(0, region[3] - 1);
                if sx < 0 || sy < 0 || sx >= img.width as i32 || sy >= img.height as i32 {
                    continue;
                }
                let i = ((sy as u32 * img.width + sx as u32) * 4) as usize;
                let p = &img.rgba[i..i + 4];
                self.blend(
                    dx + ox,
                    dy + oy,
                    [
                        (p[0] as f32 * tint[0]).min(255.0) as u8,
                        (p[1] as f32 * tint[1]).min(255.0) as u8,
                        (p[2] as f32 * tint[2]).min(255.0) as u8,
                        (p[3] as f32 * alpha).min(255.0) as u8,
                    ],
                );
            }
        }
    }

    /// Draw `region` of a picture with nothing to see through, smoothly, into the rectangle `dst` of layout units. What is
    /// shown of a picture that moves all the time is another piece at every step: nothing here is kept, and it is quick.
    pub fn draw_moving(&mut self, img: &Image, region: [i32; 4], dst: [i32; 4]) {
        let [dx, dy, dw, dh] = self.to_pixels(dst[0], dst[1], dst[2], dst[3]);
        if dw <= 0 || dh <= 0 || region[2] <= 0 || region[3] <= 0 || img.width == 0 {
            return;
        }
        let [left, top, right, bottom] = self.in_clip([dx, dy, dx + dw, dy + dh]);
        let [left, top] = [left.max(0), top.max(0)];
        let [right, bottom] = [right.min(self.width as i32), bottom.min(self.height as i32)];
        if left >= right || top >= bottom {
            return;
        }
        let across = smooth_stops(region[0], region[2], dw, img.width as i32 - 1);
        let down = smooth_stops(region[1], region[3], dh, img.height as i32 - 1);
        let width = self.width as usize;
        let from_width = img.width as usize;
        let (pixels, _) = img.rgba.as_chunks::<4>();
        let parts = |p: &[u8; 4]| {
            let v = u32::from_le_bytes(*p);
            [v & 0x00ff_00ff, (v >> 8) & 0x0000_00ff]
        };
        for y in top..bottom {
            let (y0, y1, ty) = down[(y - dy) as usize];
            let (upper, lower) = (y0 as usize * from_width, y1 as usize * from_width);
            let at = y as usize * width + left as usize;
            let (row, _) =
                self.rgba[at * 4..(at + (right - left) as usize) * 4].as_chunks_mut::<4>();
            for (d, &(x0, x1, tx)) in row.iter_mut().zip(&across[(left - dx) as usize..]) {
                let (x0, x1) = (x0 as usize, x1 as usize);
                let up = towards(parts(&pixels[upper + x0]), parts(&pixels[upper + x1]), tx);
                let low = towards(parts(&pixels[lower + x0]), parts(&pixels[lower + x1]), tx);
                let [rb, g] = towards(up, low, ty);
                *d = (rb | (g << 8) | 0xff00_0000).to_le_bytes();
            }
        }
    }

    /// Draw a whole picture centred on a point of the layout, `side` layout units high and square in pixels, turned
    /// `angle` radians clockwise.
    pub fn draw_turned(
        &mut self,
        img: &Image,
        at: (f32, f32),
        side: f32,
        angle: f32,
        tint: [f32; 3],
    ) {
        let (cx, cy) = (at.0 * self.scale_x, at.1 * self.scale);
        let half = side * self.scale / 2.0;
        if half < 0.5 || img.width == 0 || img.height == 0 {
            return;
        }
        let (sin, cos) = angle.sin_cos();
        let reach = (half * std::f32::consts::SQRT_2).ceil() as i32;
        let region = [0, 0, img.width as i32, img.height as i32];
        for y in -reach..=reach {
            for x in -reach..=reach {
                let (dx, dy) = (x as f32 + 0.5, y as f32 + 0.5);
                let u = (dx * cos + dy * sin) / (half * 2.0) + 0.5;
                let v = (-dx * sin + dy * cos) / (half * 2.0) + 0.5;
                if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
                    continue;
                }
                let p = bilinear(img, region, u, v);
                self.blend(
                    cx as i32 + x,
                    cy as i32 + y,
                    [
                        (p[0] * tint[0]).min(255.0) as u8,
                        (p[1] * tint[1]).min(255.0) as u8,
                        (p[2] * tint[2]).min(255.0) as u8,
                        p[3] as u8,
                    ],
                );
            }
        }
    }

    /// Nearest-neighbour enlargement by a whole factor.
    pub fn scaled(&self, k: u32) -> Canvas {
        let mut out = Canvas::new(self.width * k, self.height * k);
        for y in 0..out.height {
            for x in 0..out.width {
                let p = self.pixel(x / k, y / k);
                let i = ((y * out.width + x) * 4) as usize;
                out.rgba[i..i + 4].copy_from_slice(&p);
            }
        }
        out
    }
}

#[cfg(test)]
mod scaled_tests {
    use super::*;

    #[test]
    fn a_scaled_picture_kept_for_next_time_draws_the_same_and_follows_a_change() {
        let mut img = Image {
            width: 2,
            height: 2,
            rgba: vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 128,
            ],
        };
        let draw = |img: &Image, tint: [f32; 3]| {
            let mut c = Canvas::scaled_layout((40, 40), 2.0);
            c.draw_image(
                img,
                [0, 0, 2, 2],
                [0, 0, 20, 20],
                tint,
                1.0,
                Orient::default(),
            );
            c.rgba
        };
        let first = draw(&img, [1.0; 3]);
        assert_eq!(first, draw(&img, [1.0; 3]));
        assert_ne!(first, draw(&img, [0.5; 3]), "another tint");
        // The same allocation with other pixels, as a picture freed and made again would be.
        img.rgba[0] = 0;
        img.rgba[1] = 255;
        assert_ne!(first, draw(&img, [1.0; 3]));
    }
}

#[cfg(test)]
mod remembered_tests {
    use super::*;

    fn picture(side: u32, seed: u8) -> Image {
        Image {
            width: side,
            height: side,
            rgba: (0..side * side * 4)
                .map(|n| (n as u8).wrapping_mul(seed))
                .collect(),
        }
    }

    #[test]
    fn a_large_picture_is_known_by_what_it_holds_at_the_same_address() {
        let (mut a, b) = (picture(128, 3), picture(128, 5));
        let first = content_of(&a);
        assert_eq!(content_of(&a), first, "the same again");
        assert_ne!(content_of(&b), first, "another picture");
        // Another picture loaded where this one was: the same address and size, other words.
        a.rgba.copy_from_slice(&b.rgba);
        assert_eq!(content_of(&a), content_of(&b));
    }
}
