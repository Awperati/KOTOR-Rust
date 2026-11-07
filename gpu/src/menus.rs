//! The screens over the game, as a child of the stage: which one is open, what it shows, and what its buttons do.
//! Drawing and hit tests are `kotor_ui::menus`.

use super::{Cue, Stage};
use kotor_game::{
    read_save_info, CannotEquip, ItemView, ObjId, Quest, SaveInfo, SLOT_BELT, SLOT_BODY,
    SLOT_HANDS, SLOT_HEAD, SLOT_IMPLANT, SLOT_LEFT_ARM, SLOT_LEFT_HAND, SLOT_RIGHT_ARM,
    SLOT_RIGHT_HAND,
};
use kotor_ui::{CombatHud, Hit, MenuView, Row};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[path = "abilityscreen.rs"]
mod abilityscreen;
#[path = "chargenscreen.rs"]
mod chargenscreen;
#[path = "fightscreen.rs"]
mod fightscreen;
#[path = "figure.rs"]
mod figure;
#[path = "levelup.rs"]
mod levelup;
#[path = "mainscene.rs"]
mod mainscene;
#[path = "mapscreen.rs"]
mod mapscreen;
#[path = "turretscreen.rs"]
mod turretscreen;
pub(super) use fightscreen::target_slot;
#[path = "galaxyscreen.rs"]
mod galaxyscreen;
#[path = "keyscreen.rs"]
mod keyscreen;
#[path = "notices.rs"]
mod notices;
#[path = "optionpages.rs"]
mod optionpages;
#[path = "partyscreen.rs"]
mod partyscreen;
#[path = "pazaakscreen.rs"]
mod pazaakscreen;
#[path = "storescreen.rs"]
mod storescreen;
#[path = "workbenchscreen.rs"]
mod workbenchscreen;

/// Longest name a save can be given, and the characters the interface font has.
const SAVE_NAME_MAX: usize = 32;
/// What the first action slot says when it is the Pickup button, and what it says when it finds nothing. Neither is in the
/// talk table: the button is not in the game.
const PICKUP_WORDS: &str = "Pickup";
const NOTHING_NEARBY: &str = "Nothing nearby";

/// The first action slot is the Pickup button, unless the target has something else to do: attack, talk, a door.
pub(super) fn slot_is_pickup(action: Option<&str>) -> bool {
    action.is_none_or(|a| a == "Use")
}

/// A container's top two buttons, as the owner named them. The file's words are "Get Items" and "Switch To".
const TAKE_ALL: &str = "Take All";
const TAKE_SELECTED: &str = "Take Selected";
const BTN_TAKE_SELECTED: &str = "BTN_GIVEITEMS";
/// How far an arrow of a stepper goes with Shift down.
const STEP_WITH_SHIFT: i32 = 10;

/// The interface's narrow font, for text the wide button font would not fit.
const SMALL_FONT: &str = "fnt_d16x16";
/// Strings of the save screen in `dialog.tlk`.
const STR_LOAD_GAME: u32 = 1585;
/// Enemies seen this soon after the last save are the same fight. **Assumed.**
const ONE_FIGHT_SECONDS: f32 = 30.0;
const STR_WRONG_SIDE: u32 = 1512;
const STR_SAVE_GAME: u32 = 1588;

/// The picture an empty slot shows, by the slot's button: stock names, seen in the reference video at 7:05.
fn empty_slot_picture(button: &str) -> &'static str {
    match button {
        "BTN_INV_HEAD" => "ihead",
        "BTN_INV_IMPLANT" => "iimplant",
        "BTN_INV_BODY" => "iarmor",
        "BTN_INV_ARM_L" => "iforearm_l",
        "BTN_INV_ARM_R" => "iforearm_r",
        "BTN_INV_WEAP_L" => "iweap_l",
        "BTN_INV_WEAP_R" => "iweap_r",
        "BTN_INV_BELT" => "ibelt",
        _ => "ihands",
    }
}

/// `top.gui`: the row of tabs over the party's screens. `lbl_icn_*2`, the file's `HILIGHT`, is the green picture the
/// reference video shows for the screen that is up.
const TABS: [(&str, &str, &str); 8] = [
    ("BTN_EQU", "LBLH_EQU", "lbl_icn_equ"),
    ("BTN_INV", "LBLH_INV", "lbl_icn_inv"),
    ("BTN_CHAR", "LBLH_CHA", "lbl_icn_char"),
    ("BTN_ABI", "LBLH_ABI", "lbl_icn_abi"),
    ("BTN_MSG", "LBLH_MSG", "lbl_icn_msg"),
    ("BTN_JOU", "LBLH_JOU", "lbl_icn_que"),
    ("BTN_MAP", "LBLH_MAP", "lbl_icn_map"),
    ("BTN_OPT", "LBLH_OPT", "lbl_icn_opt"),
];
/// The journal's orders, each with the talk table's name.
const QUEST_SORTS: [u32; 4] = [32173, 32174, 32175, 32176];
const STR_SORT: u32 = 42566;
const STR_ACTIVE_QUESTS: u32 = 32178;
const STR_COMPLETED_QUESTS: u32 = 32177;
/// The interface's sounds, from `guisounds.2da`. **Assumed:** which goes with what, from its labels. The video was compared
/// by frames only, so none of this was heard against it.
const SOUND_ENTER: &str = "gui_scroll";
const SOUND_CLICK: &str = "gui_click";
const SOUND_CHECK: &str = "gui_check";
const SOUND_ERROR: &str = "gui_error";
const SOUND_OPEN: &str = "gui_open";
const SOUND_CLOSE: &str = "gui_close";
const SOUND_SLOT_USE: &str = "gui_actuse";
const SOUND_SLOT_SCROLL: &str = "gui_actscroll";
const SOUND_SLOT_QUEUE: &str = "gui_button";
const SOUND_ITEM_TAKE: &str = "gui_invadd";
const SOUND_ITEM_SELECT: &str = "gui_invselect";
const STR_COMBAT_MODE: u32 = 48208;
const ATTACK_PICTURE: &str = "i_attack";
const STR_CONTAINER: u32 = 393;
const STR_SELECT_ITEM: u32 = 38154;
/// What the pause menu's right side says under the cursor: the buttons the talk table has words for.
fn pause_words(tag: &str) -> Option<u32> {
    match tag {
        "BTN_LOADGAME" => Some(42300),
        "BTN_SAVEGAME" => Some(42301),
        "BTN_QUIT" => Some(42302),
        _ => None,
    }
}

/// The equipment screen's slots: button, the label that shows the item, slot, and the name shown above them.
const EQUIP_SLOTS: [(&str, &str, i32, &str); 9] = [
    ("BTN_INV_HEAD", "LBL_INV_HEAD", SLOT_HEAD, "Head"),
    (
        "BTN_INV_IMPLANT",
        "LBL_INV_IMPLANT",
        SLOT_IMPLANT,
        "Implant",
    ),
    ("BTN_INV_BODY", "LBL_INV_BODY", SLOT_BODY, "Body"),
    ("BTN_INV_ARM_L", "LBL_INV_ARM_L", SLOT_LEFT_ARM, "Left arm"),
    (
        "BTN_INV_WEAP_L",
        "LBL_INV_WEAP_L",
        SLOT_LEFT_HAND,
        "Left hand",
    ),
    ("BTN_INV_BELT", "LBL_INV_BELT", SLOT_BELT, "Belt"),
    (
        "BTN_INV_WEAP_R",
        "LBL_INV_WEAP_R",
        SLOT_RIGHT_HAND,
        "Right hand",
    ),
    (
        "BTN_INV_ARM_R",
        "LBL_INV_ARM_R",
        SLOT_RIGHT_ARM,
        "Right arm",
    ),
    ("BTN_INV_HANDS", "LBL_INV_HANDS", SLOT_HANDS, "Hands"),
];
const STR_CANNOT_EQUIP: u32 = 38450;
const STR_NONE: u32 = 363;
const STR_EQUIPPED: u32 = 32346;
const STR_OK: u32 = 1580;
const STR_CANCEL: u32 = 1581;
/// The picture of the row that leaves a slot empty, seen in the reference video at 3:36.
const NONE_PICTURE: &str = "inone";

/// A screen. The last one on the stack has the input; closing it shows the one under it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Screen {
    Main,
    Pause,
    Saves(Mode),
    /// The name dialog over the save screen.
    Name,
    Inventory,
    Equip,
    Character,
    /// `container.gui`, up for as long as the world has something open. The game runs on behind it.
    Container,
    /// `optionsmain.gui`, from the main menu.
    Options,
    Gameplay,
    Graphics,
    Sound,
    /// The level being taken: its steps, and a page for each.
    LevelUp,
    LevelAbilities,
    LevelSkills,
    LevelFeats,
    LevelPowers,
    /// `journal.gui`.
    Journal,
    /// `abilities.gui` and `messages.gui`.
    Abilities,
    Messages,
    /// `questitem.gui`, over the journal or the inventory.
    QuestItems,
    /// `store.gui`.
    Store,
    /// `galaxymap.gui`.
    Galaxy,
    /// `pazaakwager.gui`, `pazaaksetup.gui` or `pazaakgame.gui`.
    Pazaak,
    /// `confirm.gui`, asking about Solo Mode.
    Solo,
    /// `confirm.gui` again, after the death sequence: Load Game or Quit. See `deathscreen.rs`.
    GameOver,
    /// `upgradesel.gui`, `upgradeitems.gui` and `upgrade.gui`.
    BenchKinds,
    BenchItems,
    Bench,
    /// Making a character: `classsel`, `maincg` with its panels, `portcust`, `abchrgen`, `name`.
    ChargenClass,
    ChargenMain,
    ChargenPortrait,
    ChargenAttributes,
    ChargenName,
    /// `partyselection.gui`.
    Party,
    /// `map.gui`.
    Map,
    /// A tutorial pop-up over whatever is under it.
    Tutorial,
    /// `optfeedback.gui` and `optautopause.gui`.
    Feedback,
    AutoPause,
    /// `optgraphicsadv.gui`.
    GraphicsAdvanced,
    /// `optkeymapping.gui` and `optmouse.gui`.
    KeyMap,
    Mouse,
}

/// What the player chose on the option pages. Kept in a text file beside the saves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// Levels from 0 to 1.
    pub music: f32,
    pub voice: f32,
    pub effects: f32,
    /// 0 to 1; a half changes nothing.
    pub brightness: f32,
    /// Dragging the camera up tilts it the other way.
    pub invert_y: bool,
    /// Save on every change of module.
    pub autosave: bool,
    /// 0 easy, 1 normal, 2 difficult.
    pub difficulty: i32,
    /// The window's size, once it has been chosen.
    pub size: Option<(u32, u32)>,
    /// Tutorial pop-ups come up. The talk table has "Tutorial Popups" but the file has no box for it, so this is in the
    /// settings file only.
    pub tutorials: bool,
    /// The player's levels are taken as they come, with what the class recommends.
    pub auto_level: bool,
    /// The player can move in a fight.
    pub combat_movement: bool,
    /// The equipment list leaves out what cannot be worn.
    pub hide_unequippable: bool,
    pub subtitles: bool,
    /// Underwear style: Conventional, or the game's own.
    pub conventional_underwear: bool,
    /// Early Force sensitivity, for new games. A game keeps what it began with, in its save.
    pub early_force: bool,
    /// A cutscene line moves on by itself when its voice ends; off, it waits for a click or Space.
    pub auto_advance: bool,
    pub minimap: bool,
    pub floating_numbers: bool,
    /// What pauses the game by itself.
    pub pause_round: bool,
    pub pause_enemy: bool,
    pub pause_party: bool,
    pub pause_action: bool,
    pub pause_target: bool,
    /// 0 to 1; a half is as the camera has always turned.
    pub mouse_sensitivity: f32,
    /// The left button looks, and the right one clicks.
    pub reverse_buttons: bool,
    /// A frame waits for the screen.
    pub vsync: bool,
    /// All text in Noto Sans, not the game's own fonts.
    pub noto_font: bool,
    /// Text boxes black with a glowing edge, not the stock look.
    pub glow_boxes: bool,
    /// Modern movement: momentum, gravity, a faster sprint and a roll. Off is the game's own walk.
    pub movement_modern: bool,
    /// A click on the floor walks there.
    pub click_to_walk: bool,
    /// What is shown is spelt as in Australia, and not as the game's files have it.
    pub australian_english: bool,
    /// Doors between modules open onto the next area and the party walks through, with no loading in between.
    pub seamless_areas: bool,
    /// A name and health bar over each enemy in sight, and the rest of what the F6 key turns off.
    pub nameplates: bool,
    /// The same all the time for neutral creatures and for friends, and not only when the cursor is on them.
    pub nameplates_neutral: bool,
    pub nameplates_friendly: bool,
    /// Labels on what can be looted or used, on doors too, and whether what is empty still says so.
    pub object_labels: bool,
    pub door_labels: bool,
    pub hide_empty: bool,
    /// Metres, and per cent.
    pub nameplate_range: i32,
    pub nameplate_size: i32,
    /// Cel shading: 0 off, 1 subtle, 2 strong.
    pub cel_shading: i32,
    /// The level the F7 key brings back after it turned the shading off. Not kept in the settings file.
    pub cel_last: i32,
    /// How far the Pickup button reaches, in metres.
    pub pickup_radius: i32,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            music: 1.0,
            voice: 1.0,
            effects: 1.0,
            brightness: 0.5,
            invert_y: false,
            autosave: false,
            difficulty: 1,
            size: None,
            tutorials: true,
            auto_level: false,
            combat_movement: true,
            hide_unequippable: false,
            subtitles: true,
            conventional_underwear: true,
            early_force: true,
            auto_advance: true,
            minimap: true,
            floating_numbers: true,
            pause_round: false,
            pause_enemy: true,
            pause_party: true,
            pause_action: false,
            pause_target: false,
            mouse_sensitivity: 0.5,
            reverse_buttons: false,
            vsync: true,
            noto_font: true,
            glow_boxes: true,
            movement_modern: true,
            click_to_walk: true,
            australian_english: true,
            seamless_areas: true,
            nameplates: true,
            nameplates_neutral: false,
            nameplates_friendly: false,
            object_labels: true,
            door_labels: false,
            hide_empty: false,
            nameplate_range: 60,
            nameplate_size: 100,
            cel_shading: 1,
            cel_last: 1,
            pickup_radius: 3,
        }
    }
}

impl Settings {
    fn to_text(self) -> String {
        let size = self
            .size
            .map_or("-".to_string(), |(w, h)| format!("{w}x{h}"));
        format!(
            "music={}\nvoice={}\neffects={}\nbrightness={}\ninvert_y={}\nautosave={}\ndifficulty={}\nsize={size}\ntutorials={}\nauto_level={}\ncombat_movement={}\nhide_unequippable={}\nsubtitles={}\nminimap={}\nfloating_numbers={}\npause_round={}\npause_enemy={}\npause_party={}\npause_action={}\npause_target={}\nmouse_sensitivity={}\nreverse_buttons={}\nvsync={}\nconventional_underwear={}\nearly_force={}\nauto_advance={}\nnoto_font={}\nglow_boxes={}\nmovement_modern={}\nclick_to_walk={}\nseamless_areas={}\naustralian_english={}\nnameplates={}\nnameplates_neutral={}\nnameplates_friendly={}\nobject_labels={}\ndoor_labels={}\nhide_empty={}\nnameplate_range={}\nnameplate_size={}\ncel_shading={}\npickup_radius={}\n",
            self.music,
            self.voice,
            self.effects,
            self.brightness,
            self.invert_y,
            self.autosave,
            self.difficulty,
            self.tutorials,
            self.auto_level,
            self.combat_movement,
            self.hide_unequippable,
            self.subtitles,
            self.minimap,
            self.floating_numbers,
            self.pause_round,
            self.pause_enemy,
            self.pause_party,
            self.pause_action,
            self.pause_target,
            self.mouse_sensitivity,
            self.reverse_buttons,
            self.vsync,
            self.conventional_underwear,
            self.early_force,
            self.auto_advance,
            self.noto_font,
            self.glow_boxes,
            self.movement_modern,
            self.click_to_walk,
            self.seamless_areas,
            self.australian_english,
            self.nameplates,
            self.nameplates_neutral,
            self.nameplates_friendly,
            self.object_labels,
            self.door_labels,
            self.hide_empty,
            self.nameplate_range,
            self.nameplate_size,
            self.cel_shading,
            self.pickup_radius
        )
    }

    /// Anything missing or unreadable keeps its default.
    fn from_text(text: &str) -> Settings {
        let mut s = Settings::default();
        let level = |v: &str, old: f32| v.parse().map_or(old, |n: f32| n.clamp(0.0, 1.0));
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            match k {
                "music" => s.music = level(v, s.music),
                "voice" => s.voice = level(v, s.voice),
                "effects" => s.effects = level(v, s.effects),
                "brightness" => s.brightness = level(v, s.brightness),
                // Not `invert_camera`: that box was called Mouse Look, and a tick left in an old file would flip the mouse unasked.
                "invert_y" => s.invert_y = v == "true",
                "movement_modern" => s.movement_modern = v == "true",
                "click_to_walk" => s.click_to_walk = v == "true",
                "autosave" => s.autosave = v == "true",
                "difficulty" => s.difficulty = v.parse().unwrap_or(1).clamp(0, 2),
                "tutorials" => s.tutorials = v == "true",
                "auto_level" => s.auto_level = v == "true",
                "combat_movement" => s.combat_movement = v == "true",
                "hide_unequippable" => s.hide_unequippable = v == "true",
                "subtitles" => s.subtitles = v == "true",
                "conventional_underwear" => s.conventional_underwear = v == "true",
                "early_force" => s.early_force = v == "true",
                "auto_advance" => s.auto_advance = v == "true",
                "minimap" => s.minimap = v == "true",
                "floating_numbers" => s.floating_numbers = v == "true",
                "pause_round" => s.pause_round = v == "true",
                "pause_enemy" => s.pause_enemy = v == "true",
                "pause_party" => s.pause_party = v == "true",
                "pause_action" => s.pause_action = v == "true",
                "pause_target" => s.pause_target = v == "true",
                "mouse_sensitivity" => s.mouse_sensitivity = level(v, s.mouse_sensitivity),
                "reverse_buttons" => s.reverse_buttons = v == "true",
                "vsync" => s.vsync = v == "true",
                "noto_font" => s.noto_font = v == "true",
                "glow_boxes" => s.glow_boxes = v == "true",
                "seamless_areas" => s.seamless_areas = v == "true",
                "australian_english" => s.australian_english = v == "true",
                "nameplates" => s.nameplates = v == "true",
                "nameplates_neutral" => s.nameplates_neutral = v == "true",
                "nameplates_friendly" => s.nameplates_friendly = v == "true",
                "object_labels" => s.object_labels = v == "true",
                "door_labels" => s.door_labels = v == "true",
                "hide_empty" => s.hide_empty = v == "true",
                "nameplate_range" => s.nameplate_range = v.parse().unwrap_or(s.nameplate_range),
                "nameplate_size" => s.nameplate_size = v.parse().unwrap_or(s.nameplate_size),
                "cel_shading" => {
                    s.cel_shading = v.parse().unwrap_or(s.cel_shading).clamp(0, 2);
                    s.cel_last = s.cel_shading.max(1);
                }
                "pickup_radius" => s.pickup_radius = v.parse().unwrap_or(s.pickup_radius),
                "size" => {
                    s.size = v
                        .split_once('x')
                        .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
                }
                _ => {}
            }
        }
        s
    }
}

/// `%LOCALAPPDATA%\kotor-rs\settings.txt`, outside the install and the repository.
pub fn default_settings_file() -> Option<PathBuf> {
    Some(
        PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
            .join("kotor-rs")
            .join("settings.txt"),
    )
}

/// The sizes the Screen Resolution button goes through, as `docs/PLAY.md` lists them.
const SIZES: [(u32, u32); 4] = [(1280, 720), (1920, 1080), (2560, 1440), (3840, 2160)];
/// The names of the rows of `difficultyopt.2da`, from its `desc` column.
const STR_DIFFICULTY: u32 = 42335;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Mode {
    Load,
    Save,
}

impl Screen {
    fn name(self) -> &'static str {
        match self {
            Screen::Main => "main",
            Screen::Pause => "pause",
            Screen::Saves(Mode::Load) => "load",
            Screen::Saves(Mode::Save) => "save",
            Screen::Name => "name",
            Screen::Inventory => "inventory",
            Screen::Equip => "equip",
            Screen::Character => "character",
            Screen::Container => "container",
            Screen::Options => "options",
            Screen::Gameplay => "gameplay",
            Screen::Graphics => "graphics",
            Screen::Sound => "sound",
            Screen::LevelUp => "levelup",
            Screen::LevelAbilities => "level_attributes",
            Screen::LevelSkills => "level_skills",
            Screen::LevelFeats => "level_feats",
            Screen::LevelPowers => "level_powers",
            Screen::Journal => "journal",
            Screen::Abilities => "abilities",
            Screen::Messages => "messages",
            Screen::QuestItems => "quest_items",
            Screen::Store => "store",
            Screen::Galaxy => "galaxy",
            Screen::Pazaak => "pazaak",
            Screen::Solo => "solo",
            Screen::GameOver => "gameover",
            Screen::BenchKinds => "bench_kinds",
            Screen::BenchItems => "bench_items",
            Screen::Bench => "bench",
            Screen::ChargenClass => "chargen_class",
            Screen::ChargenMain => "chargen",
            Screen::ChargenPortrait => "chargen_portrait",
            Screen::ChargenAttributes => "chargen_attributes",
            Screen::ChargenName => "chargen_name",
            Screen::Party => "party",
            Screen::Map => "map",
            Screen::Tutorial => "tutorial",
            Screen::Feedback => "feedback",
            Screen::AutoPause => "autopause",
            Screen::GraphicsAdvanced => "graphics_advanced",
            Screen::KeyMap => "keymap",
            Screen::Mouse => "mouse",
        }
    }

    fn is_level(self) -> bool {
        matches!(
            self,
            Screen::LevelUp
                | Screen::LevelAbilities
                | Screen::LevelSkills
                | Screen::LevelFeats
                | Screen::LevelPowers
        )
    }
}

struct SaveFile {
    path: PathBuf,
    info: SaveInfo,
}

#[derive(Default)]
pub(super) struct Menus {
    pub stack: Vec<Screen>,
    pub(super) hover: Option<Hit>,
    message: String,
    /// Exit was pressed: the window should close.
    pub exit: bool,
    /// Where saves live; `None` when the system does not say where local data goes.
    saves_dir: Option<PathBuf>,
    /// Saves, newest first.
    saves: Vec<SaveFile>,
    /// The chosen row of the save list, and the first row shown.
    pub(super) selected: Option<usize>,
    first: usize,
    /// Delete was pressed once on this file.
    delete_armed: Option<PathBuf>,
    /// The name being typed.
    name: String,
    /// The equipment slot clicked: its list is up to choose from, with OK and Cancel.
    slot: Option<i32>,
    /// The row chosen in that list.
    chosen: Option<usize>,
    /// The item whose words are on show.
    about: Option<ObjId>,
    /// The last slot under the cursor: the list shows what fits it.
    hover_slot: Option<i32>,
    /// The rows chosen in an open container, and how many of a stack where that has been changed.
    take_chosen: BTreeSet<ObjId>,
    take_counts: BTreeMap<ObjId, i32>,
    /// Control and Shift are down: a click means something more on some screens.
    ctrl: bool,
    shift: bool,
    /// The item chosen could not go on.
    refused: bool,
    /// The level being taken.
    level: Option<levelup::Draft>,
    /// The party being chosen.
    party: Option<partyscreen::Draft>,
    map: Option<mapscreen::Shown>,
    /// Pages of tutorial pop-ups waiting, and whether they come up at all.
    tutorials: std::collections::VecDeque<u32>,
    tutorials_on: bool,
    /// The game is paused, why, and whether things pause it by themselves.
    paused: bool,
    pause_note: String,
    auto_pause_on: bool,
    /// The keys, as they were when the page opened, its filter, and the action waiting for a key.
    keys: keyscreen::KeyMap,
    keys_before: keyscreen::KeyMap,
    key_page: usize,
    key_capture: Option<&'static str>,
    /// How many screens were up when last looked at.
    stack_seen: Option<usize>,
    /// The fight's slots and rising words.
    fight: fightscreen::Fight,
    /// A shop, while one is open.
    shop: storescreen::Shop,
    /// Pazaak, while it is up.
    pazaak: pazaakscreen::Pazaak,
    /// Which figure is the big one on the first screen of making a character.
    chargen_big: usize,
    /// When a fight was last saved before, on the game's clock.
    last_fight_save: Option<f32>,
    /// The galaxy map, once it has been up.
    galaxy: Option<galaxyscreen::Galaxy>,
    /// The words of the option the cursor last rested on.
    option_words: Option<u32>,
    /// Why that option is greyed.
    option_reason: Option<&'static str>,
    /// V-Sync when its page opened, and what it should become.
    vsync_before: bool,
    wanted_vsync: Option<bool>,
    /// The icons flashing on the HUD and when they stop, and what they were last compared with.
    notices: Vec<(&'static str, f32)>,
    had: Option<notices::Had>,
    /// The journal lists completed quests, and in which of `QUEST_SORTS`.
    journal_completed: bool,
    journal_sort: usize,
    /// Which tab of the Abilities screen is up, the Messages screen shows the fight's lines, and how far back it is.
    abilities_tab: usize,
    messages_feedback: bool,
    message_back: usize,
    /// The upgrade bench as it is being used, and how far the item on it has turned.
    bench: workbenchscreen::Draft,
    bench_turn: f32,
    /// New Game goes through the character's screens, the one being made, and the one made.
    chargen_on: bool,
    pub(super) early_force_on: bool,
    chargen: Option<chargenscreen::Draft>,
    new_character: Option<kotor_game::CharacterSpec>,
    /// The mist of the screen it was made for.
    chargen_mist: Option<(Screen, crate::effects::Effects)>,
    /// The mist behind a figure, for a screen and an alignment.
    figure_mist: Option<(Screen, i32, crate::effects::Effects)>,
    /// How the body on a figure screen was fitted to its hole.
    figure_fit: Option<figure::FigureFit>,
    /// The main menu's scene, once loaded, and whether it could not be.
    main_scene: Option<mainscene::MainScene>,
    main_scene_failed: bool,
    settings: Settings,
    /// Where the settings are kept; `None` keeps them in memory, as in tests.
    settings_file: Option<PathBuf>,
    wanted_size: Option<(u32, u32)>,
}

/// `%LOCALAPPDATA%\kotor-rs\saves`, outside the install and the repository.
pub fn default_saves_dir() -> Option<PathBuf> {
    Some(
        PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
            .join("kotor-rs")
            .join("saves"),
    )
}

/// The save files in a folder, newest first. A file this engine cannot read is left out.
fn list_saves(dir: &Path) -> Vec<SaveFile> {
    let mut found: Vec<(std::time::SystemTime, SaveFile)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let path = e.ok()?.path();
            if !path.extension()?.eq_ignore_ascii_case("sav") {
                return None;
            }
            let info = read_save_info(&path).ok()?;
            let when = std::fs::metadata(&path).ok()?.modified().ok()?;
            Some((when, SaveFile { path, info }))
        })
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.path.cmp(&b.1.path)));
    found.into_iter().map(|f| f.1).collect()
}

/// What the player wears in a slot.
fn worn_in(w: &kotor_game::World, slot: i32) -> Option<ObjId> {
    w.object(w.pc).and_then(|o| o.equipped.get(&slot).copied())
}

fn played(seconds: f32) -> String {
    let m = (seconds / 60.0) as u32;
    format!("{}:{:02}", m / 60, m % 60)
}

impl Stage {
    /// Whether a screen is up, which also stops the game's clock.
    pub fn menu_open(&self) -> bool {
        self.top().is_some()
    }

    /// The screen that has the input.
    fn top(&self) -> Option<Screen> {
        let w = &self.game.world;
        let container = w.opened.is_some() && !w.in_dialogue();
        let shop = w.store.is_some() && !w.in_dialogue();
        let pazaak = w.pazaak.is_some() && !w.in_dialogue();
        self.menus
            .stack
            .last()
            .copied()
            .or(shop.then_some(Screen::Store))
            .or(pazaak.then_some(Screen::Pazaak))
            .or(container.then_some(Screen::Container))
    }

    /// A container is up, and the game goes on behind it.
    pub(super) fn container_up(&self) -> bool {
        self.top() == Some(Screen::Container)
    }

    /// The name of the screen on top, for a test driver: `main`, `pause`, `load`, `save`, `name`; `-` in the game.
    pub fn menu_name(&self) -> &'static str {
        self.top().map_or("-", Screen::name)
    }

    /// Exit was pressed on the main menu.
    pub fn wants_exit(&self) -> bool {
        self.menus.exit
    }

    /// The line at the foot of the screen.
    pub fn menu_message(&self) -> &str {
        &self.menus.message
    }

    /// How many saves the save screen lists.
    pub fn saves_listed(&self) -> usize {
        self.menus.saves.len()
    }

    /// Keep saves in another folder. For tests.
    pub fn set_saves_dir(&mut self, dir: PathBuf) {
        self.menus.saves_dir = Some(dir);
    }

    pub(super) fn menu_view(&mut self) -> Option<MenuView> {
        let top = self.top()?;
        let hover = self.menus.hover.clone();
        // A pop-up shows over what is under it, or by itself over the game.
        let under = if top == Screen::Tutorial {
            self.menus.stack.iter().rev().nth(1).copied()
        } else {
            Some(top)
        };
        let mut v = match under {
            Some(screen) => self.screen_view(screen),
            None => MenuView {
                over_scene: true,
                ..MenuView::default()
            },
        };
        if top == Screen::Tutorial {
            v.notice = Some(self.tutorial_page());
        }
        if top != Screen::Name {
            v.hover = hover;
        }
        if !self.menus.message.is_empty() {
            v.message = self.menus.message.clone();
        }
        let active = match under {
            Some(Screen::Equip) => Some("BTN_EQU"),
            Some(Screen::Inventory) => Some("BTN_INV"),
            Some(Screen::Character) => Some("BTN_CHAR"),
            Some(Screen::Abilities) => Some("BTN_ABI"),
            Some(Screen::Messages) => Some("BTN_MSG"),
            Some(Screen::Journal) => Some("BTN_JOU"),
            Some(Screen::Map) => Some("BTN_MAP"),
            Some(Screen::Pause) if self.started => Some("BTN_OPT"),
            _ => None,
        };
        if let Some(active) = active {
            let mut row = MenuView::new("top");
            for (button, label, picture) in TABS {
                let lit = matches!(&v.hover, Some(Hit::Button(t)) if t == button);
                let suffix = if button == active || lit { "2" } else { "" };
                row.fill(label, format!("{picture}{suffix}"));
            }
            v.top = Some(Box::new(row));
        }
        if let (Screen::Pause, Some(Hit::Button(tag))) = (top, &v.hover) {
            if let Some(words) = pause_words(tag) {
                let text = self.ui.strref(words);
                v.text("LB_DESC", text);
            }
        }
        Some(v)
    }

    fn screen_view(&mut self, top: Screen) -> MenuView {
        let hover = self.menus.hover.clone();
        match top {
            Screen::Tutorial => MenuView::default(),
            Screen::Main => {
                let mut v = main_menu();
                if self.main_scene_ready() {
                    v.holes.push("LBL_3DVIEW".to_string());
                    v.hole_veil = mainscene::VEIL;
                }
                v
            }
            Screen::Pause => pause_menu(),
            Screen::Saves(mode) => self.saves_view(mode),
            Screen::Name => {
                let mode = Mode::Save;
                let mut under = self.saves_view(mode);
                let mut d = MenuView::new("savename");
                d.text("EDITBOX", format!("{}_", self.menus.name));
                d.hover = hover.clone();
                under.dialog = Some(Box::new(d));
                under
            }
            Screen::Inventory => self.inventory_view(),
            Screen::Equip => self.equip_view(),
            Screen::Character => self.character_view(),
            Screen::Container => self.container_view(),
            Screen::Options => options_menu(),
            Screen::Gameplay => self.gameplay_view(),
            Screen::Graphics => self.graphics_view(),
            Screen::Sound => self.sound_view(),
            Screen::LevelUp => self.level_panel_view(),
            Screen::LevelAbilities => self.level_abilities_view(),
            Screen::LevelSkills => self.level_skills_view(),
            Screen::LevelFeats => self.level_feats_view(),
            Screen::LevelPowers => self.level_powers_view(),
            Screen::Journal => self.journal_view(),
            Screen::Abilities => self.abilities_view(),
            Screen::Messages => self.messages_view(),
            Screen::QuestItems => self.quest_items_view(),
            Screen::Store => self.store_view(),
            Screen::Galaxy => self.galaxy_view(),
            Screen::Pazaak => self.pazaak_view(),
            Screen::Solo => self.solo_view(),
            Screen::GameOver => self.game_over_view(),
            Screen::BenchKinds => self.bench_kinds_view(),
            Screen::BenchItems => self.bench_items_view(),
            Screen::Bench => self.bench_view(),
            Screen::ChargenClass => self.chargen_class_view(),
            Screen::ChargenMain => self.chargen_main_view(),
            Screen::ChargenPortrait => self.chargen_portrait_view(),
            Screen::ChargenAttributes => self.chargen_attributes_view(),
            Screen::ChargenName => self.chargen_name_view(),
            Screen::Party => self.party_view_screen(),
            Screen::Map => self.map_view(),
            Screen::Feedback => self.feedback_view(),
            Screen::AutoPause => self.autopause_view(),
            Screen::GraphicsAdvanced => self.graphics_advanced_view(),
            Screen::KeyMap => self.key_map_view(),
            Screen::Mouse => self.mouse_view(),
        }
    }

    /// `saveload.gui`. There is no thumbnail of the scene yet, so `LBL_SCREENSHOT` stays dark.
    fn saves_view(&mut self, mode: Mode) -> MenuView {
        let mut v = MenuView::new("saveload");
        v.hide(&["LBL_SCREENSHOT"]);
        let title = match mode {
            Mode::Load => STR_LOAD_GAME,
            Mode::Save => STR_SAVE_GAME,
        };
        let title = self.ui.strref(title);
        v.text("LBL_PANELNAME", title);
        v.text(
            "BTN_SAVELOAD",
            match mode {
                Mode::Load => "Load",
                Mode::Save => "Save",
            },
        );
        let selected = self.menus.selected;
        let mut rows = Vec::new();
        if mode == Mode::Save {
            rows.push(Row {
                text: "New save".to_string(),
                selected: selected == Some(0),
                ..Row::default()
            });
        }
        let shift = rows.len();
        for (i, f) in self.menus.saves.iter().enumerate() {
            rows.push(Row {
                text: format!(
                    "{}\n{}  {}",
                    f.info.name,
                    f.info.area,
                    played(f.info.seconds)
                ),
                selected: selected == Some(i + shift),
                font: Some(SMALL_FONT.to_string()),
                ..Row::default()
            });
        }
        v.lists
            .insert("LB_GAMES".to_string(), (rows, self.menus.first));
        let file = selected
            .and_then(|s| s.checked_sub(shift))
            .and_then(|s| self.menus.saves.get(s));
        match file {
            Some(f) => {
                // Area names read "Planet - Area".
                let (planet, area) = f
                    .info
                    .area
                    .split_once(" - ")
                    .unwrap_or((f.info.module.as_str(), f.info.area.as_str()));
                v.text("LBL_PLANETNAME", planet);
                v.text("LBL_AREANAME", area);
                v.font("LBL_PLANETNAME", SMALL_FONT);
                v.font("LBL_AREANAME", SMALL_FONT);
                for (n, tag) in ["LBL_PM1", "LBL_PM2", "LBL_PM3"].iter().enumerate() {
                    match f.info.portraits.get(n) {
                        Some(p) if !p.is_empty() => v.fill(tag, p.clone()),
                        _ => v.hide(&[tag]),
                    }
                }
            }
            None => {
                v.hide(&[
                    "LBL_PLANETNAME",
                    "LBL_AREANAME",
                    "LBL_PM1",
                    "LBL_PM2",
                    "LBL_PM3",
                ]);
                v.inert(&["BTN_DELETE"]);
            }
        }
        if selected.is_none() {
            v.inert(&["BTN_SAVELOAD"]);
        }
        v
    }

    /// The quests the journal lists, in the order it lists them.
    fn listed_quests(&mut self) -> Vec<Quest> {
        let completed = self.menus.journal_completed;
        let mut quests = self.game.world.quests();
        quests.retain(|q| q.completed == completed);
        match self.menus.journal_sort {
            1 => quests.sort_by(|a, b| a.name.cmp(&b.name)),
            2 => quests.sort_by_key(|q| q.priority),
            3 => quests.sort_by_key(|q| q.planet),
            _ => {}
        }
        quests
    }

    /// `journal.gui`, as the reference video has it at 51:30.
    fn journal_view(&mut self) -> MenuView {
        let mut v = MenuView::new("journal");
        let (shown, other) = if self.menus.journal_completed {
            (STR_COMPLETED_QUESTS, STR_ACTIVE_QUESTS)
        } else {
            (STR_ACTIVE_QUESTS, STR_COMPLETED_QUESTS)
        };
        let sort = self.menus.journal_sort;
        let title = format!(
            "{} - {}",
            self.ui.strref(shown),
            self.ui.strref(QUEST_SORTS[sort])
        );
        v.text("LBL_TITLE", title);
        let other = self.ui.strref(other);
        v.text("BTN_SWAPTEXT", other);
        let next = QUEST_SORTS[(sort + 1) % QUEST_SORTS.len()];
        let button = format!("{} {}", self.ui.strref(STR_SORT), self.ui.strref(next));
        v.text("BTN_SORT", button);
        let quests = self.listed_quests();
        let selected = self.menus.selected;
        let rows = quests
            .iter()
            .enumerate()
            .map(|(i, q)| Row {
                text: q.name.clone(),
                selected: selected == Some(i),
                ..Row::default()
            })
            .collect();
        v.lists
            .insert("LB_ITEMS".to_string(), (rows, self.menus.first));
        let words = match selected.and_then(|s| quests.get(s)) {
            Some(q) => match q.planet {
                Some(planet) => format!("{}:\n{}", self.ui.strref(planet), q.text),
                None => q.text.clone(),
            },
            None => String::new(),
        };
        v.text("LBL_ITEM_DESCRIPTION", words);
        v
    }

    /// The first picture of an item that exists.
    fn item_row(&mut self, v: &ItemView, dim: bool) -> Row {
        let icon = v
            .icons
            .iter()
            .find(|n| self.ui.texture(n).is_some())
            .cloned();
        Row {
            text: v.name.clone(),
            icon,
            count: v.stack,
            dim,
            ..Row::default()
        }
    }

    /// The items a list on the screen shows, in order.
    fn listed_items(&mut self) -> Vec<ObjId> {
        let top = self.top();
        let w = &mut self.game.world;
        let pc = w.pc;
        let carried = w.carried(pc);
        match top {
            Some(Screen::Container) => w.container_items().into_iter().map(|i| i.0).collect(),
            Some(Screen::Inventory) => carried,
            Some(Screen::QuestItems) => carried
                .into_iter()
                .filter(|i| w.object(*i).is_some_and(|o| o.plot))
                .collect(),
            _ => Vec::new(),
        }
    }

    /// What a slot can hold, as the equipment list has it: nothing, what is worn, then what is carried that fits.
    fn equip_choices(&mut self, slot: i32) -> Vec<Option<ObjId>> {
        let w = &mut self.game.world;
        let pc = w.pc;
        let mut choices = vec![None];
        choices.extend(worn_in(w, slot).map(Some));
        let hide = self.menus.settings.hide_unequippable;
        for item in w.carried(pc) {
            if w.slots_of(item) & (1 << slot) != 0 && !(hide && w.can_wear(pc, item).is_err()) {
                choices.push(Some(item));
            }
        }
        choices
    }

    fn equip_rows(&mut self, slot: i32) -> Vec<Row> {
        let pc = self.game.world.pc;
        let none = self.ui.strref(STR_NONE);
        let equipped = self.ui.strref(STR_EQUIPPED);
        let worn = worn_in(&self.game.world, slot);
        let picking = self.menus.slot.is_some();
        let chosen = self.menus.chosen;
        self.equip_choices(slot)
            .into_iter()
            .enumerate()
            .map(|(n, choice)| {
                let view = choice.and_then(|i| self.game.world.item_view(i));
                let mut row = match view {
                    Some(v) => {
                        let dim = self.game.world.can_wear(pc, v.id).is_err();
                        let mut row = self.item_row(&v, dim);
                        if choice == worn {
                            row.text = format!("{} ({equipped})", row.text);
                        }
                        row
                    }
                    None => Row {
                        text: none.clone(),
                        icon: Some(NONE_PICTURE.to_string()),
                        ..Row::default()
                    },
                };
                row.selected = picking && chosen == Some(n);
                row
            })
            .collect()
    }

    fn item_rows(&mut self) -> Vec<Row> {
        let pc = self.game.world.pc;
        self.listed_items()
            .into_iter()
            .filter_map(|i| {
                let v = self.game.world.item_view(i)?;
                let dim = !self.container_up()
                    && v.slots != 0
                    && self.game.world.can_wear(pc, i).is_err();
                Some(self.item_row(&v, dim))
            })
            .collect()
    }

    fn portrait(&self) -> String {
        let w = &self.game.world;
        w.object(w.pc)
            .map(|o| o.portrait.clone())
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| "po_pfha2".to_string())
    }

    fn vitality(&self) -> String {
        let w = &self.game.world;
        w.object(w.pc)
            .map_or(String::new(), |o| format!("{}/{}", o.hp, o.max_hp))
    }

    /// `questitem.gui`: what is carried for a quest, with the words of the one under the cursor, else the one clicked.
    /// **Assumed:** the file's `Plot` flag is what makes one, and they stay in the inventory's list too. The reference
    /// video was not searched for this screen.
    fn quest_items_view(&mut self) -> MenuView {
        let mut v = MenuView::new("questitem");
        let selected = self.menus.selected;
        let items = self.listed_items();
        let mut rows = self.item_rows();
        for (n, row) in rows.iter_mut().enumerate() {
            row.selected = selected == Some(n);
        }
        v.lists
            .insert("LB_ITEMS".to_string(), (rows, self.menus.first));
        let under = match self.menus.hover {
            Some(Hit::Row(_, n)) => Some(n),
            _ => None,
        };
        let about = under
            .or(selected)
            .and_then(|n| items.get(n))
            .and_then(|i| self.game.world.item_view(*i));
        v.text(
            "LB_ITEM_DESCRIPTION",
            about.map_or(String::new(), |a| a.description),
        );
        v
    }

    /// `inventory.gui`.
    fn inventory_view(&mut self) -> MenuView {
        let mut v = MenuView::new("inventory");
        self.change_buttons(&mut v);
        let about_item = self.menus.about;
        let usable = about_item
            .and_then(|i| self.game.world.item_view(i))
            .is_some_and(|a| a.power.is_some());
        if !usable {
            v.inert(&["BTN_USEITEM"]);
        }
        let rows = self.item_rows();
        v.lists
            .insert("LB_ITEMS".to_string(), (rows, self.menus.first));
        v.fill("LBL_PORT", self.portrait());
        v.text("LBL_VIT", self.vitality());
        let w = &mut self.game.world;
        let pc = w.pc;
        v.text("LBL_DEF", w.defense_of(pc).to_string());
        v.text("LBL_CREDITS_VALUE", w.gold.to_string());
        let about = self.menus.about.and_then(|i| self.game.world.item_view(i));
        v.text(
            "LB_DESCRIPTION",
            about.map_or(String::new(), |a| a.description),
        );
        v
    }

    /// `equip.gui`, as the reference video has it at 7:05 and 3:41. The list shows what fits the last slot under the
    /// cursor, to look at only. A click on a slot lists it to choose from: the words take the place of the slots, and
    /// OK and Cancel are at the foot. Party switching is not built, so its buttons are hidden.
    fn equip_view(&mut self) -> MenuView {
        let mut v = MenuView::new("equip");
        self.change_buttons(&mut v);
        let picking = self.menus.slot;
        match picking {
            Some(_) => {
                let (ok, cancel) = (self.ui.strref(STR_OK), self.ui.strref(STR_CANCEL));
                v.text("BTN_EQUIP", ok);
                v.text("BTN_BACK", cancel);
            }
            None => v.hide(&["BTN_EQUIP"]),
        }
        let shown_slot = picking.or(self.menus.hover_slot);
        let rows = shown_slot.map_or(Vec::new(), |s| self.equip_rows(s));
        v.lists
            .insert("LB_ITEMS".to_string(), (rows, self.menus.first));
        v.fill("LBL_PORTRAIT", self.portrait());
        v.text("LBL_VITALITY", self.vitality());
        let pc = self.game.world.pc;
        for (button, label, slot, name) in EQUIP_SLOTS {
            if shown_slot == Some(slot) {
                v.text("LBL_SLOTNAME", name);
            }
            v.fill(label, empty_slot_picture(button));
            let w = &mut self.game.world;
            let worn = w.object(pc).and_then(|o| o.equipped.get(&slot).copied());
            let picture = worn
                .and_then(|i| w.item_view(i))
                .and_then(|view| self.item_row(&view, false).icon);
            if let Some(icon) = picture {
                v.fill(label, icon);
            }
        }
        if shown_slot.is_none() {
            v.text("LBL_SLOTNAME", "");
        }
        let select = self.ui.strref(STR_SELECT_ITEM);
        v.text("LBL_SELECTTITLE", picking.map_or(String::new(), |_| select));
        let w = &mut self.game.world;
        v.text("LBL_DEF", w.defense_of(pc).to_string());
        for (slot, attack, to_hit) in [
            (SLOT_RIGHT_HAND, "LBL_ATKR", "LBL_TOHITR"),
            (SLOT_LEFT_HAND, "LBL_ATKL", "LBL_TOHITL"),
        ] {
            match w.damage_range_of(pc, slot) {
                Some((least, most)) => {
                    v.text(attack, format!("{least}-{most}"));
                    v.text(to_hit, format!("{:+}", w.attack_bonus_of(pc, slot)));
                }
                None => v.hide(&[attack, to_hit]),
            }
        }
        v.hide(&["LBL_CANTEQUIP"]);
        match picking {
            Some(slot) => {
                let chosen = self
                    .menus
                    .chosen
                    .and_then(|n| self.equip_choices(slot).get(n).copied().flatten());
                let shown = self.menus.about.or(chosen);
                let about = shown.and_then(|i| self.game.world.item_view(i));
                v.text("LB_DESC", about.map_or(String::new(), |a| a.description));
                v.font("LB_DESC", SMALL_FONT);
                v.hide(&[
                    "LBL_PORT_BORD",
                    "LBL_PORTRAIT",
                    "LBL_SLOTNAME",
                    "LBL_TXTBAR",
                ]);
                for (button, label, _, _) in EQUIP_SLOTS {
                    v.hide(&[button, label]);
                }
            }
            None => v.hide(&["LB_DESC"]),
        }
        if self.menus.refused {
            v.message = self.ui.strref(STR_CANNOT_EQUIP);
        }
        v
    }

    /// What the player chose on the option pages.
    /// How many words were drawn since last asked, and which did not fit their box.
    pub fn words_fit(&mut self) -> (usize, Vec<String>) {
        self.ui.words_fit()
    }

    pub fn settings(&self) -> Settings {
        self.menus.settings
    }

    /// Modern or Classic movement for this run, without touching the settings file. For a test driver.
    pub fn set_movement_modern(&mut self, modern: bool) {
        self.menus.settings.movement_modern = modern;
    }

    /// Read the player's settings from their file, and keep them there from now on.
    pub fn use_saved_settings(&mut self) {
        if let Some(path) = default_settings_file() {
            if let Some(folder) = path.parent() {
                self.use_pose_folder(folder.to_path_buf());
            }
            self.use_settings_file(path);
        }
    }

    /// Read settings from this file, and keep them there from now on.
    pub fn use_settings_file(&mut self, path: PathBuf) {
        if let Ok(text) = std::fs::read_to_string(&path) {
            self.menus.settings = Settings::from_text(&text);
            self.menus.keys.read(&text);
        }
        self.menus.wanted_vsync = Some(self.menus.settings.vsync);
        self.menus.settings_file = Some(path);
        self.settings_changed();
        self.menus.wanted_size = self.menus.settings.size;
    }

    /// What follows from a change of settings: the sound, the rules and the picture.
    pub(super) fn settings_changed(&mut self) {
        let s = self.menus.settings;
        self.cues.push(Cue::Levels(s.music, s.voice, s.effects));
        self.apply_difficulty();
        self.set_conventional_underwear(s.conventional_underwear);
        self.overlay_key.clear();
        if let Some(path) = &self.menus.settings_file {
            let written = path
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(path, s.to_text() + &self.menus.keys.to_text()));
            if let Err(e) = written {
                self.menus.message = format!("Cannot keep the settings: {e}");
            }
        }
    }

    pub(super) fn apply_difficulty(&mut self) {
        self.game.world.difficulty = self.menus.settings.difficulty;
    }

    /// The window size the Screen Resolution button asked for, once.
    pub fn take_wanted_size(&mut self) -> Option<(u32, u32)> {
        self.menus.wanted_size.take()
    }

    /// Brightness, as a layer in the overlay, which is laid over the scene with premultiplied alpha: black dims what
    /// is under it, and colour with no alpha adds to it. **Assumed:** this stands in for the game's gamma, which would
    /// need the renderer.
    pub(super) fn apply_brightness(&mut self) {
        let b = self.menus.settings.brightness;
        let Some(o) = self.overlay.as_mut().filter(|_| b != 0.5) else {
            return;
        };
        if b < 0.5 {
            let keep = 1.0 - (0.5 - b) * 1.4;
            for p in o.rgba.chunks_mut(4) {
                p[0] = (p[0] as f32 * keep) as u8;
                p[1] = (p[1] as f32 * keep) as u8;
                p[2] = (p[2] as f32 * keep) as u8;
                p[3] = 255 - ((255 - p[3]) as f32 * keep) as u8;
            }
        } else {
            let add = ((b - 0.5) * 120.0) as u8;
            for p in o.rgba.chunks_mut(4) {
                p[0] = p[0].saturating_add(add);
                p[1] = p[1].saturating_add(add);
                p[2] = p[2].saturating_add(add);
            }
        }
    }

    /// With Autosave ticked, a change of module writes `autosave.sav` over the last one.
    pub(super) fn autosave(&mut self) {
        if self.menus.settings.autosave && self.why_no_save().is_none() {
            self.write_autosave();
        }
    }

    /// The owner's rule: a save before every fight, whatever the tick says, each taking the place of the last. The enemy
    /// has often begun already when they are seen, so this may be in a fight. Loading it, they come at the player again.
    pub(super) fn autosave_before_fight(&mut self) {
        let now = self.game.world.time;
        let lately = self
            .menus
            .last_fight_save
            .is_some_and(|t| now - t < ONE_FIGHT_SECONDS);
        if self.combat_autosave && !lately && !self.game.world.in_dialogue() {
            self.menus.last_fight_save = Some(now);
            self.write_autosave();
        }
    }

    fn write_autosave(&mut self) {
        if self.menus.saves_dir.is_none() {
            self.menus.saves_dir = default_saves_dir();
        }
        let Some(dir) = self.menus.saves_dir.clone() else {
            return;
        };
        let written = std::fs::create_dir_all(&dir)
            .map_err(|e| e.to_string())
            .and_then(|()| {
                let path = dir.join("autosave.sav");
                let w = &self.game.world;
                w.save_to(&path, "Autosave").map_err(|e| e.to_string())
            });
        if let Err(e) = written {
            self.game.world.errors.push(format!("autosave: {e}"));
        }
    }

    /// `optgameplay.gui`. The other boxes and buttons are about things not built, so they are greyed.
    fn gameplay_view(&mut self) -> MenuView {
        let mut v = MenuView::new("optgameplay");
        self.gameplay_words(&mut v);
        let s = self.menus.settings;
        v.value(optionpages::MOVEMENT_BOX, s.movement_modern as i32, 1);
        v.text(
            optionpages::MOVEMENT_BOX,
            if s.movement_modern {
                optionpages::MOVEMENT_MODERN
            } else {
                optionpages::MOVEMENT_CLASSIC
            },
        );
        v.value("CB_LEVELUP", s.auto_level as i32, 1);
        v.value("CB_DISABLEMOVE", s.combat_movement as i32, 1);
        v.value("CB_INVERTCAM", s.invert_y as i32, 1);
        // The file's own name for this box is "Mouse Look", and all it does is flip the mouse up and down.
        v.text("CB_INVERTCAM", "Invert Camera".to_string());
        v.value("CB_AUTOSAVE", s.autosave as i32, 1);
        let name = self.ui.strref(STR_DIFFICULTY + s.difficulty as u32);
        v.text("BTN_DIFFICULTY", name);
        v
    }

    /// `optgraphics.gui`. The renderer has no shadows or grass: their words say so.
    fn graphics_view(&mut self) -> MenuView {
        let mut v = MenuView::new("optgraphics");
        self.graphics_words(&mut v);
        v.inert(&["CB_SHADOWS", "CB_GRASS"]);
        let s = self.menus.settings;
        v.value("SLI_GAMMA", (s.brightness * 100.0) as i32, 100);
        if let Some((w, h)) = s.size {
            v.text("BTN_RESOLUTION", format!("{w} x {h}"));
        }
        v
    }

    /// `optsound.gui`. There are no movies, and no advanced page.
    fn sound_view(&mut self) -> MenuView {
        let mut v = MenuView::new("optsound");
        v.hide(&["LB_DESC", "LBL_MOVIE", "SLI_MOVIE", "BTN_ADVANCED"]);
        let s = self.menus.settings;
        v.value("SLI_MUSIC", (s.music * 100.0) as i32, 100);
        v.value("SLI_VO", (s.voice * 100.0) as i32, 100);
        v.value("SLI_FX", (s.effects * 100.0) as i32, 100);
        v
    }

    /// A slider was clicked, this far along.
    fn move_slider(&mut self, tag: &str, along: f32) {
        let s = &mut self.menus.settings;
        match tag {
            "SLI_MUSIC" => s.music = along,
            "SLI_VO" => s.voice = along,
            "SLI_FX" => s.effects = along,
            "SLI_GAMMA" => s.brightness = along,
            "SLI_MOUSESEN" => s.mouse_sensitivity = along,
            _ => return,
        }
        self.settings_changed();
    }

    /// `container.gui`, over the scene. Nothing can be put into a container yet, so the middle button, the file's
    /// "Switch To", takes what is chosen. A stack has a stepper and starts at 1, as the owner drew it.
    fn container_view(&mut self) -> MenuView {
        let mut v = MenuView::new("container");
        v.over_scene = true;
        v.text("BTN_OK", TAKE_ALL);
        v.text(BTN_TAKE_SELECTED, TAKE_SELECTED);
        let title = self.ui.strref(STR_CONTAINER);
        v.text("LBL_MESSAGE", title);
        let items = self.listed_items();
        // What has gone from the list, a script's doing perhaps, is no longer chosen.
        self.menus.take_chosen.retain(|i| items.contains(i));
        self.menus.take_counts.retain(|i, _| items.contains(i));
        if self.menus.take_chosen.is_empty() {
            v.inert(&[BTN_TAKE_SELECTED]);
        }
        let mut rows = self.item_rows();
        for (row, item) in rows.iter_mut().zip(&items) {
            row.count = self.game.world.container_count(*item);
            row.selected = self.menus.take_chosen.contains(item);
            row.bar = row.selected;
            row.stepper = (row.count > 1).then(|| self.take_count(*item));
        }
        v.lists
            .insert("LB_ITEMS".to_string(), (rows, self.menus.first));
        v
    }

    /// How many of this stack the stepper says.
    fn take_count(&self, item: ObjId) -> i32 {
        self.menus.take_counts.get(&item).copied().unwrap_or(1)
    }

    /// A click on a row of a container chooses that row alone. With Control it adds the row, or leaves it out again.
    fn choose_container_row(&mut self, row: usize) {
        let Some(item) = self.listed_items().get(row).copied() else {
            return;
        };
        let chosen = &mut self.menus.take_chosen;
        if !self.menus.ctrl {
            chosen.clear();
            chosen.insert(item);
        } else if !chosen.remove(&item) {
            chosen.insert(item);
        }
    }

    /// An arrow of a row's stepper, ten at a time with Shift. Changing how many says that row is wanted, so it is chosen
    /// along with any others.
    fn step_container_row(&mut self, row: usize, by: i32) {
        let Some(item) = self.listed_items().get(row).copied() else {
            return;
        };
        let most = self.game.world.container_count(item).max(1);
        let by = if self.menus.shift {
            by * STEP_WITH_SHIFT
        } else {
            by
        };
        let count = (self.take_count(item) + by).clamp(1, most);
        self.menus.take_counts.insert(item, count);
        self.menus.take_chosen.insert(item);
    }

    /// The chosen rows come to the player, in the chosen numbers. The rest stays, and the panel stays until it is empty.
    fn take_selected(&mut self) {
        let chosen: Vec<ObjId> = self
            .listed_items()
            .into_iter()
            .filter(|i| self.menus.take_chosen.contains(i))
            .collect();
        if chosen.is_empty() {
            return;
        }
        self.effect(SOUND_ITEM_TAKE);
        for item in chosen {
            let count = self.take_count(item);
            let w = &mut self.game.world;
            if w.container_count(item) > 1 {
                w.take_some(item, count);
            } else {
                w.take_item(item);
            }
        }
        self.menus.take_chosen.clear();
        self.menus.take_counts.clear();
        let left = self.listed_items().len();
        self.menus.first = self.menus.first.min(left.saturating_sub(1));
        if left == 0 {
            self.game.world.close_container();
        }
    }

    /// Whether Control is down, for a screen where it changes what a click does.
    pub fn set_ctrl(&mut self, held: bool) {
        self.menus.ctrl = held;
    }

    /// Whether Shift is down.
    pub fn set_shift(&mut self, held: bool) {
        self.menus.shift = held;
    }

    /// How many rows of the open container are chosen.
    pub(super) fn container_chosen(&self) -> usize {
        self.menus.take_chosen.len()
    }

    /// The rows of a list on the screen, as a test or a driver asks.
    pub fn menu_rows(&mut self, tag: &str) -> Vec<kotor_ui::Row> {
        self.menu_view()
            .and_then(|mut v| v.lists.remove(tag))
            .map_or(Vec::new(), |l| l.0)
    }

    /// The words a screen gives a control in place of the file's.
    pub fn menu_button_words(&mut self, tag: &str) -> Option<String> {
        self.menu_view()?.patches.get(tag)?.text.clone()
    }

    /// Nothing stays chosen once a container is shut.
    pub(super) fn forget_container_choices(&mut self) {
        if self.game.world.opened.is_none() {
            self.menus.take_chosen.clear();
            self.menus.take_counts.clear();
        }
    }

    /// The Pickup button and its key: what is near opens as one container, or the player says there is nothing.
    pub fn pickup(&mut self) {
        if self.menu_open() || self.choice_view().is_some() {
            return;
        }
        let radius = self.menus.settings.pickup_radius as f32;
        if self.game.world.proximity_pickup(radius) == 0 {
            self.say_over_player(NOTHING_NEARBY.to_string());
        }
    }

    /// Everything in the open container comes to the player, and it closes.
    fn take_everything(&mut self) {
        for (item, _) in self.game.world.container_items() {
            self.game.world.take_item(item);
        }
        self.game.world.close_container();
        self.menus.first = 0;
    }

    /// `character.gui`. The numbers come from the rules, and the leader stands in a hole in the picture: see `figure.rs`.
    /// Scripts are not built. The level-up buttons show when a level is waiting.
    fn character_view(&mut self) -> MenuView {
        let mut v = MenuView::new("character");
        self.change_buttons(&mut v);
        v.hide(&["LBL_CLASS2", "LBL_LEVEL2"]);
        v.inert(&["BTN_SCRIPTS"]);
        v.holes.push("LBL_3DCHAR".to_string());
        let w = &mut self.game.world;
        let pc = w.pc;
        if !w.level_ready(pc) {
            v.hide(&["BTN_AUTO", "BTN_LEVELUP"]);
        }
        let Some(s) = w.sheet(pc) else { return v };
        v.text("LBL_NAME", s.name);
        v.text("LBL_CLASS1", s.class);
        v.text("LBL_LEVEL1", s.level.to_string());
        v.text("LBL_EXPERIENCE_STAT", s.xp.to_string());
        v.text("LBL_NEEDED_XP", s.next_level_xp.to_string());
        v.text("LBL_VITALITY_STAT", format!("{}/{}", s.hp, s.max_hp));
        if s.max_force > 0 {
            v.text("LBL_FORCE_STAT", format!("{}/{}", s.force, s.max_force));
        } else {
            v.hide(&["LBL_FORCE", "LBL_FORCE_STAT"]);
        }
        v.text("LBL_DEFENSE_STAT", s.defense.to_string());
        v.text("LBL_FORTITUDE_STAT", s.fortitude.to_string());
        v.text("LBL_REFLEX_STAT", s.reflex.to_string());
        v.text("LBL_WILL_STAT", s.will.to_string());
        for (i, (score, modifier)) in [
            "LBL_STR", "LBL_DEX", "LBL_CON", "LBL_INT", "LBL_WIS", "LBL_CHA",
        ]
        .iter()
        .zip([
            "LBL_STR_MOD",
            "LBL_DEX_MOD",
            "LBL_CON_MOD",
            "LBL_INT_MOD",
            "LBL_WIS_MOD",
            "LBL_CHA_MOD",
        ])
        .enumerate()
        {
            let (value, bonus) = s.abilities[i];
            v.text(score, value.to_string());
            let shown = if bonus == 0 {
                "0".to_string()
            } else {
                format!("{bonus:+}")
            };
            v.text(modifier, shown);
        }
        // The handle on the scale says it; the marks are for something else: none shows in the reference video.
        for n in 1..=10 {
            v.hide(&[format!("LBL_GOOD{n}").as_str()]);
        }
        v.hide(&["LBL_MORE"]);
        v.value("SLD_ALIGN", 100 - s.alignment, 100);
        v
    }

    /// The Use Item button: the item whose words are on show.
    fn use_about_item(&mut self) {
        let Some(item) = self.menus.about else { return };
        let pc = self.game.world.pc;
        if self.game.world.use_item(pc, item).is_ok() {
            self.menus.about = self.game.world.object(item).map(|_| item);
            self.menus.first = 0;
        }
    }

    fn change_tab(&mut self, screen: Screen) {
        self.menus.stack.pop();
        self.menus.stack.push(screen);
        self.menus.first = 0;
        self.menus.message_back = 0;
        self.menus.selected = matches!(screen, Screen::Journal | Screen::Abilities).then_some(0);
        self.leave_slot();
        self.menus.hover_slot = None;
    }

    /// The small portraits at the foot of the party's screens: the other members, a click changes to them.
    fn change_buttons(&self, v: &mut MenuView) {
        let w = &self.game.world;
        for (n, tag) in ["BTN_CHANGE1", "BTN_CHANGE2"].iter().enumerate() {
            match w.party.get(n).and_then(|id| w.object(*id)) {
                Some(o) if !o.portrait.is_empty() => v.fill(tag, o.portrait.clone()),
                Some(_) => {}
                None => v.hide(&[tag]),
            }
        }
    }

    /// Change to another member, who then leads, on a screen or from the HUD. **Assumed:** they go on leading after the
    /// screen closes. The video does not show which.
    fn change_member(&mut self, n: usize) {
        let w = &mut self.game.world;
        let Some(id) = w.party.get(n).copied() else {
            return;
        };
        w.switch_leader(id);
        self.leave_slot();
        self.menus.hover_slot = None;
        self.menus.hover = None;
    }

    /// What a click on the HUD sounds like.
    pub(super) fn hud_click_sound(&mut self, tag: &str) {
        self.effect(
            if tag.starts_with("BTN_TARGETUP") || tag.starts_with("BTN_TARGETDOWN") {
                SOUND_SLOT_SCROLL
            } else if tag == "BTN_TARGET1" || tag == "BTN_TARGET2" {
                SOUND_SLOT_QUEUE
            } else if tag == "BTN_TARGET0" || tag == "BTN_ACTION0" {
                SOUND_SLOT_USE
            } else {
                SOUND_CLICK
            },
        );
    }

    /// Screens coming and going have a sound of their own.
    pub(super) fn panel_sound(&mut self) {
        let now = self.menus.stack.len();
        if let Some(seen) = self.menus.stack_seen.filter(|s| *s != now) {
            self.effect(if now > seen { SOUND_OPEN } else { SOUND_CLOSE });
        }
        self.menus.stack_seen = Some(now);
    }

    /// A click on one of the HUD's buttons: the screen it opens. `BTN_ACTION0` is the stage's.
    pub(super) fn hud_button(&mut self, tag: &str) {
        match tag {
            "BTN_EQU" => self.open_equip(),
            "BTN_INV" => self.open_inventory(),
            "BTN_CHAR" => self.open_character(),
            "BTN_JOU" => self.open_journal(),
            "BTN_MAP" | "BTN_MINIMAP" => self.open_map(),
            "BTN_CHAR1" => self.open_if_level_waits(),
            "BTN_CHAR3" => {
                self.change_member(0);
                self.open_if_level_waits();
            }
            "BTN_CHAR2" => {
                self.change_member(1);
                self.open_if_level_waits();
            }
            "BTN_ACTION1" | "BTN_ACTION2" | "BTN_ACTION3" => self.use_quick_slot(tag),
            "BTN_ACTIONUP1" | "BTN_ACTIONUP2" | "BTN_ACTIONUP3" | "BTN_ACTIONDOWN1"
            | "BTN_ACTIONDOWN2" | "BTN_ACTIONDOWN3" => self.turn_quick_slot(tag),
            "BTN_CLEARALL" | "BTN_CLEARONE" | "BTN_CLEARONE2" => self.disengage(),
            "TB_PAUSE" => self.toggle_pause(),
            "TB_SOLO" if self.game.world.solo_mode => self.solo_key(),
            "BTN_TARGETUP0" | "BTN_TARGETDOWN0" | "BTN_TARGETUP1" | "BTN_TARGETDOWN1"
            | "BTN_TARGETUP2" | "BTN_TARGETDOWN2" => self.turn_slot(tag),
            "BTN_OPT" => self.escape(),
            _ => {}
        }
    }

    /// The arrow on a portrait says a level is waiting, and a click on it goes to the screen with the level-up button.
    fn open_if_level_waits(&mut self) {
        let w = &self.game.world;
        if w.level_ready(w.pc) {
            self.open_character();
        }
    }

    /// The Disengage button: the leader stops attacking.
    fn disengage(&mut self) {
        let w = &mut self.game.world;
        let pc = w.pc;
        w.stop_attacking(pc);
        if let Some(o) = w.object_mut(pc) {
            o.actions.clear();
        }
    }

    /// What the HUD shows of a fight: `action` is what the target offers.
    pub(super) fn combat_view(&mut self, action: Option<&str>) -> CombatHud {
        if action == Some("Attack") {
            self.enemy_sighted();
        }
        let w = &self.game.world;
        let attacking = w.combat.target.contains_key(&w.pc);
        let mut queue = if attacking {
            vec![ATTACK_PICTURE.to_string()]
        } else {
            Vec::new()
        };
        let hostile = action == Some("Attack");
        let locked = !hostile && self.target_has_a_way_in();
        let (slots, hover) = self.slots_view(hostile || locked, &mut queue);
        let (quick, quick_hover) = self.quick_view();
        let pickup = slot_is_pickup(action);
        let on_pickup = pickup && self.over_first_action_slot();
        CombatHud {
            locked,
            quick,
            quick_hover: on_pickup.then(|| PICKUP_WORDS.to_string()).or(quick_hover),
            pickup,
            engaged: self.party_blocked(),
            message: self.ui.strref(STR_COMBAT_MODE),
            queue,
            attack: hostile.then(|| ATTACK_PICTURE.to_string()),
            slots,
            hover,
        }
    }

    /// The J key: the journal, or back to the game if it is up.
    pub fn open_journal(&mut self) {
        self.open_items(Screen::Journal);
    }

    /// The C key: the character, or back to the game if it is up.
    pub fn open_character(&mut self) {
        self.open_items(Screen::Character);
    }

    /// The I key: the inventory, or back to the game if it is up.
    pub fn open_inventory(&mut self) {
        self.open_items(Screen::Inventory);
    }

    /// The U key: the equipment, or back to the game if it is up.
    pub fn open_equip(&mut self) {
        self.open_items(Screen::Equip);
    }

    fn open_items(&mut self, screen: Screen) {
        let top = self.top();
        if top == Some(screen) {
            self.menus.stack.pop();
            return;
        }
        let from_game = top.is_none() && self.started && !self.game.world.in_dialogue();
        let swapping = matches!(
            top,
            Some(
                Screen::Inventory
                    | Screen::Equip
                    | Screen::Character
                    | Screen::Abilities
                    | Screen::Messages
                    | Screen::Journal
                    | Screen::Map
            )
        );
        if from_game || swapping {
            self.menus.stack.pop();
            self.menus.stack.push(screen);
            self.menus.first = 0;
            self.menus.message_back = 0;
            self.menus.selected =
                matches!(screen, Screen::Journal | Screen::Abilities).then_some(0);
            self.leave_slot();
            self.menus.hover_slot = None;
            self.menus.hover = None;
            match screen {
                Screen::Inventory => self.tutorial(notices::TUTORIAL_INVENTORY),
                Screen::Map => self.tutorial(notices::TUTORIAL_MAP),
                _ => {}
            }
        }
    }

    /// A click on an item in the inventory shows its words.
    fn use_listed_item(&mut self, row: usize) {
        let Some(item) = self.listed_items().get(row).copied() else {
            return;
        };
        self.menus.about = Some(item);
    }

    /// A click on a slot: its list comes up to choose from, with what it holds chosen.
    fn choose_for_slot(&mut self, slot: i32) {
        let worn = worn_in(&self.game.world, slot);
        self.menus.slot = Some(slot);
        self.menus.chosen = Some(worn.is_some() as usize);
        self.menus.about = None;
        self.menus.refused = false;
        self.menus.first = 0;
        self.tutorial(notices::TUTORIAL_EQUIP_SLOT);
    }

    /// Back from a slot's list, or off the screen: nothing more is chosen.
    fn leave_slot(&mut self) {
        self.menus.slot = None;
        self.menus.chosen = None;
        self.menus.about = None;
        self.menus.refused = false;
        self.menus.first = 0;
    }

    /// OK on a slot's list: what is chosen goes on and the slots come back. Refused, the list stays up and says why.
    fn accept_slot(&mut self) {
        let Some(slot) = self.menus.slot else { return };
        let choice = self
            .menus
            .chosen
            .and_then(|n| self.equip_choices(slot).get(n).copied());
        let Some(choice) = choice else { return };
        let w = &mut self.game.world;
        let pc = w.pc;
        let done = match choice {
            None => {
                w.unequip(pc, slot);
                Ok(())
            }
            Some(item) if Some(item) == worn_in(w, slot) => Ok(()),
            Some(item) => w.equip(pc, item, Some(slot)).map(|_| ()),
        };
        match done {
            Ok(()) => self.leave_slot(),
            Err(CannotEquip::BothHandsFull) => {
                self.effect(SOUND_ERROR);
                self.menus.message = "A weapon in the right hand takes both hands.".to_string();
            }
            Err(CannotEquip::Alignment) => {
                self.effect(SOUND_ERROR);
                self.menus.message = self.ui.strref(STR_WRONG_SIDE);
            }
            Err(_) => {
                self.effect(SOUND_ERROR);
                self.menus.refused = true
            }
        }
    }

    fn menu_layout_point(&self, pixel: (f32, f32), size: (u32, u32)) -> (f32, f32) {
        (
            pixel.0 * 640.0 / size.0.max(1) as f32,
            pixel.1 * 480.0 / size.1.max(1) as f32,
        )
    }

    /// The cursor moved over a screen: light what is under it.
    pub(super) fn menu_hover(&mut self, pixel: (f32, f32), size: (u32, u32)) {
        let Some(view) = self.menu_view() else { return };
        let (x, y) = self.menu_layout_point(pixel, size);
        let was = self.menus.hover.take();
        self.menus.hover = self.ui.menu_hit(&view, x, y);
        if self.menus.hover.is_some() && self.menus.hover != was {
            self.effect(SOUND_ENTER);
        }
        let top = self.top();
        if let Some(screen) = top {
            self.option_hover(screen);
        }
        if top == Some(Screen::Equip) {
            self.equip_hover();
        } else if let Some(screen) = top.filter(|t| t.is_level()) {
            self.level_hover(screen);
        } else if let Some(Hit::Row(_, row)) = self.menus.hover {
            if let Some(item) = self.listed_items().get(row).copied() {
                self.menus.about = Some(item);
            }
        }
    }

    /// A slot under the cursor stays the one listed. A row of a slot's list shows its words.
    fn equip_hover(&mut self) {
        let hover = self.menus.hover.clone();
        if let Some(Hit::Button(tag)) = &hover {
            if let Some(s) = EQUIP_SLOTS.iter().find(|s| s.0 == tag) {
                if self.menus.hover_slot != Some(s.2) {
                    self.menus.hover_slot = Some(s.2);
                    self.menus.first = 0;
                }
            }
        }
        self.menus.about = match (hover, self.menus.slot) {
            (Some(Hit::Row(_, row)), Some(slot)) => {
                self.equip_choices(slot).get(row).copied().flatten()
            }
            _ => None,
        };
    }

    /// A left click on a screen.
    pub(super) fn menu_click(&mut self, pixel: (f32, f32), size: (u32, u32)) {
        let Some(view) = self.menu_view() else { return };
        let (x, y) = self.menu_layout_point(pixel, size);
        let delete = self.menus.message.starts_with("Press Delete");
        self.menus.message.clear();
        match self.ui.menu_hit(&view, x, y) {
            Some(Hit::Button(tag)) => {
                let sound = if tag.starts_with("CB_") {
                    SOUND_CHECK
                } else {
                    SOUND_CLICK
                };
                self.effect(sound);
                if tag != "BTN_DELETE" || !delete {
                    self.menus.delete_armed = None;
                }
                self.press(&tag);
            }
            Some(Hit::Row(_, row)) => {
                self.menus.delete_armed = None;
                self.effect(match self.top() {
                    Some(Screen::Container | Screen::Inventory | Screen::Equip | Screen::Store) => {
                        SOUND_ITEM_SELECT
                    }
                    Some(Screen::Feedback) => SOUND_CHECK,
                    _ => SOUND_CLICK,
                });
                match self.top() {
                    Some(Screen::Inventory) => self.use_listed_item(row),
                    Some(Screen::Container) => self.choose_container_row(row),
                    Some(Screen::Feedback) => self.feedback_row(row),
                    Some(Screen::Store) => self.store_row(row),
                    Some(Screen::BenchItems | Screen::Bench) => self.bench_row(row),
                    Some(Screen::KeyMap) => self.key_row(row),
                    Some(Screen::Equip) => {
                        if self.menus.slot.is_some() {
                            self.menus.chosen = Some(row);
                            self.menus.refused = false;
                        }
                    }
                    _ => self.menus.selected = Some(row),
                }
            }
            Some(Hit::Slider(tag, along)) => {
                self.effect(SOUND_CLICK);
                self.move_slider(&tag, along)
            }
            Some(Hit::Cell(_, row, cell)) => {
                self.effect(SOUND_CLICK);
                if let Some(screen) = self.top() {
                    self.level_cell(screen, row, cell);
                }
            }
            Some(Hit::Step(_, row, by)) => {
                self.effect(SOUND_CLICK);
                self.step_container_row(row, by);
            }
            None => self.menus.delete_armed = None,
        }
    }

    /// A number key on a screen: the panels with numbered steps take it as a click on that step.
    pub(super) fn menu_number(&mut self, n: usize) {
        if matches!(self.top(), Some(Screen::LevelUp | Screen::ChargenMain)) {
            self.effect(SOUND_CLICK);
            self.press(&format!("BTN_STEPNAME{n}"));
        }
    }

    /// Where the needle beside the alignment bar is, in layout units: left, top, width and height.
    pub fn alignment_needle(&mut self) -> Option<[i32; 4]> {
        let w = &self.game.world;
        let alignment = w.object(w.pc)?.alignment;
        self.ui
            .menu_needle("character", "SLD_ALIGN", (100 - alignment, 100))
    }

    /// What a button does.
    fn press(&mut self, tag: &str) {
        let Some(top) = self.top() else {
            return;
        };
        // The tabs change between the party's screens.
        if self.menus.stack.last().is_some() && TABS.iter().any(|t| t.0 == tag) {
            let target = match tag {
                "BTN_EQU" => Some(Screen::Equip),
                "BTN_INV" => Some(Screen::Inventory),
                "BTN_CHAR" => Some(Screen::Character),
                "BTN_ABI" => Some(Screen::Abilities),
                "BTN_MSG" => Some(Screen::Messages),
                "BTN_JOU" => Some(Screen::Journal),
                "BTN_MAP" => Some(Screen::Map),
                "BTN_OPT" => Some(Screen::Pause),
                _ => None,
            };
            match target {
                Some(screen) => self.change_tab(screen),
                None => self.menus.message = "That screen is not built yet.".to_string(),
            }
            self.menus.hover = None;
            return;
        }
        match (top, tag) {
            (Screen::Store, _) => self.store_press(tag),
            (Screen::Galaxy, _) => self.galaxy_press(tag),
            (Screen::Pazaak, _) => self.pazaak_press(tag),
            (Screen::Solo, _) => self.solo_press(tag),
            (Screen::GameOver, _) => self.game_over_press(tag),
            (Screen::Container, "BTN_OK") => self.take_everything(),
            (Screen::Container, BTN_TAKE_SELECTED) => self.take_selected(),
            (Screen::Container, "BTN_CANCEL") => self.game.world.close_container(),
            (Screen::Main, "BTN_NEWGAME") => self.new_game(),
            (Screen::Main | Screen::Pause, "BTN_LOADGAME") => self.open_saves(Mode::Load),
            (Screen::Main, "BTN_OPTIONS") => self.menus.stack.push(Screen::Options),
            (Screen::Options | Screen::Pause, "BTN_GAMEPLAY") => {
                self.menus.stack.push(Screen::Gameplay)
            }
            (Screen::Options | Screen::Pause, "BTN_GRAPHICS") => {
                self.menus.stack.push(Screen::Graphics)
            }
            (Screen::Options | Screen::Pause, "BTN_SOUND") => self.menus.stack.push(Screen::Sound),
            (Screen::Options | Screen::Pause, "BTN_FEEDBACK") => {
                self.menus.first = 0;
                self.menus.stack.push(Screen::Feedback)
            }
            (Screen::Options | Screen::Pause, "BTN_AUTOPAUSE") => {
                self.menus.stack.push(Screen::AutoPause)
            }
            (Screen::Feedback | Screen::AutoPause, _) => self.option_page_press(top, tag),
            (Screen::Gameplay, "BTN_KEYMAP") => self.open_key_map(),
            (Screen::Gameplay, "BTN_MOUSE") => self.menus.stack.push(Screen::Mouse),
            (Screen::KeyMap, _) => self.key_map_press(tag),
            (Screen::Mouse, _) => self.mouse_press(tag),
            (Screen::Gameplay, optionpages::MOVEMENT_BOX) => {
                self.menus.settings.movement_modern = !self.menus.settings.movement_modern;
                self.settings_changed();
            }
            (Screen::Gameplay, "CB_LEVELUP") => {
                self.menus.settings.auto_level = !self.menus.settings.auto_level;
                self.settings_changed();
            }
            (Screen::Gameplay, "CB_DISABLEMOVE") => {
                self.menus.settings.combat_movement = !self.menus.settings.combat_movement;
                self.settings_changed();
            }
            (Screen::Options | Screen::Gameplay | Screen::Graphics | Screen::Sound, "BTN_BACK") => {
                self.menus.stack.pop();
            }
            (Screen::Gameplay, "CB_INVERTCAM") => {
                self.menus.settings.invert_y = !self.menus.settings.invert_y;
                self.settings_changed();
            }
            (Screen::Gameplay, "CB_AUTOSAVE") => {
                self.menus.settings.autosave = !self.menus.settings.autosave;
                self.settings_changed();
            }
            (Screen::Gameplay, "BTN_DIFFLEFT" | "BTN_DIFFRIGHT") => {
                let step = if tag == "BTN_DIFFLEFT" { -1 } else { 1 };
                let d = self.menus.settings.difficulty + step;
                self.menus.settings.difficulty = d.clamp(0, 2);
                self.settings_changed();
            }
            (Screen::Gameplay, "BTN_DEFAULT") => {
                let d = Settings::default();
                let s = &mut self.menus.settings;
                (s.invert_y, s.autosave, s.difficulty) = (d.invert_y, d.autosave, d.difficulty);
                (s.auto_level, s.combat_movement) = (d.auto_level, d.combat_movement);
                self.settings_changed();
            }
            (Screen::Graphics, "BTN_DEFAULT") => {
                self.menus.settings.brightness = Settings::default().brightness;
                self.settings_changed();
            }
            (Screen::Graphics, "BTN_ADVANCED") => self.open_graphics_advanced(),
            (Screen::GraphicsAdvanced, _) => self.graphics_advanced_press(tag),
            (Screen::Graphics, "BTN_RESOLUTION") => {
                let at = SIZES
                    .iter()
                    .position(|s| Some(*s) == self.menus.settings.size)
                    .map_or(0, |i| (i + 1) % SIZES.len());
                self.menus.settings.size = Some(SIZES[at]);
                self.menus.wanted_size = Some(SIZES[at]);
                self.settings_changed();
            }
            (Screen::Sound, "BTN_DEFAULT") => {
                let d = Settings::default();
                let s = &mut self.menus.settings;
                (s.music, s.voice, s.effects) = (d.music, d.voice, d.effects);
                self.settings_changed();
            }
            (Screen::Main, "BTN_EXIT") => self.menus.exit = true,
            (Screen::Pause, "BTN_SAVEGAME") => match self.why_no_save() {
                Some(why) => self.menus.message = why.to_string(),
                None => self.open_saves(Mode::Save),
            },
            (Screen::Pause, "BTN_QUIT") => self.quit_to_main(),
            (Screen::Pause, "BTN_EXIT") => {
                self.menus.stack.pop();
            }
            (Screen::Saves(_), "BTN_BACK") => {
                self.menus.stack.pop();
            }
            (Screen::Saves(mode), "BTN_SAVELOAD") => self.save_or_load(mode),
            (Screen::Saves(mode), "BTN_DELETE") => self.delete_selected(mode),
            (Screen::Name, "BTN_OK") => self.write_save(),
            (Screen::Name, "BTN_CANCEL") => {
                self.menus.stack.pop();
            }
            (Screen::Inventory, "BTN_USEITEM") => self.use_about_item(),
            (Screen::Inventory | Screen::Character, "BTN_EXIT") => {
                self.menus.stack.pop();
            }
            (Screen::Journal | Screen::Inventory, "BTN_QUESTITEMS") => {
                self.menus.stack.push(Screen::QuestItems);
                self.menus.selected = None;
                self.menus.first = 0;
                self.menus.hover = None;
            }
            (Screen::QuestItems, "BTN_BACK") => {
                self.menus.stack.pop();
                self.menus.selected = None;
            }
            (Screen::Journal, "BTN_EXIT") => {
                self.menus.stack.pop();
            }
            (Screen::Journal, "BTN_SWAPTEXT") => {
                self.menus.journal_completed = !self.menus.journal_completed;
                self.menus.selected = Some(0);
                self.menus.first = 0;
            }
            (Screen::Journal, "BTN_SORT") => {
                self.menus.journal_sort = (self.menus.journal_sort + 1) % QUEST_SORTS.len();
                self.menus.selected = Some(0);
                self.menus.first = 0;
            }
            (Screen::Abilities | Screen::Messages, _) => self.ability_press(tag),
            (Screen::Tutorial, "BTN_OK") => self.tutorial_next(),
            (Screen::Inventory | Screen::Equip | Screen::Character, "BTN_CHANGE1") => {
                self.change_member(0)
            }
            (Screen::Inventory | Screen::Equip | Screen::Character, "BTN_CHANGE2") => {
                self.change_member(1)
            }
            (Screen::Party, _) => self.party_press(tag),
            (Screen::BenchKinds | Screen::BenchItems | Screen::Bench, _) => {
                self.bench_press(top, tag)
            }
            (
                Screen::ChargenClass
                | Screen::ChargenMain
                | Screen::ChargenPortrait
                | Screen::ChargenAttributes
                | Screen::ChargenName,
                _,
            ) => self.chargen_press(top, tag),
            (Screen::Map, _) => self.map_press(tag),
            (Screen::Character, "BTN_LEVELUP") => self.start_level_up(),
            (Screen::Character, "BTN_AUTO") => self.auto_level_up(),
            (screen, _) if screen.is_level() => self.level_press(screen, tag),
            (Screen::Equip, "BTN_EQUIP") => self.accept_slot(),
            (Screen::Equip, "BTN_BACK") if self.menus.slot.is_some() => self.leave_slot(),
            (Screen::Equip, "BTN_BACK") => {
                self.menus.stack.pop();
            }
            (Screen::Equip, _) => {
                if let Some(s) = EQUIP_SLOTS.iter().find(|s| s.0 == tag) {
                    self.choose_for_slot(s.2);
                }
            }
            _ => {}
        }
        self.menus.hover = None;
    }

    /// Enter: the screen's main button, where it has one.
    pub(super) fn menu_enter(&mut self) {
        match self.top() {
            Some(Screen::Container) => self.game.world.close_container(),
            Some(Screen::Store) => self.store_accept(),
            Some(Screen::Saves(mode)) => self.save_or_load(mode),
            Some(Screen::Name) => self.write_save(),
            Some(Screen::ChargenName) => self.chargen_name_accept(),
            Some(Screen::Equip) => self.accept_slot(),
            _ => {}
        }
    }

    fn new_game(&mut self) {
        if !self.begin_new_game() {
            self.new_game_now();
        }
    }

    fn new_game_now(&mut self) {
        self.menus.paused = false;
        let module = self.first_module.clone();
        match self.start_module(&module) {
            Ok(()) => {
                self.menus.stack.clear();
                self.menus.hover = None;
            }
            Err(e) => self.menus.message = format!("Cannot start {module}: {e}"),
        }
    }

    /// Esc: close what is open, back out of a screen, or in the game open the pause menu. The main menu stays.
    pub fn escape(&mut self) {
        match self.top() {
            Some(Screen::Main | Screen::GameOver) => {}
            Some(Screen::Container) => self.game.world.close_container(),
            Some(Screen::Store) => self.store_escape(),
            Some(Screen::Galaxy) => self.galaxy_escape(),
            Some(Screen::Pazaak) => self.pazaak_escape(),
            Some(Screen::Equip) if self.menus.slot.is_some() => {
                self.leave_slot();
                self.menus.message.clear();
            }
            Some(Screen::Tutorial) => self.tutorial_next(),
            Some(Screen::KeyMap) => self.cancel_key_map(),
            Some(Screen::Party) => {
                self.party_press("BTN_BACK");
                self.menus.hover = None;
            }
            Some(Screen::LevelUp) => self.abandon_level(),
            Some(
                screen @ (Screen::ChargenClass
                | Screen::ChargenMain
                | Screen::ChargenPortrait
                | Screen::ChargenAttributes
                | Screen::ChargenName),
            ) => self.chargen_escape(screen),
            Some(screen) if screen.is_level() => {
                self.cancel_page();
                self.menus.message.clear();
            }
            Some(_) => {
                self.menus.stack.pop();
                self.menus.message.clear();
                self.menus.hover = None;
            }

            None if self.started && self.game.world.death_phase().is_none() => {
                self.menus.stack.push(Screen::Pause)
            }
            None => {}
        }
    }

    /// The Exit Game button: back to the main menu, with the game gone.
    pub(super) fn quit_to_main(&mut self) {
        match Stage::fresh_world(&self.dir, self.original) {
            Ok(world) => self.game = kotor_game::Game::new(world),
            Err(e) => {
                self.menus.message = e;
                return;
            }
        }
        self.forget_view();
        self.started = false;
        self.module.clear();
        self.floor = None;
        self.static_scene = Default::default();
        self.static_version += 1;
        self.effects = Default::default();
        self.cues.push(Cue::StopVoice);
        self.cues.push(Cue::StopMusic);
        self.menus.stack = vec![Screen::Main];
    }

    /// Why the game cannot be saved right now: this engine does not save a conversation or a fight in progress.
    fn why_no_save(&self) -> Option<&'static str> {
        let w = &self.game.world;
        if w.in_dialogue() {
            return Some("You cannot save during a conversation.");
        }
        let pc = w.pc;
        let party = |id: &u32| *id == pc || w.party.contains(id);
        let fighting = w.combat.target.iter().any(|(a, t)| party(a) || party(t));
        fighting.then_some("You cannot save during combat.")
    }

    pub(super) fn open_saves(&mut self, mode: Mode) {
        self.reload_saves();
        self.menus.selected = (mode == Mode::Save).then_some(0);
        self.menus.first = 0;
        self.menus.delete_armed = None;
        self.menus.stack.push(Screen::Saves(mode));
    }

    fn reload_saves(&mut self) {
        if self.menus.saves_dir.is_none() {
            self.menus.saves_dir = default_saves_dir();
        }
        self.menus.saves = self
            .menus
            .saves_dir
            .as_deref()
            .map(list_saves)
            .unwrap_or_default();
    }

    /// The file behind the chosen row, if it has one.
    fn selected_file(&self, mode: Mode) -> Option<&SaveFile> {
        let shift = (mode == Mode::Save) as usize;
        self.menus
            .selected?
            .checked_sub(shift)
            .and_then(|i| self.menus.saves.get(i))
    }

    fn save_or_load(&mut self, mode: Mode) {
        match mode {
            Mode::Save if self.menus.selected.is_some() => {
                self.menus.name = match self.selected_file(mode) {
                    Some(f) => f.info.name.clone(),
                    None => {
                        let w = &self.game.world;
                        w.object(w.area).map(|a| a.name.clone()).unwrap_or_default()
                    }
                };
                self.menus.name.truncate(SAVE_NAME_MAX);
                self.menus.stack.push(Screen::Name);
            }
            Mode::Load => self.load_selected(),
            Mode::Save => {}
        }
    }

    fn load_selected(&mut self) {
        // What the last game had is no change to tell of.
        self.menus.had = None;
        let Some(path) = self.selected_file(Mode::Load).map(|f| f.path.clone()) else {
            return;
        };
        if let Err(e) = self.game.world.load_from(&path) {
            self.menus.message = format!("Cannot load: {e}");
            return;
        }
        self.forget_view();
        self.menus.paused = false;
        self.menus.map = None;
        self.menus.had = None;
        self.menus.fight = Default::default();
        if let Err(e) = self.enter_module() {
            self.game.world.errors.push(format!("stage: {e}"));
        }
        self.events_seen = self.game.world.events.len();
        self.started = true;
        self.yaw = self.game.world.pc_mover.heading;
        self.menus.stack.clear();
        self.menus.hover = None;
    }

    /// The name dialog's OK: write the chosen file, or a new one, and go back to the list.
    fn write_save(&mut self) {
        let name = self.menus.name.trim().to_string();
        if name.is_empty() {
            self.menus.message = "Give the save a name.".to_string();
            return;
        }
        let Some(dir) = self.menus.saves_dir.clone() else {
            self.menus.message = "The system has no folder for local data.".to_string();
            return;
        };
        let path = match self.selected_file(Mode::Save) {
            Some(f) => f.path.clone(),
            None => (1..)
                .map(|n| dir.join(format!("save_{n:04}.sav")))
                .find(|p| !p.exists())
                .expect("a free name"),
        };
        let written = std::fs::create_dir_all(&dir)
            .map_err(|e| e.to_string())
            .and_then(|()| {
                self.game
                    .world
                    .save_to(&path, &name)
                    .map_err(|e| e.to_string())
            });
        self.menus.stack.pop();
        match written {
            Ok(()) => {
                self.menus.message = format!("Saved {name}.");
                self.reload_saves();
                self.menus.first = 0;
                let at = self.menus.saves.iter().position(|f| f.path == path);
                self.menus.selected = at.map(|i| i + 1);
            }
            Err(e) => self.menus.message = format!("Cannot save: {e}"),
        }
    }

    /// Delete asks once, then takes the file off the disk.
    fn delete_selected(&mut self, mode: Mode) {
        let Some((path, name)) = self
            .selected_file(mode)
            .map(|f| (f.path.clone(), f.info.name.clone()))
        else {
            return;
        };
        if self.menus.delete_armed.as_ref() != Some(&path) {
            self.menus.message = format!("Press Delete again to delete {name}.");
            self.menus.delete_armed = Some(path);
            return;
        }
        self.menus.delete_armed = None;
        if let Err(e) = std::fs::remove_file(&path) {
            self.menus.message = format!("Cannot delete: {e}");
        }
        self.reload_saves();
        self.menus.first = 0;
        self.menus.selected = (mode == Mode::Save).then_some(0);
    }

    /// Whether keys go to a text box.
    pub fn typing(&self) -> bool {
        matches!(self.menus.stack.last(), Some(Screen::Name)) || self.chargen_typing()
    }

    /// Characters typed into the text box.
    pub fn type_text(&mut self, text: &str) {
        for c in text
            .chars()
            .filter(|c| !c.is_control() && (*c as u32) < 256)
        {
            if self.menus.name.chars().count() < SAVE_NAME_MAX {
                self.menus.name.push(c);
            }
        }
    }

    pub fn backspace(&mut self) {
        self.menus.name.pop();
    }

    /// The wheel over a list.
    pub fn menu_wheel(&mut self, lines: f32) {
        if self.top() == Some(Screen::Messages) {
            self.messages_wheel(lines);
            return;
        }
        let Some(view) = self.menu_view() else { return };
        let Some((tag, (rows, _))) = view.lists.iter().next() else {
            return;
        };
        let fits = self.ui.menu_rows_shown(&view.gui, tag);
        let last = rows.len().saturating_sub(fits) as i32;
        self.menus.first =
            (self.menus.first as i32 - lines.signum() as i32).clamp(0, last) as usize;
    }

    /// The point on a window of `size` in the middle of a button on the screen that is up, or of a list row.
    pub fn menu_pixel(
        &mut self,
        tag: &str,
        row: Option<usize>,
        size: (u32, u32),
    ) -> Option<(f32, f32)> {
        let view = self.menu_view()?;
        let (x, y) = self.ui.menu_point(&view, tag, row)?;
        Some((x * size.0 as f32 / 640.0, y * size.1 as f32 / 480.0))
    }

    /// The point on a window of `size` in the middle of an arrow of a row's stepper: `by` is -1 or 1.
    pub fn menu_step_pixel(
        &mut self,
        tag: &str,
        row: usize,
        by: i32,
        size: (u32, u32),
    ) -> Option<(f32, f32)> {
        let view = self.menu_view()?;
        let (x, y) = self.ui.menu_step_point(&view, tag, row, by)?;
        Some((x * size.0 as f32 / 640.0, y * size.1 as f32 / 480.0))
    }

    /// The point on a window of `size` in the middle of a picture of a list's row.
    pub fn menu_cell_pixel(
        &mut self,
        tag: &str,
        row: usize,
        cell: usize,
        size: (u32, u32),
    ) -> Option<(f32, f32)> {
        let view = self.menu_view()?;
        let (x, y) = self.ui.menu_cell_point(&view, tag, row, cell)?;
        Some((x * size.0 as f32 / 640.0, y * size.1 as f32 / 480.0))
    }

    /// The name in the text box.
    pub fn save_name_typed(&self) -> &str {
        &self.menus.name
    }

    /// The name of the save on the chosen row.
    pub fn selected_save_name(&self) -> Option<String> {
        let mode = self.menus.stack.iter().rev().find_map(|s| match s {
            Screen::Saves(m) => Some(*m),
            _ => None,
        })?;
        self.selected_file(mode).map(|f| f.info.name.clone())
    }
}

/// `mainmenu.gui`, with its scene behind a hole: see `mainscene.rs`. The debug warp, the module list and the notice about new content stay out, and the
/// movies are greyed.
fn main_menu() -> MenuView {
    let mut v = MenuView::new("mainmenu");
    v.hide(&[
        "LB_MODULES",
        "BTN_WARP",
        "LBL_NEWCONTENT",
        "LBL_BW",
        "LBL_LUCAS",
    ]);
    v.plain("BTN_EXIT");
    v.inert(&["BTN_MOVIES"]);
    v
}

/// `optionsingame.gui`.
fn pause_menu() -> MenuView {
    let mut v = MenuView::new("optionsingame");
    v.text("LB_DESC", about_words());
    v
}

/// `optionsmain.gui`, as the pause menu's pages from the main menu.
fn options_menu() -> MenuView {
    let mut v = MenuView::new("optionsmain");
    v.text("LB_DESC", about_words());
    v
}

/// The build, and the notice the font's licence asks to go with it.
pub fn about_words() -> String {
    format!(
        "{}\n\nText in Noto Sans. Copyright 2022 The Noto Project Authors. SIL Open Font License 1.1: scripts.sil.org/OFL",
        crate::build_line()
    )
}
