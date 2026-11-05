//! The screens over the game, as a child of the stage: which one is open, what it shows, and what its buttons do.
//! Drawing and hit tests are `kotor_ui::menus`.

use super::{Cue, Stage};
use kotor_game::{
    read_save_info, CannotEquip, ItemView, ObjId, SaveInfo, SLOT_BELT, SLOT_BODY, SLOT_HANDS,
    SLOT_HEAD, SLOT_IMPLANT, SLOT_LEFT_ARM, SLOT_LEFT_HAND, SLOT_RIGHT_ARM, SLOT_RIGHT_HAND,
};
use kotor_ui::{Hit, MenuView, Row};
use std::path::{Path, PathBuf};

/// Longest name a save can be given, and the characters the interface font has.
const SAVE_NAME_MAX: usize = 32;
/// The interface's narrow font, for text the wide button font would not fit.
const SMALL_FONT: &str = "fnt_d16x16";
/// Strings of the save screen in `dialog.tlk`.
const STR_LOAD_GAME: u32 = 1585;
const STR_SAVE_GAME: u32 = 1588;

/// The equipment screen's slots: button, the label that shows the item, slot, and the name shown above them.
const EQUIP_SLOTS: [(&str, &str, i32, &str); 9] = [
    ("BTN_INV_HEAD", "LBL_INV_HEAD", SLOT_HEAD, "Head"),
    (
        "BTN_INV_IMPLANT",
        "LBL_INV_IMPLANT",
        SLOT_IMPLANT,
        "Implant",
    ),
    ("BTN_INV_BODY", "LBL_INV_BODY", SLOT_BODY, "Body"),
    ("BTN_INV_ARM_L", "LBL_INV_ARM_L", SLOT_LEFT_ARM, "Left arm"),
    (
        "BTN_INV_WEAP_L",
        "LBL_INV_WEAP_L",
        SLOT_LEFT_HAND,
        "Left hand",
    ),
    ("BTN_INV_BELT", "LBL_INV_BELT", SLOT_BELT, "Belt"),
    (
        "BTN_INV_WEAP_R",
        "LBL_INV_WEAP_R",
        SLOT_RIGHT_HAND,
        "Right hand",
    ),
    (
        "BTN_INV_ARM_R",
        "LBL_INV_ARM_R",
        SLOT_RIGHT_ARM,
        "Right arm",
    ),
    ("BTN_INV_HANDS", "LBL_INV_HANDS", SLOT_HANDS, "Hands"),
];
const STR_CANNOT_EQUIP: u32 = 38450;

/// A screen. The last one on the stack has the input; closing it shows the one under it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Screen {
    Main,
    Pause,
    Saves(Mode),
    /// The name dialog over the save screen.
    Name,
    Inventory,
    Equip,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Mode {
    Load,
    Save,
}

impl Screen {
    fn name(self) -> &'static str {
        match self {
            Screen::Main => "main",
            Screen::Pause => "pause",
            Screen::Saves(Mode::Load) => "load",
            Screen::Saves(Mode::Save) => "save",
            Screen::Name => "name",
            Screen::Inventory => "inventory",
            Screen::Equip => "equip",
        }
    }
}

struct SaveFile {
    path: PathBuf,
    info: SaveInfo,
}

#[derive(Default)]
pub(super) struct Menus {
    pub stack: Vec<Screen>,
    hover: Option<Hit>,
    message: String,
    /// Exit was pressed: the window should close.
    pub exit: bool,
    /// Where saves live; `None` when the system does not say where local data goes.
    saves_dir: Option<PathBuf>,
    /// Saves, newest first.
    saves: Vec<SaveFile>,
    /// The chosen row of the save list, and the first row shown.
    selected: Option<usize>,
    first: usize,
    /// Delete was pressed once on this file.
    delete_armed: Option<PathBuf>,
    /// The name being typed.
    name: String,
    /// The equipment slot chosen: the list shows what fits it.
    slot: Option<i32>,
    /// The item whose words are on show, and the slot under the cursor.
    about: Option<ObjId>,
    hover_slot: Option<i32>,
    /// The last item clicked could not go on.
    refused: bool,
}

/// `%LOCALAPPDATA%\kotor-rs\saves`, outside the install and the repository.
pub fn default_saves_dir() -> Option<PathBuf> {
    Some(
        PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
            .join("kotor-rs")
            .join("saves"),
    )
}

/// The save files in a folder, newest first. A file this engine cannot read is left out.
fn list_saves(dir: &Path) -> Vec<SaveFile> {
    let mut found: Vec<(std::time::SystemTime, SaveFile)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let path = e.ok()?.path();
            if !path.extension()?.eq_ignore_ascii_case("sav") {
                return None;
            }
            let info = read_save_info(&path).ok()?;
            let when = std::fs::metadata(&path).ok()?.modified().ok()?;
            Some((when, SaveFile { path, info }))
        })
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.path.cmp(&b.1.path)));
    found.into_iter().map(|f| f.1).collect()
}

fn played(seconds: f32) -> String {
    let m = (seconds / 60.0) as u32;
    format!("{}:{:02}", m / 60, m % 60)
}

impl Stage {
    /// Whether a screen is up, which also stops the game's clock.
    pub fn menu_open(&self) -> bool {
        !self.menus.stack.is_empty()
    }

    /// The name of the screen on top, for a test driver: `main`, `pause`, `load`, `save`, `name`; `-` in the game.
    pub fn menu_name(&self) -> &'static str {
        self.menus.stack.last().map_or("-", |s| s.name())
    }

    /// Exit was pressed on the main menu.
    pub fn wants_exit(&self) -> bool {
        self.menus.exit
    }

    /// The line at the foot of the screen.
    pub fn menu_message(&self) -> &str {
        &self.menus.message
    }

    /// How many saves the save screen lists.
    pub fn saves_listed(&self) -> usize {
        self.menus.saves.len()
    }

    /// Keep saves in another folder. For tests.
    pub fn set_saves_dir(&mut self, dir: PathBuf) {
        self.menus.saves_dir = Some(dir);
    }

    pub(super) fn menu_view(&mut self) -> Option<MenuView> {
        let top = *self.menus.stack.last()?;
        let hover = self.menus.hover.clone();
        let mut v = match top {
            Screen::Main => main_menu(),
            Screen::Pause => pause_menu(),
            Screen::Saves(mode) => self.saves_view(mode),
            Screen::Name => {
                let mode = Mode::Save;
                let mut under = self.saves_view(mode);
                let mut d = MenuView::new("savename");
                d.text("EDITBOX", format!("{}_", self.menus.name));
                d.hover = hover.clone();
                under.dialog = Some(Box::new(d));
                under
            }
            Screen::Inventory => self.inventory_view(),
            Screen::Equip => self.equip_view(),
        };
        if top != Screen::Name {
            v.hover = hover;
        }
        if !self.menus.message.is_empty() {
            v.message = self.menus.message.clone();
        }
        Some(v)
    }

    /// `saveload.gui`. There is no thumbnail of the scene yet, so `LBL_SCREENSHOT` stays dark.
    fn saves_view(&mut self, mode: Mode) -> MenuView {
        let mut v = MenuView::new("saveload");
        v.hide(&["LBL_SCREENSHOT"]);
        let title = match mode {
            Mode::Load => STR_LOAD_GAME,
            Mode::Save => STR_SAVE_GAME,
        };
        let title = self.ui.strref(title);
        v.text("LBL_PANELNAME", title);
        v.text(
            "BTN_SAVELOAD",
            match mode {
                Mode::Load => "Load",
                Mode::Save => "Save",
            },
        );
        let selected = self.menus.selected;
        let mut rows = Vec::new();
        if mode == Mode::Save {
            rows.push(Row {
                text: "New save".to_string(),
                selected: selected == Some(0),
                ..Row::default()
            });
        }
        let shift = rows.len();
        for (i, f) in self.menus.saves.iter().enumerate() {
            rows.push(Row {
                text: format!(
                    "{}\n{}  {}",
                    f.info.name,
                    f.info.area,
                    played(f.info.seconds)
                ),
                selected: selected == Some(i + shift),
                font: Some(SMALL_FONT.to_string()),
                ..Row::default()
            });
        }
        v.lists
            .insert("LB_GAMES".to_string(), (rows, self.menus.first));
        let file = selected
            .and_then(|s| s.checked_sub(shift))
            .and_then(|s| self.menus.saves.get(s));
        match file {
            Some(f) => {
                // Area names read "Planet - Area".
                let (planet, area) = f
                    .info
                    .area
                    .split_once(" - ")
                    .unwrap_or((f.info.module.as_str(), f.info.area.as_str()));
                v.text("LBL_PLANETNAME", planet);
                v.text("LBL_AREANAME", area);
                v.font("LBL_PLANETNAME", SMALL_FONT);
                v.font("LBL_AREANAME", SMALL_FONT);
                for (n, tag) in ["LBL_PM1", "LBL_PM2", "LBL_PM3"].iter().enumerate() {
                    match f.info.portraits.get(n) {
                        Some(p) if !p.is_empty() => v.fill(tag, p.clone()),
                        _ => v.hide(&[tag]),
                    }
                }
            }
            None => {
                v.hide(&[
                    "LBL_PLANETNAME",
                    "LBL_AREANAME",
                    "LBL_PM1",
                    "LBL_PM2",
                    "LBL_PM3",
                ]);
                v.inert(&["BTN_DELETE"]);
            }
        }
        if selected.is_none() {
            v.inert(&["BTN_SAVELOAD"]);
        }
        v
    }

    /// The first picture of an item that exists.
    fn item_row(&mut self, v: &ItemView, dim: bool) -> Row {
        let icon = v
            .icons
            .iter()
            .find(|n| self.ui.texture(n).is_some())
            .cloned();
        Row {
            text: v.name.clone(),
            icon,
            count: v.stack,
            dim,
            ..Row::default()
        }
    }

    /// The items a list on the screen shows, in order.
    fn listed_items(&mut self) -> Vec<ObjId> {
        let w = &mut self.game.world;
        let pc = w.pc;
        let carried = w.carried(pc);
        match self.menus.stack.last() {
            Some(Screen::Inventory) => carried,
            Some(Screen::Equip) => {
                let slot = self.menus.slot;
                carried
                    .into_iter()
                    .filter(|i| {
                        let mask = w.slots_of(*i);
                        slot.map_or(mask != 0, |s| mask & (1 << s) != 0)
                    })
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    fn item_rows(&mut self) -> Vec<Row> {
        let pc = self.game.world.pc;
        self.listed_items()
            .into_iter()
            .filter_map(|i| {
                let v = self.game.world.item_view(i)?;
                let dim = v.slots != 0 && self.game.world.can_wear(pc, i).is_err();
                Some(self.item_row(&v, dim))
            })
            .collect()
    }

    fn portrait(&self) -> String {
        let w = &self.game.world;
        w.object(w.pc)
            .map(|o| o.portrait.clone())
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| "po_pfha2".to_string())
    }

    fn vitality(&self) -> String {
        let w = &self.game.world;
        w.object(w.pc)
            .map_or(String::new(), |o| format!("{}/{}", o.hp, o.max_hp))
    }

    /// `inventory.gui`. Party switching is not built, so its buttons are hidden; quest items and using an item are
    /// greyed.
    fn inventory_view(&mut self) -> MenuView {
        let mut v = MenuView::new("inventory");
        v.hide(&["BTN_CHANGE1", "BTN_CHANGE2"]);
        v.inert(&["BTN_QUESTITEMS", "BTN_USEITEM"]);
        let rows = self.item_rows();
        v.lists
            .insert("LB_ITEMS".to_string(), (rows, self.menus.first));
        v.fill("LBL_PORT", self.portrait());
        v.text("LBL_VIT", self.vitality());
        let w = &mut self.game.world;
        let pc = w.pc;
        v.text("LBL_DEF", w.defense_of(pc).to_string());
        v.text("LBL_CREDITS_VALUE", w.gold.to_string());
        let about = self.menus.about.and_then(|i| self.game.world.item_view(i));
        v.text(
            "LB_DESCRIPTION",
            about.map_or(String::new(), |a| a.description),
        );
        if self.menus.refused {
            v.message = self.ui.strref(STR_CANNOT_EQUIP);
        }
        v
    }

    /// `equip.gui`. Party switching is not built and a click on an item wears it at once, so the buttons for those
    /// are hidden.
    fn equip_view(&mut self) -> MenuView {
        let mut v = MenuView::new("equip");
        v.hide(&["BTN_CHANGE1", "BTN_CHANGE2", "BTN_EQUIP"]);
        let rows = self.item_rows();
        v.lists
            .insert("LB_ITEMS".to_string(), (rows, self.menus.first));
        v.fill("LBL_PORTRAIT", self.portrait());
        v.text("LBL_VITALITY", self.vitality());
        let pc = self.game.world.pc;
        let shown_slot = self.menus.hover_slot.or(self.menus.slot);
        for (_, label, slot, name) in EQUIP_SLOTS {
            if shown_slot == Some(slot) {
                v.text("LBL_SLOTNAME", name);
            }
            let w = &mut self.game.world;
            let worn = w.object(pc).and_then(|o| o.equipped.get(&slot).copied());
            let picture = worn
                .and_then(|i| w.item_view(i))
                .and_then(|view| self.item_row(&view, false).icon);
            if let Some(icon) = picture {
                v.fill(label, icon);
            }
        }
        if shown_slot.is_none() {
            v.text("LBL_SLOTNAME", "");
        }
        v.text(
            "LBL_SELECTTITLE",
            self.menus.slot.map_or("Items", |_| "Items for this slot"),
        );
        let w = &mut self.game.world;
        v.text("LBL_DEF", w.defense_of(pc).to_string());
        for (slot, attack, to_hit) in [
            (SLOT_RIGHT_HAND, "LBL_ATKR", "LBL_TOHITR"),
            (SLOT_LEFT_HAND, "LBL_ATKL", "LBL_TOHITL"),
        ] {
            match w.damage_range_of(pc, slot) {
                Some((least, most)) => {
                    v.text(attack, format!("{least}-{most}"));
                    v.text(to_hit, format!("{:+}", w.attack_bonus_of(pc, slot)));
                }
                None => v.hide(&[attack, to_hit]),
            }
        }
        // An item under the cursor shows its words where the portrait and the slots are, as the panel's own place for
        // them lies over those. **Assumed:** the game keeps them apart in some such way.
        let about = self.menus.about.and_then(|i| self.game.world.item_view(i));
        v.hide(&["LBL_CANTEQUIP"]);
        match about {
            Some(a) => {
                v.text("LB_DESC", a.description);
                v.font("LB_DESC", SMALL_FONT);
                v.hide(&[
                    "LBL_PORT_BORD",
                    "LBL_PORTRAIT",
                    "LBL_SLOTNAME",
                    "LBL_TXTBAR",
                ]);
                for (button, label, _, _) in EQUIP_SLOTS {
                    v.hide(&[button, label]);
                }
            }
            None => v.hide(&["LB_DESC"]),
        }
        if self.menus.refused {
            v.message = self.ui.strref(STR_CANNOT_EQUIP);
        }
        v
    }

    /// The I key: the inventory, or back to the game if it is up.
    pub fn open_inventory(&mut self) {
        self.open_items(Screen::Inventory);
    }

    /// The U key: the equipment, or back to the game if it is up.
    pub fn open_equip(&mut self) {
        self.open_items(Screen::Equip);
    }

    fn open_items(&mut self, screen: Screen) {
        let top = self.menus.stack.last().copied();
        if top == Some(screen) {
            self.menus.stack.pop();
            return;
        }
        let from_game = top.is_none() && self.started && !self.game.world.in_dialogue();
        let swapping = matches!(top, Some(Screen::Inventory | Screen::Equip));
        if from_game || swapping {
            self.menus.stack.pop();
            self.menus.stack.push(screen);
            self.menus.first = 0;
            self.menus.slot = None;
            self.menus.about = None;
            self.menus.hover_slot = None;
            self.menus.refused = false;
            self.menus.hover = None;
        }
    }

    /// A click on an item: wear it, in the chosen slot if one is chosen. Anything else just shows its words.
    fn use_listed_item(&mut self, row: usize) {
        let Some(item) = self.listed_items().get(row).copied() else {
            return;
        };
        self.menus.about = Some(item);
        let pc = self.game.world.pc;
        let slot = self.menus.slot;
        match self.game.world.equip(pc, item, slot) {
            Ok(_) => {
                // The cursor stays on the row, which now holds the next item.
                self.menus.about = None;
                self.menus.refused = false;
                self.menus.slot = None;
                self.menus.first = 0;
            }
            Err(CannotEquip::NotWearable) => {}
            Err(CannotEquip::BothHandsFull) => {
                self.menus.refused = false;
                self.menus.message = "A weapon in the right hand takes both hands.".to_string();
            }
            Err(_) => self.menus.refused = true,
        }
    }

    /// A click on a slot: take off what is there. An empty slot shows what fits it.
    fn use_slot(&mut self, slot: i32) {
        let pc = self.game.world.pc;
        self.menus.refused = false;
        self.menus.first = 0;
        if self.game.world.unequip(pc, slot) {
            self.menus.slot = None;
        } else {
            self.menus.slot = (self.menus.slot != Some(slot)).then_some(slot);
        }
        self.menus.about = None;
    }

    fn menu_layout_point(&self, pixel: (f32, f32), size: (u32, u32)) -> (f32, f32) {
        (
            pixel.0 * self.ui_width as f32 / size.0.max(1) as f32,
            pixel.1 * 480.0 / size.1.max(1) as f32,
        )
    }

    /// The cursor moved over a screen: light what is under it.
    pub(super) fn menu_hover(&mut self, pixel: (f32, f32), size: (u32, u32)) {
        let Some(view) = self.menu_view() else { return };
        let (x, y) = self.menu_layout_point(pixel, size);
        self.menus.hover = self.ui.menu_hit(&view, x, y);
        self.menus.hover_slot = match &self.menus.hover {
            Some(Hit::Button(t)) => EQUIP_SLOTS.iter().find(|s| s.0 == t).map(|s| s.2),
            _ => None,
        };
        if let Some(Hit::Row(_, row)) = self.menus.hover {
            if let Some(item) = self.listed_items().get(row).copied() {
                self.menus.about = Some(item);
                self.menus.refused = false;
            }
        } else if self.menus.stack.last() == Some(&Screen::Equip) {
            self.menus.about = None;
        }
    }

    /// A left click on a screen.
    pub(super) fn menu_click(&mut self, pixel: (f32, f32), size: (u32, u32)) {
        let Some(view) = self.menu_view() else { return };
        let (x, y) = self.menu_layout_point(pixel, size);
        let delete = self.menus.message.starts_with("Press Delete");
        self.menus.message.clear();
        match self.ui.menu_hit(&view, x, y) {
            Some(Hit::Button(tag)) => {
                if tag != "BTN_DELETE" || !delete {
                    self.menus.delete_armed = None;
                }
                self.press(&tag);
            }
            Some(Hit::Row(_, row)) => {
                self.menus.delete_armed = None;
                match self.menus.stack.last() {
                    Some(Screen::Inventory | Screen::Equip) => self.use_listed_item(row),
                    _ => self.menus.selected = Some(row),
                }
            }
            None => self.menus.delete_armed = None,
        }
    }

    /// What a button does.
    fn press(&mut self, tag: &str) {
        let Some(top) = self.menus.stack.last().copied() else {
            return;
        };
        match (top, tag) {
            (Screen::Main, "BTN_NEWGAME") => self.new_game(),
            (Screen::Main | Screen::Pause, "BTN_LOADGAME") => self.open_saves(Mode::Load),
            (Screen::Main, "BTN_OPTIONS") => {
                self.menus.message = "The options are not built yet.".to_string();
            }
            (Screen::Main, "BTN_EXIT") => self.menus.exit = true,
            (Screen::Pause, "BTN_SAVEGAME") => match self.why_no_save() {
                Some(why) => self.menus.message = why.to_string(),
                None => self.open_saves(Mode::Save),
            },
            (Screen::Pause, "BTN_QUIT") => self.quit_to_main(),
            (Screen::Pause, "BTN_EXIT") => {
                self.menus.stack.pop();
            }
            (Screen::Saves(_), "BTN_BACK") => {
                self.menus.stack.pop();
            }
            (Screen::Saves(mode), "BTN_SAVELOAD") => self.save_or_load(mode),
            (Screen::Saves(mode), "BTN_DELETE") => self.delete_selected(mode),
            (Screen::Name, "BTN_OK") => self.write_save(),
            (Screen::Name, "BTN_CANCEL") => {
                self.menus.stack.pop();
            }
            (Screen::Inventory, "BTN_EXIT") | (Screen::Equip, "BTN_BACK") => {
                self.menus.stack.pop();
            }
            (Screen::Equip, _) => {
                if let Some(s) = EQUIP_SLOTS.iter().find(|s| s.0 == tag) {
                    self.use_slot(s.2);
                }
            }
            _ => {}
        }
        self.menus.hover = None;
    }

    /// Enter: the screen's main button, where it has one.
    pub(super) fn menu_enter(&mut self) {
        match self.menus.stack.last().copied() {
            Some(Screen::Saves(mode)) => self.save_or_load(mode),
            Some(Screen::Name) => self.write_save(),
            _ => {}
        }
    }

    fn new_game(&mut self) {
        let module = self.first_module.clone();
        match self.start_module(&module) {
            Ok(()) => {
                self.menus.stack.clear();
                self.menus.hover = None;
            }
            Err(e) => self.menus.message = format!("Cannot start {module}: {e}"),
        }
    }

    /// Esc: close what is open, back out of a screen, or in the game open the pause menu. The main menu stays.
    pub fn escape(&mut self) {
        match self.menus.stack.last() {
            Some(Screen::Main) => {}
            Some(_) => {
                self.menus.stack.pop();
                self.menus.message.clear();
                self.menus.hover = None;
            }
            None if self.game.world.opened.is_some() => self.game.world.close_container(),
            None if self.started => self.menus.stack.push(Screen::Pause),
            None => {}
        }
    }

    /// The Exit Game button: back to the main menu, with the game gone.
    fn quit_to_main(&mut self) {
        match Stage::fresh_world(&self.dir, self.original) {
            Ok(world) => self.game = kotor_game::Game::new(world),
            Err(e) => {
                self.menus.message = e;
                return;
            }
        }
        self.forget_view();
        self.started = false;
        self.module.clear();
        self.floor = None;
        self.static_scene = Default::default();
        self.static_version += 1;
        self.effects = Default::default();
        self.cues.push(Cue::StopVoice);
        self.cues.push(Cue::StopMusic);
        self.menus.stack = vec![Screen::Main];
    }

    /// Why the game cannot be saved right now: this engine does not save a conversation or a fight in progress.
    fn why_no_save(&self) -> Option<&'static str> {
        let w = &self.game.world;
        if w.in_dialogue() {
            return Some("You cannot save during a conversation.");
        }
        let pc = w.pc;
        let party = |id: &u32| *id == pc || w.party.contains(id);
        let fighting = w.combat.target.iter().any(|(a, t)| party(a) || party(t));
        fighting.then_some("You cannot save during combat.")
    }

    fn open_saves(&mut self, mode: Mode) {
        self.reload_saves();
        self.menus.selected = (mode == Mode::Save).then_some(0);
        self.menus.first = 0;
        self.menus.delete_armed = None;
        self.menus.stack.push(Screen::Saves(mode));
    }

    fn reload_saves(&mut self) {
        if self.menus.saves_dir.is_none() {
            self.menus.saves_dir = default_saves_dir();
        }
        self.menus.saves = self
            .menus
            .saves_dir
            .as_deref()
            .map(list_saves)
            .unwrap_or_default();
    }

    /// The file behind the chosen row, if it has one.
    fn selected_file(&self, mode: Mode) -> Option<&SaveFile> {
        let shift = (mode == Mode::Save) as usize;
        self.menus
            .selected?
            .checked_sub(shift)
            .and_then(|i| self.menus.saves.get(i))
    }

    fn save_or_load(&mut self, mode: Mode) {
        match mode {
            Mode::Save if self.menus.selected.is_some() => {
                self.menus.name = match self.selected_file(mode) {
                    Some(f) => f.info.name.clone(),
                    None => {
                        let w = &self.game.world;
                        w.object(w.area).map(|a| a.name.clone()).unwrap_or_default()
                    }
                };
                self.menus.name.truncate(SAVE_NAME_MAX);
                self.menus.stack.push(Screen::Name);
            }
            Mode::Load => self.load_selected(),
            Mode::Save => {}
        }
    }

    fn load_selected(&mut self) {
        let Some(path) = self.selected_file(Mode::Load).map(|f| f.path.clone()) else {
            return;
        };
        if let Err(e) = self.game.world.load_from(&path) {
            self.menus.message = format!("Cannot load: {e}");
            return;
        }
        self.forget_view();
        if let Err(e) = self.enter_module() {
            self.game.world.errors.push(format!("stage: {e}"));
        }
        self.events_seen = self.game.world.events.len();
        self.started = true;
        self.yaw = self.game.world.pc_mover.heading;
        self.menus.stack.clear();
        self.menus.hover = None;
    }

    /// The name dialog's OK: write the chosen file, or a new one, and go back to the list.
    fn write_save(&mut self) {
        let name = self.menus.name.trim().to_string();
        if name.is_empty() {
            self.menus.message = "Give the save a name.".to_string();
            return;
        }
        let Some(dir) = self.menus.saves_dir.clone() else {
            self.menus.message = "The system has no folder for local data.".to_string();
            return;
        };
        let path = match self.selected_file(Mode::Save) {
            Some(f) => f.path.clone(),
            None => (1..)
                .map(|n| dir.join(format!("save_{n:04}.sav")))
                .find(|p| !p.exists())
                .expect("a free name"),
        };
        let written = std::fs::create_dir_all(&dir)
            .map_err(|e| e.to_string())
            .and_then(|()| {
                self.game
                    .world
                    .save_to(&path, &name)
                    .map_err(|e| e.to_string())
            });
        self.menus.stack.pop();
        match written {
            Ok(()) => {
                self.menus.message = format!("Saved {name}.");
                self.reload_saves();
                self.menus.first = 0;
                let at = self.menus.saves.iter().position(|f| f.path == path);
                self.menus.selected = at.map(|i| i + 1);
            }
            Err(e) => self.menus.message = format!("Cannot save: {e}"),
        }
    }

    /// Delete asks once, then takes the file off the disk.
    fn delete_selected(&mut self, mode: Mode) {
        let Some((path, name)) = self
            .selected_file(mode)
            .map(|f| (f.path.clone(), f.info.name.clone()))
        else {
            return;
        };
        if self.menus.delete_armed.as_ref() != Some(&path) {
            self.menus.message = format!("Press Delete again to delete {name}.");
            self.menus.delete_armed = Some(path);
            return;
        }
        self.menus.delete_armed = None;
        if let Err(e) = std::fs::remove_file(&path) {
            self.menus.message = format!("Cannot delete: {e}");
        }
        self.reload_saves();
        self.menus.first = 0;
        self.menus.selected = (mode == Mode::Save).then_some(0);
    }

    /// Whether keys go to a text box.
    pub fn typing(&self) -> bool {
        matches!(self.menus.stack.last(), Some(Screen::Name))
    }

    /// Characters typed into the text box.
    pub fn type_text(&mut self, text: &str) {
        for c in text
            .chars()
            .filter(|c| !c.is_control() && (*c as u32) < 256)
        {
            if self.menus.name.chars().count() < SAVE_NAME_MAX {
                self.menus.name.push(c);
            }
        }
    }

    pub fn backspace(&mut self) {
        self.menus.name.pop();
    }

    /// The wheel over a list.
    pub fn menu_wheel(&mut self, lines: f32) {
        let Some(view) = self.menu_view() else { return };
        let Some((tag, (rows, _))) = view.lists.iter().next() else {
            return;
        };
        let fits = self.ui.menu_rows_shown(&view.gui, tag);
        let last = rows.len().saturating_sub(fits) as i32;
        self.menus.first =
            (self.menus.first as i32 - lines.signum() as i32).clamp(0, last) as usize;
    }

    /// The point on a window of `size` in the middle of a button on the screen that is up, or of a list row.
    pub fn menu_pixel(
        &mut self,
        tag: &str,
        row: Option<usize>,
        size: (u32, u32),
    ) -> Option<(f32, f32)> {
        let view = self.menu_view()?;
        let (x, y) = self.ui.menu_point(&view, tag, row)?;
        Some((
            x * size.0 as f32 / self.ui_width as f32,
            y * size.1 as f32 / 480.0,
        ))
    }

    /// The name in the text box.
    pub fn save_name_typed(&self) -> &str {
        &self.menus.name
    }

    /// The name of the save on the chosen row.
    pub fn selected_save_name(&self) -> Option<String> {
        let mode = self.menus.stack.iter().rev().find_map(|s| match s {
            Screen::Saves(m) => Some(*m),
            _ => None,
        })?;
        self.selected_file(mode).map(|f| f.info.name.clone())
    }
}

/// `mainmenu.gui`. The 3D model, the debug warp, the module list and the notice about new content stay out, and the
/// movies are greyed.
fn main_menu() -> MenuView {
    let mut v = MenuView::new("mainmenu");
    v.stretch = vec!["TGuiPanel".to_string(), "LBL_3DVIEW".to_string()];
    v.hide(&["LB_MODULES", "BTN_WARP", "LBL_NEWCONTENT"]);
    v.inert(&["BTN_MOVIES", "BTN_OPTIONS"]);
    v
}

/// `optionsingame.gui`. Load, Save, Exit Game and Close work; the option pages are not built, so they are greyed.
fn pause_menu() -> MenuView {
    let mut v = MenuView::new("optionsingame");
    v.hide(&["LB_DESC"]);
    v.inert(&[
        "BTN_GAMEPLAY",
        "BTN_FEEDBACK",
        "BTN_AUTOPAUSE",
        "BTN_GRAPHICS",
        "BTN_SOUND",
    ]);
    v
}
