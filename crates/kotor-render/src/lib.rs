//! Scene assembly and a software renderer. Nothing here opens a window or touches the GPU:
//! frames are rendered into memory and written as PNG, so it runs headless and doubles as a
//! reference image for a later GPU renderer.

mod character;
pub mod custom_clip;
pub mod custom_mesh;
pub mod face;
pub mod holster;
mod json;
pub mod marks;
mod objects;
pub mod races;
mod scene;
pub mod shape;
mod soft;
pub mod underwear;
mod walk;

pub use character::{
    load_character, load_character_with, load_creature, mouth_at, mouth_from_lip, parse_lip,
    rig_for_appearance, rig_for_outfit, rig_for_outfit_with_head, rig_for_race_body, Rig,
    LIP_SHAPE_OPEN, MOUTH_RATE, TALK_PEAK, VOICE_STEPS,
};
pub use objects::{
    add_area_creatures, add_area_objects, add_area_objects_except, add_area_objects_with,
    CreatureStats, ObjectStats,
};
pub use scene::{is_additive, load_area, load_texture, mark_additive, Scene, SceneMesh, SkinCtx};
pub use soft::{render, Camera};
pub use walk::{contact_sheet, render_walk, walk_path, Floor, Step, WalkOptions};
