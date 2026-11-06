//! Interface drawing without a window: GUI files, bitmap fonts, and the dialogue box and HUD laid out and
//! software-rendered to an image. It reads the installed game and never writes it.

mod canvas;
mod font;
pub mod glow;
mod gui;
mod menus;
mod screens;
mod spelling;
mod ttf;

pub use canvas::{Canvas, Orient};
pub use font::{flip_rows, parse_font_txi, Font, Glyph};
pub use gui::{Border, Control, Extent, Gui, GuiText};
pub use menus::{Cell, Hit, Mark, MenuView, Notice, Row};
pub use screens::{
    CombatHud, DialogueView, Float, HudMember, MiniMap, Patch, SlotShown, Ui, SCREEN,
};
pub use screens::{TARGET_BAR_AND_SLOTS_HIGH, TARGET_BAR_HIGH};
pub use spelling::set_australian;
pub use ttf::{Face, Sized};
