//! The game world without graphics: objects, a clock, per-object action queues, delayed commands,
//! the routines scripts call, and the dialogue runner. It reads the installed game and never writes it.

mod chargen;
mod combat;
mod cutscene;
mod dialogue;
mod follow;
mod force;
mod game;
mod host;
mod play;
mod rules;
mod save;
mod world;

pub use chargen::{
    attribute_cost, BodySize, CharacterSpec, ABILITY_NAMES, ATTRIBUTE_BASE, ATTRIBUTE_MAX,
    POINT_POOL,
};
pub use combat::{
    ability_mod, damage, resolve_attack, AttackLog, CombatState, Outcome, Weapon,
    HEARTBEAT_SECONDS, MELEE_REACH, RANGED_REACH, ROUND_SECONDS,
};
pub use cutscene::{Beat, CameraShot, Conversation, LengthFrom, Lookup, Player, Step, Timeline};
pub use dialogue::{Chooser, FirstReply, Line, LineKind};
pub use follow::{CREATURE_RADIUS, FOLLOW_START, FOLLOW_STOP, REPLAN_SECONDS};
pub use force::{SpellContext, DURATION_INSTANT, DURATION_PERMANENT, DURATION_TEMPORARY};
pub use game::{Clip, Game, Pose};
pub use play::{
    pixel_ray, DOOR_ANIM_SECONDS, DOOR_BLOCK_RADIUS, DOOR_RANGE, TALK_RANGE, USE_RANGE,
};
pub use rules::{save_passes, skill_passes, Check, LevelUp, Save};
pub use save::SAVE_VERSION;
pub use world::{Action, Delayed, DoorState, EffectRec, Kind, ObjId, Object, World};
