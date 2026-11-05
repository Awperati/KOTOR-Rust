//! Texture readers: TGA and TPC (the game's own texture container with DXT
//! compression), decoded to RGBA8, plus a small PNG writer for inspection.

mod dxt;
mod png;
mod tga;
mod tpc;

pub use png::{parse_png, to_png};
pub use tga::parse_tga;
pub use tpc::{parse_tpc, Tpc, TpcFormat};

/// Decoded image: `width * height * 4` bytes, RGBA, first row at the top.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
