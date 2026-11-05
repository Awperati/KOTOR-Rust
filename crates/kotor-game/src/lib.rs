//! The game world without graphics: objects, a clock, per-object action queues, delayed commands,
//! the routines scripts call, and the dialogue runner. It reads the installed game and never writes it.

mod dialogue;
mod follow;
mod game;
mod host;
mod play;
mod save;
mod world;

pub use dialogue::{Chooser, FirstReply, Line, LineKind};
pub use follow::{CREATURE_RADIUS, FOLLOW_START, FOLLOW_STOP, REPLAN_SECONDS};
pub use game::{Clip, Game, Pose};
pub use play::{
    pixel_ray, DOOR_ANIM_SECONDS, DOOR_BLOCK_RADIUS, DOOR_RANGE, TALK_RANGE, USE_RANGE,
};
pub use save::SAVE_VERSION;
pub use world::{Action, Delayed, DoorState, Kind, ObjId, Object, World};
