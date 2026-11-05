//! The game world without graphics: objects, a clock, per-object action queues, delayed commands,
//! the routines scripts call, and the dialogue runner. It reads the installed game and never writes it.

mod dialogue;
mod host;
mod world;

pub use dialogue::{Chooser, FirstReply, Line, LineKind};
pub use world::{Action, Delayed, Kind, ObjId, Object, World};
