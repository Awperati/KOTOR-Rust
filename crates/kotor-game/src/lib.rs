//! The game world without graphics: objects, a clock, per-object action queues, delayed commands,
//! the routines scripts call, and the dialogue runner. It reads the installed game and never writes it.

mod dialogue;
mod game;
mod host;
mod play;
mod world;

pub use dialogue::{Chooser, FirstReply, Line, LineKind};
pub use game::{Clip, Game, Pose};
pub use play::{pixel_ray, DOOR_BLOCK_RADIUS, DOOR_RANGE, TALK_RANGE, USE_RANGE};
pub use world::{Action, Delayed, Kind, ObjId, Object, World};
