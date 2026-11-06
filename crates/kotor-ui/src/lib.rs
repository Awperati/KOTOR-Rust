//! Interface drawing without a window: GUI files, bitmap fonts, and the dialogue box and HUD laid out and
//! software-rendered to an image. It reads the installed game and never writes it.

mod canvas;
mod font;
mod gui;
mod menus;
mod screens;

pub use canvas::{Canvas, Orient};
pub use font::{flip_rows, parse_font_txi, Font, Glyph};
pub use gui::{Border, Control, Extent, Gui, GuiText};
pub use menus::{Cell, Hit, MenuView, Row};
pub use screens::{DialogueView, HudMember, Patch, Ui, SCREEN};
