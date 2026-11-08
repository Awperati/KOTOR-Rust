//! The game world without graphics: objects, a clock, per-object action queues, delayed commands,
//! the routines scripts call, and the dialogue runner. It reads the installed game and never writes it.

mod aim;
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
mod game_mode;
mod gear;
mod host;
mod items;
mod journal;
mod k2feats;
mod labels;
mod law;
mod lift;
mod medication;
pub mod quest_route;
mod respawn;
mod slots;
mod soundset;
pub use medication::{MedicationTable, Sickness, UseOutcome, UseRequest};
mod stance;
mod story_branches;
pub use story_branches::{
    StoryState, ABANDONED_CLIP, BANDON_BLOWS, BANDON_FULL_HEALTH, BANDON_HEALTH, STORY_POINTS,
    TRASK_DAMAGE,
};
mod volley;
pub use items::name_is_a_tag;
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
mod script_probe;
pub mod seamless;
pub mod seamless_walk;
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

pub use aim::{sway_path, AimHit, AIM_REACH};
pub use area_map::{AreaMap, MapNote, SEEN_COLS, SEEN_ROWS};
pub use autopilot::{lead_with_the_best_at, state_digest, Autopilot, FightResult, Stall};
pub use chargen::{
    attribute_cost, BodySize, CharacterLook, CharacterSpec, ABILITY_NAMES, ATTRIBUTE_BASE,
    ATTRIBUTE_MAX, POINT_POOL,
};
pub use combat::{
    ability_mod, damage, resolve_attack, AttackLog, CombatState, Outcome, Weapon,
    HEARTBEAT_SECONDS, MELEE_REACH, RANGED_REACH, ROUND_SECONDS,
};
pub use combat_actions::{CombatAction, CombatChoice};
pub use cutscene::{
    sound_file, Beat, CameraShot, Conversation, LengthFrom, Lookup, Player, Step, Timeline,
};
pub use dialogue::{Chooser, FirstReply, Line, LineKind};
pub use earlyforce::FORCE_PUSH;
pub use flight::{Flight, InFlight, RELEASE_SECONDS, THROW_SPEED};
pub use follow::{CREATURE_RADIUS, FOLLOW_START, FOLLOW_STOP, REPLAN_SECONDS};
pub use force::{push_metres, Thud};
pub use force::{SpellContext, DURATION_INSTANT, DURATION_PERMANENT, DURATION_TEMPORARY};
pub use galaxy::{GalaxyOpen, PlanetLine};
pub use game::item_use_clip as item_use_clip_for_test;
pub use game::{Clip, Game, Pose};
pub use game_mode::GameMode;
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

/// The models of the rooms and what stands in them, which the game logic does not load: a window that has them answers for
/// what they stop.
pub trait Barriers {
    /// Whether any surface crosses the upright sheet standing on the line from `a` to `b`, between heights `low` and `high`.
    /// A sheet, not a ray: a thin rail is hard to hit with a ray.
    fn across(&self, a: [f32; 2], b: [f32; 2], low: f32, high: f32) -> bool;
    /// How far over `at` the first surface is, within `reach` metres.
    fn above(&self, at: [f32; 3], reach: f32) -> Option<f32>;
    /// The outlines, as lines on the floor, of what stands in the sheet over open floor: where `floor(x, y, max_z)` gives the
    /// walkmesh's floor, a line is kept when there is floor on both of its sides. Nothing by default.
    fn outline(&self, _floor: &dyn Fn(f32, f32, f32) -> Option<f32>) -> Vec<([f32; 3], [f32; 3])> {
        Vec::new()
    }
}
pub use law::{KillKind, LawState, LawTable};
pub use lift::{ride_between, LiftRide, LiftWay, DEFAULT_SECONDS as LIFT_SECONDS};
pub use party::{BenchedMember, PartyRequest, PARTY_SIZE};
pub use pazaak::{PazaakCard, PazaakGame, PazaakMove, PazaakSide, PazaakView};
pub use play::{
    pixel_ray, DOOR_ANIM_SECONDS, DOOR_BLOCK_RADIUS, DOOR_RANGE, RUN_SPEED, TALK_RANGE, USE_RANGE,
};
pub use quest_route::{NextStep, StepKind};
pub use respawn::{Pending, RespawnState, RespawnTable, SpawnPoint};
pub use rules::{
    save_passes, skill_passes, Check, DeathPhase, LevelUp, Save, DEATH_FADE_SECONDS,
    DEATH_FALL_SECONDS,
};
pub use save::{read_save_info, SaveInfo, SAVE_VERSION};
pub use seamless_world::{
    Attached, Crossing, Figure, Scenery, State as SeamlessState, LET_GO, THRESHOLD,
};
pub use sheet::{CannotUse, Sheet};
pub use slots::{PowerSlot, SlotsTable};
pub use stealth::StealthXp;
pub use store::{CannotTrade, StoreLine, StoreOpen};
pub use swoop::{
    FollowerFile, GunBankFile, HitBox, MiniGameFile, Obstacle, ObstacleFile, PlayerExtras, Race,
    Rider,
};
pub use traps::{Disarm, Trap};
pub use volley::{fire_times, is_ranged_attack};
pub use workbench::BenchKind;
pub use world::HoldInPlace;
pub use world::{Action, Delayed, DoorState, EffectRec, Kind, ObjId, Object, World};
