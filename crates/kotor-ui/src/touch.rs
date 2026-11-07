//! The on-screen controls: a stick and round buttons over the HUD, for a touch screen or the mouse. The owner: "Can you add
//! a top right blue button with a gamepad icon for on-screen controls playable on mobile". The game has no such pictures,
//! so they are drawn here. Sizes are worked out in pixels, so that what is round stays round in a narrow window, where the
//! 640 wide layout is squeezed across. They grow with the window's shorter side.

use crate::canvas::{Canvas, Orient};
use crate::gui::Control;
use crate::screens::Ui;
use kotor_image::Image;

/// The blue button: a copy of the options button, left of the menu keys.
pub const TOUCH_BUTTON: &str = "BTN_TOUCH";
const TOUCH_BUTTON_FROM: &str = "BTN_OPT";
pub(crate) const GAMEPAD_PICTURE: &str = "kotorrs_gamepad";
const DISC_PICTURE: &str = "kotorrs_touch_disc";
const DISC_LIT_PICTURE: &str = "kotorrs_touch_disc_lit";
const KNOB_PICTURE: &str = "kotorrs_touch_knob";
const PICTURE_SIZE: u32 = 128;
const DISC_RING: f32 = 5.0;

/// A round button across, as a share of the window's shorter side, and never less than this many pixels.
const SIDE_SHARE: f32 = 0.09;
const SIDE_LEAST: f32 = 56.0;
/// The menu keys and the blue button, the smaller buttons, the stick's half, and the knob, in round buttons across.
const MENU_KEY_SIDES: f32 = 0.75;
const SMALL_SIDES: f32 = 0.8;
const STICK_SIDES: f32 = 1.4;
const KNOB_SIDES: f32 = 0.45;
/// A finger this many times the stick's half from its middle still takes it.
const STICK_GRABS: f32 = 1.3;
/// How far from the middle of the four big buttons each is, the gap at the edges, and the slots' least size.
const DIAMOND_SIDES: f32 = 1.04;
const GAP_SIDES: f32 = 0.12;
const SLOT_WIDE_SIDES: f32 = 0.85;
const SLOT_HIGH_SIDES: f32 = 1.4;
const ROW_TOP: i32 = 3;
const BLUE_GAP: i32 = 2;
/// The menu keys, left to right.
const MENU_KEYS: [&str; 8] = [
    "BTN_EQU", "BTN_INV", "BTN_CHAR", "BTN_ABI", "BTN_MSG", "BTN_JOU", "BTN_MAP", "BTN_OPT",
];
/// What is behind the menu keys, in the file.
const MENU_KEYS_BACK: &str = "LBL_MENUBG";
/// The bar of slots grows with these, from the window's corner.
const SLOT_BAR: [&str; 3] = ["BTN_ACTION", "LBL_ACTION", "LBL_MOULDING3"];
/// What is above the slots, and what is above the portrait.
const SLOT_BAR_TOP: &str = "LBL_ACTIONDESCBG";
const PORTRAITS_TOP: &str = "TB_PAUSE";
/// A button's word: its font, and how high a line is, as a share of a round button.
const WORD_FONT: &str = "dialogfont10x10";
const WORD_FONT_UNITS: f32 = 10.0;
const WORD_SHARE: f32 = 0.23;
const WORD_COLOR: [f32; 3] = [0.92, 0.97, 1.0];
const WORD_DIM_COLOR: [f32; 3] = [0.5, 0.55, 0.6];
/// What a part draws past its box, in layout units.
const PART_PAD: i32 = 4;

/// What the round buttons do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchKey {
    Action,
    Attack,
    Jump,
    Roll,
    Pickup,
    Pause,
    AttackMode,
}

/// All of them, in the order a driver is told of them.
pub const TOUCH_KEYS: [TouchKey; 7] = [
    TouchKey::Action,
    TouchKey::Attack,
    TouchKey::Jump,
    TouchKey::Roll,
    TouchKey::Pickup,
    TouchKey::Pause,
    TouchKey::AttackMode,
];

/// One thing of the layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchPart {
    Stick,
    /// The other stick: it turns the camera.
    Look,
    Key(TouchKey),
}

/// The boxes of the layer, and of the HUD's parts it must stay off by their tags, in layout units.
pub type TouchBoxes = (Vec<[i32; 4]>, Vec<(String, [i32; 4])>);

/// What the layer shows now.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TouchView {
    pub on: bool,
    /// Where the knob is, each way from -1 to 1: right and down.
    pub stick: [f32; 2],
    /// And the look stick's.
    pub look: [f32; 2],
    /// Being pressed now.
    pub held: Vec<TouchKey>,
    /// A thing that is on: the pause, Attack mode.
    pub lit: Vec<TouchKey>,
    /// It does nothing now: greyed.
    pub dim: Vec<TouchKey>,
    /// What the Action button says.
    pub action: String,
}

impl TouchView {
    fn word(&self, key: TouchKey) -> &str {
        match key {
            TouchKey::Action => &self.action,
            TouchKey::Attack => "Attack",
            TouchKey::Jump => "Jump",
            TouchKey::Roll => "Roll",
            TouchKey::Pickup => "Pickup",
            TouchKey::Pause => "Pause",
            TouchKey::AttackMode => "Attack mode",
        }
    }

    /// Whether a part looks different in the other view.
    fn differs(&self, other: &TouchView, part: TouchPart) -> bool {
        match part {
            TouchPart::Stick => self.stick != other.stick,
            TouchPart::Look => self.look != other.look,
            TouchPart::Key(k) => {
                self.held.contains(&k) != other.held.contains(&k)
                    || self.lit.contains(&k) != other.lit.contains(&k)
                    || self.dim.contains(&k) != other.dim.contains(&k)
                    || self.word(k) != other.word(k)
            }
        }
    }
}

/// A picture of the layer by name, drawn here.
pub(crate) fn touch_picture(name: &str) -> Option<Image> {
    match name {
        GAMEPAD_PICTURE => Some(gamepad_picture()),
        DISC_PICTURE => Some(disc_picture([8, 26, 52, 120], [96, 170, 255, 235])),
        DISC_LIT_PICTURE => Some(disc_picture([40, 120, 235, 215], [190, 225, 255, 255])),
        KNOB_PICTURE => Some(disc_picture([60, 130, 225, 225], [205, 230, 255, 255])),
        _ => None,
    }
}

/// How much of a pixel a shape covers, from how far inside it the pixel is: the edge is smoothed over one pixel.
pub(crate) fn cover(inside: f32) -> f32 {
    (inside + 0.5).clamp(0.0, 1.0)
}

/// How far inside a capsule between two points a point is.
pub(crate) fn in_capsule(p: (f32, f32), a: (f32, f32), b: (f32, f32), radius: f32) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let along =
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / (dx * dx + dy * dy).max(1e-6)).clamp(0.0, 1.0);
    radius - (p.0 - a.0 - dx * along).hypot(p.1 - a.1 - dy * along)
}

/// A disc with a ring round it, in straight alpha.
fn disc_picture(fill: [u8; 4], ring: [u8; 4]) -> Image {
    let centre = PICTURE_SIZE as f32 / 2.0;
    let mut rgba = Vec::with_capacity((PICTURE_SIZE * PICTURE_SIZE * 4) as usize);
    for y in 0..PICTURE_SIZE {
        for x in 0..PICTURE_SIZE {
            let from_middle = (x as f32 + 0.5 - centre).hypot(y as f32 + 0.5 - centre);
            let edge = cover(centre - 1.0 - from_middle);
            let on_ring = cover(from_middle - (centre - 1.0 - DISC_RING));
            let mix = |a: u8, b: u8| a as f32 + (b as f32 - a as f32) * on_ring;
            rgba.extend_from_slice(&[
                mix(fill[0], ring[0]) as u8,
                mix(fill[1], ring[1]) as u8,
                mix(fill[2], ring[2]) as u8,
                (mix(fill[3], ring[3]) * edge) as u8,
            ]);
        }
    }
    Image {
        width: PICTURE_SIZE,
        height: PICTURE_SIZE,
        rgba,
    }
}

/// The blue button's picture: a white gamepad on a blue square with round corners.
fn gamepad_picture() -> Image {
    const BLUE: [f32; 3] = [20.0, 105.0, 235.0];
    const EDGE: [f32; 3] = [150.0, 205.0, 255.0];
    const WHITE: [f32; 3] = [245.0, 250.0, 255.0];
    const SQUARE_HALF: f32 = 62.0;
    const CORNER: f32 = 16.0;
    const EDGE_THICK: f32 = 5.0;
    let centre = PICTURE_SIZE as f32 / 2.0;
    let mut rgba = Vec::with_capacity((PICTURE_SIZE * PICTURE_SIZE * 4) as usize);
    for y in 0..PICTURE_SIZE {
        for x in 0..PICTURE_SIZE {
            let p = (x as f32 + 0.5 - centre, y as f32 + 0.5 - centre);
            let (qx, qy) = (
                p.0.abs() - SQUARE_HALF + CORNER,
                p.1.abs() - SQUARE_HALF + CORNER,
            );
            let in_square = CORNER - qx.max(0.0).hypot(qy.max(0.0)) - qx.max(qy).min(0.0);
            let body = in_capsule(p, (-22.0, -8.0), (22.0, -8.0), 20.0);
            let grip_left = in_capsule(p, (-30.0, -4.0), (-40.0, 24.0), 11.0);
            let grip_right = in_capsule(p, (30.0, -4.0), (40.0, 24.0), 11.0);
            let pad = body.max(grip_left).max(grip_right);
            // What is cut out of the gamepad: a cross on the left, two buttons on the right.
            let across = (3.5 - (p.1 + 8.0).abs()).min(11.0 - (p.0 + 25.0).abs());
            let down = (3.5 - (p.0 + 25.0).abs()).min(11.0 - (p.1 + 8.0).abs());
            let buttons =
                (5.0 - (p.0 - 19.0).hypot(p.1 + 3.0)).max(5.0 - (p.0 - 30.0).hypot(p.1 + 13.0));
            let holes = across.max(down).max(buttons);
            let white = cover(pad.min(-holes));
            let ring = cover(EDGE_THICK - in_square);
            let color = |k: usize| {
                let ground = BLUE[k] + (EDGE[k] - BLUE[k]) * ring;
                (ground + (WHITE[k] - ground) * white) as u8
            };
            rgba.extend_from_slice(&[
                color(0),
                color(1),
                color(2),
                (cover(in_square) * 255.0) as u8,
            ]);
        }
    }
    Image {
        width: PICTURE_SIZE,
        height: PICTURE_SIZE,
        rgba,
    }
}

fn find<'c>(panel: &'c Control, tag: &str) -> Option<&'c Control> {
    panel.children.iter().find(|c| c.tag == tag)
}

impl Ui<'_> {
    /// A round button across, in pixels.
    fn touch_side(&self) -> f32 {
        (self.pixels.0.min(self.pixels.1) as f32 * SIDE_SHARE).max(SIDE_LEAST)
    }

    /// A box of layout units about a point of pixels, this many pixels across, round on the screen.
    pub(crate) fn round_box(&self, middle: (f32, f32), across: f32) -> [i32; 4] {
        let (x, y) = (self.hud_across(), self.scale);
        [
            ((middle.0 - across / 2.0) / x).round() as i32,
            ((middle.1 - across / 2.0) / y).round() as i32,
            (across / x).round() as i32,
            (across / y).round() as i32,
        ]
    }

    /// The HUD's own controls for the layer: the blue button always, and with the layer on bigger menu keys and slots.
    pub(crate) fn place_for_touch(&self, panel: &mut Control) {
        // As low as the keys come as the file has them, in a narrow window, whether they are big now or not.
        let file_row_bottom =
            find(panel, TOUCH_BUTTON_FROM).map_or(ROW_TOP, |c| c.extent.top + c.extent.height);
        let (across, down) = (self.hud_across(), self.scale);
        let side = self.touch_side();
        let wide = self.width as i32;
        let key = (
            (side * MENU_KEY_SIDES / across).round() as i32,
            (side * MENU_KEY_SIDES / down).round() as i32,
        );
        if self.touch.on {
            for (n, tag) in MENU_KEYS.iter().rev().enumerate() {
                if let Some(c) = panel.children.iter_mut().find(|c| c.tag == *tag) {
                    c.extent.left = wide - ROW_TOP - (n as i32 + 1) * key.0;
                    c.extent.top = ROW_TOP;
                    c.extent.width = key.0;
                    c.extent.height = key.1;
                }
            }
            if let Some(c) = panel.children.iter_mut().find(|c| c.tag == MENU_KEYS_BACK) {
                let keys = MENU_KEYS.len() as i32;
                c.extent.left = wide - ROW_TOP - keys * key.0;
                c.extent.top = ROW_TOP;
                c.extent.width = keys * key.0;
                c.extent.height = key.1;
            }
            let slot =
                find(panel, "BTN_ACTION0").map_or([1, 1], |c| [c.extent.width, c.extent.height]);
            let more_wide = (side * SLOT_WIDE_SIDES / (slot[0] as f32 * across)).max(1.0);
            let more_high = (side * SLOT_HIGH_SIDES / (slot[1] as f32 * down)).max(1.0);
            let high = crate::screens::SCREEN.1 as i32;
            let bar = panel
                .children
                .iter_mut()
                .filter(|c| SLOT_BAR.iter().any(|p| c.tag.starts_with(p)));
            for c in bar {
                let e = &mut c.extent;
                let right = e.left + e.width;
                let bottom = e.top + e.height;
                let from_right = ((wide - right) as f32 * more_wide).round() as i32;
                let from_bottom = ((high - bottom) as f32 * more_high).round() as i32;
                e.width = (e.width as f32 * more_wide).round() as i32;
                e.height = (e.height as f32 * more_high).round() as i32;
                e.left = wide - from_right - e.width;
                e.top = high - from_bottom - e.height;
            }
        }
        // In the corner under the keys, as big as they are when they are big. It stays put, on or off: a second tap in the same
        // place turns the layer off again.
        if let Some(mut blue) = find(panel, TOUCH_BUTTON_FROM).cloned() {
            blue.tag = TOUCH_BUTTON.to_string();
            blue.extent.left = wide - ROW_TOP - key.0;
            blue.extent.top = self.touch_row_bottom().max(file_row_bottom) + BLUE_GAP;
            blue.extent.width = key.0;
            blue.extent.height = key.1;
            panel.children.push(blue);
        }
    }

    /// How low the bigger menu keys come, in layout units: what goes under them starts here.
    pub(crate) fn touch_row_bottom(&self) -> i32 {
        ROW_TOP + (self.touch_side() * MENU_KEY_SIDES / self.scale).round() as i32
    }

    /// What the corner keeps for the blue button, in layout units across.
    pub(crate) fn blue_button_wide(&self) -> i32 {
        (self.touch_side() * MENU_KEY_SIDES / self.hud_across()).round() as i32 + ROW_TOP
    }

    /// Where each part of the layer is, in layout units. The stick stands over the portraits, against the left edge. The
    /// four big buttons stand over the slots, against the right edge, with the three small ones to their left and the look
    /// stick left of those, as the right stick of a controller is further in than its buttons.
    pub(crate) fn touch_places(&self, panel: &Control) -> Vec<(TouchPart, [i32; 4])> {
        let side = self.touch_side();
        let gap = side * GAP_SIDES;
        let down = self.scale;
        let top_of =
            |tag: &str, or: f32| find(panel, tag).map_or(or, |c| c.extent.top as f32 * down);
        let high = self.pixels.1 as f32;
        let slots_top = top_of(SLOT_BAR_TOP, high);
        let portraits_top = top_of(PORTRAITS_TOP, high);
        let half = side * STICK_SIDES;
        let mut out = vec![(
            TouchPart::Stick,
            self.round_box((gap + half, portraits_top - gap - half), half * 2.0),
        )];
        let reach = side * DIAMOND_SIDES;
        let right = self.pixels.0 as f32 - gap;
        let middle = (
            right - reach - side / 2.0,
            slots_top - gap - reach - side / 2.0,
        );
        for (key, dx, dy) in [
            (TouchKey::Action, 0.0, reach),
            (TouchKey::Attack, reach, 0.0),
            (TouchKey::Jump, 0.0, -reach),
            (TouchKey::Roll, -reach, 0.0),
        ] {
            let at = (middle.0 + dx, middle.1 + dy);
            out.push((TouchPart::Key(key), self.round_box(at, side)));
        }
        let small = side * SMALL_SIDES;
        let column = middle.0 - reach - side / 2.0 - gap - small / 2.0;
        let step = small + gap;
        for (key, dy) in [
            (TouchKey::Pickup, step),
            (TouchKey::AttackMode, 0.0),
            (TouchKey::Pause, -step),
        ] {
            let at = (column, middle.1 + dy);
            out.push((TouchPart::Key(key), self.round_box(at, small)));
        }
        let look = (column - small / 2.0 - gap - half, middle.1);
        out.push((TouchPart::Look, self.round_box(look, half * 2.0)));
        out
    }

    /// What the layer gives a window this view is for.
    pub fn set_touch(&mut self, view: TouchView) {
        self.touch = view;
    }

    /// What of the layer is at a point of the layout, if it is on.
    pub fn touch_at(&mut self, x: f32, y: f32) -> Option<TouchPart> {
        if !self.touch.on {
            return None;
        }
        let gui = self.hud_layout(None)?;
        let places = self.touch_places(&gui.panel);
        places
            .into_iter()
            .find(|(_, e)| {
                x >= e[0] as f32
                    && x < (e[0] + e[2]) as f32
                    && y >= e[1] as f32
                    && y < (e[1] + e[3]) as f32
            })
            .map(|(part, _)| part)
    }

    /// A stick's middle in pixels and how far a finger goes from it for the most, in pixels.
    fn stick_middle(&mut self, stick: TouchPart) -> Option<((f32, f32), f32)> {
        let gui = self.hud_layout(None)?;
        let places = self.touch_places(&gui.panel);
        let (_, e) = places.into_iter().find(|p| p.0 == stick)?;
        let middle = (
            (e[0] as f32 + e[2] as f32 / 2.0) * self.hud_across(),
            (e[1] as f32 + e[3] as f32 / 2.0) * self.scale,
        );
        Some((middle, self.touch_side() * (STICK_SIDES - KNOB_SIDES / 2.0)))
    }

    /// The stick a finger at this point of the layout takes, though it is a little off it.
    pub fn touch_stick_takes(&mut self, x: f32, y: f32) -> Option<TouchPart> {
        if !self.touch.on {
            return None;
        }
        let across = self.hud_across();
        let down = self.scale;
        let reach = self.touch_side() * STICK_SIDES * STICK_GRABS;
        [TouchPart::Stick, TouchPart::Look].into_iter().find(|s| {
            self.stick_middle(*s)
                .is_some_and(|(m, _)| (x * across - m.0).hypot(y * down - m.1) <= reach)
        })
    }

    /// Where the knob goes for a finger at this point of the layout, each way from -1 to 1: right and down.
    pub fn touch_stick_value(&mut self, stick: TouchPart, x: f32, y: f32) -> [f32; 2] {
        let across = self.hud_across();
        let down = self.scale;
        let Some((m, most)) = self.stick_middle(stick) else {
            return [0.0; 2];
        };
        let (dx, dy) = ((x * across - m.0) / most, (y * down - m.1) / most);
        let far = dx.hypot(dy).max(1.0);
        [dx / far, dy / far]
    }

    /// The middle of a part of the layer, in layout units, and each key's name for a driver.
    pub fn touch_point(&mut self, part: TouchPart) -> Option<(f32, f32)> {
        let gui = self.hud_layout(None)?;
        let places = self.touch_places(&gui.panel);
        let (_, e) = places.into_iter().find(|p| p.0 == part)?;
        Some((
            e[0] as f32 + e[2] as f32 / 2.0,
            e[1] as f32 + e[3] as f32 / 2.0,
        ))
    }

    /// The boxes of the layer, the HUD's parts it must stay off, and the menu keys, in layout units. For a test.
    pub fn touch_boxes(&mut self) -> Option<TouchBoxes> {
        let gui = self.hud_layout(None)?;
        let places = self.touch_places(&gui.panel);
        let hud = gui
            .panel
            .children
            .iter()
            .filter(|c| {
                [
                    "LBL_MAPBORDER",
                    "BTN_MINIMAP",
                    "BTN_ACTION",
                    "LBL_ACTION",
                    "LBL_BACK",
                ]
                .iter()
                .any(|p| c.tag.starts_with(p))
                    || [
                        "LBL_CHAR1",
                        "LBL_CHAR2",
                        "LBL_CHAR3",
                        "TB_PAUSE",
                        "TB_STEALTH",
                        TOUCH_BUTTON,
                    ]
                    .contains(&c.tag.as_str())
                    || MENU_KEYS.contains(&c.tag.as_str())
            })
            .map(|c| {
                let e = &c.extent;
                (c.tag.clone(), [e.left, e.top, e.width, e.height])
            })
            .collect();
        Some((places.into_iter().map(|p| p.1).collect(), hud))
    }

    /// The pixels that change between two views of the layer, which is on in both.
    pub(crate) fn touch_changes(
        &self,
        panel: &Control,
        was: &TouchView,
        now: &TouchView,
    ) -> Vec<[u32; 4]> {
        self.touch_places(panel)
            .into_iter()
            .filter(|(part, _)| was.differs(now, *part))
            .filter_map(|(_, e)| self.pixel_box(e, PART_PAD))
            .collect()
    }

    /// Draw the layer, over everything else on the HUD.
    pub(crate) fn draw_touch(&mut self, cv: &mut Canvas, panel: &Control) {
        if !self.touch.on {
            return;
        }
        for (part, e) in self.touch_places(panel) {
            match part {
                TouchPart::Stick => self.draw_stick(cv, e, self.touch.stick),
                TouchPart::Look => self.draw_stick(cv, e, self.touch.look),
                TouchPart::Key(key) => self.draw_touch_key(cv, key, e),
            }
        }
    }

    fn draw_picture(&mut self, cv: &mut Canvas, name: &str, at: [i32; 4]) {
        if let Some(img) = self.texture_shared(name) {
            let full = [0, 0, img.width as i32, img.height as i32];
            cv.draw_image(&img, full, at, [1.0; 3], 1.0, Orient::default());
        }
    }

    fn draw_stick(&mut self, cv: &mut Canvas, e: [i32; 4], knob_at: [f32; 2]) {
        self.draw_picture(cv, DISC_PICTURE, e);
        let (across, down) = (self.hud_across(), self.scale);
        let side = self.touch_side();
        let knob = side * KNOB_SIDES * 2.0;
        let most = side * STICK_SIDES - knob / 2.0;
        let middle = (
            (e[0] as f32 + e[2] as f32 / 2.0) * across + knob_at[0] * most,
            (e[1] as f32 + e[3] as f32 / 2.0) * down + knob_at[1] * most,
        );
        let at = self.round_box(middle, knob);
        self.draw_picture(cv, KNOB_PICTURE, at);
    }

    fn draw_touch_key(&mut self, cv: &mut Canvas, key: TouchKey, e: [i32; 4]) {
        let on = self.touch.held.contains(&key) || self.touch.lit.contains(&key);
        let dim = self.touch.dim.contains(&key);
        self.draw_picture(cv, if on { DISC_LIT_PICTURE } else { DISC_PICTURE }, e);
        let word = self.touch.word(key).to_string();
        let (across, down) = (self.hud_across(), self.scale);
        let place = [
            e[0] as f32 * across,
            e[1] as f32 * down,
            e[2] as f32 * across,
            e[3] as f32 * down,
        ];
        let color = if dim { WORD_DIM_COLOR } else { WORD_COLOR };
        let line = self.touch_side() * WORD_SHARE;
        self.draw_even_words(cv, &word, place, line, color);
    }

    /// Words in the middle of a box of pixels, with a line this many pixels high, whichever way the window is squeezed.
    pub(crate) fn draw_even_words(
        &mut self,
        cv: &mut Canvas,
        words: &str,
        place: [f32; 4],
        line: f32,
        color: [f32; 3],
    ) {
        let k = line / WORD_FONT_UNITS;
        let r = place.map(|v| (v / k).round() as i32);
        let kept = (cv.scale, cv.scale_x);
        (cv.scale, cv.scale_x) = (k, k);
        self.measure_evenly();
        self.draw_text(cv, WORD_FONT, words, r, 18, color);
        (cv.scale, cv.scale_x) = kept;
        self.measure_for_hud();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pictures_are_there_and_round_at_the_corners() {
        for name in [
            GAMEPAD_PICTURE,
            DISC_PICTURE,
            DISC_LIT_PICTURE,
            KNOB_PICTURE,
        ] {
            let img = touch_picture(name).unwrap();
            assert_eq!(img.rgba.len() as u32, img.width * img.height * 4);
            assert_eq!(img.rgba[3], 0, "{name}: the corner is see-through");
        }
        assert!(touch_picture("nothing").is_none());
    }

    #[test]
    fn the_blue_button_is_blue_with_a_white_gamepad_on_it() {
        let img = gamepad_picture();
        let at = |x: u32, y: u32| {
            let i = ((y * img.width + x) * 4) as usize;
            [img.rgba[i], img.rgba[i + 1], img.rgba[i + 2]]
        };
        let ground = at(64, 110);
        assert!(ground[2] > 200 && ground[0] < 60, "blue, not {ground:?}");
        let pad = at(64, 56);
        assert!(pad[0] > 220, "white, not {pad:?}");
    }

    #[test]
    fn a_view_says_what_changed() {
        let a = TouchView {
            on: true,
            action: "Talk".to_string(),
            ..TouchView::default()
        };
        let mut b = a.clone();
        assert!(!a.differs(&b, TouchPart::Stick));
        b.stick = [0.5, 0.0];
        assert!(a.differs(&b, TouchPart::Stick));
        assert!(!a.differs(&b, TouchPart::Key(TouchKey::Jump)));
        b.held.push(TouchKey::Jump);
        assert!(a.differs(&b, TouchPart::Key(TouchKey::Jump)));
        b.action = "Open".to_string();
        assert!(a.differs(&b, TouchPart::Key(TouchKey::Action)));
    }
}
