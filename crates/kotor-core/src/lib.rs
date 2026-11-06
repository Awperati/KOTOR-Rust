//! Shared primitives for the KotOR engine crates: errors, a bounds-checked
//! little-endian reader, resource references and resource type ids.

mod codepage;
mod error;
mod reader;
mod restype;

pub use codepage::cp1252_encode;
pub use error::{Error, Result};
pub use reader::{cp1252, latin1, Reader};
pub use restype::{ResRef, ResType};

use std::path::PathBuf;

/// Location of the installed game, for runtime reads and tests.
///
/// Uses the `KOTOR_DIR` environment variable, else the default GOG path.
/// Returns `None` when the folder does not exist, so tests can skip.
pub fn install_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("KOTOR_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"E:\Games\Star Wars - KotOR"));
    dir.join("chitin.key").is_file().then_some(dir)
}

/// Where the second game is, for the few things read from it: `KOTOR2_DIR`, else the default GOG path.
/// Nothing needs it: `None` means what comes from it is not offered.
pub fn second_game_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("KOTOR2_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"E:\Games\Star Wars - KotOR2"));
    dir.join("chitin.key").is_file().then_some(dir)
}

/// Text numbers from here on are the second game's talk table, this much further on.
pub const SECOND_GAME_STRREF: u32 = 0x0100_0000;
