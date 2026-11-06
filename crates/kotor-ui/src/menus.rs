//! Menus drawn from stock GUI files: the main menu, the pause menu, the save screens, inventory and equipment. A screen
//! says what it wants changed in a [`MenuView`], and the same view answers what is under the cursor.
//!
//! **Assumptions, not compared with the game:** the 640x480 layout stays centred on a wider screen with black at the
//! sides, except controls a view names to stretch; controls are drawn in file order, so later ones lie on top (the
//! opposite of the HUD's order: here the full-screen `LBL_3DVIEW` comes before the logo and buttons); list rows follow
//! the file's `PROTOITEM`, whose extent is relative to the panel; a button under the cursor has yellow text.

use crate::canvas::Canvas;
use crate::gui::Control;
use crate::screens::{Patch, Ui, SCREEN};
use kotor_image::Image;
use std::collections::BTreeMap;

const LIT: [f32; 3] = [1.0, 1.0, 0.3];
const DIM: [f32; 3] = [0.45, 0.5, 0.55];
/// `PROTOITEM` and `SCROLLBAR` are children; these two kinds are drawn with their list.
const LIST: i32 = 11;
const SCROLLBAR: i32 = 9;

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
}

/// What is under the cursor.
#[derive(Clone, Debug, PartialEq)]
pub enum Hit {
    Button(String),
    Row(String, usize),
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
    /// Controls drawn across the whole width of a wide screen (`TGuiPanel` is the panel's own picture).
    pub stretch: Vec<String>,
    /// A dialog over this screen. It takes all the input.
    pub dialog: Option<Box<MenuView>>,
    /// The scene shows around the panel, instead of black.
    pub over_scene: bool,
}

impl MenuView {
    pub fn new(gui: &str) -> MenuView {
        MenuView {
            gui: gui.to_string(),
            ..MenuView::default()
        }
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

    /// Draw this control's text in another font.
    pub fn font(&mut self, tag: &str, font: &str) {
        self.patches.entry(tag.to_string()).or_default().font = Some(font.to_string());
    }

    /// Grey these buttons: they do nothing yet.
    pub fn inert(&mut self, tags: &[&str]) {
        for t in tags {
            self.patches.entry(t.to_string()).or_default().color = Some(DIM);
        }
    }

    fn is_hidden(&self, tag: &str) -> bool {
        self.patches.get(tag).is_some_and(|p| p.hidden)
    }
}

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
        let extra = (self.width.saturating_sub(SCREEN.0) / 2) as i32;
        (extra + panel.extent.left, panel.extent.top)
    }

    /// A screen on black, or with `over` left showing around a dialog.
    pub fn menu_overlay(&mut self, view: &MenuView) -> Option<Image> {
        let mut cv = Canvas::scaled_layout(self.pixels, self.scale);
        cv.clear(if view.over_scene {
            [0, 0, 0, 0]
        } else {
            [0, 0, 0, 255]
        });
        self.draw_view(&mut cv, view)?;
        if let Some(d) = &view.dialog {
            cv.fill_rect(0, 0, self.width as i32 + 1, 481, [0, 0, 0, 140]);
            self.draw_view(&mut cv, d)?;
        }
        Some(cv.into_image())
    }

    fn draw_view(&mut self, cv: &mut Canvas, view: &MenuView) -> Option<()> {
        let gui = self.gui(&view.gui)?;
        let p = &gui.panel;
        let (ox, oy) = self.panel_origin(p);
        let whole = [0, 0, self.width as i32, SCREEN.1 as i32];
        let r = if view.stretch.iter().any(|t| t == &p.tag) {
            whole
        } else {
            [ox, oy, p.extent.width, p.extent.height]
        };
        // **Assumed:** a panel over the scene gets a dark backing, since its picture is see-through and text must read.
        if view.over_scene {
            cv.fill_rect(r[0], r[1], r[2], r[3], [0, 0, 0, 190]);
        }
        self.draw_border(cv, &p.border, r, None);
        for c in &p.children {
            if view.is_hidden(&c.tag) || c.kind == SCROLLBAR {
                continue;
            }
            let rect = if view.stretch.iter().any(|t| t == &c.tag) {
                whole
            } else {
                let e = &c.extent;
                [ox + e.left, oy + e.top, e.width, e.height]
            };
            if c.kind == LIST {
                self.draw_list(cv, view, c, (ox, oy));
            } else {
                self.draw_plain(cv, view, c, rect);
            }
        }
        if !view.message.is_empty() {
            let w = self.width as i32;
            cv.fill_rect(0, 446, w + 1, 24, [0, 0, 0, 200]);
            let text = view.message.clone();
            self.draw_text(cv, "fnt_d16x16", &text, [0, 446, w, 24], 18, LIT);
        }
        Some(())
    }

    fn draw_plain(&mut self, cv: &mut Canvas, view: &MenuView, c: &Control, r: [i32; 4]) {
        let patch = view.patches.get(&c.tag).cloned().unwrap_or_default();
        let lit = matches!(&view.hover, Some(Hit::Button(t)) if t == &c.tag);
        self.draw_border(cv, &c.border, r, patch.fill.as_deref());
        if let (true, Some(h)) = (lit, &c.hilight) {
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
        self.draw_text(cv, font, &text, r, t.alignment, color);
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
                let font = patch.font.as_deref().unwrap_or(&t.font);
                let Some(f) = self.font(font).cloned() else {
                    return;
                };
                let [x, y, w, _] = first_row;
                let room = list.extent.top + list.extent.height - y;
                let n = (room / f.line_height.max(1)).max(0) as usize;
                for (i, l) in f.wrap(&text, w).iter().take(n).enumerate() {
                    let top = at.1 + y + i as i32 * f.line_height;
                    f.draw_line(cv, at.0 + x, top, l, t.color);
                }
            }
            return;
        };
        for (i, row) in rows.iter().enumerate().skip(*first).take(fits) {
            let y = at.1 + first_row[1] + (i - first) as i32 * step;
            let r = [at.0 + first_row[0], y, first_row[2], first_row[3]];
            let lit = matches!(&view.hover, Some(Hit::Row(l, n)) if l == &list.tag && *n == i);
            self.draw_border(cv, &proto.border, r, None);
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
                    if let Some(img) = self.texture(icon).cloned() {
                        let full = [0, 0, img.width as i32, img.height as i32];
                        let dst = [r[0] + 2, r[1] + 2, side, side];
                        cv.draw_image(&img, full, dst, [1.0; 3], 1.0, Default::default());
                    }
                    if row.count > 1 {
                        let n = row.count.to_string();
                        let spot = [r[0] + 2, r[1] + 2, side - 2, side];
                        self.draw_text(cv, "fnt_d16x16", &n, spot, 36, [1.0; 3]);
                    }
                    ([r[0] + side + 6, r[1], r[2] - side - 8, r[3]], 17)
                }
                None => (r, t.alignment),
            };
            let font = row.font.as_deref().unwrap_or(&t.font);
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

    /// What is at a point of the layout: the row, else the button whose centre is nearest (slots overlap).
    pub fn menu_hit(&mut self, view: &MenuView, x: f32, y: f32) -> Option<Hit> {
        if let Some(d) = &view.dialog {
            return self.menu_hit(d, x, y);
        }
        let gui = self.gui(&view.gui)?;
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
            for i in *first..rows.len().min(first + fits) {
                let top = r0[1] + (i - first) as i32 * step;
                if inside([r0[0], top, r0[2], r0[3]]) {
                    return Some(Hit::Row(c.tag.clone(), i));
                }
            }
        }
        gui.panel
            .children
            .iter()
            .filter(|c| matches!(c.kind, 5 | 6) && !view.is_hidden(&c.tag))
            .filter(|c| {
                let e = &c.extent;
                inside([e.left, e.top, e.width, e.height])
            })
            .map(|c| {
                let e = &c.extent;
                let dx = x - (e.left + e.width / 2) as f32;
                let dy = y - (e.top + e.height / 2) as f32;
                (dx * dx + dy * dy, c.tag.clone())
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, tag)| Hit::Button(tag))
    }

    /// The layout point in the middle of a button, or of a row with `row`.
    pub fn menu_point(
        &mut self,
        view: &MenuView,
        tag: &str,
        row: Option<usize>,
    ) -> Option<(f32, f32)> {
        if let Some(d) = &view.dialog {
            return self.menu_point(d, tag, row);
        }
        let gui = self.gui(&view.gui)?;
        let (ox, oy) = self.panel_origin(&gui.panel);
        let c = gui.find(tag)?;
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

    /// How many rows of a list fit on screen.
    pub fn menu_rows_shown(&mut self, gui: &str, tag: &str) -> usize {
        self.gui(gui)
            .and_then(|g| row_layout(g.find(tag)?))
            .map_or(1, |l| l.2)
    }
}
