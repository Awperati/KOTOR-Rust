//! Laying out and drawing GUI files onto a canvas: the dialogue box and the main HUD.
//!
//! **Assumptions, not measured from the game:** edge and corner textures of a border are stretched along their
//! side (top edge as stored, then turned a quarter per side clockwise) rather than tiled; the selected reply is
//! drawn yellow and unselected replies in the list's own colour; a dark translucent backing is drawn behind the
//! dialogue panel so text reads over any scene; the speaker's name is drawn in yellow above the panel.

use crate::canvas::{Canvas, Orient};
use crate::compass::CompassView;
use crate::font::Font;
use crate::gui::{Border, Control, Gui};
use crate::touch::TouchView;
use kotor_archive::Install;
use kotor_image::Image;
use std::collections::HashMap;
use std::rc::Rc;

/// The layout space of every GUI file.
pub const SCREEN: (u32, u32) = (640, 480);
/// The black bars of a conversation, as the game has them: `docs/MENU_REFERENCE.md`.
const LETTERBOX_TOP: i32 = 60;
/// Noto Sans's size for a stock font of a line height: its letters and their gaps then fill the line as the stock's do.
const NOTO_EM_OF_LINE: f32 = 0.72;
/// What each slot under an enemy's bar is called, for its key, and the font: `LBL_NAME`'s.
const SLOT_WORDS: [u32; 3] = [48303, 48307, 48311];
const SLOT_WORDS_FONT: &str = "dialogfont10x10";
/// What darkens a picture that cannot be used.
pub(crate) const GREYED: [u8; 4] = [0, 0, 0, 170];
/// Round a power that is on: the yellow of what the HUD has chosen.
const POWER_ON_LINE: [u8; 3] = [255, 255, 76];
/// How strong a quick slot with nothing in it is. By eye from the owner's picture of the game.
const EMPTY_SLOT_STRENGTH: f32 = 0.4;
/// How thick that is, in layout units.
const LINE_THICK: i32 = 2;
/// The half-dark fills of the rounded buttons and panels: `BTN_LEVELUP`, `container.gui`, `confirm.gui`.
pub(crate) const ROUNDED_FILLS: [&str; 2] = ["dialog2", "dialog3"];
/// What a slot does and what the cursor is on: the HUD's text boxes.
const HUD_TEXT_BOXES: [&str; 1] = ["LBL_ACTIONDESCBG"];
/// The main menu's title card by the name the interface draws it under, and its file in the owner's own pictures.
pub const TITLE_CARD: &str = "kotorrs_title";
const TITLE_CARD_FILE: &str = "title-card.png";
/// Attack mode's button beside the hourglass: the place the file keeps for stealth, which has no button here.
pub const ATTACK_MODE_BUTTON: &str = "TB_STEALTH";
const ATTACK_MODE_PICTURE: &str = "i_attack";
/// The Pickup button's picture: see [`pickup_picture`].
const PICKUP_PICTURE: &str = "kotorrs_pickup";
/// The lock on the button of a locked door or container: the Security skill's picture without its frame, see [`lock_glyph`].
pub const LOCK_PICTURE: &str = "kotorrs_lock";
const LOCK_FROM: &str = "isk_security";
/// How much of that picture is frame on each side, in 32nds of it: clear, black, white, black.
const LOCK_FRAME: u32 = 6;
const LOCK_FRAME_OF: u32 = 32;
/// The button is this share of the slots' size. The owner: "about 25% smaller".
const LOCK_BUTTON_PERCENT: i32 = 75;
/// How far under the name it goes.
const LOCK_BUTTON_GAP: i32 = 2;
/// How many words that do not fit are kept for whoever asks.
const MOST_TOO_BIG: usize = 64;
/// The words of a conversation, as the reference video has them at 2:30: the file's are white.
const TALK_BLUE: [f32; 3] = [0.0, 0.65, 0.97];
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
    /// The owner's own pictures, outside the repository.
    assets_dir: Option<std::path::PathBuf>,
    /// What the left mouse button would do to the target, shown under its name.
    action: String,
    /// The icon in the first action slot while there is a target.
    action_icon: String,
    textures: HashMap<String, Option<Rc<Image>>>,
    fonts: HashMap<String, Option<Rc<Font>>>,
    /// The screens' files, as read: a screen is drawn each time anything on it changes.
    guis: HashMap<String, Option<Gui>>,
    /// The talk table, read the first time a control needs it.
    tlk: Option<Option<kotor_tables::Tlk>>,
    /// The second game's, and its files, for the few things that come from it.
    second_tlk: Option<Option<kotor_tables::Tlk>>,
    second_install: Option<Option<Install>>,
    target_at: Option<(i32, i32)>,
    minimap: Option<MiniMap>,
    hud_drawn: Option<HudDrawn>,
    /// The picture `compose_seen_map` last made, what it was made from, and what had been seen.
    seen_made: Option<(String, String, Vec<bool>)>,
    combat: CombatHud,
    /// The icons flashing at the top.
    notices: Vec<String>,
    /// The small map and its frame are switched off.
    minimap_off: bool,
    floats: Vec<Float>,
    pub(crate) plates: Vec<crate::plates::Plate>,
    pub(crate) crosshair: Option<crate::plates::Crosshair>,
    slots_above_bar: bool,
    /// How big the target's slots are, in hundredths of their size in the file.
    slots_percent: i32,
    /// How many portraits the HUD shows.
    hud_party: usize,
    /// The game is paused, with the box at the top right if something paused it.
    paused: bool,
    /// The party is not following: `TB_SOLO` has a line round it.
    solo: bool,
    pause_note: String,
    /// Text in Noto Sans, and text boxes in the glowing style; and the font, once looked for.
    noto_on: bool,
    glow_on: bool,
    noto: Option<Option<std::sync::Arc<crate::ttf::Face>>>,
    /// Words drawn since last asked, and the ones that did not fit their box.
    words_drawn: usize,
    words_too_big: Vec<String>,
    /// What the on-screen controls show: `touch.rs`.
    pub(crate) touch: TouchView,
    /// What the compass shows: `compass.rs`.
    pub(crate) compass: CompassView,
    /// What the detection bars show: `detection.rs`.
    pub(crate) detection: crate::detection::DetectionView,
}

/// What a slot under an enemy's bar shows now.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SlotShown {
    pub picture: String,
    /// It has more to go through, with its arrows.
    pub more: bool,
    /// It is too dear for now: greyed.
    pub greyed: bool,
    /// A power it shows is on now: a line round it.
    pub lit: bool,
    /// How much of a medical item's cooldown is left, 0 to 1: a white square over that share of the slot, from its foot. The
    /// owner's Medication Sickness: full at a use, and its top comes down as the cooldown ends.
    pub veil: f32,
}

/// What the HUD shows of a fight, as the reference video has it at 5:50 and 6:05.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CombatHud {
    /// The panel with the fight's words is up, and the line at the top says so.
    pub engaged: bool,
    pub message: String,
    /// The pictures of what is queued, what is happening now first.
    pub queue: Vec<String>,
    /// How many of them are queued and not yet under way.
    pub queued: usize,
    /// The target is an enemy: its bar is red, with slots under it.
    pub attack: Option<String>,
    pub slots: [Option<SlotShown>; 3],
    /// The slot under the cursor, and what it does, for the name bar.
    pub hover: Option<(usize, String)>,
    /// The target is locked, and the first slot under its bar has ways to try it.
    pub locked: bool,
    /// The three slots at the bottom right after the action's: consumables, shields, and the powers with a lasting effect.
    pub quick: [Option<SlotShown>; 3],
    /// With no target to act on: the first of the four has the powers that work at once.
    pub first: Option<SlotShown>,
    /// What the one under the cursor is called.
    pub quick_hover: Option<String>,
    /// The first of the four slots is the Pickup button.
    pub pickup: bool,
    /// The leader is in attack mode or fighting: the button has a line round it.
    pub attack_mode: bool,
}

/// One control as the HUD last drew it.
#[derive(Clone, Debug, PartialEq)]
struct ControlDrawn {
    place: [i32; 4],
    patch: Patch,
    font: Option<String>,
}

/// What the HUD on screen was drawn from, for [`Ui::hud_part`] to see what has changed.
#[derive(Clone, Debug, PartialEq)]
struct HudDrawn {
    slots_percent: i32,
    pixels: (u32, u32),
    controls: Vec<ControlDrawn>,
    minimap: Option<MiniMap>,
    floats: Vec<Float>,
    target_at: Option<(i32, i32)>,
    hover: Option<(usize, String)>,
    paused: bool,
    pause_note: String,
    touch: TouchView,
    compass: CompassView,
    detection: crate::detection::DetectionView,
}

/// The lines rising words can wrap to.
const FLOAT_MOST_LINES: i32 = 3;
/// A text box's glow reaches past it, in layout units.
pub(crate) const CONTROL_BOX_PAD: i32 = 8;

/// The box that holds both of two: left, top, width, height.
pub(crate) fn union_box(a: Option<[u32; 4]>, b: Option<[u32; 4]>) -> Option<[u32; 4]> {
    match (a, b) {
        (Some(a), Some(b)) => {
            let (left, top) = (a[0].min(b[0]), a[1].min(b[1]));
            let right = (a[0] + a[2]).max(b[0] + b[2]);
            let bottom = (a[1] + a[3]).max(b[1] + b[3]);
            Some([left, top, right - left, bottom - top])
        }
        (a, b) => a.or(b),
    }
}

/// Boxes that overlap enough to be cheaper as one, as one.
pub(crate) fn merged_boxes(mut boxes: Vec<[u32; 4]>) -> Vec<[u32; 4]> {
    let area = |b: [u32; 4]| b[2] as u64 * b[3] as u64;
    let mut n = 0;
    while n < boxes.len() {
        let near = (n + 1..boxes.len()).find(|&m| {
            let both = union_box(Some(boxes[n]), Some(boxes[m])).expect("both are there");
            area(both) <= area(boxes[n]) + area(boxes[m])
        });
        match near {
            Some(m) => {
                let other = boxes.swap_remove(m);
                boxes[n] = union_box(Some(boxes[n]), Some(other)).expect("both are there");
            }
            None => n += 1,
        }
    }
    boxes
}

/// Words rising over someone in a fight, in layout units.
#[derive(Clone, Debug, PartialEq)]
pub struct Float {
    pub x: i32,
    pub y: i32,
    pub text: String,
    pub color: [f32; 3],
}

/// What a fight's text and bar are drawn in.
const COMBAT_RED: [f32; 3] = [1.0, 0.16, 0.1];
const HOSTILE_SLOT: &str = "lbl_miscroll_h";
const HOVER_SLOT: &str = "lbl_miscroll_hi";
const FLOAT_FONT: &str = "fnt_d16x16";
const PAUSE_MARK: [u8; 4] = [255, 255, 76, 255];
const PAUSE_NOTE_WIDTH: i32 = 300;
/// What stays in the middle of a wide window, as the video has it.
const HUD_CENTRED: [&str; 4] = ["LBL_COMBATBG", "LBL_QUEUE", "BTN_CLEAR", "LBL_CMBT"];

/// The small map on the HUD.
#[derive(Clone, Debug, PartialEq)]
pub struct MiniMap {
    pub picture: String,
    /// Where the player is, in the picture's own 512 by 256.
    pub at: [f32; 2],
    /// Which way the player faces: radians clockwise from up.
    pub facing: f32,
}

/// The picture a small map is cut from is written for this many units across, and the arrow's picture.
const MINIMAP_FILE_WIDTH: f32 = 512.0;
const MINIMAP_ARROW: &str = "mm_barrow";
/// The medication cooldown's square: white, and see-through enough that the item shows under it.
const VEIL: [u8; 4] = [255, 255, 255, 80];

/// Per-control changes a screen applies on top of the file: replacement fill, text, bar value, or hiding.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Patch {
    /// Its words show at this share of their strength.
    pub faint: Option<f32>,
    /// Its picture shows at this share of its strength.
    pub faded: Option<f32>,
    /// A see-through white square over this share of its height, from its foot.
    pub veil: Option<f32>,
    /// Its picture upside down.
    pub flipped: bool,
    pub fill: Option<String>,
    pub text: Option<String>,
    pub value: Option<(i32, i32)>,
    pub hidden: bool,
    /// Replacement colour and font for the control's text.
    pub color: Option<[f32; 3]>,
    pub font: Option<String>,
    /// No border, whatever the file says.
    pub plain: bool,
    /// A label that answers to the cursor as a button does.
    pub live: bool,
    /// A list of words: show the page that ends this many lines before the last, with a bar.
    pub from_end: Option<usize>,
    /// A list of words starts this many layout units higher, for more lines than the file left room for.
    pub rise: i32,
    /// Its picture is darkened: it cannot be used now.
    pub greyed: bool,
    /// Its words stand this many layout units in from each side.
    pub inset: i32,
    /// A list: this colour behind it, in place of the screen's picture.
    pub behind: Option<[u8; 3]>,
    /// A thin line of this colour instead of the file's border.
    pub line: Option<[u8; 3]>,
    /// Where it goes, in layout units, in place of where the file has it.
    pub place: Option<[i32; 4]>,
    /// In place of the control, this rectangle of layout units in this colour.
    pub cover: Option<([i32; 4], [u8; 3])>,
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
    /// A level is waiting to be taken.
    pub level_up: bool,
    /// In attack mode, or fighting: the portrait has a line round it.
    pub attack_mode: bool,
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
            assets_dir: None,
            action: String::new(),
            action_icon: String::new(),
            textures: HashMap::new(),
            fonts: HashMap::new(),
            guis: HashMap::new(),
            tlk: None,
            second_tlk: None,
            second_install: None,
            target_at: None,
            minimap: None,
            hud_drawn: None,
            seen_made: None,
            combat: CombatHud::default(),
            notices: Vec::new(),
            minimap_off: false,
            floats: Vec::new(),
            plates: Vec::new(),
            crosshair: None,
            slots_above_bar: false,
            touch: TouchView::default(),
            compass: CompassView::default(),
            detection: Default::default(),
            slots_percent: 100,
            hud_party: 1,
            paused: false,
            solo: false,
            pause_note: String::new(),
            noto_on: false,
            glow_on: false,
            words_drawn: 0,
            words_too_big: Vec::new(),
            noto: None,
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
            crate::canvas::forget_picture_addresses();
        }
    }

    /// Use upscaled pictures from this folder when the scale calls for them.
    pub fn set_hd_dir(&mut self, dir: Option<std::path::PathBuf>) {
        self.hd_dir = dir;
        self.textures.clear();
        self.fonts.clear();
        crate::canvas::forget_picture_addresses();
    }

    /// Look in this folder for the owner's own pictures.
    pub fn set_assets_dir(&mut self, dir: Option<std::path::PathBuf>) {
        self.assets_dir = dir;
        self.textures.remove(TITLE_CARD);
    }

    /// The owner's title card, or its upscaled copy when the scale calls for it, on a black ground that is seen through.
    /// With no card in the folder there is none, whatever copies are left.
    fn title_card(&self) -> Option<Image> {
        let file = self.assets_dir.as_ref()?.join(TITLE_CARD_FILE);
        let card = std::fs::read(file).ok()?;
        let mut img = self
            .hd_picture(TITLE_CARD)
            .or_else(|| kotor_image::parse_png(&card))?;
        brightness_to_alpha(&mut img);
        Some(img)
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
            "partyselection",
            "map",
            "optkeymapping",
            "optmouse",
            "optgraphicsadv",
            "optfeedback",
            "optautopause",
            "maincg",
            "leveluppnl",
            "abchrgen",
            "skchrgen",
            "ftchrgen",
            "pwrlvlup",
            "optionsmain",
            "optgameplay",
            "optgraphics",
            "optsound",
            "top",
            "mipc28x6",
            "inventory",
            "equip",
            "character",
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
        // What items look like in the slots and the lists: `ia_`, `ii_` and `iw_`, from the item's kind.
        names.extend(self.inst.overlay_resources().filter_map(|(name, kind, _)| {
            let name = name.to_string().to_ascii_lowercase();
            let is_item = ["ia_", "ii_", "iw_"].iter().any(|p| name.starts_with(p));
            (is_item && kind == kotor_core::ResType::TPC).then_some(name)
        }));
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
        let key = name.to_ascii_lowercase();
        if !self.guis.contains_key(&key) {
            let read = self.inst.read(name, kotor_core::ResType(2047)).ok();
            let gui = read.and_then(|bytes| Gui::parse(&bytes).ok());
            self.guis.insert(key.clone(), gui);
        }
        self.guis[&key].clone()
    }

    /// Whether a picture from `compose_seen_map` is here: changing the window's size forgets pictures.
    pub fn has_picture(&self, name: &str) -> bool {
        matches!(self.textures.get(name), Some(Some(_)))
    }

    /// Make a map as far as it has been seen: over the file's picture, what has not been seen shows a grid, as in the
    /// reference video at 1:25:00. `seen` covers the part of a 512 wide picture that is the map. The edge is soft.
    pub fn compose_seen_map(
        &mut self,
        out: &str,
        picture: &str,
        seen: &[bool],
        cols: usize,
        rows: usize,
    ) {
        let Some(img) = self.texture_shared(picture) else {
            return;
        };
        let (w, h) = (img.width as usize, img.height as usize);
        let across = cols as f32 / (w as f32 * MAP_SHOWN_WIDTH / MINIMAP_FILE_WIDTH);
        let down = rows as f32 / h as f32;
        let cell = |c: i32, r: i32| -> f32 {
            let inside = (0..cols as i32).contains(&c) && (0..rows as i32).contains(&r);
            (inside && seen[r as usize * cols + c as usize]) as i32 as f32
        };
        let grid = (MAP_GRID * w as f32 / MINIMAP_FILE_WIDTH).max(1.0) as usize;
        let key = out.to_ascii_lowercase();
        // A cell seen as the player walks changes a few pixels of the map. An upscaled map is millions of them.
        let made = self.seen_made.take();
        let before = made
            .filter(|m| {
                (m.0.as_str(), m.1.as_str()) == (key.as_str(), picture) && m.2.len() == seen.len()
            })
            .map(|m| m.2);
        let held = self.textures.remove(&key).flatten();
        let held = held.filter(|i| before.is_some() && i.rgba.len() == img.rgba.len());
        let cells_changed = before.as_ref().filter(|_| held.is_some()).map(|b| {
            let changed = (0..seen.len()).filter(|&n| b[n] != seen[n]);
            changed.fold(None, |all: Option<[usize; 4]>, n| {
                let (c, r) = (n % cols, n / cols);
                Some(all.map_or([c, r, c, r], |a| {
                    [a[0].min(c), a[1].min(r), a[2].max(c), a[3].max(r)]
                }))
            })
        });
        // Each pixel falls between two cells, and the cell's edge is soft.
        let [left, top, right, bottom] = match cells_changed {
            Some(None) => [0, 0, 0, 0],
            Some(Some([c0, r0, c1, r1])) => [
                (((c0 as f32 - 1.0) / across).floor() as i32 - 1).max(0) as usize,
                (((r0 as f32 - 1.0) / down).floor() as i32 - 1).max(0) as usize,
                ((((c1 + 2) as f32 / across).ceil() as usize) + 2).min(w),
                ((((r1 + 2) as f32 / down).ceil() as usize) + 2).min(h),
            ],
            None => [0, 0, w, h],
        };
        let mut shown = match held {
            Some(rc) => match Rc::try_unwrap(rc) {
                Ok(image) => image,
                Err(rc) => (*rc).clone(),
            },
            None => (*img).clone(),
        };
        crate::canvas::forget_content(&shown);
        let from = &img.rgba;
        for y in top..bottom {
            let fy = y as f32 * down - 0.5;
            let (r0, ty) = (fy.floor() as i32, fy - fy.floor());
            for x in left..right {
                let fx = x as f32 * across - 0.5;
                let (c0, tx) = (fx.floor() as i32, fx - fx.floor());
                let up = cell(c0, r0) * (1.0 - tx) + cell(c0 + 1, r0) * tx;
                let low = cell(c0, r0 + 1) * (1.0 - tx) + cell(c0 + 1, r0 + 1) * tx;
                let seen = up * (1.0 - ty) + low * ty;
                let line = x % grid == 0 || y % grid == 0;
                let unseen = if line { MAP_GRID_LINE } else { MAP_GRID_BACK };
                let at = (y * w + x) * 4;
                for k in 0..3 {
                    shown.rgba[at + k] =
                        (from[at + k] as f32 * seen + unseen[k] * (1.0 - seen)) as u8;
                }
                shown.rgba[at + 3] = 255;
            }
        }
        // The small map shows it, and has to be drawn again.
        if let Some(d) = self.hud_drawn.as_mut() {
            d.minimap = None;
        }
        self.seen_made = Some((key.clone(), picture.to_string(), seen.to_vec()));
        self.textures.insert(key, Some(Rc::new(shown)));
    }

    /// A picture made while the game runs, such as the player's portrait from their face, by a name of its own.
    pub fn set_texture(&mut self, name: &str, img: Image) {
        self.textures
            .insert(name.to_ascii_lowercase(), Some(Rc::new(img)));
    }

    pub fn texture(&mut self, name: &str) -> Option<&Image> {
        if name.is_empty() {
            return None;
        }
        let key = name.to_ascii_lowercase();
        if key == TITLE_CARD && !self.textures.contains_key(&key) {
            let card = self.title_card();
            self.textures.insert(key.clone(), card.map(Rc::new));
        }
        if key == LOCK_PICTURE && !self.textures.contains_key(&key) {
            let glyph = self.texture_shared(LOCK_FROM).map(|i| lock_glyph(&i));
            self.textures.insert(key.clone(), glyph.map(Rc::new));
        }
        if !self.textures.contains_key(&key) {
            let mut img = self
                .hd_picture(&key)
                .or_else(|| load_texture(self.inst, &key))
                .or_else(|| self.second_game_icon(&key))
                .or_else(|| (key == PICKUP_PICTURE).then(pickup_picture))
                .or_else(|| crate::touch::touch_picture(&key))
                .or_else(|| crate::compass::compass_picture(&key));
            if let (Some(i), true) = (img.as_mut(), additive(self.inst, &key)) {
                brightness_to_alpha(i);
            }
            self.textures.insert(key.clone(), img.map(Rc::new));
        }
        self.textures[&key].as_deref()
    }

    /// A feat's picture the first game has not got: the second game's, in its own folder.
    fn second_game_icon(&mut self, name: &str) -> Option<Image> {
        if !name.starts_with("if_") {
            return None;
        }
        let inst = self.second_install.get_or_insert_with(|| {
            let mut inst = Install::open(kotor_core::second_game_dir()?).ok()?;
            inst.load_texture_packs().ok()?;
            Some(inst)
        });
        load_texture(inst.as_mut()?, name)
    }

    /// A picture, to draw with while other things are asked of this.
    pub(crate) fn texture_shared(&mut self, name: &str) -> Option<Rc<Image>> {
        self.texture(name)?;
        self.textures.get(&name.to_ascii_lowercase())?.clone()
    }

    /// Noto Sans or the game's own fonts, and glowing text boxes or the stock ones. Returns whether anything changed.
    pub fn set_text_style(&mut self, noto: bool, glow: bool) -> bool {
        let changed = noto != self.noto_on || glow != self.glow_on;
        if noto != self.noto_on {
            self.fonts.clear();
        }
        self.noto_on = noto;
        self.glow_on = glow;
        changed
    }

    /// Whether text boxes are in the glowing style.
    pub(crate) fn glowing(&self) -> bool {
        self.glow_on
    }

    /// How many words were drawn since last asked, and which did not fit their box.
    pub fn words_fit(&mut self) -> (usize, Vec<String>) {
        let drawn = std::mem::take(&mut self.words_drawn);
        (drawn, std::mem::take(&mut self.words_too_big))
    }

    /// Whether the font is being used.
    pub fn noto_in_use(&mut self) -> bool {
        self.noto_on && self.noto_face().is_some()
    }

    fn noto_face(&mut self) -> Option<std::sync::Arc<crate::ttf::Face>> {
        self.noto
            .get_or_insert_with(|| crate::ttf::Face::bundled().map(std::sync::Arc::new))
            .clone()
    }

    /// Text is about to be laid out on a 640 wide screen stretched over the window: its letters stay in shape, so they
    /// take less of that width.
    pub(crate) fn measure_for_menus(&mut self) {
        let ratio = self.scale * SCREEN.0 as f32 / self.pixels.0.max(1) as f32;
        if let Some(f) = self.noto_face() {
            f.set_across(ratio);
        }
    }

    /// Text is about to be laid out on a canvas with the same scale both ways.
    pub(crate) fn measure_for_hud(&mut self) {
        let ratio = self.scale / self.hud_across();
        if let Some(f) = self.noto_face() {
            f.set_across(ratio);
        }
    }

    /// Text is about to be laid out at one scale both ways, whatever the HUD's is.
    pub(crate) fn measure_evenly(&mut self) {
        if let Some(f) = self.noto_face() {
            f.set_across(1.0);
        }
    }

    /// Pixels to a layout unit across the HUD. The same as down it, unless the window is too narrow for the 640 wide layout: it
    /// is squeezed in, as a click takes it to be.
    pub(crate) fn hud_across(&self) -> f32 {
        let least = SCREEN.0 as f32 * self.scale;
        if (self.pixels.0 as f32) < least {
            self.pixels.0 as f32 / SCREEN.0 as f32
        } else {
            self.scale
        }
    }

    /// An empty picture of the window, to draw the HUD on.
    fn hud_canvas(&self) -> Canvas {
        let mut cv = Canvas::scaled_layout(self.pixels, self.scale);
        cv.scale_x = self.hud_across();
        cv
    }

    /// More of the HUD on a picture of the window.
    pub(crate) fn hud_canvas_over(&self, picture: Image) -> Canvas {
        let mut cv = Canvas::over(picture, self.scale);
        cv.scale_x = self.hud_across();
        cv
    }

    pub fn font(&mut self, name: &str) -> Option<&Font> {
        let key = name.to_ascii_lowercase();
        if !self.fonts.contains_key(&key) {
            // **Assumption:** a font whose TXI does not fit its picture is replaced by the dialogue font of the
            // same family, `fnt_dialog16x16`.
            // Fonts stay as the game has them: upscaling fills in the gaps inside `fnt_console`'s tiny letters and turns
            // them into blocks at 4K.
            let mut f =
                Font::load(self.inst, &key).or_else(|| Font::load(self.inst, "fnt_dialog16x16"));
            if self.noto_on {
                if let (Some(font), Some(face)) = (f.as_mut(), self.noto_face()) {
                    font.drawn_in = Some(crate::ttf::Sized {
                        face,
                        em: font.line_height as f32 * NOTO_EM_OF_LINE,
                        line_height: font.line_height,
                    });
                }
            }
            self.fonts.insert(key.clone(), f.map(Rc::new));
        }
        self.fonts[&key].as_deref()
    }

    /// A font, to draw with while other things are asked of this.
    pub(crate) fn font_shared(&mut self, name: &str) -> Option<Rc<Font>> {
        self.font(name)?;
        self.fonts.get(&name.to_ascii_lowercase())?.clone()
    }

    /// Text from the talk table, `dialog.tlk` beside the game's data files.
    pub fn strref(&mut self, strref: u32) -> String {
        if let Some(theirs) = strref.checked_sub(kotor_core::SECOND_GAME_STRREF) {
            return self
                .second_tlk
                .get_or_insert_with(|| {
                    let dir = kotor_core::second_game_dir()?;
                    kotor_tables::Tlk::parse(&std::fs::read(dir.join("dialog.tlk")).ok()?).ok()
                })
                .as_ref()
                .and_then(|t| t.get(theirs))
                .unwrap_or_default();
        }
        let file = self.inst.talk_table_file();
        self.tlk
            .get_or_insert_with(|| {
                std::fs::read(file)
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
        let d = b.dimension;
        // Its edges are half dark on their inner side and clear beyond the line. The same half dark under them shows as a
        // square plate, with a lighter bar through the middle: so it goes only where they do not.
        let within = if ROUNDED_FILLS.contains(&fill.as_str()) {
            [r[0] + d, r[1] + d, r[2] - 2 * d, r[3] - 2 * d]
        } else {
            r
        };
        if let Some(img) = self.texture_shared(&fill) {
            let full = [0, 0, img.width as i32, img.height as i32];
            cv.draw_image(&img, full, within, b.color, 1.0, Orient::default());
        }
        if d <= 0 {
            return;
        }
        let [x, y, w, h] = r;
        if let Some(img) = self.texture_shared(&b.edge) {
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
        if let Some(img) = self.texture_shared(&b.corner) {
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
        let Some(f) = self.font_shared(font) else {
            return 0;
        };
        let lines = f.wrap(text, r[2]);
        let total = lines.len() as i32 * f.line_height;
        self.words_drawn += 1;
        if (total > r[3] || lines.iter().any(|l| f.measure(l) > r[2]))
            && self.words_too_big.len() < MOST_TOO_BIG
        {
            self.words_too_big
                .push(format!("{text:?} in {} by {}", r[2], r[3]));
        }
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
                // A colour of its own: the video's enemy bar, for which I found no picture.
                if let Some([red, green, blue]) = patch.color {
                    let c = [
                        (red * 255.0) as u8,
                        (green * 255.0) as u8,
                        (blue * 255.0) as u8,
                        255,
                    ];
                    cv.fill_rect(dst[0], dst[1], dst[2], dst[3], c);
                } else if let Some(img) = self.texture_shared(&fill.fill) {
                    let full = [0, 0, img.width as i32, img.height as i32];
                    cv.draw_image(&img, full, dst, fill.color, 1.0, Orient::default());
                }
            }
            return;
        }
        let kept = patch
            .faded
            .map(|share| (cv.keep_box(r[0], r[1], r[2], r[3]), share));
        if self.glow_on && HUD_TEXT_BOXES.contains(&c.tag.as_str()) {
            crate::glow::boxed(cv, r);
        } else if patch.flipped {
            if let Some(img) = self.texture_shared(patch.fill.as_deref().unwrap_or(&c.border.fill))
            {
                let full = [0, 0, img.width as i32, img.height as i32];
                let over = Orient {
                    flip_y: true,
                    ..Orient::default()
                };
                cv.draw_image(&img, full, r, c.border.color, 1.0, over);
            }
        } else if c.kind != 11 {
            self.draw_border(cv, &c.border, r, patch.fill.as_deref());
        }
        if let Some((kept, share)) = kept {
            cv.fade_to(&kept, share);
        }
        if patch.greyed {
            cv.fill_rect(r[0], r[1], r[2], r[3], GREYED);
        }
        if let Some(share) = patch.veil {
            let high = (r[3] as f32 * share.clamp(0.0, 1.0)).round() as i32;
            cv.fill_rect(r[0], r[1] + r[3] - high, r[2], high, VEIL);
        }
        if let Some([red, green, blue]) = patch.line {
            let c = [red, green, blue, 255];
            let t = LINE_THICK;
            cv.fill_rect(r[0], r[1], r[2], t, c);
            cv.fill_rect(r[0], r[1] + r[3] - t, r[2], t, c);
            cv.fill_rect(r[0], r[1], t, r[3], c);
            cv.fill_rect(r[0] + r[2] - t, r[1], t, r[3], c);
        }
        if let Some(t) = &c.text {
            let text = patch.text.clone().unwrap_or_else(|| match t.strref {
                Some(s) if t.text.is_empty() => self.strref(s),
                _ => t.text.clone(),
            });
            if !text.is_empty() {
                let color = patch.color.unwrap_or(t.color);
                self.draw_text(cv, &t.font, &text, r, t.alignment, color);
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

    /// The black bars of a conversation, with nothing on them.
    pub fn letterbox_overlay(&mut self) -> Option<Image> {
        let mut cv = self.hud_canvas();
        self.letterbox(&mut cv);
        Some(cv.into_image())
    }

    fn letterbox(&mut self, cv: &mut Canvas) {
        let w = self.width as i32 + 1;
        cv.fill_rect(0, 0, w, LETTERBOX_TOP, [0, 0, 0, 255]);
        let bottom = self.gui("dialog").map_or(374, |g| g.panel.extent.top - 4);
        cv.fill_rect(0, bottom, w, SCREEN.1 as i32 - bottom + 1, [0, 0, 0, 255]);
        if self.glow_on {
            crate::glow::bar_edge(cv, LETTERBOX_TOP, true);
            crate::glow::bar_edge(cv, bottom, false);
        }
    }

    fn dialogue_on(
        &mut self,
        view: &DialogueView,
        scene: Option<&Image>,
        clear: [u8; 4],
    ) -> Option<Image> {
        self.measure_for_hud();
        let gui = self.gui("dialog")?;
        let mut cv = self.hud_canvas();
        let extra = (self.width - SCREEN.0) as i32;
        // A new canvas is clear already, and its untouched pages cost nothing until they are.
        if clear != [0; 4] {
            cv.clear(clear);
        }
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
        // Over a live scene: the game's black bars, and no name, since the voice and the shot say who speaks.
        let live = clear[3] == 0;
        if live {
            self.letterbox(&mut cv);
        } else {
            cv.fill_rect(
                0,
                py - 4,
                self.width as i32 + 1,
                SCREEN.1 as i32 - py + 5,
                [0, 0, 0, 160],
            );
        }
        self.draw_border(&mut cv, &p.border, [px, py, width, p.extent.height], None);
        if live && !view.replies.is_empty() && !view.subtitle.is_empty() {
            // The line being answered stays up, at the foot of the top bar.
            let m = gui.find("LBL_MESSAGE")?;
            let t = m.text.as_ref()?;
            let font = t.font.clone();
            let color = TALK_BLUE;
            let r = [px, 2, width, LETTERBOX_TOP - 4];
            let line = view.subtitle.clone();
            self.draw_text(&mut cv, &font, &line, r, 34, color);
        }
        if !live && !view.speaker.is_empty() {
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
            let color = if live { TALK_BLUE } else { t.color };
            self.draw_text(&mut cv, &t.font, &view.subtitle, r, t.alignment, color);
        } else {
            let list = gui.find("LB_REPLIES")?;
            let proto = list.proto_item.as_ref()?;
            let t = proto.text.as_ref()?;
            let (lx, ly) = (px + list.extent.left, py + list.extent.top);
            let width = proto.extent.width + extra;
            let line_h = self.font(&t.font)?.line_height;
            // Scroll so the selected reply is visible, then draw the rows that fit.
            let rows: Vec<Vec<String>> = {
                let f = self.font_shared(&t.font)?;
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
                } else if live {
                    TALK_BLUE
                } else {
                    t.color
                };
                let f = self.font_shared(&t.font)?;
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
        self.measure_for_hud();
        let gui = self.gui("dialog")?;
        let p = &gui.panel;
        let list = gui.find("LB_REPLIES")?;
        let proto = list.proto_item.as_ref()?;
        let t = proto.text.as_ref()?;
        let width = proto.extent.width + (self.width - SCREEN.0) as i32;
        let f = self.font_shared(&t.font)?;
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
        self.hud_party = party.len();
        let gui = self.hud_layout(self.target_at)?;
        self.measure_for_hud();
        let patches = self.hud_patches(&gui, party, target, action, icon);
        let mut cv = self.hud_canvas();
        self.hud_pass(&mut cv, &gui, &patches);
        self.hud_drawn = Some(self.hud_as_drawn(&gui, &patches));
        Some(cv.into_image())
    }

    /// Draw again only what has changed since the HUD was last drawn, on that picture. The whole of it takes 6 ms at
    /// 3840x2160, copying it another 6 and sending it to the card another 6, and in a fight something changes on it several
    /// times a second. Gives the picture and the box of pixels that changed, or nothing if the whole has to be drawn.
    pub fn hud_part(
        &mut self,
        picture: Image,
        party: &[HudMember],
        target: Option<(&str, i32, i32)>,
        action: &str,
        icon: &str,
    ) -> Option<(Image, [u32; 4])> {
        self.hud_party = party.len();
        let was = self.hud_drawn.take()?;
        if (picture.width, picture.height) != self.pixels || was.pixels != self.pixels {
            return None;
        }
        // Under the cursor a slot says its key, past the slot. The pause is drawn over the controls.
        let hover = self.combat.hover.clone();
        if was.hover != hover
            || (hover.is_some()
                && (was.target_at != self.target_at || was.slots_percent != self.slots_percent))
            || was.paused != self.paused
            || was.pause_note != self.pause_note
            || was.touch.on != self.touch.on
            || was.compass.on != self.compass.on
            || was.detection.on != self.detection.on
        {
            return None;
        }
        let gui = self.hud_layout(self.target_at)?;
        self.measure_for_hud();
        let patches = self.hud_patches(&gui, party, target, action, icon);
        let now = self.hud_as_drawn(&gui, &patches);
        if was.controls.len() != now.controls.len() {
            return None;
        }
        let mut boxes = Vec::new();
        for (a, b) in was.controls.iter().zip(&now.controls) {
            if a != b {
                boxes.extend(self.control_box(a));
                boxes.extend(self.control_box(b));
            }
        }
        if was.minimap != now.minimap {
            let map = gui.panel.children.iter().find(|c| c.tag == "LBL_MAPVIEW")?;
            let e = &map.extent;
            boxes.extend(self.pixel_box([e.left, e.top, e.width, e.height], 0));
        }
        if was.floats != now.floats {
            let high = self.font_shared(FLOAT_FONT)?.line_height * FLOAT_MOST_LINES;
            for f in was.floats.iter().chain(&now.floats) {
                // Centred on the line's box, so more lines than fit in it spread up as well as down.
                boxes.extend(self.pixel_box([f.x - 70, f.y - high, 140, high * 2 + 20], 0));
            }
        }
        if was.touch != now.touch {
            boxes.extend(self.touch_changes(&gui.panel, &was.touch, &now.touch));
        }
        if was.compass != now.compass {
            boxes.extend(self.compass_changes(&gui.panel, &was.compass, &now.compass));
        }
        if was.detection != now.detection {
            boxes.extend(self.detection_changes(&was.detection, &now.detection));
        }
        let mut cv = self.hud_canvas_over(picture);
        let mut changed = None;
        for b in merged_boxes(boxes) {
            cv.redraw_only(b);
            self.hud_pass(&mut cv, &gui, &patches);
            changed = union_box(changed, Some(b));
        }
        self.hud_drawn = Some(now);
        Some((cv.into_image(), changed.unwrap_or([0; 4])))
    }

    /// What the HUD is drawn from now.
    fn hud_as_drawn(&self, gui: &Gui, patches: &HashMap<String, Patch>) -> HudDrawn {
        let mut controls = Vec::new();
        for c in gui.panel.children.iter().filter(|c| c.kind != 9) {
            controls_drawn(c, (0, 0), patches, &mut controls);
        }
        HudDrawn {
            pixels: self.pixels,
            controls,
            minimap: self.minimap.clone(),
            floats: self.floats.clone(),
            target_at: self.target_at,
            slots_percent: self.slots_percent,
            hover: self.combat.hover.clone(),
            paused: self.paused,
            pause_note: self.pause_note.clone(),
            touch: self.touch.clone(),
            compass: self.compass.clone(),
            detection: self.detection.clone(),
        }
    }

    /// The pixels a control can have drawn on. Words too long for it go past it.
    fn control_box(&mut self, c: &ControlDrawn) -> Option<[u32; 4]> {
        if c.patch.hidden {
            return None;
        }
        let [_, _, w, h] = c.place;
        let past = match (&c.font, &c.patch.text) {
            (Some(font), Some(text)) => {
                let f = self.font_shared(font)?;
                let lines = f.wrap(text, w);
                let wide = lines.iter().map(|l| f.measure(l)).max().unwrap_or(0);
                (wide - w)
                    .max(lines.len() as i32 * f.line_height - h)
                    .max(0)
            }
            _ => 0,
        };
        self.pixel_box(c.place, CONTROL_BOX_PAD + past)
    }

    pub(crate) fn hud_layout(&mut self, target_at: Option<(i32, i32)>) -> Option<Gui> {
        let mut gui = self.gui(HUD_GUI)?;
        let slots = (self.slots_above_bar, self.slots_percent, self.combat.locked);
        place_hud(&mut gui.panel, self.width, target_at, slots);
        self.place_for_touch(&mut gui.panel);
        self.place_for_compass(&mut gui.panel);
        Some(gui)
    }

    /// The pixels of a box of layout units, with room round it for rounding and `pad` more units.
    pub(crate) fn pixel_box(&self, [x, y, w, h]: [i32; 4], pad: i32) -> Option<[u32; 4]> {
        let low = |v: i32, k: f32| (((v - pad) as f32 * k).floor() as i32 - 1).max(0) as u32;
        let high = |v: i32, most: u32, k: f32| {
            (((v + pad) as f32 * k).ceil() as i32 + 1)
                .max(0)
                .min(most as i32) as u32
        };
        let (across, down) = (self.hud_across(), self.scale);
        let (left, top) = (low(x, across), low(y, down));
        let (right, bottom) = (
            high(x + w, self.pixels.0, across),
            high(y + h, self.pixels.1, down),
        );
        (right > left && bottom > top).then(|| [left, top, right - left, bottom - top])
    }

    fn hud_patches(
        &self,
        gui: &Gui,
        party: &[HudMember],
        target: Option<(&str, i32, i32)>,
        action: &str,
        icon: &str,
    ) -> HashMap<String, Patch> {
        let mut patches: HashMap<String, Patch> = HashMap::new();
        for tag in all_tags(&gui.panel) {
            if tag != "LBL_MAPBORDER" && HUD_HIDDEN.iter().any(|p| tag.starts_with(p)) {
                patches.insert(
                    tag,
                    Patch {
                        hidden: true,
                        ..Patch::default()
                    },
                );
            }
        }
        // The file has the same picture for the arrow over a slot and the one under it, and so both pointed up.
        for tag in all_tags(&gui.panel) {
            if tag.starts_with("BTN_TARGETDOWN") || tag.starts_with("BTN_ACTIONDOWN") {
                patches.entry(tag).or_default().flipped = true;
            }
        }
        let hide = |patches: &mut HashMap<String, Patch>, tag: String| {
            patches.entry(tag).or_default().hidden = true;
        };
        for (i, n) in HUD_MEMBER_SLOTS.into_iter().enumerate() {
            match party.get(i) {
                Some(m) => {
                    let portrait = patches.entry(format!("LBL_CHAR{n}")).or_default();
                    portrait.fill = Some(m.portrait.clone());
                    portrait.line = m.attack_mode.then_some(POWER_ON_LINE);
                    patches.entry(format!("PB_VIT{n}")).or_default().value = Some((m.hp, m.max_hp));
                    patches.entry(format!("PB_FORCE{n}")).or_default().value =
                        Some((m.force, m.max_force));
                    if m.level_up {
                        patches.entry(format!("LBL_LEVELUP{n}")).or_default().hidden = false;
                    }
                }
                None => {
                    for tag in ["LBL_BACK", "LBL_CHAR", "PB_VIT", "PB_FORCE"] {
                        hide(&mut patches, format!("{tag}{n}"));
                    }
                }
            }
        }
        match target {
            Some((name, hp, max)) => {
                patches.entry("LBL_NAME".to_string()).or_default().text = Some(name.to_string());
                patches.entry("PB_HEALTH".to_string()).or_default().value = Some((hp, max));
                patches.entry("LBL_ACTION0".to_string()).or_default().fill = Some(icon.to_string());
                patches
                    .entry("LBL_ACTIONDESC".to_string())
                    .or_default()
                    .text = Some(action.to_string());
                // The target's plate is its bar, with a frame.
                if self.target_at.is_none() || self.plates.iter().any(|p| p.framed) {
                    for tag in ["LBL_NAMEBG", "LBL_NAME", "LBL_HEALTHBG", "PB_HEALTH"] {
                        hide(&mut patches, tag.to_string());
                    }
                }
            }
            None => {
                for tag in [
                    "LBL_NAMEBG",
                    "LBL_NAME",
                    "LBL_HEALTHBG",
                    "PB_HEALTH",
                    "LBL_ACTION0",
                    "LBL_ACTIONDESC",
                    "LBL_ACTIONDESCBG",
                ] {
                    hide(&mut patches, tag.to_string());
                }
            }
        }
        if self.minimap.is_some() {
            hide(&mut patches, "LBL_ARROW".to_string());
        }
        self.patch_combat(&mut patches);
        patches.entry("TB_SOLO".to_string()).or_default().line = self.solo.then_some(POWER_ON_LINE);
        let blue = patches
            .entry(crate::touch::TOUCH_BUTTON.to_string())
            .or_default();
        blue.fill = Some(crate::touch::GAMEPAD_PICTURE.to_string());
        blue.line = self.touch.on.then_some(POWER_ON_LINE);
        if self.minimap_off {
            for tag in ["LBL_MAPBORDER", "LBL_MAPVIEW", "LBL_ARROW"] {
                hide(&mut patches, tag.to_string());
            }
        }
        for tag in &self.notices {
            patches.entry(tag.clone()).or_default().hidden = false;
        }
        patches
    }

    fn hud_pass(&mut self, cv: &mut Canvas, gui: &Gui, patches: &HashMap<String, Patch>) {
        // File order: later controls lie on top, as in the menus.
        for c in gui.panel.children.iter().filter(|c| c.kind != 9) {
            self.draw_control(cv, c, (0, 0), patches);
            if c.tag == "LBL_MAPVIEW" && !self.minimap_off {
                let e = &c.extent;
                self.draw_minimap(cv, [e.left, e.top, e.width, e.height]);
            }
            if c.tag == "TB_PAUSE" && self.paused {
                let [x, y, w, h] = [c.extent.left, c.extent.top, c.extent.width, c.extent.height];
                cv.fill_rect(x, y, w, 1, PAUSE_MARK);
                cv.fill_rect(x, y + h - 1, w, 1, PAUSE_MARK);
                cv.fill_rect(x, y, 1, h, PAUSE_MARK);
                cv.fill_rect(x + w - 1, y, 1, h, PAUSE_MARK);
            }
        }
        if self.paused && !self.pause_note.is_empty() {
            self.draw_pause_note(cv, &gui.panel);
        }
        // Under the cursor a slot says its key, where the video has it at 6:05.
        if let Some(n) = self.combat.hover.as_ref().map(|h| h.0) {
            let at = gui
                .find(&format!("BTN_TARGET{n}"))
                .map(|c| c.extent.clone());
            if let (Some(e), Some(words)) = (at, SLOT_WORDS.get(n)) {
                let words = format!("{} : {}", self.strref(*words), n + 1);
                let beside = if self.combat.locked { e.width + 1 } else { 22 };
                let r = [e.left + beside, e.top + e.height / 2 - 2, 200, 14];
                self.draw_text(cv, SLOT_WORDS_FONT, &words, r, 17, [0.9; 3]);
            }
        }
        for f in self.floats.clone() {
            let r = [f.x - 60, f.y, 120, 20];
            self.draw_text(cv, FLOAT_FONT, &f.text, r, 18, f.color);
        }
        self.draw_compass(cv, &gui.panel);
        self.draw_detection(cv);
        self.draw_touch(cv, &gui.panel);
    }

    /// The part of the map round the player, with an arrow that turns. The file gives the map 120 units of its 800,
    /// and they are the picture's own pixels, so the piece is the same whatever the window is.
    fn draw_minimap(&mut self, cv: &mut Canvas, r: [i32; 4]) {
        let Some(m) = self.minimap.clone() else {
            return;
        };
        cv.fill_rect(r[0], r[1], r[2], r[3], [0, 0, 0, 255]);
        if let Some(img) = self.texture_shared(&m.picture) {
            let k = img.width as f32 / MINIMAP_FILE_WIDTH;
            let side = HUD_FILE_MINIMAP * k;
            let (left, top) = (m.at[0] * k - side / 2.0, m.at[1] * k - side / 2.0);
            let (x0, y0) = (left.max(0.0), top.max(0.0));
            let x1 = (left + side).min(img.width as f32);
            let y1 = (top + side).min(img.height as f32);
            if x1 > x0 && y1 > y0 {
                let scale = (r[2] as f32 / side, r[3] as f32 / side);
                let dst = [
                    r[0] + ((x0 - left) * scale.0) as i32,
                    r[1] + ((y0 - top) * scale.1) as i32,
                    ((x1 - x0) * scale.0) as i32,
                    ((y1 - y0) * scale.1) as i32,
                ];
                let region = [x0 as i32, y0 as i32, (x1 - x0) as i32, (y1 - y0) as i32];
                cv.draw_moving(&img, region, dst);
            }
        }
        if let Some(arrow) = self.texture_shared(MINIMAP_ARROW) {
            let at = (
                r[0] as f32 + r[2] as f32 / 2.0,
                r[1] as f32 + r[3] as f32 / 2.0,
            );
            cv.draw_turned(&arrow, at, r[3] as f32 / 3.75, m.facing, [1.0; 3]);
        }
    }

    /// Bring up what the HUD hides until there is a fight.
    fn patch_combat(&self, patches: &mut HashMap<String, Patch>) {
        let combat = &self.combat;
        let mut show = |tag: String| patches.entry(tag).or_default().hidden = false;
        if combat.engaged {
            for tag in [
                "LBL_COMBATBG2",
                "LBL_COMBATBG3",
                "LBL_COMBATBG1",
                "LBL_CMBTMSGBG",
                "LBL_CMBTMODEMSG",
            ] {
                show(tag.to_string());
            }
        }
        // The queue shows with something in it, in a fight or not. The circle takes back the last thing queued (BTN_CLEARONE) and
        // the strip clears the queue (BTN_CLEARONE2). The Disengage button (BTN_CLEARALL) is never shown: the Attack mode button,
        // lit in a fight, ends it.
        if combat.queued > 0 {
            for tag in ["LBL_COMBATBG1", "BTN_CLEARONE", "BTN_CLEARONE2"] {
                show(tag.to_string());
            }
        }
        if combat.engaged || combat.queued > 0 {
            for n in 0..combat.queue.len().min(4) {
                show(format!("LBL_QUEUE{n}"));
            }
        }
        let hostile = combat.attack.is_some() && self.target_at.is_some();
        let bar = hostile || (combat.locked && self.target_at.is_some());
        if bar {
            show("BTN_TARGET0".to_string());
            show("LBL_TARGET0".to_string());
            // An enemy's first slot always has them. A lock's has them for a choice of ways.
            if !combat.locked || combat.slots[0].as_ref().is_some_and(|s| s.more) {
                show("BTN_TARGETUP0".to_string());
                show("BTN_TARGETDOWN0".to_string());
            }
        }
        if hostile {
            for tag in ["BTN_TARGET1", "BTN_TARGET2"] {
                show(tag.to_string());
            }
        }
        if combat.engaged {
            let line = patches.entry("LBL_CMBTMODEMSG".to_string()).or_default();
            line.text = Some(combat.message.clone());
            line.color = Some(COMBAT_RED);
        }
        if combat.engaged || combat.queued > 0 {
            for (n, icon) in combat.queue.iter().take(4).enumerate() {
                patches.entry(format!("LBL_QUEUE{n}")).or_default().fill = Some(icon.clone());
            }
        }
        let button = patches.entry(ATTACK_MODE_BUTTON.to_string()).or_default();
        button.hidden = false;
        button.fill = Some(ATTACK_MODE_PICTURE.to_string());
        button.line = combat.attack_mode.then_some(POWER_ON_LINE);
        if combat.pickup {
            let picture = patches.entry("LBL_ACTION0".to_string()).or_default();
            picture.hidden = false;
            picture.fill = Some(PICKUP_PICTURE.to_string());
        }
        if combat.pickup || combat.first.is_some() {
            // Its name shows under the cursor, as the other three's do.
            for tag in ["LBL_ACTIONDESC", "LBL_ACTIONDESCBG"] {
                patches.entry(tag.to_string()).or_default().hidden = true;
            }
        }
        let slots = std::iter::once(&combat.first).chain(combat.quick.iter());
        for (n, slot) in slots.enumerate() {
            let Some(shown) = slot else {
                // One with nothing in it is dim, arrows and all, as in the owner's picture of the game.
                if n > 0 {
                    for tag in [
                        format!("BTN_ACTION{n}"),
                        format!("BTN_ACTIONUP{n}"),
                        format!("BTN_ACTIONDOWN{n}"),
                    ] {
                        let frame = patches.entry(tag).or_default();
                        frame.hidden = false;
                        frame.faded = Some(EMPTY_SLOT_STRENGTH);
                    }
                }
                continue;
            };
            let picture = patches.entry(format!("LBL_ACTION{n}")).or_default();
            picture.hidden = false;
            picture.fill = Some(shown.picture.clone());
            picture.greyed = shown.greyed;
            picture.veil = (shown.veil > 0.0).then_some(shown.veil);
            if shown.lit {
                picture.line = Some(POWER_ON_LINE);
            }
            // On every slot with something in it, as in the owner's picture, with one thing or more.
            for arrow in ["UP", "DOWN"] {
                let tag = format!("BTN_ACTION{arrow}{n}");
                patches.entry(tag).or_default().hidden = false;
            }
        }
        if let Some(words) = &combat.quick_hover {
            let line = patches.entry("LBL_ACTIONDESC".to_string()).or_default();
            line.hidden = false;
            line.text = Some(words.clone());
            patches
                .entry("LBL_ACTIONDESCBG".to_string())
                .or_default()
                .hidden = false;
        }
        if bar {
            for n in 0..3 {
                if hostile {
                    patches.entry(format!("BTN_TARGET{n}")).or_default().fill =
                        Some(HOSTILE_SLOT.to_string());
                }
            }
            for (n, slot) in combat.slots.iter().enumerate() {
                if let Some(shown) = slot {
                    let picture = patches.entry(format!("LBL_TARGET{n}")).or_default();
                    picture.hidden = false;
                    picture.fill = Some(shown.picture.clone());
                    picture.greyed = shown.greyed;
                    if shown.more {
                        for arrow in ["UP", "DOWN"] {
                            let tag = format!("BTN_TARGET{arrow}{n}");
                            patches.entry(tag).or_default().hidden = false;
                        }
                    }
                }
            }
            if let Some((n, words)) = &combat.hover {
                patches.entry(format!("BTN_TARGET{n}")).or_default().fill =
                    Some(HOVER_SLOT.to_string());
                patches.entry("LBL_NAME".to_string()).or_default().text = Some(words.clone());
            }
        }
        if combat.attack.is_some() {
            patches.entry("LBL_NAME".to_string()).or_default().color = Some(COMBAT_RED);
            patches.entry("PB_HEALTH".to_string()).or_default().color = Some(COMBAT_RED);
        }
    }

    /// The icons flashing at the top of the HUD, by their tags.
    pub fn set_notices(&mut self, notices: Vec<String>) {
        self.notices = notices;
    }

    /// What the HUD shows of a fight.
    pub fn set_combat(&mut self, combat: CombatHud) {
        self.combat = combat;
    }

    /// The box at the top right, under the menu buttons.
    fn draw_pause_note(&mut self, cv: &mut Canvas, panel: &Control) {
        let lines: Vec<String> = self.pause_note.lines().map(str::to_string).collect();
        let step = 16;
        let (w, h) = (PAUSE_NOTE_WIDTH, lines.len() as i32 * step + 8);
        let top = if self.touch.on {
            self.touch_row_bottom() + 3
        } else {
            32
        };
        let top = top.max(self.compass_bottom(panel).map_or(0, |b| b + 3));
        // Left of the blue button.
        let (x, y) = (self.width as i32 - w - 6 - self.blue_button_wide(), top);
        if self.glow_on {
            crate::glow::boxed(cv, [x, y, w, h]);
        } else {
            cv.fill_rect(x, y, w, h, [0, 0, 8, 190]);
        }
        for (i, line) in lines.iter().enumerate() {
            let at = [x, y + 4 + i as i32 * step, w, step];
            self.draw_text(cv, FLOAT_FONT, line, at, 18, [0.0, 0.65, 0.97]);
        }
    }

    /// Whether the party is not following.
    pub fn set_solo(&mut self, solo: bool) {
        self.solo = solo;
    }

    /// Whether the game is paused, and what to say about it.
    pub fn set_pause(&mut self, paused: bool, note: String) {
        self.paused = paused;
        self.pause_note = note;
    }

    /// The words rising over people in a fight.
    /// The plates last given, for a test.
    pub fn plates(&self) -> &[crate::plates::Plate] {
        &self.plates
    }

    /// The slots of a target over its bar, and not under it.
    pub fn set_slots_above_bar(&mut self, above: bool) {
        self.slots_above_bar = above;
    }

    pub fn set_plates(&mut self, plates: Vec<crate::plates::Plate>) {
        self.plates = plates;
    }

    pub fn set_floats(&mut self, floats: Vec<Float>) {
        self.floats = floats;
    }

    /// The Mini Map box on the Feedback page.
    pub fn set_minimap_off(&mut self, off: bool) {
        self.minimap_off = off;
    }

    /// The small map on the HUD, if the area has one.
    pub fn set_minimap(&mut self, map: Option<MiniMap>) {
        self.minimap = map;
    }

    /// How big the target's slots are, in hundredths of their size in the file.
    pub fn set_slots_percent(&mut self, percent: i32) {
        self.slots_percent = percent;
    }

    pub fn slots_percent(&self) -> i32 {
        self.slots_percent
    }

    /// The box the target's name bar and slots take up, in layout units: left, top, right and bottom.
    pub fn target_box(&mut self) -> Option<[i32; 4]> {
        let gui = self.hud_layout(self.target_at)?;
        let parts = gui
            .panel
            .children
            .iter()
            .filter(|c| is_target_slot(&c.tag) || c.tag == "LBL_NAMEBG");
        parts
            .map(|c| &c.extent)
            .map(|e| [e.left, e.top, e.left + e.width, e.top + e.height])
            .reduce(|a, b| {
                [
                    a[0].min(b[0]),
                    a[1].min(b[1]),
                    a[2].max(b[2]),
                    a[3].max(b[3]),
                ]
            })
    }

    /// Where the target's name bar goes, in layout units: above it on the screen.
    pub fn set_target_at(&mut self, at: Option<(i32, i32)>) {
        self.target_at = at;
    }

    /// What the HUD has at a point of the layout: one of its menu buttons, or `BTN_ACTION0`.
    pub fn hud_hit(&mut self, x: f32, y: f32) -> Option<String> {
        let gui = self.hud_layout(self.target_at)?;
        let shown = |tag: &str| match tag {
            "BTN_CLEARONE" | "BTN_CLEARONE2" => self.combat.queued > 0,
            "BTN_CLEARALL" => false,
            "BTN_TARGET0" => {
                (self.combat.attack.is_some() || self.combat.locked) && self.target_at.is_some()
            }
            "BTN_TARGETUP0" | "BTN_TARGETDOWN0" => {
                let arrows =
                    !self.combat.locked || self.combat.slots[0].as_ref().is_some_and(|s| s.more);
                arrows
                    && (self.combat.attack.is_some() || self.combat.locked)
                    && self.target_at.is_some()
            }
            "BTN_TARGET1" | "BTN_TARGETUP1" | "BTN_TARGETDOWN1" => {
                self.combat.slots[1].is_some() && self.target_at.is_some()
            }
            "BTN_TARGET2" | "BTN_TARGETUP2" | "BTN_TARGETDOWN2" => {
                self.combat.slots[2].is_some() && self.target_at.is_some()
            }
            "BTN_MINIMAP" => !self.minimap_off,
            "BTN_CHAR3" => self.hud_party >= 2,
            "BTN_CHAR2" => self.hud_party >= 3,
            "BTN_ACTIONUP0" | "BTN_ACTIONDOWN0" => self.combat.first.is_some(),
            "BTN_ACTION1" => self.combat.quick[0].is_some(),
            "BTN_ACTION2" => self.combat.quick[1].is_some(),
            "BTN_ACTION3" => self.combat.quick[2].is_some(),
            "BTN_ACTIONUP1" | "BTN_ACTIONDOWN1" => self.combat.quick[0].is_some(),
            "BTN_ACTIONUP2" | "BTN_ACTIONDOWN2" => self.combat.quick[1].is_some(),
            "BTN_ACTIONUP3" | "BTN_ACTIONDOWN3" => self.combat.quick[2].is_some(),
            _ => true,
        };
        gui.panel
            .children
            .iter()
            .filter(|c| HUD_BUTTONS.contains(&c.tag.as_str()) && shown(&c.tag))
            // What is drawn last lies on top: the arrows on their slots.
            .rev()
            .find(|c| {
                let e = &c.extent;
                x >= e.left as f32
                    && x < (e.left + e.width) as f32
                    && y >= e.top as f32
                    && y < (e.top + e.height) as f32
            })
            .map(|c| c.tag.clone())
    }

    /// Where a HUD control is, in layout units: left, top, width, height.
    pub fn hud_place(&mut self, tag: &str) -> Option<[i32; 4]> {
        let gui = self.hud_layout(self.target_at)?;
        let e = &gui.find(tag)?.extent;
        Some([e.left, e.top, e.width, e.height])
    }

    /// The layout point in the middle of a HUD control.
    pub fn hud_point(&mut self, tag: &str) -> Option<(f32, f32)> {
        let gui = self.hud_layout(self.target_at)?;
        let e = &gui.find(tag)?.extent;
        Some((
            e.left as f32 + e.width as f32 / 2.0,
            e.top as f32 + e.height as f32 / 2.0,
        ))
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
                    line: m.attack_mode.then_some(POWER_ON_LINE),
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
            for n in HUD_MEMBER_SLOTS.into_iter().skip(party.len()) {
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

/// The HUD the game shows on this layout: portraits at the bottom left, actions at the bottom right, the map at the top
/// left, the menu buttons at the top right. It is written for 800x600.
const HUD_GUI: &str = "mipc28x6";
const HUD_FILE_SIZE: (i32, i32) = (800, 600);
const HUD_FILE_MINIMAP: f32 = 120.0;
/// The part of a 512 wide map picture that is the map. The grid over what has not been seen is measured from the video.
const MAP_SHOWN_WIDTH: f32 = 440.0;
const MAP_GRID: f32 = 18.0;
const MAP_GRID_LINE: [f32; 3] = [0.0, 70.0, 105.0];
const MAP_GRID_BACK: [f32; 3] = [2.0, 4.0, 22.0];
/// What the HUD does not show now: combat, notices, the action queue and the map's picture are not built.
const HUD_HIDDEN: [&str; 25] = [
    "LBL_CMBT",
    "LBL_COMBATBG",
    "LBL_MAP",
    "LBL_ARROW_MARGIN",
    "LBL_DISABLE",
    "LBL_DEBILATATED",
    "LBL_LVLUPBG",
    "LBL_LEVELUP",
    "LBL_STEALTHXP",
    "LBL_DARKSHIFT",
    "LBL_LIGHTSHIFT",
    "LBL_PLOTXP",
    "LBL_ITEM",
    "LBL_JOURNAL",
    "LBL_CASH",
    "LBL_QUEUE",
    "BTN_CLEAR",
    "BTN_TARGET",
    "LBL_TARGET",
    "BTN_ACTIONUP",
    "BTN_ACTIONDOWN",
    "TB_STEALTH",
    "LBL_ACTION1",
    "LBL_ACTION2",
    "LBL_ACTION3",
];
/// Which of the file's three places each member has, in the party's order. The file's second place is the far one: the video
/// has one companion right beside the leader at 7:30.
const HUD_MEMBER_SLOTS: [usize; 3] = [1, 3, 2];
const HUD_BUTTONS: [&str; 40] = [
    crate::touch::TOUCH_BUTTON,
    "BTN_CHAR1",
    "BTN_ACTION1",
    "BTN_ACTIONUP1",
    "BTN_ACTIONDOWN1",
    "BTN_ACTION2",
    "BTN_ACTIONUP2",
    "BTN_ACTIONDOWN2",
    "BTN_ACTION3",
    "BTN_ACTIONUP3",
    "BTN_ACTIONDOWN3",
    "BTN_EQU",
    "BTN_INV",
    "BTN_CHAR",
    "BTN_ABI",
    "BTN_MSG",
    "BTN_JOU",
    "BTN_MAP",
    "BTN_MINIMAP",
    "BTN_CLEARALL",
    "BTN_CLEARONE",
    "BTN_CLEARONE2",
    "TB_PAUSE",
    "TB_SOLO",
    ATTACK_MODE_BUTTON,
    "BTN_CHAR2",
    "BTN_CHAR3",
    "BTN_TARGET0",
    "BTN_TARGETUP0",
    "BTN_TARGETDOWN0",
    "BTN_TARGET1",
    "BTN_TARGETUP1",
    "BTN_TARGETDOWN1",
    "BTN_TARGET2",
    "BTN_TARGETUP2",
    "BTN_TARGETDOWN2",
    "BTN_OPT",
    "BTN_ACTION0",
    "BTN_ACTIONUP0",
    "BTN_ACTIONDOWN0",
];

/// Lay the HUD's file over a screen `width` units wide and 480 high, keeping its proportions: the left half stays at the
/// left and the right half at the right. **Assumed:** the game keeps these at their pixel size, which at 1080p is smaller
/// than this. The target's name bar goes over the target.
fn place_hud(
    panel: &mut Control,
    width: u32,
    target_at: Option<(i32, i32)>,
    (slots_above, slots_percent, locked): (bool, i32, bool),
) {
    let k = SCREEN.1 as f32 / HUD_FILE_SIZE.1 as f32;
    let scale = |v: i32| (v as f32 * k).round() as i32;
    for c in &mut panel.children {
        let centred = HUD_CENTRED.iter().any(|p| c.tag.starts_with(p));
        let e = &mut c.extent;
        let right = e.left + e.width / 2 >= HUD_FILE_SIZE.0 / 2;
        e.left = if centred {
            scale(e.left) + (width as i32 - SCREEN.0 as i32) / 2
        } else if right {
            width as i32 - scale(HUD_FILE_SIZE.0 - e.left)
        } else {
            scale(e.left)
        };
        e.top = scale(e.top);
        e.width = scale(e.width);
        e.height = scale(e.height);
    }
    let Some((x, y)) = target_at else { return };
    if locked {
        place_lock_button(panel, (x, y), slots_above, slots_percent);
    }
    // As thin as the video has it at 7:50, and not the file's 26 high: this sits over the fight.
    for c in &mut panel.children {
        let (down, high) = match c.tag.as_str() {
            "LBL_NAMEBG" | "LBL_NAME" => (0, BAR_NAME_HIGH),
            "LBL_HEALTHBG" | "PB_HEALTH" => (BAR_NAME_HIGH, BAR_HEALTH_HIGH),
            _ => continue,
        };
        c.extent.left = x - c.extent.width / 2;
        c.extent.top = y + down;
        c.extent.height = high;
    }
    // The slots under the bar are placed from its left and its top in the file.
    let bar = panel.children.iter().find(|c| c.tag == "LBL_NAMEBG");
    let bar = bar.map_or(0, |c| c.extent.width);
    let first = panel.children.iter().find(|c| c.tag == "BTN_TARGET0");
    let up = first.map_or(0, |c| c.extent.top) - BAR_NAME_HIGH - BAR_HEALTH_HIGH - BAR_GAP;
    if locked {
        return;
    }
    for c in &mut panel.children {
        if is_target_slot(&c.tag) {
            c.extent.left += x - bar / 2;
            c.extent.top += y - up - if slots_above { SLOTS_UP } else { 0 };
        }
    }
    scale_target_slots(panel, slots_above, slots_percent);
}

/// The button that tries a lock: under the name, in the middle of it, smaller than an enemy's slots. The owner: "Have the security
/// button and the security button with the arrows be centred underneath the name of the door or container". All of it stays
/// in the middle, though the file has the lower arrow a unit off.
fn place_lock_button(panel: &mut Control, (x, y): (i32, i32), plate: bool, percent: i32) {
    const PARTS: [&str; 4] = [
        "BTN_TARGET0",
        "BTN_TARGETUP0",
        "BTN_TARGETDOWN0",
        "LBL_TARGET0",
    ];
    let name_high = if plate {
        crate::plates::PLATE_TEXT_HIGH
    } else {
        TARGET_BAR_HIGH
    };
    let top = y + name_high + LOCK_BUTTON_GAP;
    let k = (percent * LOCK_BUTTON_PERCENT) as f32 / 10000.0;
    let scaled = |v: i32| (v as f32 * k).round() as i32;
    let Some(from) = panel.children.iter().find(|c| c.tag == PARTS[0]) else {
        return;
    };
    let from = from.extent.top;
    for c in &mut panel.children {
        if PARTS.contains(&c.tag.as_str()) {
            let e = &mut c.extent;
            e.top = top + scaled(e.top - from);
            e.width = scaled(e.width);
            e.height = scaled(e.height);
            e.left = x - e.width / 2;
        }
    }
}

fn is_target_slot(tag: &str) -> bool {
    tag.starts_with("BTN_TARGET") || tag.starts_with("LBL_TARGET")
}

/// The slots at their size, about the middle of the edge that faces the bar, so the gap to it stays.
fn scale_target_slots(panel: &mut Control, slots_above: bool, percent: i32) {
    let slots = || panel.children.iter().filter(|c| is_target_slot(&c.tag));
    let (Some(left), Some(right), Some(top), Some(bottom)) = (
        slots().map(|c| c.extent.left).min(),
        slots().map(|c| c.extent.left + c.extent.width).max(),
        slots().map(|c| c.extent.top).min(),
        slots().map(|c| c.extent.top + c.extent.height).max(),
    ) else {
        return;
    };
    let (middle, edge) = ((left + right) / 2, if slots_above { bottom } else { top });
    let k = percent as f32 / 100.0;
    let scaled = |v: i32| (v as f32 * k).round() as i32;
    for c in panel.children.iter_mut().filter(|c| is_target_slot(&c.tag)) {
        let e = &mut c.extent;
        e.left = middle + scaled(e.left - middle);
        e.top = edge + scaled(e.top - edge);
        e.width = scaled(e.width);
        e.height = scaled(e.height);
    }
}

/// The name, the health and the gap under them, in layout units.
const BAR_NAME_HIGH: i32 = 13;
const BAR_HEALTH_HIGH: i32 = 3;
const BAR_GAP: i32 = 1;
/// A target's bar with no slots, and with them.
pub const TARGET_BAR_HIGH: i32 = BAR_NAME_HIGH + BAR_HEALTH_HIGH;
pub const TARGET_BAR_AND_SLOTS_HIGH: i32 = TARGET_BAR_HIGH + BAR_GAP + 47;
/// From under the bar to over it. A plate stays where it is when it becomes the target, or the cursor on it would lose it.
const SLOTS_UP: i32 = TARGET_BAR_AND_SLOTS_HIGH + BAR_GAP;

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

/// A control and its children as `draw_control` draws them.
fn controls_drawn(
    c: &Control,
    origin: (i32, i32),
    patches: &HashMap<String, Patch>,
    out: &mut Vec<ControlDrawn>,
) {
    let patch = patches.get(&c.tag).cloned().unwrap_or_default();
    if patch.hidden {
        // Where a hidden one is makes no difference.
        out.push(ControlDrawn {
            place: [0; 4],
            patch: Patch {
                hidden: true,
                ..Patch::default()
            },
            font: None,
        });
        return;
    }
    let e = &c.extent;
    let place = [origin.0 + e.left, origin.1 + e.top, e.width, e.height];
    let font = c.text.as_ref().map(|t| t.font.clone());
    out.push(ControlDrawn { place, patch, font });
    for k in c.children.iter().filter(|k| k.kind != 9) {
        controls_drawn(k, (place[0], place[1]), patches, out);
    }
}

fn all_tags(c: &Control) -> Vec<String> {
    let mut v = vec![c.tag.clone()];
    for k in &c.children {
        v.extend(all_tags(k));
    }
    v
}

/// Whether a picture's TXI says it is added to what is under it, so that its black is see-through. The HUD's mouldings
/// are, and drawn plainly they are black boxes.
fn additive(inst: &mut Install, name: &str) -> bool {
    // These say nothing, but the reference video at 0:27 and 0:29 sees through their black.
    const SEEN_THROUGH: [&str; 4] = [
        "lbl_custpnl",
        "lbl_quickpnl",
        "lbl_qorcpnl",
        "lbl_qorcustbox",
    ];
    if SEEN_THROUGH.contains(&name) {
        return true;
    }
    inst.read(name, kotor_core::ResType::TPC)
        .ok()
        .and_then(|d| kotor_image::parse_tpc(&d).ok())
        .is_some_and(|t| t.txi.to_ascii_lowercase().contains("blending additive"))
}

/// The Pickup button's picture, drawn here as the game has none: an arrow coming down into a ring, the reach around the player.
fn pickup_picture() -> Image {
    const SIZE: u32 = 128;
    const RING: f32 = 50.0;
    const RING_THICK: f32 = 6.0;
    const SHAFT_HALF: f32 = 8.0;
    const HEAD_TOP: f32 = 12.0;
    const HEAD_HALF: f32 = 26.0;
    const TIP: f32 = 36.0;
    let centre = SIZE as f32 / 2.0;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (dx, dy) = (x as f32 + 0.5 - centre, y as f32 + 0.5 - centre);
            // How far inside each shape: the edge is smoothed over one pixel, for when it is scaled.
            let ring = RING_THICK - (dx.hypot(dy) - RING).abs();
            let shaft = (SHAFT_HALF - dx.abs()).min(HEAD_TOP - dy).min(dy + TIP);
            let across = HEAD_HALF * (TIP - dy) / (TIP - HEAD_TOP);
            let head = (across - dx.abs()).min(dy - HEAD_TOP).min(TIP - dy);
            let inside = ring.max(shaft).max(head);
            let alpha = (inside + 0.5).clamp(0.0, 1.0);
            rgba.extend_from_slice(&[225, 240, 255, (alpha * 255.0) as u8]);
        }
    }
    Image {
        width: SIZE,
        height: SIZE,
        rgba,
    }
}

/// The Security skill's picture without the frame round it: what is left is the lock, with nothing round it to see.
fn lock_glyph(from: &Image) -> Image {
    let edge = from.width * LOCK_FRAME / LOCK_FRAME_OF;
    let (width, height) = (from.width - 2 * edge, from.height - 2 * edge);
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in edge..edge + height {
        let start = ((y * from.width + edge) * 4) as usize;
        rgba.extend_from_slice(&from.rgba[start..start + (width * 4) as usize]);
    }
    Image {
        width,
        height,
        rgba,
    }
}

/// Make an added picture drawable as a plain one: how bright a pixel is becomes how much of it shows.
fn brightness_to_alpha(img: &mut Image) {
    for p in img.rgba.chunks_mut(4) {
        let most = p[0].max(p[1]).max(p[2]);
        if most == 0 {
            p[3] = 0;
            continue;
        }
        let k = 255.0 / most as f32;
        p[0] = (p[0] as f32 * k) as u8;
        p[1] = (p[1] as f32 * k) as u8;
        p[2] = (p[2] as f32 * k) as u8;
        p[3] = ((most as u32 * p[3] as u32) / 255) as u8;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lock_comes_out_of_its_frame() {
        // A frame of white, and a mark of grey in the middle.
        let (mut rgba, side) = (Vec::new(), 32u32);
        for y in 0..side {
            for x in 0..side {
                let far = x.min(y).min(side - 1 - x).min(side - 1 - y);
                rgba.extend_from_slice(&if far < LOCK_FRAME {
                    [255, 255, 255, 255]
                } else {
                    [90, 90, 90, 255]
                });
            }
        }
        let glyph = lock_glyph(&Image {
            width: side,
            height: side,
            rgba,
        });
        assert_eq!((glyph.width, glyph.height), (20, 20));
        assert!(
            glyph.rgba.chunks(4).all(|p| p[0] == 90),
            "none of the frame"
        );
    }
}
