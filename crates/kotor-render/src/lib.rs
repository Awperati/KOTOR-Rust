//! Scene assembly and a software renderer. Nothing here opens a window or touches the GPU:
//! frames are rendered into memory and written as PNG, so it runs headless and doubles as a
//! reference image for a later GPU renderer.

mod character;
mod scene;
mod soft;

pub use character::load_character;
pub use scene::{load_area, load_texture, Scene, SceneMesh};
pub use soft::{render, Camera};
