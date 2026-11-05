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

/// Caches for textures, fonts and GUIs while drawing.
pub struct Ui<'a> {
    inst: &'a mut Install,
    /// Width of the layout space: 640, or more on a wider screen (the height stays 480).
    width: u32,
    textures: HashMap<String, Option<Image>>,
    fonts: HashMap<String, Option<Font>>,
}

/// Per-control changes a screen applies on top of the file: replacement fill, text, bar value, or hiding.
#[derive(Clone, Debug, Default)]
pub struct Patch {
    pub fill: Option<String>,
    pub text: Option<String>,
    pub value: Option<(i32, i32)>,
    pub hidden: bool,
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
            textures: HashMap::new(),
            fonts: HashMap::new(),
        }
    }

    /// Lay out for a screen `width` units wide and 480 high. Corners stay at the corners (see `anchored`).
    pub fn set_width(&mut self, width: u32) {
        self.width = width.max(SCREEN.0);
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
            let img = load_texture(self.inst, &key);
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
            self.fonts.insert(key.clone(), f);
        }
        self.fonts[&key].as_ref()
    }

    /// Text from the talk table.
    pub fn strref(&mut self, strref: u32) -> String {
        self.inst
            .read("dialog", kotor_core::ResType(2018))
            .ok()
            .and_then(|b| kotor_tables::Tlk::parse(&b).ok())
            .and_then(|t| t.get(strref))
            .unwrap_or_default()
    }

    fn draw_border(&mut self, cv: &mut Canvas, b: &Border, r: [i32; 4], fill: Option<&str>) {
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
    fn draw_text(
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
        let mut y = match alignment & 0x18 {
            8 => r[1] + (r[3] - total) / 2,
            16 => r[1] + r[3] - total,
            _ => r[1],
        };
        for l in &lines {
            let w = f.measure(l);
            let x = match alignment & 7 {
                2 => r[0] + (r[2] - w) / 2,
                3 => r[0] + r[2] - w,
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
        let mut cv = Canvas::new(self.width, SCREEN.1);
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
            self.width as i32,
            SCREEN.1 as i32 - py + 4,
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

    /// Write one line of text at the top left of `img`, on a dark strip.
    pub fn caption(&mut self, img: Image, text: &str) -> Option<Image> {
        let font = self.font("fnt_console")?.clone();
        let mut cv = Canvas {
            width: img.width,
            height: img.height,
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
    ) -> Option<Image> {
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
        let mut cv = Canvas::new(self.width, SCREEN.1);
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
        }
        let p = &gui.panel;
        self.draw_control(&mut cv, p, (0, 0), &patches);
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
