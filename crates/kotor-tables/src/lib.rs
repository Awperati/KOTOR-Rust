//! Table formats: 2DA (game rules and lookup tables) and TLK (string table).

mod tlk;
mod twoda;

pub use tlk::{Tlk, TlkEntry};
pub use twoda::TwoDa;
