//! Shared primitives for the KotOR engine crates: errors, a bounds-checked
//! little-endian reader, resource references and resource type ids.

mod error;
mod reader;
mod restype;

pub use error::{Error, Result};
pub use reader::{latin1, Reader};
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
