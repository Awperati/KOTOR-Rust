//! A software canvas: RGBA pixels with alpha-blended rectangles and image blits.

use kotor_image::Image;

#[derive(Clone, Debug)]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
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

impl Canvas {
    pub fn new(width: u32, height: u32) -> Canvas {
        Canvas {
            width,
            height,
            rgba: vec![0; (width * height * 4) as usize],
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
        for yy in y..y + h {
            for xx in x..x + w {
                self.blend(xx, yy, c);
            }
        }
    }

    /// Draw `region` (x, y, w, h in source pixels) of `img` into the rectangle `(x, y, w, h)`, scaling with
    /// nearest sampling. RGB is multiplied by `tint` (0..=1 per channel) and alpha by `alpha`.
    pub fn draw_image(
        &mut self,
        img: &Image,
        region: [i32; 4],
        dst: [i32; 4],
        tint: [f32; 3],
        alpha: f32,
        orient: Orient,
    ) {
        let [dx, dy, dw, dh] = dst;
        if dw <= 0 || dh <= 0 || region[2] <= 0 || region[3] <= 0 {
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
