//! The game world without graphics: objects, a clock, per-object action queues, delayed commands,
//! the routines scripts call, and the dialogue runner. It reads the installed game and never writes it.

mod alignment;
mod area_map;
mod autopilot;
mod chargen;
mod combat;
mod combat_actions;
mod cutscene;
mod dialogue;
mod earlyforce;
mod encounters;
mod flight;
mod follow;
mod force;
mod galaxy;
mod game;
mod gear;
mod host;
mod items;
mod journal;
mod k2feats;
mod labels;
pub use labels::ObjectLabel;
mod keymap;
mod levelup;
mod minigame;
mod movement;
mod party;
mod pazaak;
mod play;
mod rules;
mod save;
pub mod seamless;
mod seamless_world;
mod sheet;
mod stealth;
mod store;
mod swoop;
mod traps;
mod tutorial;
mod weapon_focus;
mod workbench;
mod world;

pub use area_map::{AreaMap, MapNote, SEEN_COLS, SEEN_ROWS};
pub use autopilot::{lead_with_the_best_at, state_digest, Autopilot, FightResult, Stall};
pub use chargen::{
    attribute_cost, BodySize, CharacterSpec, ABILITY_NAMES, ATTRIBUTE_BASE, ATTRIBUTE_MAX,
    POINT_POOL,
};
pub use combat::{
    ability_mod, damage, resolve_attack, AttackLog, CombatState, Outcome, Weapon,
    HEARTBEAT_SECONDS, MELEE_REACH, RANGED_REACH, ROUND_SECONDS,
};
pub use combat_actions::{CombatAction, CombatChoice};
pub use cutscene::{Beat, CameraShot, Conversation, LengthFrom, Lookup, Player, Step, Timeline};
pub use dialogue::{Chooser, FirstReply, Line, LineKind};
pub use earlyforce::FORCE_PUSH;
pub use flight::{Flight, InFlight, RELEASE_SECONDS, THROW_SPEED};
pub use follow::{CREATURE_RADIUS, FOLLOW_START, FOLLOW_STOP, REPLAN_SECONDS};
pub use force::{SpellContext, DURATION_INSTANT, DURATION_PERMANENT, DURATION_TEMPORARY};
pub use galaxy::{GalaxyOpen, PlanetLine};
pub use game::{Clip, Game, Pose};
pub use host::RoutineStatus;
pub use items::{
    CannotEquip, ItemView, SLOTS, SLOT_BELT, SLOT_BODY, SLOT_HANDS, SLOT_HEAD, SLOT_IMPLANT,
    SLOT_LEFT_ARM, SLOT_LEFT_HAND, SLOT_RIGHT_ARM, SLOT_RIGHT_HAND,
};
pub use journal::Quest;
pub use levelup::Choice;
pub use minigame::{Bullet, Follower, GunBank, MiniGame, PlayedAnimation};
pub use movement::{
    Body, Events, Gait, Ground, Input, MovementMode, Tuning, RUN, SPRINT, SPRINT_RATIO,
};
pub use party::{BenchedMember, PartyRequest, PARTY_SIZE};
pub use pazaak::{PazaakCard, PazaakGame, PazaakMove, PazaakSide, PazaakView};
pub use play::{
    pixel_ray, DOOR_ANIM_SECONDS, DOOR_BLOCK_RADIUS, DOOR_RANGE, RUN_SPEED, TALK_RANGE, USE_RANGE,
};
pub use rules::{
    save_passes, skill_passes, Check, DeathPhase, LevelUp, Save, DEATH_FADE_SECONDS,
    DEATH_FALL_SECONDS,
};
pub use save::{read_save_info, SaveInfo, SAVE_VERSION};
pub use seamless_world::{
    Attached, Crossing, Figure, Scenery, State as SeamlessState, LET_GO, THRESHOLD,
};
pub use sheet::{CannotUse, Sheet};
pub use stealth::StealthXp;
pub use store::{CannotTrade, StoreLine, StoreOpen};
pub use swoop::{
    FollowerFile, GunBankFile, HitBox, MiniGameFile, Obstacle, ObstacleFile, PlayerExtras, Race,
    Rider,
};
pub use traps::{Disarm, Trap};
pub use workbench::BenchKind;
pub use world::{Action, Delayed, DoorState, EffectRec, Kind, ObjId, Object, World};
