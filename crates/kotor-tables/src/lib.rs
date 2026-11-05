//! Table formats: 2DA (game rules and lookup tables) and TLK (string table).

mod twoda;
mod tlk;

pub use tlk::{Tlk, TlkEntry};
pub use twoda::TwoDa;
