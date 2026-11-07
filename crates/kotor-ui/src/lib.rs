//! Interface drawing without a window: GUI files, bitmap fonts, and the dialogue box and HUD laid out and
//! software-rendered to an image. It reads the installed game and never writes it.

mod canvas;
mod compass;
mod font;
pub mod glow;
mod gui;
mod menus;
mod plates;
mod screens;
mod spelling;
mod touch;
mod ttf;

pub use canvas::{Canvas, Orient};
pub use compass::{wrapped, CompassIcon, CompassMark, CompassView, COMPASS_ICONS};
pub use font::{flip_rows, parse_font_txi, Font, Glyph};
pub use gui::{Border, Control, Extent, Gui, GuiText};
pub use menus::{Cell, EndButton, Hit, Mark, MenuView, Notice, Row};
pub use plates::{Crosshair, Plate, PLATE_BAR_HIGH, PLATE_TEXT_HIGH};
pub use screens::{
    CombatHud, DialogueView, Float, HudMember, MiniMap, Patch, SlotShown, Ui, SCREEN,
};
pub use screens::{
    ATTACK_MODE_BUTTON, LOCK_PICTURE, TARGET_BAR_AND_SLOTS_HIGH, TARGET_BAR_HIGH, TITLE_CARD,
};
pub use spelling::{set_australian, shown};
pub use touch::{
    TouchBoxes, TouchKey, TouchPart, TouchView, MODERN_KEYS, TOUCH_BUTTON, TOUCH_KEYS,
};
pub use ttf::{Face, Sized};
