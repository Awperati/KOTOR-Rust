//! A software canvas: RGBA pixels with alpha-blended rectangles and image blits.

use kotor_image::Image;

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
}

/// How a texture is turned the right way up when it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Orient {
    /// Quarter turns clockwise, 0..=3.
    pub turns: u8,
    pub flip_x: bool,
    pub flip_y: bool,
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

impl Canvas {
    pub fn new(width: u32, height: u32) -> Canvas {
        Canvas {
            width,
            height,
            scale: 1.0,
            scale_x: 1.0,
            rgba: vec![0; (width * height * 4) as usize],
        }
    }

    /// A canvas of exactly `pixels`, with `scale` pixels to a layout unit.
    pub fn scaled_layout(pixels: (u32, u32), scale: f32) -> Canvas {
        let mut c = Canvas::new(pixels.0, pixels.1);
        c.scale = scale;
        c.scale_x = scale;
        c
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
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 || src[3] == 0 {
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
        for yy in y..y + h {
            for xx in x..x + w {
                self.blend(xx, yy, c);
            }
        }
    }

    /// Leave nothing in a rectangle of layout units, for what lies behind the picture to show.
    pub fn clear_rect(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let [x, y, w, h] = self.to_pixels(x, y, w, h);
        for yy in y.max(0)..(y + h).min(self.height as i32) {
            for xx in x.max(0)..(x + w).min(self.width as i32) {
                let i = ((yy as u32 * self.width + xx as u32) * 4) as usize;
                self.rgba[i..i + 4].fill(0);
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
        let smooth = self.scale != 1.0 || self.scale_x != 1.0;
        // Shrinking a large picture: average a few samples per pixel.
        let samples = if dw * 4 < region[2] * 3 || dh * 4 < region[3] * 3 {
            2
        } else {
            1
        };
        for oy in 0..dh {
            for ox in 0..dw {
                if smooth {
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
                        self.blend(
                            dx + ox,
                            dy + oy,
                            [
                                (acc[0] / acc[3] * tint[0]).min(255.0) as u8,
                                (acc[1] / acc[3] * tint[1]).min(255.0) as u8,
                                (acc[2] / acc[3] * tint[2]).min(255.0) as u8,
                                (acc[3] / n * 255.0 * alpha).min(255.0) as u8,
                            ],
                        );
                    }
                    continue;
                }
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
