//! Menus drawn from stock GUI files: the main menu, the pause menu, the save screens, inventory and equipment. A screen
//! says what it wants changed in a [`MenuView`], and the same view answers what is under the cursor.
//!
//! The 640x480 layout is stretched over the whole window, as in the reference video (`docs/MENU_REFERENCE.md`).
//! **Assumptions:** controls are drawn in file order, so later ones lie on top (the full-screen `LBL_3DVIEW` comes before
//! the logo and buttons); list rows follow the file's `PROTOITEM`, whose extent is relative to the panel; a button under
//! the cursor has yellow text.

use crate::canvas::Canvas;
use crate::gui::{Control, Gui};
use crate::screens::{merged_boxes, union_box, Patch, Ui, CONTROL_BOX_PAD, SCREEN};
use kotor_image::Image;
use std::collections::BTreeMap;

const LIT: [f32; 3] = [1.0, 1.0, 0.3];
const DIM: [f32; 3] = [0.45, 0.5, 0.55];
const TEXT_BLUE: [f32; 3] = [0.0, 0.65, 0.97];
/// `PROTOITEM` and `SCROLLBAR` are children; these two kinds are drawn with their list.
const LIST: i32 = 11;
const SCROLLBAR: i32 = 9;
const CHECKBOX: i32 = 7;
const SLIDER: i32 = 8;

/// A line one pixel wide round a rectangle of layout units.
fn thin_line(cv: &mut Canvas, r: [i32; 4], color: [u8; 3]) {
    let (sx, sy) = (cv.scale_x, cv.scale);
    let left = (r[0] as f32 * sx).round() as i32;
    let top = (r[1] as f32 * sy).round() as i32;
    let right = ((r[0] + r[2]) as f32 * sx).round() as i32 - 1;
    let bottom = ((r[1] + r[3]) as f32 * sy).round() as i32 - 1;
    let pixel = [color[0], color[1], color[2], 255];
    for x in left..=right {
        cv.blend(x, top, pixel);
        cv.blend(x, bottom, pixel);
    }
    for y in top..=bottom {
        cv.blend(left, y, pixel);
        cv.blend(right, y, pixel);
    }
}

/// One row of a list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Row {
    pub text: String,
    /// Picture at the left of the row.
    pub icon: Option<String>,
    /// How many are stacked, shown over the icon from 2.
    pub count: i32,
    pub selected: bool,
    /// Greyed: it cannot be chosen.
    pub dim: bool,
    /// Another font for the row's text.
    pub font: Option<String>,
    /// Pictures across the row in place of its text, each leading to the next.
    pub cells: Vec<Cell>,
    /// How many of its `count` are chosen, with an arrow each side to change it.
    pub stepper: Option<i32>,
    /// A bar behind it: it is one of the rows chosen, in a list whose file has no highlight for that.
    pub bar: bool,
}

/// One picture of a row that has several, as a chain of feats does.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cell {
    pub icon: String,
    /// Greyed: it cannot be chosen.
    pub dim: bool,
    /// Framed in green: it is had already.
    pub owned: bool,
    /// Framed in yellow.
    pub selected: bool,
}

/// A box of words with one button over a screen, in the colours of `confirm.gui`, sized to its words.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Notice {
    pub text: String,
    pub button: String,
}

/// The tag a [`Notice`]'s button answers to.
const NOTICE_BUTTON: &str = "BTN_OK";
const NOTICE_FONT: &str = "fnt_d16x16";
const NOTICE_WIDTH: i32 = 460;
/// A notice's box, its button, its lines and how far apart they are.
type NoticeLayout = ([i32; 4], [i32; 4], Vec<String>, i32);

/// A picture laid over a screen, centred on a point of the layout.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mark {
    pub at: (f32, f32),
    pub picture: String,
    pub size: (i32, i32),
    /// How much of the picture across and down shows, from its top left.
    pub part: [f32; 2],
    pub tint: [f32; 3],
    /// Turned this many radians clockwise, for a square picture.
    pub angle: Option<f32>,
}

/// What is under the cursor.
#[derive(Clone, Debug, PartialEq)]
pub enum Hit {
    Button(String),
    Row(String, usize),
    /// A picture of a row that has several.
    Cell(String, usize, usize),
    /// A slider, and how far along it, 0 to 1.
    Slider(String, f32),
    /// An arrow of a row's stepper: the list, the row, and -1 or 1.
    Step(String, usize, i32),
}

/// One screen: a GUI file and what to change in it.
#[derive(Clone, Debug, Default)]
pub struct MenuView {
    pub gui: String,
    pub patches: BTreeMap<String, Patch>,
    /// Lists by tag: rows, and the first row shown.
    pub lists: BTreeMap<String, (Vec<Row>, usize)>,
    pub hover: Option<Hit>,
    /// A line at the foot of the screen.
    pub message: String,
    /// A row of controls of its own, `top.gui`, drawn after this screen and asked first what is under the cursor.
    pub top: Option<Box<MenuView>>,
    /// Holes darkened from their left, for a lit thing that the renderer lights evenly: the galaxy map's planet.
    pub hole_shades: Vec<String>,
    /// A dialog over this screen. It takes all the input.
    pub dialog: Option<Box<MenuView>>,
    /// The scene shows around the panel, instead of black.
    pub over_scene: bool,
    /// Pictures over the screen, in order.
    pub marks: Vec<Mark>,
    /// A box over it all that takes all the input.
    pub notice: Option<Notice>,
    /// Controls that are holes in the picture, for a figure behind them, and how much that is dimmed.
    pub holes: Vec<String>,
    pub hole_veil: u8,
    /// Controls the file has not got: a copy of one it has, with a new tag, at a place of the panel.
    pub copies: Vec<(String, String, [i32; 4])>,
}

impl MenuView {
    pub fn new(gui: &str) -> MenuView {
        MenuView {
            gui: gui.to_string(),
            ..MenuView::default()
        }
    }

    /// One more control, a copy of one the file has, at a place of the panel: left, top, width, height.
    pub fn copy_control(&mut self, from: &str, tag: &str, at: [i32; 4]) {
        self.copies.push((from.to_string(), tag.to_string(), at));
    }

    pub fn hide(&mut self, tags: &[&str]) {
        for t in tags {
            self.patches.entry(t.to_string()).or_default().hidden = true;
        }
    }

    pub fn text(&mut self, tag: &str, text: impl Into<String>) {
        self.patches.entry(tag.to_string()).or_default().text = Some(text.into());
    }

    pub fn fill(&mut self, tag: &str, fill: impl Into<String>) {
        self.patches.entry(tag.to_string()).or_default().fill = Some(fill.into());
    }

    /// A slider's place, or a checkbox: ticked when not zero.
    pub fn value(&mut self, tag: &str, cur: i32, max: i32) {
        self.patches.entry(tag.to_string()).or_default().value = Some((cur, max));
    }

    /// Draw this control's text in another font.
    /// Draw this control without its border.
    pub fn plain(&mut self, tag: &str) {
        self.patches.entry(tag.to_string()).or_default().plain = true;
    }

    pub fn font(&mut self, tag: &str, font: &str) {
        self.patches.entry(tag.to_string()).or_default().font = Some(font.to_string());
    }

    /// Grey these buttons: they do nothing yet.
    pub fn inert(&mut self, tags: &[&str]) {
        for t in tags {
            self.patches.entry(t.to_string()).or_default().color = Some(DIM);
        }
    }

    /// This control's text is lit whatever the cursor does.
    pub fn lit(&mut self, tag: &str) {
        self.patches.entry(tag.to_string()).or_default().color = Some(LIT);
    }

    /// This label answers to the cursor as a button does.
    pub fn live(&mut self, tag: &str) {
        self.patches.entry(tag.to_string()).or_default().live = true;
    }

    fn is_hidden(&self, tag: &str) -> bool {
        self.patches.get(tag).is_some_and(|p| p.hidden)
    }
}

/// Where the pictures of a row with several go: squares at its left, middle and right.
fn cell_rects(row: [i32; 4]) -> [[i32; 4]; 3] {
    let side = row[3] - 8;
    let step = (row[2] - side) / 2;
    [0, 1, 2].map(|i| [row[0] + i * step, row[1] + 4, side, side])
}

/// Behind a chosen row: the interface's blue, thin enough for the row's words to read.
const CHOSEN_BAR: [u8; 4] = [0, 166, 247, 70];
/// A row's stepper, in layout units: each arrow's width, the number's and how tall it is.
const STEP_ARROW: i32 = 18;
const STEP_NUMBER: i32 = 36;
const STEP_HIGH: i32 = 24;
const STEP_WIDTH: i32 = 2 * STEP_ARROW + STEP_NUMBER + 4;

/// Where a stepper goes, at the right of its row: less, the number, and more.
fn stepper_rects(row: [i32; 4]) -> [[i32; 4]; 3] {
    let x = row[0] + row[2] - STEP_WIDTH;
    let y = row[1] + (row[3] - STEP_HIGH) / 2;
    [
        [x, y, STEP_ARROW, STEP_HIGH],
        [x + STEP_ARROW, y, STEP_NUMBER, STEP_HIGH],
        [x + STEP_ARROW + STEP_NUMBER, y, STEP_ARROW, STEP_HIGH],
    ]
}

/// Whether a control of a screen is drawn differently in another view of it.
fn control_changed(was: &MenuView, now: &MenuView, c: &Control) -> bool {
    let lit = |v: &MenuView| match &v.hover {
        Some(Hit::Button(t)) | Some(Hit::Slider(t, _)) => t == &c.tag,
        _ => false,
    };
    if lit(was) != lit(now) || was.patches.get(&c.tag) != now.patches.get(&c.tag) {
        return true;
    }
    let row = |v: &MenuView| match &v.hover {
        Some(h @ (Hit::Row(t, _) | Hit::Cell(t, _, _) | Hit::Step(t, _, _))) if t == &c.tag => {
            Some(h.clone())
        }
        _ => None,
    };
    c.kind == LIST && (row(was) != row(now) || was.lists.get(&c.tag) != now.lists.get(&c.tag))
}

/// How far across a shaded hole the dark comes, and how dark it starts: by eye from the video at 8:29:45.
const SHADE_REACH: f32 = 0.75;
const SHADE_DARKEST: f32 = 245.0;
/// A needle's width in layout units, and how solid a picture's pixel has to be to count as the picture.
const NEEDLE_WIDTH: i32 = 14;
const SOLID: u8 = 128;
/// How many rows of the picture stay beyond the needle at each end of its way.
const NEEDLE_INSET: usize = 4;

/// Where a list's rows go: the first row's rectangle in panel units, the step down, and how many fit.
fn row_layout(list: &Control) -> Option<([i32; 4], i32, usize)> {
    let p = list.proto_item.as_ref()?;
    let e = &p.extent;
    let step = (e.height + list.padding).max(1);
    let room = list.extent.top + list.extent.height - e.top + list.padding;
    Some((
        [e.left, e.top, e.width, e.height],
        step,
        (room / step).max(1) as usize,
    ))
}

impl Ui<'_> {
    /// The layout's left and top on this screen.
    fn panel_origin(&self, panel: &Control) -> (i32, i32) {
        (panel.extent.left, panel.extent.top)
    }

    /// A screen on black, or with `over` left showing around a dialog.
    /// A view's file, with the controls it has added.
    fn view_gui(&mut self, view: &MenuView) -> Option<Gui> {
        let mut gui = self.gui(&view.gui)?;
        for (from, tag, at) in &view.copies {
            let Some(mut c) = gui.find(from).cloned() else {
                continue;
            };
            c.tag.clone_from(tag);
            c.extent.left = at[0];
            c.extent.top = at[1];
            c.extent.width = at[2];
            c.extent.height = at[3];
            gui.panel.children.push(c);
        }
        Some(gui)
    }

    pub fn menu_overlay(&mut self, view: &MenuView) -> Option<Image> {
        self.measure_for_menus();
        let mut cv = Canvas::scaled_layout(self.pixels, self.scale);
        cv.scale_x = self.pixels.0 as f32 / SCREEN.0 as f32;
        cv.clear(if view.over_scene {
            [0, 0, 0, 0]
        } else {
            [0, 0, 0, 255]
        });
        if !view.gui.is_empty() {
            self.draw_view(&mut cv, view)?;
        }
        if let Some(t) = &view.top {
            self.draw_view(&mut cv, t)?;
        }
        if let Some(d) = &view.dialog {
            cv.fill_rect(0, 0, SCREEN.0 as i32 + 1, 481, [0, 0, 0, 140]);
            self.draw_view(&mut cv, d)?;
            if self.glowing() {
                let e = self.gui(&d.gui)?.panel.extent.clone();
                crate::glow::edge(&mut cv, [e.left, e.top, e.width, e.height]);
            }
        }
        if let Some(n) = &view.notice {
            self.draw_notice(&mut cv, n, &view.hover);
        }
        Some(cv.into_image())
    }

    /// Draw again only what has changed between two views of a screen, on the picture of the first. The whole of one takes 20
    /// ms at 3840x2160, and the cursor changes what is lit each time it moves onto something. Gives the picture and the box of
    /// pixels that changed, or nothing if the whole has to be drawn.
    pub fn menu_part(
        &mut self,
        picture: Image,
        was: &MenuView,
        now: &MenuView,
    ) -> Option<(Image, [u32; 4])> {
        if (picture.width, picture.height) != self.pixels {
            return None;
        }
        self.measure_for_menus();
        let boxes = self.changed_boxes(was, now)?;
        let across = self.pixels.0 as f32 / SCREEN.0 as f32;
        let down = self.scale;
        let (wide, high) = (self.pixels.0 as i32, self.pixels.1 as i32);
        let pixels = boxes.iter().filter_map(|&[x, y, w, h]| {
            let left = ((x as f32 * across).floor() as i32 - 1).clamp(0, wide);
            let top = ((y as f32 * down).floor() as i32 - 1).clamp(0, high);
            let right = (((x + w) as f32 * across).ceil() as i32 + 1).clamp(0, wide);
            let bottom = (((y + h) as f32 * down).ceil() as i32 + 1).clamp(0, high);
            (right > left && bottom > top).then_some([
                left as u32,
                top as u32,
                (right - left) as u32,
                (bottom - top) as u32,
            ])
        });
        let pixels = merged_boxes(pixels.collect());
        let mut cv = Canvas::over(picture, self.scale);
        cv.scale_x = across;
        let mut changed = None;
        for b in pixels {
            cv.redraw_only(b);
            if !now.over_scene {
                cv.fill_rect(0, 0, SCREEN.0 as i32 + 1, 481, [0, 0, 0, 255]);
            }
            self.draw_view(&mut cv, now)?;
            if let Some(t) = &now.top {
                self.draw_view(&mut cv, t)?;
            }
            changed = union_box(changed, Some(b));
        }
        Some((cv.into_image(), changed.unwrap_or([0; 4])))
    }

    /// The boxes of layout units that hold all that has changed between two views, or nothing if they are too unlike.
    fn changed_boxes(&mut self, was: &MenuView, now: &MenuView) -> Option<Vec<[i32; 4]>> {
        let alike = was.gui == now.gui
            && !now.gui.is_empty()
            && was.over_scene == now.over_scene
            && was.holes == now.holes
            && was.hole_veil == now.hole_veil
            && was.hole_shades == now.hole_shades
            && was.copies == now.copies
            && was.marks == now.marks
            && was.message == now.message
            && was.notice.is_none()
            && now.notice.is_none()
            && was.dialog.is_none()
            && now.dialog.is_none();
        if !alike {
            return None;
        }
        let gui = self.view_gui(now)?;
        let (ox, oy) = self.panel_origin(&gui.panel);
        let mut boxes = Vec::new();
        for c in gui.panel.children.iter().filter(|c| c.kind != SCROLLBAR) {
            if !control_changed(was, now, c) {
                continue;
            }
            for view in [was, now] {
                let patch = view.patches.get(&c.tag);
                if patch.is_some_and(|p| p.cover.is_some()) {
                    return None;
                }
                let e = &c.extent;
                let [x, y, w, h] = patch
                    .and_then(|p| p.place)
                    .unwrap_or([e.left, e.top, e.width, e.height]);
                let mut pad = CONTROL_BOX_PAD + self.words_past(c, patch, [w, h]);
                if c.kind == SLIDER {
                    pad += NEEDLE_WIDTH * 2;
                }
                boxes.push([ox + x - pad, oy + y - pad, w + pad * 2, h + pad * 2]);
                if let Some((r, step, fits)) = row_layout(c).filter(|_| c.kind == LIST) {
                    let rise = patch.map_or(0, |p| p.rise);
                    let tall = step * fits as i32;
                    boxes.push([
                        ox + r[0] - CONTROL_BOX_PAD,
                        oy + r[1] - rise - CONTROL_BOX_PAD,
                        r[2] + CONTROL_BOX_PAD * 2,
                        tall + rise + CONTROL_BOX_PAD * 2,
                    ]);
                }
            }
        }
        match (&was.top, &now.top) {
            (Some(a), Some(b)) => boxes.extend(self.changed_boxes(a, b)?),
            (None, None) => {}
            _ => return None,
        }
        Some(boxes)
    }

    /// How far a control's words can go past it: words too long for it do, across and down.
    fn words_past(&mut self, c: &Control, patch: Option<&Patch>, [w, h]: [i32; 2]) -> i32 {
        let Some(t) = &c.text else { return 0 };
        let text = patch
            .and_then(|p| p.text.clone())
            .unwrap_or_else(|| match t.strref {
                Some(s) if t.text.is_empty() => self.strref(s),
                _ => t.text.clone(),
            });
        let font = patch.and_then(|p| p.font.as_deref()).unwrap_or(&t.font);
        let Some(f) = self.font_shared(font) else {
            return 0;
        };
        let inset = patch.map_or(0, |p| p.inset);
        let lines = f.wrap(&text, w - inset * 2);
        let wide = lines.iter().map(|l| f.measure(l)).max().unwrap_or(0);
        (wide - w)
            .max(lines.len() as i32 * f.line_height - h)
            .max(0)
    }

    /// Where a notice goes: its box, its button, and its lines.
    fn notice_layout(&mut self, n: &Notice) -> Option<NoticeLayout> {
        let f = self.font(NOTICE_FONT)?;
        let lines = f.wrap(&n.text, NOTICE_WIDTH - 24);
        let step = f.line_height;
        let height = lines.len() as i32 * step + 54;
        let x = (SCREEN.0 as i32 - NOTICE_WIDTH) / 2;
        let y = (SCREEN.1 as i32 - height) / 2;
        let button = [SCREEN.0 as i32 / 2 - 50, y + height - 34, 100, 26];
        Some(([x, y, NOTICE_WIDTH, height], button, lines, step))
    }

    fn draw_notice(&mut self, cv: &mut Canvas, n: &Notice, hover: &Option<Hit>) {
        let Some((r, button, lines, step)) = self.notice_layout(n) else {
            return;
        };
        const EDGE: [u8; 4] = [20, 50, 110, 255];
        let frame = |cv: &mut Canvas, r: [i32; 4], c: [u8; 4]| {
            cv.fill_rect(r[0], r[1], r[2], 1, c);
            cv.fill_rect(r[0], r[1] + r[3] - 1, r[2], 1, c);
            cv.fill_rect(r[0], r[1], 1, r[3], c);
            cv.fill_rect(r[0] + r[2] - 1, r[1], 1, r[3], c);
        };
        if self.glowing() {
            crate::glow::boxed(cv, r);
        } else {
            cv.fill_rect(r[0], r[1], r[2], r[3], [0, 0, 8, 235]);
            frame(cv, r, EDGE);
        }
        for (i, line) in lines.iter().enumerate() {
            let at = [r[0] + 12, r[1] + 10 + i as i32 * step, r[2] - 24, step];
            self.draw_text(cv, NOTICE_FONT, line, at, 18, TEXT_BLUE);
        }
        let lit = matches!(hover, Some(Hit::Button(t)) if t == NOTICE_BUTTON);
        frame(cv, button, if lit { [255, 255, 76, 255] } else { EDGE });
        let text = n.button.clone();
        self.draw_text(cv, NOTICE_FONT, &text, button, 18, LIT);
    }

    fn draw_view(&mut self, cv: &mut Canvas, view: &MenuView) -> Option<()> {
        let gui = self.view_gui(view)?;
        let p = &gui.panel;
        let (ox, oy) = self.panel_origin(p);
        let r = [ox, oy, p.extent.width, p.extent.height];
        // **Assumed:** a panel over the scene gets a dark backing, since its picture is see-through and text must read. A
        // rounded panel has its own shape twice over, which is as dark and has no square corners.
        if view.over_scene {
            if crate::screens::ROUNDED_FILLS.contains(&p.border.fill.as_str()) {
                self.draw_border(cv, &p.border, r, None);
            } else {
                cv.fill_rect(r[0], r[1], r[2], r[3], [0, 0, 0, 140]);
            }
        }
        self.draw_border(cv, &p.border, r, None);
        for c in &p.children {
            if view.is_hidden(&c.tag) || c.kind == SCROLLBAR {
                continue;
            }
            let moved = view.patches.get(&c.tag).and_then(|p| p.place);
            if let Some([left, top, width, height]) = moved {
                let r = [ox + left, oy + top, width, height];
                if view.holes.contains(&c.tag) {
                    cv.clear_rect(r[0], r[1], r[2], r[3], view.hole_veil);
                } else {
                    self.draw_plain(cv, view, c, r);
                }
                continue;
            }
            if view.holes.contains(&c.tag) {
                let e = &c.extent;
                cv.clear_rect(ox + e.left, oy + e.top, e.width, e.height, view.hole_veil);
                if view.hole_shades.contains(&c.tag) {
                    let reach = (e.width as f32 * SHADE_REACH) as i32;
                    for x in 0..reach {
                        let left = 1.0 - x as f32 / reach as f32;
                        let dark = [0, 0, 0, (SHADE_DARKEST * left) as u8];
                        cv.fill_rect(ox + e.left + x, oy + e.top, 1, e.height, dark);
                    }
                }
            } else if c.kind == LIST {
                self.draw_list(cv, view, c, (ox, oy));
            } else {
                let e = &c.extent;
                self.draw_plain(cv, view, c, [ox + e.left, oy + e.top, e.width, e.height]);
            }
        }
        for m in &view.marks {
            if let Some(img) = self.texture_shared(&m.picture) {
                let shown = [
                    0,
                    0,
                    (img.width as f32 * m.part[0]) as i32,
                    (img.height as f32 * m.part[1]) as i32,
                ];
                if let Some(angle) = m.angle {
                    cv.draw_turned(&img, m.at, m.size.1 as f32, angle, m.tint);
                    continue;
                }
                let (w, h) = m.size;
                let at = [m.at.0 as i32 - w / 2, m.at.1 as i32 - h / 2, w, h];
                cv.draw_image(&img, shown, at, m.tint, 1.0, Default::default());
            }
        }
        if !view.message.is_empty() {
            let w = SCREEN.0 as i32;
            if self.glowing() {
                crate::glow::boxed(cv, [0, 446, w, 24]);
            } else {
                cv.fill_rect(0, 446, w + 1, 24, [0, 0, 0, 200]);
            }
            let text = view.message.clone();
            self.draw_text(cv, "fnt_d16x16", &text, [0, 446, w, 24], 18, LIT);
        }
        Some(())
    }

    fn draw_plain(&mut self, cv: &mut Canvas, view: &MenuView, c: &Control, r: [i32; 4]) {
        let patch = view.patches.get(&c.tag).cloned().unwrap_or_default();
        let lit = match &view.hover {
            Some(Hit::Button(t)) | Some(Hit::Slider(t, _)) => t == &c.tag,
            _ => false,
        };
        if let Some((at, color)) = patch.cover {
            cv.fill_rect(
                at[0],
                at[1],
                at[2],
                at[3],
                [color[0], color[1], color[2], 255],
            );
            return;
        }
        // A screen is stretched across a wide window, and a portrait is square. It stays in the middle of its place.
        let fill = patch.fill.as_deref().unwrap_or(&c.border.fill);
        let r = if fill.to_ascii_lowercase().starts_with("po_") {
            let wide = ((r[2] as f32 * cv.scale / cv.scale_x).round() as i32).min(r[2]);
            [r[0] + (r[2] - wide) / 2, r[1], wide, r[3]]
        } else {
            r
        };
        if c.kind == SLIDER {
            return self.draw_slider(cv, c, r, &patch, lit);
        }
        if c.kind == CHECKBOX {
            let ticked = patch.value.is_some_and(|v| v.0 != 0);
            let picture = c.selected.as_ref().filter(|_| ticked).unwrap_or(&c.border);
            // A square one with no words is a frame round what lies under it, as on the party screen.
            let side = r[3] * 3 / 4;
            let tick = if r[2] == r[3] {
                r
            } else {
                [r[0], r[1] + (r[3] - side) / 2, side, side]
            };
            self.draw_border(cv, picture, tick, None);
            if let (true, Some(h)) = (lit, &c.hilight) {
                self.draw_border(cv, h, tick, None);
            }
            let Some(t) = &c.text else { return };
            let text = patch.text.clone().unwrap_or_else(|| match t.strref {
                Some(s) if t.text.is_empty() => self.strref(s),
                _ => t.text.clone(),
            });
            let color = patch.color.unwrap_or(if lit { LIT } else { t.color });
            let words = [r[0] + side + 6, r[1], r[2] - side - 6, r[3]];
            self.draw_text(cv, &t.font, &text, words, 17, color);
            return;
        }
        if let Some(line) = patch.line {
            thin_line(cv, r, line);
        } else if !patch.plain {
            self.draw_border(cv, &c.border, r, patch.fill.as_deref());
        }
        if patch.greyed {
            cv.fill_rect(r[0], r[1], r[2], r[3], crate::screens::GREYED);
        }
        if let (true, Some(h), None) = (lit, &c.hilight, patch.line) {
            self.draw_border(cv, h, r, None);
        }
        let Some(t) = &c.text else { return };
        let text = patch.text.clone().unwrap_or_else(|| match t.strref {
            Some(s) if t.text.is_empty() => self.strref(s),
            _ => t.text.clone(),
        });
        if text.is_empty() {
            return;
        }
        let color = match (lit, patch.color) {
            (_, Some(c)) => c,
            (true, None) => LIT,
            (false, None) => t.color,
        };
        let font = patch.font.as_deref().unwrap_or(&t.font);
        let words = [r[0] + patch.inset, r[1], r[2] - patch.inset * 2, r[3]];
        self.draw_text(cv, font, &text, words, t.alignment, color);
    }

    /// Where a needle goes beside a tall scale, as `docs/MENU_REFERENCE.md` has it: small, along the scale's own picture from
    /// its top to its foot, with its point against the picture's side. The side curves, so that is looked up row by row.
    /// The layout is whole units, so it goes a little into the picture and not a little short of it.
    fn needle_place(&mut self, c: &Control, r: [i32; 4], img: &Image, along: f32) -> [i32; 4] {
        let w = NEEDLE_WIDTH;
        let h = w * img.height as i32 / img.width.max(1) as i32;
        let solid =
            |i: &Image, x: usize, y: usize| i.rgba[(y * i.width as usize + x) * 4 + 3] > SOLID;
        let (iw, ih) = (img.width as usize, img.height as usize);
        let point = (0..iw)
            .find(|x| (0..ih).any(|y| solid(img, *x, y)))
            .unwrap_or(0);
        let point = point as f32 * w as f32 / iw.max(1) as f32;
        let beside = |y: f32| [r[0] + r[2] / 2 + 4, y as i32 - h / 2, w, h];
        let Some(bar) = self.texture_shared(&c.border.fill) else {
            return beside(r[1] as f32 + r[3] as f32 * along);
        };
        let (bw, bh) = (bar.width as usize, bar.height as usize);
        let right = |y: usize| (0..bw).rev().find(|x| solid(&bar, *x, y));
        let rows: Vec<usize> = (0..bh).filter(|y| right(*y).is_some()).collect();
        let (Some(&first), Some(&last)) = (rows.first(), rows.last()) else {
            return beside(r[1] as f32 + r[3] as f32 * along);
        };
        // A little in from its ends, where the picture's corners would leave the point against nothing.
        let (first, last) = (
            first + NEEDLE_INSET,
            last.saturating_sub(NEEDLE_INSET).max(first + NEEDLE_INSET),
        );
        let row = first + ((last - first) as f32 * along) as usize;
        let row = *rows
            .iter()
            .min_by_key(|y| y.abs_diff(row))
            .unwrap_or(&first);
        let side = right(row).map_or(0, |x| x + 1) as f32 * r[2] as f32 / bw as f32;
        let y = r[1] as f32 + row as f32 * r[3] as f32 / bh as f32;
        [
            (r[0] as f32 + side - point).floor() as i32,
            y as i32 - h / 2,
            w,
            h,
        ]
    }

    /// Where the needle of a tall slider of this screen is at this value.
    pub fn menu_needle(&mut self, gui: &str, tag: &str, value: (i32, i32)) -> Option<[i32; 4]> {
        let g = self.gui(gui)?;
        let c = g.find(tag)?.clone();
        let (ox, oy) = self.panel_origin(&g.panel);
        let e = &c.extent;
        let r = [ox + e.left, oy + e.top, e.width, e.height];
        let img = self.texture_shared(c.thumb.as_deref()?)?;
        let along = value.0.clamp(0, value.1.max(1)) as f32 / value.1.max(1) as f32;
        Some(self.needle_place(&c, r, &img, along))
    }

    /// The track, and the handle along it: across, or down from the top when the slider is taller than wide.
    fn draw_slider(&mut self, cv: &mut Canvas, c: &Control, r: [i32; 4], patch: &Patch, lit: bool) {
        let track = c.hilight.as_ref().filter(|_| lit).unwrap_or(&c.border);
        self.draw_border(cv, track, r, None);
        let (cur, max) = patch.value.unwrap_or((c.cur_value, c.max_value));
        let Some(img) = c.thumb.as_deref().and_then(|n| self.texture_shared(n)) else {
            return;
        };
        let along = cur.clamp(0, max.max(1)) as f32 / max.max(1) as f32;
        let full = [0, 0, img.width as i32, img.height as i32];
        let dst = if r[3] > r[2] {
            self.needle_place(c, r, &img, along)
        } else {
            let w = r[3] * img.width as i32 / img.height.max(1) as i32;
            [r[0] + ((r[2] - w) as f32 * along) as i32, r[1], w, r[3]]
        };
        cv.draw_image(&img, full, dst, [1.0; 3], 1.0, Default::default());
    }

    fn draw_list(&mut self, cv: &mut Canvas, view: &MenuView, list: &Control, at: (i32, i32)) {
        let Some(proto) = list.proto_item.as_deref() else {
            return;
        };
        let Some(t) = proto.text.as_ref() else { return };
        let Some((first_row, step, fits)) = row_layout(list) else {
            return;
        };
        let Some((rows, first)) = view.lists.get(&list.tag) else {
            // A list with text instead of rows: its lines, as many as fit.
            if let Some(patch) = view.patches.get(&list.tag).filter(|p| p.text.is_some()) {
                let text = patch.text.clone().unwrap_or_default();
                let e = &list.extent;
                let box_ = [at.0 + e.left, at.1 + e.top, e.width, e.height];
                self.draw_border(cv, &list.border, box_, None);
                // The Messages screen's words are a text box.
                if self.glowing() && patch.from_end.is_some() {
                    crate::glow::boxed(cv, box_);
                }
                let font = patch.font.as_deref().unwrap_or(&t.font);
                let Some(f) = self.font_shared(font) else {
                    return;
                };
                let [x, y, w, _] = first_row;
                let y = y - patch.rise;
                let room = list.extent.top + list.extent.height - y;
                let n = (room / f.line_height.max(1)).max(0) as usize;
                let lines = f.wrap(&text, w);
                let skip = patch
                    .from_end
                    .map_or(0, |back| lines.len().saturating_sub(n + back));
                for (i, l) in lines.iter().skip(skip).take(n).enumerate() {
                    let top = at.1 + y + i as i32 * f.line_height;
                    f.draw_line(cv, at.0 + x, top, l, t.color);
                }
                let bar = list.children.iter().find(|k| k.kind == SCROLLBAR);
                if let Some(bar) = bar.filter(|_| patch.from_end.is_some() && lines.len() > n) {
                    let e = &bar.extent;
                    let r = [at.0 + e.left, at.1 + e.top, e.width, e.height];
                    self.draw_border(cv, &bar.border, r, None);
                    let h = (r[3] * n as i32 / lines.len() as i32).max(8);
                    let y = r[1] + (r[3] - h) * skip as i32 / (lines.len() - n) as i32;
                    cv.fill_rect(r[0] + 3, y + 3, r[2] - 6, h - 6, [0, 166, 247, 170]);
                }
            }
            return;
        };
        for (i, row) in rows.iter().enumerate().skip(*first).take(fits) {
            let y = at.1 + first_row[1] + (i - first) as i32 * step;
            let r = [at.0 + first_row[0], y, first_row[2], first_row[3]];
            if !row.cells.is_empty() {
                self.draw_cells(cv, view, &list.tag, i, &row.cells, r);
                continue;
            }
            let lit = matches!(&view.hover, Some(Hit::Row(l, n)) if l == &list.tag && *n == i);
            if proto.kind == CHECKBOX {
                let ticked = proto.selected.as_ref().filter(|_| row.selected);
                let side = r[3] * 3 / 4;
                let tick = [r[0], r[1] + (r[3] - side) / 2, side, side];
                self.draw_border(cv, ticked.unwrap_or(&proto.border), tick, None);
                let words = [r[0] + side + 6, r[1], r[2] - side - 6, r[3]];
                let color = if lit { LIT } else { t.color };
                self.draw_text(cv, &t.font, &row.text, words, 17, color);
                continue;
            }
            self.draw_border(cv, &proto.border, r, None);
            if row.bar {
                cv.fill_rect(r[0], r[1] + 1, r[2], r[3] - 2, CHOSEN_BAR);
            }
            if let (true, Some(h)) = (lit || row.selected, &proto.hilight) {
                self.draw_border(cv, h, r, None);
            }
            let color = if row.dim {
                DIM
            } else if lit || row.selected {
                LIT
            } else {
                t.color
            };
            let (text_rect, alignment) = match &row.icon {
                Some(icon) => {
                    let side = r[3] - 4;
                    if let Some(img) = self.texture_shared(icon) {
                        let full = [0, 0, img.width as i32, img.height as i32];
                        // A screen is stretched across a wide window, and these pictures are square.
                        let wide = ((side as f32 * cv.scale / cv.scale_x).round() as i32).min(side);
                        let dst = [r[0] + 2 + (side - wide) / 2, r[1] + 2, wide, side];
                        cv.draw_image(&img, full, dst, [1.0; 3], 1.0, Default::default());
                    }
                    if row.count > 1 {
                        let n = row.count.to_string();
                        let spot = [r[0] + 2, r[1] + 2, side - 2, side];
                        self.draw_text(cv, "fnt_d16x16", &n, spot, 36, [1.0; 3]);
                    }
                    ([r[0] + side + 6, r[1], r[2] - side - 8, r[3]], 17)
                }
                // Words from the left keep off the row's border.
                None if t.alignment & 1 != 0 => ([r[0] + 8, r[1], r[2] - 8, r[3]], t.alignment),
                None => (r, t.alignment),
            };
            let font = row.font.as_deref().unwrap_or(&t.font);
            let mut text_rect = text_rect;
            if let Some(chosen) = row.stepper {
                text_rect[2] -= STEP_WIDTH;
                self.draw_stepper(cv, view, (&list.tag, i), (chosen, row.count), r);
            }
            self.draw_text(cv, font, &row.text, text_rect, alignment, color);
        }
        if rows.len() > fits {
            if let Some(bar) = list.children.iter().find(|k| k.kind == SCROLLBAR) {
                let e = &bar.extent;
                let r = [at.0 + e.left, at.1 + e.top, e.width, e.height];
                self.draw_border(cv, &bar.border, r, None);
                let h = (r[3] * fits as i32 / rows.len() as i32).max(8);
                let y = r[1] + (r[3] - h) * *first as i32 / (rows.len() - fits) as i32;
                cv.fill_rect(r[0] + 3, y + 3, r[2] - 6, h - 6, [0, 166, 247, 170]);
            }
        }
    }

    /// `< 1 >` at the right of a row: `amount` is how many are chosen, and of how many.
    fn draw_stepper(
        &mut self,
        cv: &mut Canvas,
        view: &MenuView,
        (tag, row): (&str, usize),
        (chosen, most): (i32, i32),
        r: [i32; 4],
    ) {
        let [less, number, more] = stepper_rects(r);
        // A screen is stretched across a wide window, and these pictures are square.
        let across = cv.scale / cv.scale_x;
        let arrows = [
            (less, "lbl_leftplus", -1, chosen > 1),
            (more, "lbl_rightminus", 1, chosen < most),
        ];
        for (at, picture, by, can) in arrows {
            let lit = matches!(&view.hover, Some(Hit::Step(l, a, b))
                if l == tag && *a == row && *b == by);
            let tint = match (can, lit) {
                (false, _) => [0.3; 3],
                (true, true) => LIT,
                (true, false) => [0.0, 0.65, 0.97],
            };
            if let Some(img) = self.texture_shared(picture) {
                let full = [0, 0, img.width as i32, img.height as i32];
                let side = ((at[3] as f32 * across).round() as i32).min(at[2]);
                let dst = [at[0] + (at[2] - side) / 2, at[1], side, at[3]];
                cv.draw_image(&img, full, dst, tint, 1.0, Default::default());
            }
        }
        self.draw_text(cv, "fnt_d16x16", &chosen.to_string(), number, 18, LIT);
    }

    /// A row's pictures with an arrow from each to the next, as the reference video has the feats at 1:23:40.
    fn draw_cells(
        &mut self,
        cv: &mut Canvas,
        view: &MenuView,
        tag: &str,
        row: usize,
        cells: &[Cell],
        r: [i32; 4],
    ) {
        let rects = cell_rects(r);
        // A screen is stretched across a wide window, and these pictures are square.
        let across = cv.scale / cv.scale_x;
        for (n, cell) in cells.iter().enumerate().take(rects.len()) {
            let [x, y, w, h] = rects[n];
            let side = ((h as f32 * across).round() as i32).min(w);
            let at = [x + (w - side) / 2, y, side, h];
            let tint = if cell.dim { [0.3; 3] } else { [1.0; 3] };
            if let Some(img) = self.texture_shared(&cell.icon) {
                let full = [0, 0, img.width as i32, img.height as i32];
                cv.draw_image(&img, full, at, tint, 1.0, Default::default());
            }
            let lit = matches!(&view.hover, Some(Hit::Cell(l, a, b))
                if l == tag && *a == row && *b == n);
            let frame = if lit || cell.selected {
                Some([255, 255, 76, 255])
            } else if cell.owned {
                Some([0, 220, 70, 255])
            } else {
                None
            };
            if let Some(c) = frame {
                let [x, y, w, h] = at;
                cv.fill_rect(x, y, w, 1, c);
                cv.fill_rect(x, y + h - 1, w, 1, c);
                cv.fill_rect(x, y, 1, h, c);
                cv.fill_rect(x + w - 1, y, 1, h, c);
            }
            if n + 1 < cells.len() {
                if let Some(img) = self.texture_shared("lbl_rightminus") {
                    let full = [0, 0, img.width as i32, img.height as i32];
                    let high = 14;
                    let side = (high as f32 * across).round() as i32;
                    let x = (rects[n][0] + rects[n][2] + rects[n + 1][0] - side) / 2;
                    let arrow = [x, at[1] + (at[3] - high) / 2, side, high];
                    cv.draw_image(
                        &img,
                        full,
                        arrow,
                        [0.0, 0.65, 0.97],
                        1.0,
                        Default::default(),
                    );
                }
            }
        }
    }

    /// What is at a point of the layout: the row, else the button whose centre is nearest (slots overlap).
    pub fn menu_hit(&mut self, view: &MenuView, x: f32, y: f32) -> Option<Hit> {
        self.measure_for_menus();
        if let Some(n) = &view.notice {
            let (_, b, _, _) = self.notice_layout(n)?;
            let on = x >= b[0] as f32
                && x < (b[0] + b[2]) as f32
                && y >= b[1] as f32
                && y < (b[1] + b[3]) as f32;
            return on.then(|| Hit::Button(NOTICE_BUTTON.to_string()));
        }
        if let Some(d) = &view.dialog {
            return self.menu_hit(d, x, y);
        }
        if let Some(hit) = view.top.as_ref().and_then(|t| self.menu_hit(t, x, y)) {
            return Some(hit);
        }
        let gui = self.view_gui(view)?;
        let (ox, oy) = self.panel_origin(&gui.panel);
        let (x, y) = (x - ox as f32, y - oy as f32);
        let inside = |r: [i32; 4]| {
            x >= r[0] as f32
                && x < (r[0] + r[2]) as f32
                && y >= r[1] as f32
                && y < (r[1] + r[3]) as f32
        };
        for c in &gui.panel.children {
            let Some((rows, first)) = view.lists.get(&c.tag) else {
                continue;
            };
            let Some((r0, step, fits)) = row_layout(c) else {
                continue;
            };
            for (i, row) in rows.iter().enumerate().skip(*first).take(fits) {
                let top = r0[1] + (i - first) as i32 * step;
                let r = [r0[0], top, r0[2], r0[3]];
                if inside(r) {
                    if row.stepper.is_some() {
                        let [less, _, more] = stepper_rects(r);
                        if inside(less) {
                            return Some(Hit::Step(c.tag.clone(), i, -1));
                        }
                        if inside(more) {
                            return Some(Hit::Step(c.tag.clone(), i, 1));
                        }
                    }
                    if row.cells.is_empty() {
                        return Some(Hit::Row(c.tag.clone(), i));
                    }
                    let n = cell_rects(r).iter().position(|a| inside(*a))?;
                    return (n < row.cells.len()).then(|| Hit::Cell(c.tag.clone(), i, n));
                }
            }
        }
        gui.panel
            .children
            .iter()
            .filter(|c| {
                let live = view.patches.get(&c.tag).is_some_and(|p| p.live);
                (live || matches!(c.kind, 5 | 6 | CHECKBOX | SLIDER)) && !view.is_hidden(&c.tag)
            })
            .filter(|c| {
                let e = &c.extent;
                let moved = view.patches.get(&c.tag).and_then(|p| p.place);
                inside(moved.unwrap_or([e.left, e.top, e.width, e.height]))
            })
            .map(|c| {
                let e = &c.extent;
                let dx = x - (e.left + e.width / 2) as f32;
                let dy = y - (e.top + e.height / 2) as f32;
                let hit = if c.kind == SLIDER {
                    let along = (x - e.left as f32) / e.width.max(1) as f32;
                    Hit::Slider(c.tag.clone(), along.clamp(0.0, 1.0))
                } else {
                    Hit::Button(c.tag.clone())
                };
                (dx * dx + dy * dy, hit)
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, hit)| hit)
    }

    /// The layout point in the middle of an arrow of a row's stepper.
    pub fn menu_step_point(
        &mut self,
        view: &MenuView,
        tag: &str,
        row: usize,
        by: i32,
    ) -> Option<(f32, f32)> {
        let gui = self.view_gui(view)?;
        let (ox, oy) = self.panel_origin(&gui.panel);
        let c = gui.find(tag)?;
        let first = view.lists.get(tag).map_or(0, |l| l.1);
        let (r0, step, _) = row_layout(c)?;
        let top = r0[1] + row.checked_sub(first)? as i32 * step;
        let [less, _, more] = stepper_rects([r0[0], top, r0[2], r0[3]]);
        let at = if by < 0 { less } else { more };
        Some((
            (ox + at[0]) as f32 + at[2] as f32 / 2.0,
            (oy + at[1]) as f32 + at[3] as f32 / 2.0,
        ))
    }

    /// The layout point in the middle of a picture of a list's row.
    pub fn menu_cell_point(
        &mut self,
        view: &MenuView,
        tag: &str,
        row: usize,
        cell: usize,
    ) -> Option<(f32, f32)> {
        let gui = self.view_gui(view)?;
        let (ox, oy) = self.panel_origin(&gui.panel);
        let c = gui.find(tag)?;
        let first = view.lists.get(tag).map_or(0, |l| l.1);
        let (r0, step, _) = row_layout(c)?;
        let top = r0[1] + row.checked_sub(first)? as i32 * step;
        let at = cell_rects([r0[0], top, r0[2], r0[3]]).get(cell).copied()?;
        Some((
            (ox + at[0]) as f32 + at[2] as f32 / 2.0,
            (oy + at[1]) as f32 + at[3] as f32 / 2.0,
        ))
    }

    /// The layout point in the middle of a button, or of a row with `row`.
    pub fn menu_point(
        &mut self,
        view: &MenuView,
        tag: &str,
        row: Option<usize>,
    ) -> Option<(f32, f32)> {
        if let Some(n) = &view.notice {
            if tag != NOTICE_BUTTON {
                return None;
            }
            let (_, b, _, _) = self.notice_layout(n)?;
            return Some((
                b[0] as f32 + b[2] as f32 / 2.0,
                b[1] as f32 + b[3] as f32 / 2.0,
            ));
        }
        if let Some(d) = &view.dialog {
            return self.menu_point(d, tag, row);
        }
        let gui = self.view_gui(view)?;
        let (ox, oy) = self.panel_origin(&gui.panel);
        let Some(c) = gui.find(tag) else {
            return self.menu_point(view.top.as_ref()?, tag, row);
        };
        if view.is_hidden(tag) {
            return None;
        }
        let r = match row {
            Some(i) => {
                let first = view.lists.get(tag).map_or(0, |l| l.1);
                let (r0, step, _) = row_layout(c)?;
                [
                    r0[0],
                    r0[1] + i.checked_sub(first)? as i32 * step,
                    r0[2],
                    r0[3],
                ]
            }
            None => {
                let e = &c.extent;
                [e.left, e.top, e.width, e.height]
            }
        };
        Some((
            (ox + r[0]) as f32 + r[2] as f32 / 2.0,
            (oy + r[1]) as f32 + r[3] as f32 / 2.0,
        ))
    }

    /// How many lines some words make in a list, and how many fit on screen.
    pub fn menu_text_lines(&mut self, gui: &str, tag: &str, text: &str) -> (usize, usize) {
        self.measure_for_menus();
        let Some(list) = self.gui(gui).and_then(|g| g.find(tag).cloned()) else {
            return (0, 1);
        };
        let proto = list.proto_item.as_deref();
        let font = proto.and_then(|p| p.text.as_ref()).map(|t| t.font.clone());
        let (Some(layout), Some(font)) = (row_layout(&list), font) else {
            return (0, 1);
        };
        let Some(f) = self.font(&font) else {
            return (0, 1);
        };
        let room = list.extent.top + list.extent.height - layout.0[1];
        (
            f.wrap(text, layout.0[2]).len(),
            (room / f.line_height.max(1)).max(1) as usize,
        )
    }

    /// How many rows of a list fit on screen.
    pub fn menu_rows_shown(&mut self, gui: &str, tag: &str) -> usize {
        self.gui(gui)
            .and_then(|g| row_layout(g.find(tag)?))
            .map_or(1, |l| l.2)
    }
}
