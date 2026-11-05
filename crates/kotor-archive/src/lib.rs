//! Container formats: KEY/BIF (game data), ERF/MOD/SAV (module and save
//! archives) and RIM (module resource images), plus an `Install` that layers
//! them the way the game does.

mod bif;
mod erf;
mod install;
mod key;
mod source;

pub use bif::Bif;
pub use erf::{Erf, ErfEntry, ErfKind};
pub use install::{Install, ResourceId};
pub use key::{Key, KeyEntry};
pub use source::Source;
