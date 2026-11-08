//! Detection bars: a ring of small arrow marks round the middle of the screen, one for each enemy that has noticed the
//! party, each pointing the way that enemy is. A mark fills from its base to its tip as the enemy notices, white at first,
//! then yellow, and red when full (alerted). When the enemy loses the party it drains and fades.
//!
//! **Assumed:** the sizes are a share of the screen's short side, so they follow the window: the ring is 0.30 of it from the
//! middle, a mark is 0.045 long and 0.056 across, scaled by the setting's size. The owner gave no numbers.

use crate::canvas::Canvas;
use crate::screens::Ui;

/// How far from the middle of the screen the marks sit, as a share of its short side.
pub const RING: f32 = 0.30;
/// A mark's length and half its width, the same way.
pub const LENGTH: f32 = 0.045;
pub const HALF_WIDTH: f32 = 0.028;
/// The colours at empty, half and full.
const WHITE: [f32; 3] = [1.0, 1.0, 1.0];
const YELLOW: [f32; 3] = [1.0, 0.85, 0.2];
const RED: [f32; 3] = [1.0, 0.18, 0.12];
/// How solid the part of a mark that is not yet filled is.
const EMPTY_STRENGTH: f32 = 0.28;

/// One enemy's mark.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DetectionMarkView {
    /// Radians to the right of where the camera looks, from -pi to pi.
    pub right: f32,
    /// 0 to 1.
    pub fill: f32,
    /// 1 while the enemy perceives the party, and falling to 0 as it forgets them.
    pub fade: f32,
}

/// What the detection bars show now.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DetectionView {
    pub on: bool,
    /// In hundredths.
    pub size: i32,
    pub marks: Vec<DetectionMarkView>,
}

/// White, then yellow, then red as `fill` goes from 0 to 1.
pub fn detection_color(fill: f32) -> [f32; 3] {
    let f = fill.clamp(0.0, 1.0);
    let mix = |a: [f32; 3], b: [f32; 3], t: f32| [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t);
    if f < 0.5 {
        mix(WHITE, YELLOW, f * 2.0)
    } else {
        mix(YELLOW, RED, (f - 0.5) * 2.0)
    }
}

impl Ui<'_> {
    /// What the detection bars show, for the HUD to draw at its next chance.
    pub fn set_detection(&mut self, view: DetectionView) {
        self.detection = view;
    }

    /// The pixels the ring can touch: a square round the middle of the window, left, top, width and height.
    fn detection_box(&self, size: i32) -> Option<[u32; 4]> {
        let (w, h) = (self.pixels.0 as f32, self.pixels.1 as f32);
        let short = w.min(h);
        let k = size.max(10) as f32 / 100.0;
        let reach = (RING + (LENGTH + HALF_WIDTH) * k) * short + 2.0;
        let (left, top) = ((w / 2.0 - reach).max(0.0), (h / 2.0 - reach).max(0.0));
        let (right, bottom) = ((w / 2.0 + reach).min(w), (h / 2.0 + reach).min(h));
        (right > left && bottom > top).then(|| {
            [
                left as u32,
                top as u32,
                (right - left) as u32,
                (bottom - top) as u32,
            ]
        })
    }

    /// The pixels that change between two views of the bars.
    pub(crate) fn detection_changes(
        &self,
        was: &DetectionView,
        now: &DetectionView,
    ) -> Vec<[u32; 4]> {
        if was == now {
            return Vec::new();
        }
        self.detection_box(was.size.max(now.size))
            .into_iter()
            .collect()
    }

    /// Draw the bars, over the HUD.
    pub(crate) fn draw_detection(&mut self, cv: &mut Canvas) {
        if !self.detection.on || self.detection.marks.is_empty() {
            return;
        }
        let (w, h) = (cv.width as f32, cv.height as f32);
        let short = w.min(h);
        let k = self.detection.size.max(10) as f32 / 100.0;
        let (length, half) = (LENGTH * short * k, HALF_WIDTH * short * k);
        let (cx, cy) = (w / 2.0, h / 2.0);
        for m in self.detection.marks.clone() {
            // Straight ahead is up the screen, and a bearing to the right goes right.
            let out = (m.right.sin(), -m.right.cos());
            let side = (-out.1, out.0);
            let ring = RING * short;
            let base = (cx + out.0 * ring, cy + out.1 * ring);
            let color = detection_color(m.fill);
            let strength = m.fade.clamp(0.0, 1.0);
            let reach = (length + half).ceil() as i32 + 1;
            for dy in -reach..=reach {
                for dx in -reach..=reach {
                    let (px, py) = (base.0.round() as i32 + dx, base.1.round() as i32 + dy);
                    let rel = (px as f32 + 0.5 - base.0, py as f32 + 0.5 - base.1);
                    // Along the arrow from its base, and across it.
                    let u = rel.0 * out.0 + rel.1 * out.1 + length / 2.0;
                    let v = rel.0 * side.0 + rel.1 * side.1;
                    if u < -1.0 || u > length + 1.0 {
                        continue;
                    }
                    // How far inside the triangle, in pixels, and so how much of this pixel it covers.
                    let narrow = half * (1.0 - (u / length).clamp(0.0, 1.0));
                    let inside = (narrow - v.abs()).min(u).min(length - u);
                    let cover = (0.5 + inside).clamp(0.0, 1.0);
                    if cover <= 0.0 {
                        continue;
                    }
                    let filled = u / length <= m.fill;
                    let own = if filled { 1.0 } else { EMPTY_STRENGTH };
                    let rim = inside < 1.2;
                    // A dark edge, so a white mark reads on a bright floor.
                    let c = if rim { [0.08, 0.08, 0.08] } else { color };
                    let a = (cover * strength * if rim { own.max(0.5) } else { own } * 255.0) as u8;
                    cv.blend(
                        px,
                        py,
                        [
                            (c[0] * 255.0) as u8,
                            (c[1] * 255.0) as u8,
                            (c[2] * 255.0) as u8,
                            a,
                        ],
                    );
                }
            }
        }
    }
}
