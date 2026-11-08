//! Container formats: KEY/BIF (game data), ERF/MOD/SAV (module and save
//! archives) and RIM (module resource images), plus an `Install` that layers
//! them the way the game does.

mod bif;
mod erf;
mod install;
mod key;
mod overrides;
mod source;
mod write;

pub use bif::Bif;
pub use erf::{Erf, ErfEntry, ErfKind};
pub use install::{Install, ResourceId};
pub use key::{Key, KeyEntry};
pub use overrides::{
    is_gff, looks_right, mods_root, note_override_problem, override_problems, override_used,
    set_mod_packs, set_override_problems_file, set_override_use, OverrideUse,
};
pub use source::Source;
pub use write::{write_archive, NewResource};
