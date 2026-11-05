//! The screens over the game, as a child of the stage: which one is open, what it shows, and what its buttons do.
//! Drawing and hit tests are `kotor_ui::menus`.

use super::{Cue, Stage};
use kotor_ui::{Hit, MenuView};

/// `%LOCALAPPDATA%\kotor-rs\saves`, outside the install and the repository.
pub fn default_saves_dir() -> Option<std::path::PathBuf> {
    Some(
        std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
            .join("kotor-rs")
            .join("saves"),
    )
}

/// A screen. The last one on the stack has the input; closing it shows the one under it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Screen {
    Main,
    Pause,
}

impl Screen {
    fn name(self) -> &'static str {
        match self {
            Screen::Main => "main",
            Screen::Pause => "pause",
        }
    }
}

#[derive(Default)]
pub(super) struct Menus {
    pub stack: Vec<Screen>,
    hover: Option<Hit>,
    message: String,
    /// Exit was pressed: the window should close.
    pub exit: bool,
}

impl Stage {
    /// Whether a screen is up, which also stops the game's clock.
    pub fn menu_open(&self) -> bool {
        !self.menus.stack.is_empty()
    }

    /// The name of the screen on top, for a test driver: `main`, `pause`; `-` in the game.
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

    pub(super) fn menu_view(&mut self) -> Option<MenuView> {
        let top = *self.menus.stack.last()?;
        let mut v = match top {
            Screen::Main => main_menu(),
            Screen::Pause => pause_menu(),
        };
        v.hover = self.menus.hover.clone();
        v.message = self.menus.message.clone();
        Some(v)
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
        self.menus.message.clear();
        if let Some(Hit::Button(tag)) = self.ui.menu_hit(&view, x, y) {
            self.press(&tag);
        }
    }

    /// What a button does.
    fn press(&mut self, tag: &str) {
        let Some(top) = self.menus.stack.last().copied() else {
            return;
        };
        match (top, tag) {
            (Screen::Main, "BTN_NEWGAME") => self.new_game(),
            (Screen::Main | Screen::Pause, "BTN_LOADGAME") | (Screen::Pause, "BTN_SAVEGAME") => {
                self.menus.message = "The save screens come next.".to_string();
            }
            (Screen::Main, "BTN_OPTIONS") => {
                self.menus.message = "The options are not built yet.".to_string();
            }
            (Screen::Main, "BTN_EXIT") => self.menus.exit = true,
            (Screen::Pause, "BTN_QUIT") => self.quit_to_main(),
            (Screen::Pause, "BTN_EXIT") => {
                self.menus.stack.pop();
            }
            _ => {}
        }
        self.menus.hover = None;
    }

    /// Enter: the screen's main button, where it has one.
    pub(super) fn menu_enter(&mut self) {}

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

    /// Whether keys go to a text box. None yet.
    pub fn typing(&self) -> bool {
        false
    }

    pub fn type_text(&mut self, _text: &str) {}

    pub fn backspace(&mut self) {}

    pub fn menu_wheel(&mut self, _lines: f32) {}

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
