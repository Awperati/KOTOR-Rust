//! The owner's look for text boxes: black inside, with a glowing blue and yellow edge. Everything to tweak is here.

use crate::canvas::Canvas;

/// What lies inside a box.
pub const BOX_BLACK: [u8; 4] = [0, 0, 0, 240];
/// The line along the edge.
pub const LINE_YELLOW: [u8; 3] = [255, 214, 64];
/// The light round it.
pub const GLOW_BLUE: [u8; 3] = [50, 160, 255];
/// How strong the glow is at the line, 0 to 255, and how far it reaches, in layout units.
pub const GLOW_STRENGTH: f32 = 170.0;
pub const GLOW_REACH: f32 = 7.0;
/// How thick the line is, in layout units.
pub const LINE_THICK: f32 = 1.0;

fn fade(distance: f32, reach: f32) -> u8 {
    let left = (1.0 - distance / reach).max(0.0);
    (GLOW_STRENGTH * left * left) as u8
}

/// Glow outside and a yellow line, round a rectangle of layout units.
pub fn edge(cv: &mut Canvas, r: [i32; 4]) {
    let (sx, sy) = (cv.scale_x, cv.scale);
    let left = (r[0] as f32 * sx).round() as i32;
    let top = (r[1] as f32 * sy).round() as i32;
    let right = ((r[0] + r[2]) as f32 * sx).round() as i32;
    let bottom = ((r[1] + r[3]) as f32 * sy).round() as i32;
    let reach = GLOW_REACH * sy;
    let line = (LINE_THICK * sy).round().max(1.0) as i32;
    let far = reach.ceil() as i32;
    for y in (top - far)..(bottom + far) {
        for x in (left - far)..(right + far) {
            let dx = (left - x).max(x - (right - 1)).max(0);
            let dy = (top - y).max(y - (bottom - 1)).max(0);
            if dx == 0 && dy == 0 {
                // Inside: the line, and a little of the glow past it.
                let d = (x - left)
                    .min(right - 1 - x)
                    .min(y - top)
                    .min(bottom - 1 - y);
                if d < line {
                    cv.blend(x, y, [LINE_YELLOW[0], LINE_YELLOW[1], LINE_YELLOW[2], 255]);
                } else {
                    let a = fade((d - line) as f32, reach / 3.0);
                    cv.blend(x, y, [GLOW_BLUE[0], GLOW_BLUE[1], GLOW_BLUE[2], a]);
                }
            } else {
                let d = ((dx * dx + dy * dy) as f32).sqrt();
                let a = fade(d, reach);
                cv.blend(x, y, [GLOW_BLUE[0], GLOW_BLUE[1], GLOW_BLUE[2], a]);
            }
        }
    }
}

/// A whole box: black, with its edge.
pub fn boxed(cv: &mut Canvas, r: [i32; 4]) {
    cv.fill_rect(r[0], r[1], r[2], r[3], BOX_BLACK);
    edge(cv, r);
}

/// The edge of a bar across the window: the yellow line at `y` in layout units, and the glow on the side away from the bar.
pub fn bar_edge(cv: &mut Canvas, y: i32, glow_down: bool) {
    let line = (LINE_THICK * cv.scale).round().max(1.0) as i32;
    let at = (y as f32 * cv.scale).round() as i32;
    let reach = GLOW_REACH * cv.scale;
    let width = cv.width as i32;
    let far = reach.ceil() as i32;
    for d in 0..far {
        let yy = if glow_down { at + line + d } else { at - 1 - d };
        let a = fade(d as f32, reach);
        for x in 0..width {
            cv.blend(x, yy, [GLOW_BLUE[0], GLOW_BLUE[1], GLOW_BLUE[2], a]);
        }
    }
    for yy in at..at + line {
        for x in 0..width {
            cv.blend(x, yy, [LINE_YELLOW[0], LINE_YELLOW[1], LINE_YELLOW[2], 255]);
        }
    }
}
