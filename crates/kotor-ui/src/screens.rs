//! Laying out and drawing GUI files onto a canvas: the dialogue box and the main HUD.
//!
//! **Assumptions, not measured from the game:** edge and corner textures of a border are stretched along their
//! side (top edge as stored, then turned a quarter per side clockwise) rather than tiled; the selected reply is
//! drawn yellow and unselected replies in the list's own colour; a dark translucent backing is drawn behind the
//! dialogue panel so text reads over any scene; the speaker's name is drawn in yellow above the panel.

use crate::canvas::{Canvas, Orient};
use crate::font::Font;
use crate::gui::{Border, Control, Gui};
use kotor_archive::Install;
use kotor_image::Image;
use std::collections::HashMap;

/// The layout space of every GUI file.
pub const SCREEN: (u32, u32) = (640, 480);
/// From this many pixels a unit the stock pictures would be enlarged at least twice, and upscaled ones are worth using.
const HD_FROM_SCALE: f32 = 2.0;

/// Caches for textures, fonts and GUIs while drawing.
pub struct Ui<'a> {
    pub(crate) inst: &'a mut Install,
    /// Width of the layout space: 640, or more on a wider screen (the height stays 480).
    pub(crate) width: u32,
    /// Pixels per layout unit, and the size of the picture in pixels.
    pub(crate) scale: f32,
    pub(crate) pixels: (u32, u32),
    /// A folder of upscaled pictures (see `docs/PLAY.md`), used once the interface is drawn at twice the file's size.
    hd_dir: Option<std::path::PathBuf>,
    /// What the left mouse button would do to the target, shown under its name.
    action: String,
    /// The icon in the first action slot while there is a target.
    action_icon: String,
    textures: HashMap<String, Option<Image>>,
    fonts: HashMap<String, Option<Font>>,
    /// The talk table, read the first time a control needs it.
    tlk: Option<Option<kotor_tables::Tlk>>,
}

/// Per-control changes a screen applies on top of the file: replacement fill, text, bar value, or hiding.
#[derive(Clone, Debug, Default)]
pub struct Patch {
    pub fill: Option<String>,
    pub text: Option<String>,
    pub value: Option<(i32, i32)>,
    pub hidden: bool,
    /// Replacement colour and font for the control's text.
    pub color: Option<[f32; 3]>,
    pub font: Option<String>,
}

/// What the dialogue box shows.
#[derive(Clone, Debug, Default)]
pub struct DialogueView {
    pub speaker: String,
    /// The line being spoken (shown when `replies` is empty).
    pub subtitle: String,
    pub replies: Vec<String>,
    pub selected: usize,
}

/// One party member on the HUD.
#[derive(Clone, Debug, Default)]
pub struct HudMember {
    /// Portrait texture name (from `portraits.2da`, for example `po_pcarth`).
    pub portrait: String,
    pub hp: i32,
    pub max_hp: i32,
    pub force: i32,
    pub max_force: i32,
}

impl<'a> Ui<'a> {
    pub fn new(inst: &'a mut Install) -> Ui<'a> {
        let _ = inst.load_texture_packs();
        Ui {
            inst,
            width: SCREEN.0,
            scale: 1.0,
            pixels: SCREEN,
            hd_dir: None,
            action: String::new(),
            action_icon: String::new(),
            textures: HashMap::new(),
            fonts: HashMap::new(),
            tlk: None,
        }
    }

    /// Lay out for a screen `width` units wide and 480 high. Corners stay at the corners (see `anchored`).
    pub fn set_width(&mut self, width: u32) {
        self.width = width.max(SCREEN.0);
        self.pixels = (self.width, SCREEN.1);
    }

    /// Draw for a window of `pixels`: 480 layout units high and as wide as that makes it, pixel for pixel.
    pub fn set_view(&mut self, pixels: (u32, u32)) {
        let was = self.scale >= HD_FROM_SCALE;
        self.scale = pixels.1.max(1) as f32 / SCREEN.1 as f32;
        self.width = ((pixels.0 as f32 / self.scale).round() as u32).max(SCREEN.0);
        self.pixels = pixels;
        if was != (self.scale >= HD_FROM_SCALE) && self.hd_dir.is_some() {
            self.textures.clear();
            self.fonts.clear();
        }
    }

    /// Use upscaled pictures from this folder when the scale calls for them.
    pub fn set_hd_dir(&mut self, dir: Option<std::path::PathBuf>) {
        self.hd_dir = dir;
        self.textures.clear();
        self.fonts.clear();
    }

    /// An upscaled picture: `<name>.png`, with its alpha from `<name>__a.png` when there is one.
    fn hd_picture(&self, key: &str) -> Option<Image> {
        if self.scale < HD_FROM_SCALE {
            return None;
        }
        let dir = self.hd_dir.as_ref()?;
        let read = |n: String| kotor_image::parse_png(&std::fs::read(dir.join(n)).ok()?);
        let mut img = read(format!("{key}.png"))?;
        if let Some(a) = read(format!("{key}__a.png")) {
            if (a.width, a.height) != (img.width, img.height) {
                return None;
            }
            for (p, m) in img.rgba.chunks_mut(4).zip(a.rgba.chunks(4)) {
                p[3] = m[0];
            }
        }
        Some(img)
    }

    /// Every picture the interface draws, by name: for making upscaled copies.
    pub fn picture_names(&mut self) -> Vec<String> {
        let mut names: Vec<String> = ["i_attack", "i_dialog", "i_opendoor", "i_openplace"]
            .iter()
            .chain(["fnt_console", "fnt_dialog16x16", "fnt_d16x16"].iter())
            .map(|s| s.to_string())
            .collect();
        for g in [
            "maininterface",
            "dialog",
            "mainmenu",
            "optionsingame",
            "saveload",
            "savename",
            "container",
            "inventory",
            "equip",
        ] {
            if let Some(g) = self.gui(g) {
                gather_pictures(&g.panel, &mut names);
            }
        }
        if let Some(t) = self
            .inst
            .read("portraits", kotor_core::ResType::TWODA)
            .ok()
            .and_then(|b| kotor_tables::TwoDa::parse(&b).ok())
        {
            names.extend(
                (0..t.row_count()).filter_map(|r| t.get(r, "baseresref").map(str::to_string)),
            );
        }
        names.retain(|n| !n.is_empty());
        names.iter_mut().for_each(|n| *n = n.to_ascii_lowercase());
        names.sort();
        names.dedup();
        names
    }

    /// A picture as the game has it, first row on top, whatever the upscaled folder holds.
    pub fn stock_picture(&mut self, name: &str) -> Option<Image> {
        load_texture(self.inst, &name.to_ascii_lowercase())
    }

    pub fn gui(&mut self, name: &str) -> Option<Gui> {
        Gui::parse(&self.inst.read(name, kotor_core::ResType(2047)).ok()?).ok()
    }

    pub fn texture(&mut self, name: &str) -> Option<&Image> {
        if name.is_empty() {
            return None;
        }
        let key = name.to_ascii_lowercase();
        if !self.textures.contains_key(&key) {
            let img = self
                .hd_picture(&key)
                .or_else(|| load_texture(self.inst, &key));
            self.textures.insert(key.clone(), img);
        }
        self.textures[&key].as_ref()
    }

    pub fn font(&mut self, name: &str) -> Option<&Font> {
        let key = name.to_ascii_lowercase();
        if !self.fonts.contains_key(&key) {
            // **Assumption:** a font whose TXI does not fit its picture is replaced by the dialogue font of the
            // same family, `fnt_dialog16x16`.
            let f =
                Font::load(self.inst, &key).or_else(|| Font::load(self.inst, "fnt_dialog16x16"));
            let f = f.map(|f| {
                let name = f.name.to_ascii_lowercase();
                // A font's letters are white and only their alpha counts. The colours hidden under the clear parts,
                // once upscaled, would fringe the letters.
                match self.hd_picture(&name) {
                    Some(mut big) => {
                        for p in big.rgba.chunks_mut(4) {
                            p[0] = 255;
                            p[1] = 255;
                            p[2] = 255;
                        }
                        f.with_larger_image(big)
                    }
                    None => f,
                }
            });
            self.fonts.insert(key.clone(), f);
        }
        self.fonts[&key].as_ref()
    }

    /// Text from the talk table, `dialog.tlk` beside the game's data files.
    pub fn strref(&mut self, strref: u32) -> String {
        let dir = self.inst.dir().to_path_buf();
        self.tlk
            .get_or_insert_with(|| {
                std::fs::read(dir.join("dialog.tlk"))
                    .ok()
                    .and_then(|b| kotor_tables::Tlk::parse(&b).ok())
            })
            .as_ref()
            .and_then(|t| t.get(strref))
            .unwrap_or_default()
    }

    pub(crate) fn draw_border(
        &mut self,
        cv: &mut Canvas,
        b: &Border,
        r: [i32; 4],
        fill: Option<&str>,
    ) {
        let fill = fill.unwrap_or(&b.fill).to_string();
        if let Some(img) = self.texture(&fill).cloned() {
            let full = [0, 0, img.width as i32, img.height as i32];
            cv.draw_image(&img, full, r, b.color, 1.0, Orient::default());
        }
        let d = b.dimension;
        if d <= 0 {
            return;
        }
        let [x, y, w, h] = r;
        if let Some(img) = self.texture(&b.edge).cloned() {
            let full = [0, 0, img.width as i32, img.height as i32];
            let sides = [
                ([x + d, y, w - 2 * d, d], 0),
                ([x + w - d, y + d, d, h - 2 * d], 1),
                ([x + d, y + h - d, w - 2 * d, d], 2),
                ([x, y + d, d, h - 2 * d], 3),
            ];
            for (dst, turns) in sides {
                let o = Orient {
                    turns,
                    ..Orient::default()
                };
                cv.draw_image(&img, full, dst, b.color, 1.0, o);
            }
        }
        if let Some(img) = self.texture(&b.corner).cloned() {
            let full = [0, 0, img.width as i32, img.height as i32];
            let corners = [
                ([x, y, d, d], 0),
                ([x + w - d, y, d, d], 1),
                ([x + w - d, y + h - d, d, d], 2),
                ([x, y + h - d, d, d], 3),
            ];
            for (dst, turns) in corners {
                let o = Orient {
                    turns,
                    ..Orient::default()
                };
                cv.draw_image(&img, full, dst, b.color, 1.0, o);
            }
        }
    }

    /// Draw wrapped text inside `r`, aligned by the GUI alignment code. Returns the lines' total height.
    pub(crate) fn draw_text(
        &mut self,
        cv: &mut Canvas,
        font: &str,
        text: &str,
        r: [i32; 4],
        alignment: i32,
        color: [f32; 3],
    ) -> i32 {
        let Some(f) = self.font(font).cloned() else {
            return 0;
        };
        let lines = f.wrap(text, r[2]);
        let total = lines.len() as i32 * f.line_height;
        let mut y = match alignment & 0x38 {
            16 => r[1] + (r[3] - total) / 2,
            32 => r[1] + r[3] - total,
            _ => r[1],
        };
        for l in &lines {
            let w = f.measure(l);
            let x = match alignment & 7 {
                2 => r[0] + (r[2] - w) / 2,
                4 => r[0] + r[2] - w,
                _ => r[0],
            };
            f.draw_line(cv, x, y, l, color);
            y += f.line_height;
        }
        total
    }

    /// Draw a control and its children; `origin` is the parent's top-left on the canvas.
    fn draw_control(
        &mut self,
        cv: &mut Canvas,
        c: &Control,
        origin: (i32, i32),
        patches: &HashMap<String, Patch>,
    ) {
        let patch = patches.get(&c.tag).cloned().unwrap_or_default();
        if patch.hidden {
            return;
        }
        let r = [
            origin.0 + c.extent.left,
            origin.1 + c.extent.top,
            c.extent.width,
            c.extent.height,
        ];
        if c.kind == 10 {
            // Progress bar: the bar's own border, then the fill clipped to the value.
            self.draw_border(cv, &c.border, r, None);
            let (cur, max) = patch.value.unwrap_or((c.cur_value, c.max_value));
            if let (Some(fill), true) = (&c.progress_fill, max > 0) {
                let frac = cur.clamp(0, max) as f32 / max as f32;
                // A bar taller than wide fills upward from the bottom (the vitality bars beside the portraits).
                let dst = if r[3] > r[2] {
                    let h = (r[3] as f32 * frac).round() as i32;
                    [r[0], r[1] + r[3] - h, r[2], h]
                } else {
                    let w = (r[2] as f32 * frac).round() as i32;
                    [
                        if c.from_left { r[0] } else { r[0] + r[2] - w },
                        r[1],
                        w,
                        r[3],
                    ]
                };
                if let Some(img) = self.texture(&fill.fill).cloned() {
                    let full = [0, 0, img.width as i32, img.height as i32];
                    cv.draw_image(&img, full, dst, fill.color, 1.0, Orient::default());
                }
            }
            return;
        }
        if c.kind != 11 {
            self.draw_border(cv, &c.border, r, patch.fill.as_deref());
        }
        if let Some(t) = &c.text {
            let text = patch.text.clone().unwrap_or_else(|| match t.strref {
                Some(s) if t.text.is_empty() => self.strref(s),
                _ => t.text.clone(),
            });
            if !text.is_empty() {
                self.draw_text(cv, &t.font, &text, r, t.alignment, t.color);
            }
        }
        // **Assumption:** the first control in the list is the topmost, so draw from the last to the first
        // (the portrait frames sit after the portraits in the file and must be behind them).
        for k in c.children.iter().rev().filter(|k| k.kind != 9) {
            self.draw_control(cv, k, (r[0], r[1]), patches);
        }
    }

    /// The dialogue box: the NPC's line, or the numbered list of replies, over `scene` (or black).
    /// Reads `dialog.gui`. Returns `None` if the GUI or its font is missing.
    pub fn dialogue_box(&mut self, view: &DialogueView, scene: Option<&Image>) -> Option<Image> {
        self.dialogue_on(view, scene, [0, 0, 0, 255])
    }

    /// The dialogue box on a transparent background, to lay over a 3D frame (colours are premultiplied by alpha).
    pub fn dialogue_overlay(&mut self, view: &DialogueView) -> Option<Image> {
        self.dialogue_on(view, None, [0, 0, 0, 0])
    }

    fn dialogue_on(
        &mut self,
        view: &DialogueView,
        scene: Option<&Image>,
        clear: [u8; 4],
    ) -> Option<Image> {
        let gui = self.gui("dialog")?;
        let mut cv = Canvas::scaled_layout(self.pixels, self.scale);
        let extra = (self.width - SCREEN.0) as i32;
        cv.clear(clear);
        if let Some(s) = scene {
            let full = [0, 0, s.width as i32, s.height as i32];
            cv.draw_image(
                s,
                full,
                [0, 0, self.width as i32, SCREEN.1 as i32],
                [1.0; 3],
                1.0,
                Orient::default(),
            );
        }
        let p = &gui.panel;
        let (px, py) = (p.extent.left, p.extent.top);
        // The box is a bar from edge to edge and down to the bottom of the screen; the text keeps the file's margins.
        let width = p.extent.width + extra;
        cv.fill_rect(
            0,
            py - 4,
            self.width as i32 + 1,
            SCREEN.1 as i32 - py + 5,
            [0, 0, 0, 160],
        );
        self.draw_border(&mut cv, &p.border, [px, py, width, p.extent.height], None);
        if !view.speaker.is_empty() {
            let name = view.speaker.clone();
            let font = gui
                .find("LBL_MESSAGE")
                .and_then(|m| m.text.as_ref())
                .map_or("fnt_d16x16".to_string(), |t| t.font.clone());
            self.draw_text(
                &mut cv,
                &font,
                &name,
                [px, py - 22, width, 20],
                1,
                [1.0, 0.9, 0.3],
            );
        }
        if view.replies.is_empty() {
            let m = gui.find("LBL_MESSAGE")?;
            let t = m.text.as_ref()?;
            let r = [
                px + m.extent.left,
                py + m.extent.top,
                m.extent.width + extra,
                m.extent.height,
            ];
            self.draw_text(&mut cv, &t.font, &view.subtitle, r, t.alignment, t.color);
        } else {
            let list = gui.find("LB_REPLIES")?;
            let proto = list.proto_item.as_ref()?;
            let t = proto.text.as_ref()?;
            let (lx, ly) = (px + list.extent.left, py + list.extent.top);
            let width = proto.extent.width + extra;
            let line_h = self.font(&t.font)?.line_height;
            // Scroll so the selected reply is visible, then draw the rows that fit.
            let rows: Vec<Vec<String>> = {
                let f = self.font(&t.font)?.clone();
                view.replies
                    .iter()
                    .enumerate()
                    .map(|(i, s)| f.wrap(&format!("{}. {s}", i + 1), width))
                    .collect()
            };
            let heights: Vec<i32> = rows
                .iter()
                .map(|l| l.len() as i32 * line_h + list.padding)
                .collect();
            let mut first = 0;
            while first < view.selected.min(rows.len().saturating_sub(1))
                && heights[first..=view.selected.min(rows.len() - 1)]
                    .iter()
                    .sum::<i32>()
                    > list.extent.height
            {
                first += 1;
            }
            let mut y = ly;
            for (i, lines) in rows.iter().enumerate().skip(first) {
                if y + lines.len() as i32 * line_h > ly + list.extent.height {
                    break;
                }
                let color = if i == view.selected {
                    [1.0, 1.0, 0.3]
                } else {
                    t.color
                };
                let f = self.font(&t.font)?.clone();
                for l in lines {
                    f.draw_line(&mut cv, lx + proto.extent.left, y, l, color);
                    y += line_h;
                }
                y += list.padding;
            }
        }
        Some(cv.into_image())
    }

    /// Where each reply of `view` is drawn, in layout units: x, y, width, height. Same rows as `dialogue_box`.
    pub fn reply_rows(&mut self, view: &DialogueView) -> Option<Vec<[i32; 4]>> {
        let gui = self.gui("dialog")?;
        let p = &gui.panel;
        let list = gui.find("LB_REPLIES")?;
        let proto = list.proto_item.as_ref()?;
        let t = proto.text.as_ref()?;
        let width = proto.extent.width + (self.width - SCREEN.0) as i32;
        let f = self.font(&t.font)?.clone();
        let (lx, mut y) = (
            p.extent.left + list.extent.left,
            p.extent.top + list.extent.top,
        );
        let mut out = Vec::new();
        for (i, s) in view.replies.iter().enumerate() {
            let lines = f.wrap(&format!("{}. {s}", i + 1), width).len() as i32;
            let h = lines * f.line_height;
            out.push([lx + proto.extent.left, y, width, h + list.padding]);
            y += h + list.padding;
        }
        Some(out)
    }

    /// Nothing at all, the size of the other overlays.
    pub fn blank_overlay(&mut self) -> Option<Image> {
        Some(Canvas::scaled_layout(self.pixels, self.scale).into_image())
    }

    /// Write one line of text at the top left of `img`, on a dark strip.
    pub fn caption(&mut self, img: Image, text: &str) -> Option<Image> {
        let font = self.font("fnt_console")?.clone();
        let mut cv = Canvas {
            width: img.width,
            height: img.height,
            scale: self.scale,
            rgba: img.rgba,
        };
        cv.fill_rect(
            0,
            0,
            font.measure(text) + 8,
            font.line_height + 4,
            [0, 0, 0, 170],
        );
        font.draw_line(&mut cv, 4, 2, text, [0.8, 0.9, 1.0]);
        Some(cv.into_image())
    }

    /// The main HUD (`maininterface.gui`) with up to three party members' portraits and bars.
    /// Controls the screen does not use right now (level-up badges, the action queue, the target bar) are hidden.
    pub fn hud(&mut self, party: &[HudMember], target: Option<(&str, i32, i32)>) -> Option<Image> {
        self.hud_on(party, target, [30, 30, 40, 255])
    }

    /// The HUD on a transparent background, to lay over a 3D frame (colours are premultiplied by alpha).
    pub fn hud_overlay(
        &mut self,
        party: &[HudMember],
        target: Option<(&str, i32, i32)>,
        action: &str,
        icon: &str,
    ) -> Option<Image> {
        self.action = action.to_string();
        self.action_icon = icon.to_string();
        self.hud_on(party, target, [0, 0, 0, 0])
    }

    fn hud_on(
        &mut self,
        party: &[HudMember],
        target: Option<(&str, i32, i32)>,
        clear: [u8; 4],
    ) -> Option<Image> {
        let mut gui = self.gui("maininterface")?;
        anchored(&mut gui.panel, self.width);
        let mut cv = Canvas::scaled_layout(self.pixels, self.scale);
        cv.clear(clear);
        let mut patches: HashMap<String, Patch> = HashMap::new();
        for tag in all_tags(&gui.panel) {
            let hide = [
                "LBL_LEVELUP",
                "LBL_HIGHLIGHT",
                "LBL_QUEUE",
                "PB_HEALTH",
                "LBL_NAME",
                "LBL_CMBTEFCT",
                "LBL_DEBIL",
                "LBL_DISABLE",
                "LBL_LVLUPBG",
            ]
            .iter()
            .any(|p| tag.starts_with(p));
            if hide {
                patches.insert(
                    tag,
                    Patch {
                        hidden: true,
                        ..Patch::default()
                    },
                );
            }
        }
        for n in 1..=3 {
            let m = party.get(n - 1);
            let patch = |f: &dyn Fn(&HudMember) -> Patch| {
                m.map_or(
                    Patch {
                        hidden: true,
                        ..Patch::default()
                    },
                    f,
                )
            };
            patches.insert(
                format!("LBL_CHAR{n}"),
                patch(&|m| Patch {
                    fill: Some(m.portrait.clone()),
                    ..Patch::default()
                }),
            );
            patches.insert(
                format!("PB_VIT{n}"),
                patch(&|m| Patch {
                    value: Some((m.hp, m.max_hp)),
                    ..Patch::default()
                }),
            );
            patches.insert(
                format!("PB_FORCE{n}"),
                patch(&|m| Patch {
                    value: Some((m.force, m.max_force)),
                    ..Patch::default()
                }),
            );
        }
        if let Some((name, hp, max)) = target {
            patches.insert(
                "PB_HEALTH".into(),
                Patch {
                    value: Some((hp, max)),
                    ..Patch::default()
                },
            );
            patches.insert(
                "LBL_NAME".into(),
                Patch {
                    text: Some(name.to_string()),
                    ..Patch::default()
                },
            );
        }
        if clear[3] == 0 {
            // Over a live scene, only what is in use: no target yet, so no action slots; no notices to show.
            let shown_with_target = ["LBL_INDICATE"];
            let idle = [
                "LBL_JOURNAL",
                "LBL_CASH",
                "LBL_PLOTXP",
                "LBL_DARKSHIFT",
                "LBL_ITEMRCVD",
                "LBL_ITEMLOST",
                "LBL_LIGHTSHIFT",
                "LBL_STEALTHXP",
                "LBL_SOLOMODE",
                "LBL_ACTIONDESC",
                "LBL_INDICATE",
                "LBH_",
                "LBL_ICON",
                "LBL_BG1",
            ];
            let mut hide: Vec<String> = all_tags(&gui.panel)
                .into_iter()
                .filter(|t| idle.iter().any(|p| t.starts_with(p)))
                .filter(|t| {
                    target.is_none()
                        || !(shown_with_target.iter().any(|p| t.starts_with(p))
                            || t == "LBH_BORDER1"
                            || t == "LBL_ICON1")
                })
                .collect();
            for n in party.len() + 1..=3 {
                hide.push(format!("LBL_BACK{n}"));
            }
            if party.len() < 2 {
                hide.push("LBL_BG2".to_string());
            }
            for tag in hide {
                patches.insert(
                    tag,
                    Patch {
                        hidden: true,
                        ..Patch::default()
                    },
                );
            }
            if target.is_some() {
                patches.insert(
                    "LBL_ICON1".to_string(),
                    Patch {
                        fill: Some(self.action_icon.clone()),
                        ..Patch::default()
                    },
                );
            }
        }
        let p = &gui.panel;
        self.draw_control(&mut cv, p, (0, 0), &patches);
        if clear[3] == 0 && target.is_some() {
            // What the slot does, beside it. The file's description box is for longer text.
            let word = self.action.clone();
            self.draw_text(
                &mut cv,
                "fnt_console",
                &word,
                [108, 404, 160, 20],
                1,
                [1.0; 3],
            );
        }
        Some(cv.into_image())
    }
}

/// Spread a 640-wide HUD over a wider screen. The left side stays, the party and the map keep their distance from
/// the right edge, and the action slot and the notice icons stay centred.
fn anchored(panel: &mut Control, width: u32) {
    const CENTRED: [&str; 10] = [
        "LBL_BG1",
        "LBL_HIGHLIGHT",
        "LBL_JOURNAL",
        "LBL_CASH",
        "LBL_PLOTXP",
        "LBL_DARKSHIFT",
        "LBL_ITEMRCVD",
        "LBL_ITEMLOST",
        "LBL_LIGHTSHIFT",
        "LBL_STEALTHXP",
    ];
    let extra = width.saturating_sub(SCREEN.0) as i32;
    panel.extent.width += extra;
    for c in &mut panel.children {
        if CENTRED.contains(&c.tag.as_str()) {
            c.extent.left += extra / 2;
        } else if c.extent.left >= SCREEN.0 as i32 / 2 {
            c.extent.left += extra;
        }
    }
}

fn gather_pictures(c: &Control, out: &mut Vec<String>) {
    out.extend([
        c.border.fill.clone(),
        c.border.edge.clone(),
        c.border.corner.clone(),
    ]);
    if let Some(p) = &c.progress_fill {
        out.push(p.fill.clone());
    }
    if let Some(t) = &c.text {
        out.push(t.font.clone());
    }
    if let Some(p) = &c.proto_item {
        gather_pictures(p, out);
    }
    for k in &c.children {
        gather_pictures(k, out);
    }
}

fn all_tags(c: &Control) -> Vec<String> {
    let mut v = vec![c.tag.clone()];
    for k in &c.children {
        v.extend(all_tags(k));
    }
    v
}

/// A texture by name, top-down: TPC rows are flipped, TGA is read as stored.
fn load_texture(inst: &mut Install, name: &str) -> Option<Image> {
    if let Ok(d) = inst.read(name, kotor_core::ResType::TPC) {
        return kotor_image::parse_tpc(&d)
            .ok()
            .map(|t| crate::font::flip_rows(t.image));
    }
    kotor_image::parse_tga(&inst.read(name, kotor_core::ResType::TGA).ok()?).ok()
}
