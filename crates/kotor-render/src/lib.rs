//! Scene assembly and a software renderer. Nothing here opens a window or touches the GPU:
//! frames are rendered into memory and written as PNG, so it runs headless and doubles as a
//! reference image for a later GPU renderer.

mod character;
pub mod custom_clip;
pub mod custom_mesh;
mod json;
mod objects;
mod scene;
mod soft;
pub mod underwear;
mod walk;

pub use character::{
    load_character, load_character_with, load_creature, rig_for_appearance, rig_for_outfit, Rig,
    MOUTH_RATE,
};
pub use objects::{
    add_area_creatures, add_area_objects, add_area_objects_except, add_area_objects_with,
    CreatureStats, ObjectStats,
};
pub use scene::{is_additive, load_area, load_texture, mark_additive, Scene, SceneMesh, SkinCtx};
pub use soft::{render, Camera};
pub use walk::{contact_sheet, render_walk, walk_path, Floor, Step, WalkOptions};
