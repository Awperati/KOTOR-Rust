//! The screens over the game, as a child of the stage: which one is open, what it shows, and what its buttons do.
//! Drawing and hit tests are `kotor_ui::menus`.

use super::{Cue, Stage};
use kotor_game::{read_save_info, SaveInfo};
use kotor_ui::{Hit, MenuView, Row};
use std::path::{Path, PathBuf};

/// Longest name a save can be given, and the characters the interface font has.
const SAVE_NAME_MAX: usize = 32;
/// The interface's narrow font, for text the wide button font would not fit.
const SMALL_FONT: &str = "fnt_d16x16";
/// Strings of the save screen in `dialog.tlk`.
const STR_LOAD_GAME: u32 = 1585;
const STR_SAVE_GAME: u32 = 1588;

/// A screen. The last one on the stack has the input; closing it shows the one under it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Screen {
    Main,
    Pause,
    Saves(Mode),
    /// The name dialog over the save screen.
    Name,
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
        };
        if top != Screen::Name {
            v.hover = hover;
        }
        v.message = self.menus.message.clone();
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
                self.menus.selected = Some(row);
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
            None if self.game.world.opened.is_some() => self.game.world.opened = None,
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
