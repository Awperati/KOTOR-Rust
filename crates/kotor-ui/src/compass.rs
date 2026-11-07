//! The compass at the top middle of the HUD: north and the rest slide along it as the camera turns, with what is near marked on
//! it, and the line of the quest being tracked under it. The owner: "taking inspiration from Skyrim's compass but it uses its own
//! UI and design". The icons are drawn here, in the blue and yellow of this game's interface, and none is the game's art. Sizes
//! are worked out in pixels, so that what is round stays round in a narrow window.

use crate::canvas::{Canvas, Orient};
use crate::gui::Control;
use crate::screens::{Ui, SCREEN};
use crate::touch::{cover, in_capsule};
use kotor_image::Image;

const ICON_SIZE: u32 = 64;
const BAR_PICTURE: &str = "kotorrs_compass_bar";
const BAR_SIZE: (u32, u32) = (256, 32);
/// What the strip shows each side of where the camera looks. Past that a mark is off it.
const SPAN: f32 = std::f32::consts::FRAC_PI_2;
/// Sizes in pixels for each 480 of the window's shorter side, at a size of 100%: half the strip, its height, an icon at its
/// biggest and at its smallest, a heading's letter, and the tracked line.
const HALF_WIDE: f32 = 130.0;
const LEAST_HALF_WIDE: f32 = 90.0;
const BAR_HIGH: f32 = 24.0;
const ICON_MOST: f32 = 20.0;
const ICON_LEAST: f32 = 12.0;
const LETTER_LINE: f32 = 12.0;
const LINE_LINE: f32 = 11.0;
const LINE_HIGH: f32 = 16.0;
/// How far under the strip it comes on a window too narrow for it beside the menu keys, and what it stays clear of.
const STRIP_TOP: i32 = 3;
const STRIP_GAP: i32 = 2;
const LEFT_OF_IT: &str = "LBL_CASH";
const RIGHT_OF_IT: &str = "LBL_MENUBG";
const BLUE_BUTTON: &str = crate::touch::TOUCH_BUTTON;
/// Words at the top of the screen in a fight: they go under the strip.
const UNDER_IT: [&str; 2] = ["LBL_CMBTMODEMSG", "LBL_CMBTMSGBG"];
const TICK_COLOR: [u8; 4] = [96, 170, 255, 210];
const CARET_COLOR: [u8; 4] = [255, 225, 90, 255];
const NORTH_COLOR: [f32; 3] = [1.0, 0.88, 0.35];
const HEADING_COLOR: [f32; 3] = [0.55, 0.8, 1.0];
const LINE_BAND: [u8; 4] = [0, 0, 0, 150];
const LINE_COLOR: [f32; 3] = [1.0, 0.88, 0.35];
/// How dim a mark at the end of its range is.
const FAR_STRENGTH: f32 = 0.45;

/// What a mark on the compass is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompassIcon {
    Shop,
    Door,
    Area,
    Container,
    Talk,
    Pin,
    Elevator,
    Party,
    Enemy,
    Save,
}

/// All of them.
pub const COMPASS_ICONS: [CompassIcon; 10] = [
    CompassIcon::Shop,
    CompassIcon::Door,
    CompassIcon::Area,
    CompassIcon::Container,
    CompassIcon::Talk,
    CompassIcon::Pin,
    CompassIcon::Elevator,
    CompassIcon::Party,
    CompassIcon::Enemy,
    CompassIcon::Save,
];

impl CompassIcon {
    /// The name the interface draws it under.
    pub fn picture(self) -> &'static str {
        match self {
            CompassIcon::Shop => "kotorrs_compass_shop",
            CompassIcon::Door => "kotorrs_compass_door",
            CompassIcon::Area => "kotorrs_compass_area",
            CompassIcon::Container => "kotorrs_compass_container",
            CompassIcon::Talk => "kotorrs_compass_talk",
            CompassIcon::Pin => "kotorrs_compass_pin",
            CompassIcon::Elevator => "kotorrs_compass_elevator",
            CompassIcon::Party => "kotorrs_compass_party",
            CompassIcon::Enemy => "kotorrs_compass_enemy",
            CompassIcon::Save => "kotorrs_compass_save",
        }
    }
}

/// One mark on the strip.
#[derive(Clone, Debug, PartialEq)]
pub struct CompassMark {
    pub icon: CompassIcon,
    /// Radians to the right of where the camera looks, from -pi to pi.
    pub right: f32,
    /// 1 beside the player, 0 at the end of its range: it fades and shrinks.
    pub near: f32,
}

/// What the compass shows now.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompassView {
    pub on: bool,
    /// In hundredths.
    pub size: i32,
    /// Where the camera looks: radians from east towards north.
    pub facing: f32,
    pub marks: Vec<CompassMark>,
    /// What the tracked quest says, under the strip. The tracked step is shown only by the pin on the strip: the owner, "have
    /// the quest arrow only appear on the compass itself".
    pub line: String,
}

/// From -pi to pi.
pub fn wrapped(angle: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    (angle + tau * 1.5).rem_euclid(tau) - tau / 2.0
}

fn circle(p: (f32, f32), c: (f32, f32), r: f32) -> f32 {
    r - (p.0 - c.0).hypot(p.1 - c.1)
}

fn rect(p: (f32, f32), c: (f32, f32), half_wide: f32, half_high: f32) -> f32 {
    (half_wide - (p.0 - c.0).abs()).min(half_high - (p.1 - c.1).abs())
}

/// How far inside a triangle a point is, whichever way round its corners go.
fn triangle(p: (f32, f32), a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> f32 {
    let side = |a: (f32, f32), b: (f32, f32)| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        ((p.0 - a.0) * dy - (p.1 - a.1) * dx) / dx.hypot(dy).max(1e-6)
    };
    let turn = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
    let k = if turn > 0.0 { 1.0 } else { -1.0 };
    (side(a, b) * -k).min(side(b, c) * -k).min(side(c, a) * -k)
}

/// An icon's shape: how far inside it a point is, with the middle of the picture at 0,0 and its edges at 32.
fn icon_shape(icon: CompassIcon, p: (f32, f32)) -> f32 {
    match icon {
        // A credit: a ring with a bar through it.
        CompassIcon::Shop => {
            let ring = circle(p, (0.0, 0.0), 26.0).min(-circle(p, (0.0, 0.0), 18.0));
            ring.max(rect(p, (0.0, 0.0), 4.0, 15.0))
        }
        // A door in its frame, with a handle.
        CompassIcon::Door => {
            let frame = rect(p, (0.0, 0.0), 20.0, 28.0).min(-rect(p, (0.0, 4.0), 13.0, 25.0));
            frame.max(circle(p, (6.0, 2.0), 4.0))
        }
        // A frame, and an arrow going through it.
        CompassIcon::Area => {
            let frame = rect(p, (-8.0, 0.0), 18.0, 28.0).min(-rect(p, (-8.0, 4.0), 11.0, 25.0));
            let shaft = rect(p, (2.0, 0.0), 14.0, 5.0);
            let head = triangle(p, (12.0, -15.0), (12.0, 15.0), (30.0, 0.0));
            frame.max(shaft).max(head)
        }
        // A crate with a lid and a catch.
        CompassIcon::Container => {
            let lid = rect(p, (0.0, -15.0), 27.0, 7.0);
            let body = rect(p, (0.0, 9.0), 24.0, 15.0);
            lid.max(body).min(-rect(p, (0.0, 3.0), 5.0, 5.0))
        }
        // Words in a bubble, with three dots cut out of it.
        CompassIcon::Talk => {
            let bubble = in_capsule(p, (-11.0, -6.0), (11.0, -6.0), 18.0);
            let tail = triangle(p, (-12.0, 6.0), (6.0, 8.0), (-18.0, 28.0));
            let dots = [-13.0, 0.0, 13.0]
                .into_iter()
                .map(|x| circle(p, (x, -6.0), 4.0))
                .fold(f32::MIN, f32::max);
            bubble.max(tail).min(-dots)
        }
        // A pin with a hole in its head.
        CompassIcon::Pin => {
            let head = circle(p, (0.0, -9.0), 20.0);
            let point = triangle(p, (-17.0, 0.0), (17.0, 0.0), (0.0, 30.0));
            head.max(point).min(-circle(p, (0.0, -9.0), 8.0))
        }
        // A car with an arrow up and an arrow down in it.
        CompassIcon::Elevator => {
            let car = rect(p, (0.0, 0.0), 22.0, 29.0).min(-rect(p, (0.0, 0.0), 16.0, 23.0));
            let up = triangle(p, (-11.0, -3.0), (11.0, -3.0), (0.0, -19.0));
            let down = triangle(p, (-11.0, 3.0), (11.0, 3.0), (0.0, 19.0));
            car.max(up).max(down)
        }
        // A head and shoulders.
        CompassIcon::Party => {
            let head = circle(p, (0.0, -13.0), 12.0);
            let body = in_capsule(p, (-11.0, 18.0), (11.0, 18.0), 13.0).min(29.0 - p.1);
            head.max(body)
        }
        // Two blades, crossed.
        CompassIcon::Enemy => in_capsule(p, (-20.0, -22.0), (20.0, 22.0), 5.0).max(in_capsule(
            p,
            (20.0, -22.0),
            (-20.0, 22.0),
            5.0,
        )),
        // A data disc: a square with a label and a shutter cut out.
        CompassIcon::Save => rect(p, (0.0, 0.0), 26.0, 26.0)
            .min(-rect(p, (-2.0, -16.0), 12.0, 6.0))
            .min(-rect(p, (0.0, 15.0), 15.0, 7.0)),
    }
}

fn icon_color(icon: CompassIcon) -> [f32; 3] {
    match icon {
        CompassIcon::Shop | CompassIcon::Pin => [255.0, 225.0, 90.0],
        CompassIcon::Enemy => [240.0, 70.0, 55.0],
        CompassIcon::Talk | CompassIcon::Party => [225.0, 240.0, 255.0],
        _ => [110.0, 185.0, 255.0],
    }
}

fn icon_picture(icon: CompassIcon) -> Image {
    const OUTLINE: f32 = 4.0;
    const DARK: [f32; 3] = [0.0, 6.0, 24.0];
    let color = icon_color(icon);
    let middle = ICON_SIZE as f32 / 2.0;
    let mut rgba = Vec::with_capacity((ICON_SIZE * ICON_SIZE * 4) as usize);
    for y in 0..ICON_SIZE {
        for x in 0..ICON_SIZE {
            let p = (x as f32 + 0.5 - middle, y as f32 + 0.5 - middle);
            let inside = icon_shape(icon, p);
            let solid = cover(inside);
            for k in 0..3 {
                rgba.push((DARK[k] + (color[k] - DARK[k]) * solid) as u8);
            }
            rgba.push((cover(inside + OUTLINE) * 255.0) as u8);
        }
    }
    Image {
        width: ICON_SIZE,
        height: ICON_SIZE,
        rgba,
    }
}

/// The strip's ground: dark, with a thin blue line along each edge, and fading out at its ends.
fn bar_picture() -> Image {
    let (wide, high) = BAR_SIZE;
    let mut rgba = Vec::with_capacity((wide * high * 4) as usize);
    for y in 0..high {
        for x in 0..wide {
            let along = (x as f32 + 0.5) / wide as f32;
            let from_end = along.min(1.0 - along);
            let fade = (from_end * 6.0).clamp(0.0, 1.0);
            let on_line = y < 2 || y + 2 >= high;
            let (color, alpha) = if on_line {
                ([96u8, 170, 255], 230.0)
            } else {
                ([2u8, 8, 26], 150.0)
            };
            rgba.extend_from_slice(&[color[0], color[1], color[2], (alpha * fade) as u8]);
        }
    }
    Image {
        width: wide,
        height: high,
        rgba,
    }
}

/// A picture of the compass by name, drawn here.
pub(crate) fn compass_picture(name: &str) -> Option<Image> {
    match name {
        BAR_PICTURE => Some(bar_picture()),
        _ => COMPASS_ICONS
            .into_iter()
            .find(|i| i.picture() == name)
            .map(icon_picture),
    }
}

impl Ui<'_> {
    /// Pixels for one of the sizes above.
    fn compass_unit(&self) -> f32 {
        self.pixels.0.min(self.pixels.1) as f32 / SCREEN.1 as f32 * self.compass.size.max(1) as f32
            / 100.0
    }

    /// The strip: its middle across, its top and its height in pixels, and how far it goes each side of its middle.
    fn compass_strip(&self, panel: &Control) -> (f32, f32, f32, f32) {
        let (across, down) = (self.hud_across(), self.scale);
        let unit = self.compass_unit();
        let middle = self.pixels.0 as f32 / 2.0;
        let left_edge = |tag: &str, or: f32| {
            panel
                .children
                .iter()
                .find(|c| c.tag == tag)
                .map_or(or, |c| c.extent.left as f32 * across)
        };
        let clear_left = panel
            .children
            .iter()
            .find(|c| c.tag == LEFT_OF_IT)
            .map_or(0.0, |c| (c.extent.left + c.extent.width) as f32 * across);
        let wanted = HALF_WIDE * unit;
        let room = |right: f32| wanted.min(middle - clear_left).min(right - middle);
        let beside = room(left_edge(RIGHT_OF_IT, self.pixels.0 as f32));
        if beside >= LEAST_HALF_WIDE * unit {
            return (middle, STRIP_TOP as f32 * down, BAR_HIGH * unit, beside);
        }
        // No room beside the menu keys: under them, and clear of the blue button.
        let bottom = panel
            .children
            .iter()
            .filter(|c| c.tag.starts_with("BTN_OPT") || c.tag == RIGHT_OF_IT)
            .map(|c| c.extent.top + c.extent.height)
            .max()
            .unwrap_or(STRIP_TOP);
        let top = (bottom + STRIP_GAP) as f32 * down;
        (
            middle,
            top,
            BAR_HIGH * unit,
            room(left_edge(BLUE_BUTTON, self.pixels.0 as f32)),
        )
    }

    /// All of the compass and its line, in layout units.
    pub(crate) fn compass_box(&self, panel: &Control) -> [i32; 4] {
        let (middle, top, high, half) = self.compass_strip(panel);
        let (across, down) = (self.hud_across(), self.scale);
        let line = LINE_HIGH * self.compass_unit();
        [
            ((middle - half) / across).floor() as i32,
            (top / down).floor() as i32,
            (half * 2.0 / across).ceil() as i32,
            ((high + line) / down).ceil() as i32,
        ]
    }

    /// What the HUD's words do for the compass: those at the top of the screen in a fight go under it.
    pub(crate) fn place_for_compass(&self, panel: &mut Control) {
        if !self.compass.on {
            return;
        }
        let e = self.compass_box(panel);
        let under = e[1] + e[3] + STRIP_GAP;
        for c in &mut panel.children {
            if UNDER_IT.contains(&c.tag.as_str()) {
                c.extent.top = under;
            }
        }
    }

    /// How low the compass and its line come, in layout units, if it is on.
    pub(crate) fn compass_bottom(&self, panel: &Control) -> Option<i32> {
        self.compass.on.then(|| {
            let e = self.compass_box(panel);
            e[1] + e[3]
        })
    }

    /// What the compass shows, for the HUD to draw at its next chance.
    pub fn set_compass(&mut self, view: CompassView) {
        self.compass = view;
    }

    /// Where the compass is, in layout units, if it is on. For a test.
    pub fn compass_place(&mut self) -> Option<[i32; 4]> {
        let gui = self.hud_layout(None)?;
        self.compass.on.then(|| self.compass_box(&gui.panel))
    }

    /// The pixels that change between two views of the compass, which is on in both.
    pub(crate) fn compass_changes(
        &self,
        panel: &Control,
        was: &CompassView,
        now: &CompassView,
    ) -> Vec<[u32; 4]> {
        let mut boxes = Vec::new();
        if was.facing != now.facing
            || was.marks != now.marks
            || was.line != now.line
            || was.size != now.size
        {
            boxes.extend(self.pixel_box(self.compass_box(panel), 2));
        }
        boxes
    }

    fn draw_compass_picture(
        &mut self,
        cv: &mut Canvas,
        name: &str,
        middle: (f32, f32),
        side: f32,
        alpha: f32,
    ) {
        let at = self.round_box(middle, side);
        if let Some(img) = self.texture_shared(name) {
            let full = [0, 0, img.width as i32, img.height as i32];
            cv.draw_image(&img, full, at, [1.0; 3], alpha, Orient::default());
        }
    }

    /// Draw the compass, under the on-screen controls and over the rest of the HUD.
    pub(crate) fn draw_compass(&mut self, cv: &mut Canvas, panel: &Control) {
        if !self.compass.on {
            return;
        }
        let (middle, top, high, half) = self.compass_strip(panel);
        let (across, down) = (self.hud_across(), self.scale);
        let unit = self.compass_unit();
        let bar_at = [
            ((middle - half) / across).round() as i32,
            (top / down).round() as i32,
            (half * 2.0 / across).round() as i32,
            (high / down).round() as i32,
        ];
        if let Some(img) = self.texture_shared(BAR_PICTURE) {
            let full = [0, 0, img.width as i32, img.height as i32];
            cv.draw_image(&img, full, bar_at, [1.0; 3], 1.0, Orient::default());
        }
        let view = self.compass.clone();
        let spot = |right: f32| middle + right / SPAN * half;
        let row = top + high / 2.0;
        // The ticks, every 15 degrees, and the letters at the quarters. East is along +x and north along +y, as the map has them.
        for n in 0..24 {
            let world = n as f32 * std::f32::consts::TAU / 24.0;
            let right = wrapped(view.facing - world);
            if right.abs() >= SPAN * 0.97 {
                continue;
            }
            let x = spot(right);
            let letter = match n {
                0 => Some(("E", HEADING_COLOR)),
                6 => Some(("N", NORTH_COLOR)),
                12 => Some(("W", HEADING_COLOR)),
                18 => Some(("S", HEADING_COLOR)),
                _ => None,
            };
            if let Some((word, color)) = letter {
                let (wide, line) = (LETTER_LINE * 2.0 * unit, LETTER_LINE * unit);
                let box_ = [x - wide / 2.0, row - line, wide, line * 2.0];
                self.draw_even_words(cv, word, box_, line, color);
                continue;
            }
            let long = n % 3 == 0;
            let tall = high * if long { 0.32 } else { 0.18 };
            let t = [
                (x / across).round() as i32,
                ((top + high - tall - 1.0) / down).round() as i32,
                1,
                (tall / down).ceil() as i32,
            ];
            cv.fill_rect(t[0], t[1], t[2], t[3], TICK_COLOR);
        }
        // The marks, the pin last and on top, and held at the end of the strip if it is off it.
        let mut marks = view.marks.clone();
        marks.sort_by_key(|m| m.icon == CompassIcon::Pin);
        for m in &marks {
            let pin = m.icon == CompassIcon::Pin;
            if m.right.abs() > SPAN && !pin {
                continue;
            }
            let right = m.right.clamp(-SPAN, SPAN);
            let near = if pin { 1.0 } else { m.near };
            let side = (ICON_LEAST + (ICON_MOST - ICON_LEAST) * near) * unit;
            let alpha = FAR_STRENGTH + (1.0 - FAR_STRENGTH) * near;
            self.draw_compass_picture(cv, m.icon.picture(), (spot(right), row), side, alpha);
        }
        // Where the camera looks.
        let caret = [
            (middle / across).round() as i32 - 1,
            (top / down).round() as i32,
            2,
            ((high * 0.22) / down).ceil() as i32,
        ];
        cv.fill_rect(caret[0], caret[1], caret[2], caret[3], CARET_COLOR);
        if !view.line.is_empty() {
            let line_high = LINE_HIGH * unit;
            let box_ = [middle - half, top + high, half * 2.0, line_high];
            // A dark band, so that it reads over a bright wall.
            cv.fill_rect(
                bar_at[0],
                ((top + high) / down).round() as i32,
                bar_at[2],
                (line_high / down).ceil() as i32,
                LINE_BAND,
            );
            self.draw_even_words(cv, &view.line, box_, LINE_LINE * unit, LINE_COLOR);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_is_there_with_clear_corners_and_something_in_it() {
        for icon in COMPASS_ICONS {
            let img = compass_picture(icon.picture()).expect("the picture");
            assert_eq!(img.rgba.len() as u32, img.width * img.height * 4);
            assert_eq!(img.rgba[3], 0, "{icon:?}: the corner is clear");
            let solid = img.rgba.chunks(4).filter(|p| p[3] == 255).count();
            let share = solid as f32 / (img.width * img.height) as f32;
            assert!(share > 0.15 && share < 0.9, "{icon:?}: {share}");
        }
        assert!(compass_picture("nothing").is_none());
        assert!(compass_picture(BAR_PICTURE).is_some());
    }

    #[test]
    fn no_two_icons_are_the_same_picture() {
        let all: Vec<Image> = COMPASS_ICONS.iter().map(|i| icon_picture(*i)).collect();
        for (n, a) in all.iter().enumerate() {
            for b in &all[n + 1..] {
                assert_ne!(a.rgba, b.rgba);
            }
        }
    }

    #[test]
    fn an_angle_is_wrapped_to_the_short_way_round() {
        let pi = std::f32::consts::PI;
        assert!((wrapped(0.5) - 0.5).abs() < 1e-5);
        assert!((wrapped(pi + 0.5) - (0.5 - pi)).abs() < 1e-5);
        assert!((wrapped(-pi - 0.5) - (pi - 0.5)).abs() < 1e-5);
    }

    #[test]
    fn a_triangle_is_inside_either_way_round() {
        let (a, b, c) = ((-10.0, 10.0), (10.0, 10.0), (0.0, -10.0));
        assert!(triangle((0.0, 5.0), a, b, c) > 0.0);
        assert!(triangle((0.0, 5.0), a, c, b) > 0.0);
        assert!(triangle((0.0, 20.0), a, b, c) < 0.0);
    }
}
