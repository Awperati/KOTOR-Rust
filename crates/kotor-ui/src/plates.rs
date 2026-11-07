//! Nameplates: a name and a health bar over each enemy, and labels on what can be looted or used. Drawn on the HUD, so they go
//! when it goes. The idea and how it behaves are from the K1 Nameplates mod by phalladar (MIT); none of its code is here.

use crate::canvas::Canvas;
use crate::screens::Ui;
use kotor_image::Image;

/// One plate, in layout units. The target's sits where its bar did, with its slots under it.
#[derive(Clone, Debug, PartialEq)]
pub struct Plate {
    /// The middle of the plate, and its top.
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub text: String,
    pub color: [f32; 3],
    /// How full the bar is and how much of it is the last hit's chip, out of 1. Labels have none.
    pub health: Option<(f32, f32)>,
    /// A white frame: the target.
    pub framed: bool,
    pub big_text: bool,
    /// Darker when something else is the target.
    pub strength: f32,
    /// A small star, bright or dim: there is loot.
    pub sparkle: Option<bool>,
    /// How solid all of it is, 1 for all the way: remains or a container with nothing left show through.
    pub opacity: f32,
}

/// The name's line and the bar under it.
pub const PLATE_TEXT_HIGH: i32 = 12;
pub const PLATE_BAR_HIGH: i32 = 3;
const PLATE_TEXT_FONT: &str = "dialogfont10x10";
const PLATE_BIG_TEXT_FONT: &str = "fnt_d16x16";
const FRAME: [u8; 4] = [255, 255, 255, 230];
const CHIP: [u8; 4] = [255, 225, 130, 255];
const STAR_BRIGHT: [u8; 4] = [255, 235, 150, 255];
const STAR_DIM: [u8; 4] = [190, 150, 70, 200];
const TOP_CENTRE: i32 = 18;
const TEXT_BACK: [u8; 4] = [0, 0, 0, 120];
const WORDS_LIGHTER: f32 = 0.35;
/// What a plate can touch round its place, in layout units: its words are drawn in 240, and may wrap.
const PLATE_BOX_HALF: i32 = 130;
const PLATE_BOX_ABOVE: i32 = 4;
const PLATE_BOX_BELOW: i32 = 34;

impl Plate {
    /// How high it is on screen.
    pub fn high(&self) -> i32 {
        PLATE_TEXT_HIGH
            + if self.health.is_some() {
                PLATE_BAR_HIGH
            } else {
                0
            }
    }
}

fn rgba(c: [f32; 3], strength: f32, a: u8) -> [u8; 4] {
    [
        (c[0] * strength * 255.0) as u8,
        (c[1] * strength * 255.0) as u8,
        (c[2] * strength * 255.0) as u8,
        a,
    ]
}

impl Ui<'_> {
    /// The plates on a copy of the HUD.
    pub fn plates_over(&mut self, hud: &Image) -> Image {
        let mut cv = Canvas::over(hud.clone(), self.scale);
        self.draw_plates(&mut cv);
        cv.into_image()
    }

    /// The picture of a window this size in pixels that holds all the plates now given.
    pub fn plates_box(&self) -> Option<[u32; 4]> {
        let k = self.scale;
        let boxes = self.plates.iter().map(|p| {
            let at = |v: i32| (v as f32 * k).max(0.0) as u32;
            let (right, bottom) = (
                at(p.x + PLATE_BOX_HALF).min(self.pixels.0),
                at(p.y + PLATE_BOX_BELOW).min(self.pixels.1),
            );
            let (left, top) = (at(p.x - PLATE_BOX_HALF), at(p.y - PLATE_BOX_ABOVE));
            [left, top, right, bottom]
        });
        let all = boxes.reduce(|a, b| {
            [
                a[0].min(b[0]),
                a[1].min(b[1]),
                a[2].max(b[2]),
                a[3].max(b[3]),
            ]
        })?;
        (all[2] > all[0] && all[3] > all[1]).then_some([
            all[0],
            all[1],
            all[2] - all[0],
            all[3] - all[1],
        ])
    }

    /// Move the plates on a picture that is the HUD and the plates it was last given, without copying more than `dirty`,
    /// the box that holds those and these. The rest of a 3840 by 2160 picture is 33 MB, which a turning camera cannot copy.
    pub fn move_plates(&mut self, hud: &Image, picture: Image, dirty: [u32; 4]) -> Image {
        let mut picture = picture;
        let [left, top, wide, high] = dirty;
        let row = hud.width as usize * 4;
        for y in top..(top + high).min(hud.height) {
            let start = y as usize * row + left as usize * 4;
            let end = (start + wide as usize * 4).min((y as usize + 1) * row);
            picture.rgba[start..end].copy_from_slice(&hud.rgba[start..end]);
        }
        let mut cv = Canvas::over(picture, self.scale);
        self.draw_plates(&mut cv);
        cv.into_image()
    }

    fn draw_plates(&mut self, cv: &mut Canvas) {
        for p in self.plates.clone() {
            if p.opacity < 1.0 {
                let kept = cv.keep_box(
                    p.x - PLATE_BOX_HALF,
                    p.y - PLATE_BOX_ABOVE,
                    PLATE_BOX_HALF * 2,
                    PLATE_BOX_ABOVE + PLATE_BOX_BELOW,
                );
                self.draw_plate(cv, &p);
                cv.fade_to(&kept, p.opacity);
            } else {
                self.draw_plate(cv, &p);
            }
        }
    }

    fn draw_plate(&mut self, cv: &mut Canvas, p: &Plate) {
        {
            let (left, high) = (p.x - p.width / 2, p.high());
            let font = if p.big_text {
                PLATE_BIG_TEXT_FONT
            } else {
                PLATE_TEXT_FONT
            };
            if p.framed {
                let (l, t, w) = (left - 1, p.y - 1, p.width + 2);
                let h = high + 2;
                cv.fill_rect(l, t, w, h, [0, 0, 0, 150]);
                cv.fill_rect(l, t, w, 1, FRAME);
                cv.fill_rect(l, t + h - 1, w, 1, FRAME);
                cv.fill_rect(l, t, 1, h, FRAME);
                cv.fill_rect(l + w - 1, t, 1, h, FRAME);
            }
            // The bar's own colour is too dark for small words.
            let color = p
                .color
                .map(|c| (c + (1.0 - c) * WORDS_LIGHTER) * p.strength);
            let r = [p.x - 120, p.y, 240, PLATE_TEXT_HIGH + 4];
            let wide = self.font_shared(font).map_or(0, |f| f.measure(&p.text));
            // The words have to stand out on whatever is behind them.
            if !p.framed {
                cv.fill_rect(
                    p.x - wide / 2 - 2,
                    p.y,
                    wide + 4,
                    PLATE_TEXT_HIGH,
                    TEXT_BACK,
                );
            }
            self.draw_text(cv, font, &p.text, r, TOP_CENTRE, color);
            if let Some(star) = p.sparkle {
                let (x, y) = (p.x + wide / 2 + 5, p.y + 5);
                let c = if star { STAR_BRIGHT } else { STAR_DIM };
                cv.fill_rect(x - 2, y, 5, 1, c);
                cv.fill_rect(x, y - 2, 1, 5, c);
            }
            if let Some((full, chip)) = p.health {
                let y = p.y + PLATE_TEXT_HIGH;
                let filled = (p.width as f32 * full.clamp(0.0, 1.0)).round() as i32;
                let chipped = (p.width as f32 * chip.clamp(0.0, 1.0)).round() as i32;
                cv.fill_rect(left, y, p.width, PLATE_BAR_HIGH, [0, 0, 0, 190]);
                if chipped > filled {
                    cv.fill_rect(left + filled, y, chipped - filled, PLATE_BAR_HIGH, CHIP);
                }
                cv.fill_rect(
                    left,
                    y,
                    filled,
                    PLATE_BAR_HIGH,
                    rgba(p.color, p.strength, 255),
                );
            }
        }
    }
}
