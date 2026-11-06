use crate::dialogue::Line;
use crate::host::Adapter;
use kotor_archive::Install;
use kotor_core::{Error, ResType, Result};
use kotor_gff::{Gff, LocString, Struct, Value as GffValue};
use kotor_script::{
    disassemble, Outcome, Program, Routines, SavedState, Vm, VmError, OBJECT_INVALID,
};
use kotor_tables::{Tlk, TwoDa};
use kotor_walkmesh::{MoveParams, Mover, NavMesh};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::rc::Rc;

pub type ObjId = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Module,
    Area,
    Creature,
    Door,
    Placeable,
    Waypoint,
    Trigger,
    Item,
}

/// One queued action on an object.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Do nothing for this many seconds.
    Wait(f32),
    /// Run a deferred script block (`ActionDoCommand`).
    DoCommand {
        script: String,
        state: Box<SavedState>,
    },
    /// Talk with `target`, using `dialogue` (the owner's own conversation when empty).
    StartConversation {
        target: ObjId,
        dialogue: String,
        /// Talk from where the speaker stands, instead of walking up first.
        anywhere: bool,
    },
    /// Accepted and recorded, no effect on the headless world (animations, movement, ...).
    Recorded(String),
    OpenDoor(ObjId),
    CloseDoor(ObjId),
    /// Move to a position at once (`ActionJumpToLocation`).
    Jump([f32; 3]),
    /// Walk there, and finish only on arrival, so the actions behind it wait (`ActionMoveToObject` and friends, in a
    /// live world).
    MoveTo {
        target: Option<ObjId>,
        to: [f32; 3],
        range: f32,
        run: bool,
        walker: Option<Mover>,
    },
    /// Start attacking an object (`ActionAttack`).
    Attack(ObjId),
    /// Cast a Force power at an object.
    Cast {
        spell: i32,
        target: ObjId,
        cheat: bool,
    },
}

/// A command scheduled by `DelayCommand`.
/// How many actions one object may finish in one tick.
const ACTIONS_PER_TICK: u32 = 64;
/// How long a conversation may wait on an `ActionPauseConversation`.
pub(crate) const PAUSE_LIMIT_SECONDS: f32 = 60.0;
/// How long a walk that was already over takes.
const STAND_SECONDS: f32 = 0.5;

#[derive(Clone, Debug, PartialEq)]
pub struct Delayed {
    pub due: f32,
    pub caller: ObjId,
    pub script: String,
    pub state: Box<SavedState>,
}

#[derive(Clone, Debug)]
pub struct Object {
    pub id: ObjId,
    pub kind: Kind,
    pub tag: String,
    pub name: String,
    pub template: String,
    /// Dialogue resref used when someone talks to this object.
    pub conversation: String,
    pub position: [f32; 3],
    pub hp: i32,
    pub dead: bool,
    pub party_member: bool,
    /// Damage stops at one hit point: `k_pend_cut19` holds the dark Jedi up this way until the duel may end.
    pub min_one_hp: bool,
    /// Its spawn script has run.
    pub spawned: bool,
    pub locked: bool,
    pub level: i32,
    /// Event scripts by field name (`ScriptDialogue`, `OnEnter`, `OnUsed`, ...).
    pub scripts: BTreeMap<String, String>,
    pub local_numbers: HashMap<i32, i32>,
    pub local_booleans: HashMap<i32, bool>,
    pub actions: VecDeque<Action>,
    /// Doors: open now. Closed doors block the navigation mesh around them.
    pub open: bool,
    /// Doors: where the opening or closing animation is. `open` flips at once; the doorway only changes
    /// passability when the animation ends (opening) or at once (closing).
    pub door: DoorState,
    /// Half a door's width across its doorway, in metres. A closed door blocks all of it.
    pub door_half_width: f32,
    /// `Useable` of a placeable's template. Scenery and cutscene props have it off, and nothing else can be used.
    pub useable: bool,
    /// The direction across a door's doorway, from `Bearing` in the area.
    pub door_axis: [f32; 2],
    /// Doors and triggers that lead to another module: its name and the tag of the waypoint to arrive at.
    pub link_module: String,
    pub link_to: String,
    /// Triggers: outline in world XY.
    pub polygon: Vec<[f32; 2]>,
    /// Items carried, and which are equipped by slot.
    pub inventory: Vec<ObjId>,
    pub equipped: BTreeMap<i32, ObjId>,
    /// Items: the `baseitems.2da` row.
    pub base_item: i32,
    pub xp: i32,
    /// `repute.2da` row (`FactionID`), `racialtypes.2da` row (`Race`) and `SubraceIndex`.
    pub faction: i32,
    pub race: i32,
    pub subrace: i32,
    /// Force powers and feats the creature knows.
    pub spells: BTreeSet<i32>,
    pub feats: BTreeSet<i32>,
    /// Effects applied to it: handles into `World::effects`.
    pub effects: Vec<u32>,
    /// Facing in degrees, 0 = east, counter-clockwise (`GetFacing`).
    pub facing: f32,
    /// Strength, Dexterity, Constitution, Intelligence, Wisdom, Charisma.
    pub abilities: [i32; 6],
    pub natural_ac: i32,
    pub max_hp: i32,
    /// `classes.2da` row of the first class.
    pub class: i32,
    /// Metres within which an enemy is noticed.
    pub perception: f32,
    /// Ranks per skill row of `skills.2da`, unspent skill points, unspent (regular, bonus) feat choices.
    pub skills: Vec<i32>,
    pub skill_points: i32,
    pub feat_slots: (i32, i32),
    /// Knocked out (party members at 0 hit points).
    pub down: bool,
    pub force: i32,
    pub max_force: i32,
    pub challenge_rating: f32,
    /// `appearance.2da` row (the body), `heads.2da` row, 0 male / 1 female, portrait texture name.
    pub appearance: i32,
    pub head: i32,
    pub gender: i32,
    pub portrait: String,
    /// Items: the creature that carries it leaves it behind when it dies (`Dropable` in the UTC's lists).
    pub droppable: bool,
    /// Items: how many are stacked. Everything: the plot flag (cannot be hurt) and the good-evil value (0-100).
    pub stack: i32,
    pub plot: bool,
    pub alignment: i32,
}

/// A door's animation state; the number is seconds into the clip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DoorState {
    Closed,
    Opening(f32),
    Open,
    Closing(f32),
}

/// An effect created by one of the `Effect*` routines.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectRec {
    pub name: String,
    pub args: Vec<String>,
    /// The numeric arguments (ints and floats) in order.
    pub nums: Vec<f32>,
    /// For `EffectLinkEffects`: the effects joined.
    pub links: Vec<u32>,
}

impl Object {
    fn new(id: ObjId, kind: Kind, tag: &str) -> Object {
        Object {
            id,
            kind,
            tag: tag.to_string(),
            name: String::new(),
            template: String::new(),
            conversation: String::new(),
            position: [0.0; 3],
            hp: 10,
            dead: false,
            party_member: false,
            min_one_hp: false,
            spawned: false,
            locked: false,
            level: 1,
            scripts: BTreeMap::new(),
            local_numbers: HashMap::new(),
            local_booleans: HashMap::new(),
            actions: VecDeque::new(),
            open: false,
            door: DoorState::Closed,
            door_half_width: crate::DOOR_BLOCK_RADIUS,
            useable: true,
            door_axis: [1.0, 0.0],
            link_module: String::new(),
            link_to: String::new(),
            polygon: Vec::new(),
            inventory: Vec::new(),
            equipped: BTreeMap::new(),
            base_item: 0,
            xp: 0,
            faction: 0,
            race: 0,
            subrace: 0,
            spells: BTreeSet::new(),
            feats: BTreeSet::new(),
            effects: Vec::new(),
            facing: 0.0,
            abilities: [10; 6],
            natural_ac: 0,
            max_hp: 10,
            class: 0,
            perception: 20.0,
            skills: Vec::new(),
            skill_points: 0,
            feat_slots: (0, 0),
            down: false,
            force: 0,
            max_force: 0,
            challenge_rating: 1.0,
            appearance: 0,
            head: 0,
            gender: 0,
            portrait: String::new(),
            droppable: false,
            stack: 1,
            plot: false,
            alignment: 50,
        }
    }
}

/// The running game state.
pub struct World {
    pub(crate) inst: Install,
    pub(crate) routines: Rc<Routines>,
    programs: HashMap<String, Option<Rc<Program>>>,
    tlk: Option<Tlk>,
    pub objects: BTreeMap<ObjId, Object>,
    next_id: ObjId,
    pub pc: ObjId,
    pub area: ObjId,
    /// The object that entered the area (`GetEnteringObject`).
    pub entering: ObjId,
    pub global_numbers: HashMap<String, i32>,
    pub global_booleans: HashMap<String, bool>,
    /// Seconds since the world started.
    pub time: f32,
    pub delayed: Vec<Delayed>,
    /// Everything said in conversations, in order.
    pub transcript: Vec<Line>,
    /// Lines from `PrintString` and friends.
    pub log: Vec<String>,
    /// Notable things scripts did that the headless world only records (fades, movies, journal, ...).
    pub events: Vec<String>,
    /// Script failures that did not stop the run.
    pub errors: Vec<String>,
    /// Routines scripts called that are not implemented, with call counts. Calls return a default value.
    pub stubbed: BTreeMap<String, usize>,
    /// Number of the last user-defined event signalled (`GetUserDefinedEventNumber`).
    pub user_event: i32,
    /// Who last spoke to the object a script is running on (`GetLastSpeaker`).
    pub last_speaker: ObjId,
    /// Script-variable passed by `ExecuteScript`, innermost last (`GetRunScriptVar`).
    pub(crate) script_vars: Vec<i32>,
    /// A conversation is being played.
    pub in_conversation: bool,
    /// Name of the module loaded last.
    pub module: String,
    /// Walkable faces of the area; closed doors block it.
    pub nav: Option<NavMesh>,
    pub pc_mover: Mover,
    /// Movers of party members that are walking after the player, and the time until they replan.
    pub(crate) followers: BTreeMap<ObjId, Mover>,
    pub(crate) replan_in: f32,
    pub move_params: MoveParams,
    /// Object the player is walking to; reaching it triggers the interaction.
    pub pending: Option<ObjId>,
    /// (trigger, object) pairs whose object is currently inside the trigger.
    pub inside: HashSet<(ObjId, ObjId)>,
    /// Journal: quest tag to the entry state last set.
    pub journal: BTreeMap<String, i32>,
    /// Quest tags, the last changed first.
    pub journal_order: Vec<String>,
    /// `global.jrl`, once read.
    pub(crate) journal_file: Option<Rc<Gff>>,
    /// Party members besides the player, in join order.
    pub party: Vec<ObjId>,
    /// Which creature holds each `AddPartyMember` slot.
    pub npc_slots: HashMap<i32, ObjId>,
    /// A module change requested by a door or trigger: (module, waypoint tag). Carried out by `step`.
    pub transition_request: Option<(String, String)>,
    pub(crate) inv_cursor: HashMap<ObjId, usize>,
    /// Effects created so far; a handle is the index plus one (0 is the invalid effect).
    pub effects: Vec<EffectRec>,
    pub(crate) effect_cursor: HashMap<ObjId, usize>,
    /// Locations made by `Location` and `GetLocation`: position and facing in degrees. Handle = index + 1.
    pub locations: Vec<([f32; 3], f32)>,
    /// Creatures offered to the party by `AddAvailableNPCByTemplate`, by slot.
    pub available: BTreeMap<i32, String>,
    /// Who left the last trigger, who last opened a door, who last used a placeable.
    pub exiting: ObjId,
    pub last_opened_by: ObjId,
    pub last_used_by: ObjId,
    /// Routines answered with a fixed value because the feature behind them (combat, perception, item use) does not
    /// exist yet, with call counts. Unlike `stubbed` these are deliberate: "nothing is happening".
    pub constant_defaults: BTreeMap<String, usize>,
    pub(crate) repute: Option<TwoDa>,
    pub(crate) exptable: Vec<i32>,
    /// Seconds the player has pressed on people without getting nearer where it is going.
    pub(crate) pc_pressed: f32,
    /// The objects of each module the player has left, in the form a save keeps them.
    pub(crate) left_modules: BTreeMap<String, Vec<Struct>>,
    door_widths: HashMap<i64, Option<f32>>,
    pub(crate) plot: Option<TwoDa>,
    pub(crate) rng: u64,
    pub combat: crate::combat::CombatState,
    /// The Force power being cast, for scripts that ask.
    pub spell: crate::force::SpellContext,
    /// When each temporary effect (by handle) runs out.
    pub(crate) effect_expiry: HashMap<u32, f32>,
    /// Every player-side creature is knocked out.
    pub game_over: bool,
    /// An `ActionPauseConversation` ran and its resume has not yet.
    pub(crate) conv_paused: bool,
    /// When the pause in force began.
    pub(crate) conv_paused_at: Option<f32>,
    /// The character the player made: `SetPartyLeader(-1)` hands control back to it.
    pub hero: ObjId,
    /// Conversations wait for the player instead of running to the end (see `World::dialogue_view`).
    pub interactive_dialogue: bool,
    /// The player takes each level on a screen, instead of when the experience comes in.
    pub hold_levels: bool,
    /// `ShowPartySelectionGUI` waits for a screen, instead of bringing the forced members straight away.
    pub party_screen: bool,
    pub party_request: Option<crate::party::PartyRequest>,
    /// What has been seen of each map, by its picture: `SEEN_COLS` by `SEEN_ROWS` cells.
    pub explored: BTreeMap<String, Vec<bool>>,
    /// Rows of `tutorial.2da` that have come up in this game, and those scripts have asked for.
    pub tutorial_seen: BTreeSet<i32>,
    pub tutorial_requests: Vec<i32>,
    /// All the experience scripts have given, as against kills.
    pub plot_xp: i32,
    /// What the player has queued for their next attacks.
    pub combat_queue: VecDeque<crate::combat_actions::CombatAction>,
    /// Who the party screen left behind, by slot.
    pub bench: BTreeMap<i32, crate::party::BenchedMember>,
    /// The direction the player is being steered this step, and whether at a run.
    pub steer: Option<([f32; 2], bool)>,
    /// The game's difficulty: a row of `difficultyopt.2da`, 0 easy, 1 normal, 2 difficult.
    pub difficulty: i32,
    /// The container the player has open.
    pub opened: Option<ObjId>,
    /// Who last took something out of a container and what, and who last closed one.
    pub last_disturbed_by: ObjId,
    pub disturbed_item: ObjId,
    pub last_closed_by: ObjId,
    pub(crate) filled: std::collections::HashSet<ObjId>,
    pub(crate) conv: Option<crate::dialogue::ConvRun>,
    /// Party credits (`GetGold`).
    pub gold: i32,
    /// `SetCustomToken` values, substituted for `<CUSTOMn>` in text.
    pub tokens: HashMap<i32, String>,
    /// The world was loaded from a save (`GetLoadFromSaveGame`).
    pub loaded_from_save: bool,
    /// The last power each creature cast: (spell, it ran).
    pub last_power: HashMap<ObjId, (i32, bool)>,
    pub(crate) area_hits: Vec<ObjId>,
    pub(crate) area_next: usize,
    pub(crate) area_started: bool,
    /// The conversation played last, with each node's presentation data (for the cutscene sequencer).
    pub conversation: Option<crate::cutscene::Conversation>,
    /// When set, every routine a script calls is appended here (a debugging aid).
    pub trace: Option<Vec<String>>,
    /// Which spell cast each effect (`GetHasSpellEffect`).
    pub(crate) effect_spell: HashMap<u32, i32>,
    /// What is left of each `GetFirstObjectInShape` walk, by size and mask.
    pub(crate) shape_walks: HashMap<(u32, i32), std::collections::VecDeque<ObjId>>,
    tables: HashMap<String, Option<Rc<TwoDa>>>,
    /// What the item screens read from UTI files, by template.
    pub(crate) item_templates: HashMap<String, Rc<crate::items::ItemTemplate>>,
}

fn creature_defaults(o: &mut Object, utc: &Struct) {
    let classes = utc.list("ClassList").unwrap_or_default();
    let level: i64 = classes.iter().filter_map(|c| c.int("ClassLevel")).sum();
    o.level = level.max(1) as i32;
    for c in classes {
        for list in ["KnownList0", "KnownList1", "KnownList2"] {
            for s in c.list(list).unwrap_or_default() {
                if let Some(sp) = s.int("Spell") {
                    o.spells.insert(sp as i32);
                }
            }
        }
    }
    for f in utc.list("FeatList").unwrap_or_default() {
        if let Some(ft) = f.int("Feat") {
            o.feats.insert(ft as i32);
        }
    }
    for (i, a) in ["Str", "Dex", "Con", "Int", "Wis", "Cha"]
        .iter()
        .enumerate()
    {
        o.abilities[i] = utc.int(a).unwrap_or(10) as i32;
    }
    o.natural_ac = utc.int("NaturalAC").unwrap_or(0) as i32;
    o.class = classes.first().and_then(|c| c.int("Class")).unwrap_or(0) as i32;
    o.force = utc.int("CurrentForce").unwrap_or(0) as i32;
    o.max_force = utc.int("ForcePoints").unwrap_or(0) as i32;
    o.appearance = utc.int("Appearance_Type").unwrap_or(0) as i32;
    o.gender = utc.int("Gender").unwrap_or(0) as i32;
    o.alignment = utc.int("GoodEvil").unwrap_or(50) as i32;
    o.challenge_rating = utc.float("ChallengeRating").unwrap_or(1.0) as f32;
    o.skills = utc
        .list("SkillList")
        .unwrap_or_default()
        .iter()
        .map(|s| s.int("Rank").unwrap_or(0) as i32)
        .collect();
    o.faction = utc.int("FactionID").unwrap_or(0) as i32;
    o.race = utc.int("Race").unwrap_or(0) as i32;
    o.subrace = utc.int("SubraceIndex").unwrap_or(0) as i32;
    if let Some(hp) = utc.int("CurrentHitPoints") {
        o.hp = hp as i32;
    }
    o.max_hp = utc
        .int("MaxHitPoints")
        .or_else(|| utc.int("HitPoints"))
        .map_or(o.hp.max(1), |h| h as i32);
}

impl World {
    /// Open the game data and prepare an empty world containing the player character.
    pub fn new(mut inst: Install) -> Result<World> {
        let nss = inst.read("nwscript", ResType::NSS)?;
        let routines = Rc::new(Routines::parse(&kotor_core::latin1(&nss))?);
        let tlk = std::fs::read(inst.dir().join("dialog.tlk"))
            .ok()
            .and_then(|b| Tlk::parse(&b).ok());
        let mut w = World {
            inst,
            routines,
            programs: HashMap::new(),
            tlk,
            objects: BTreeMap::new(),
            next_id: 0x100,
            pc: 1,
            area: OBJECT_INVALID,
            entering: OBJECT_INVALID,
            global_numbers: HashMap::new(),
            global_booleans: HashMap::new(),
            time: 0.0,
            delayed: Vec::new(),
            transcript: Vec::new(),
            log: Vec::new(),
            events: Vec::new(),
            errors: Vec::new(),
            stubbed: BTreeMap::new(),
            user_event: 0,
            last_speaker: OBJECT_INVALID,
            script_vars: Vec::new(),
            in_conversation: false,
            module: String::new(),
            nav: None,
            pc_mover: Mover::new([0.0; 3], 0.0),
            followers: BTreeMap::new(),
            replan_in: 0.0,
            move_params: MoveParams::default(),
            pending: None,
            inside: HashSet::new(),
            journal: BTreeMap::new(),
            party: Vec::new(),
            npc_slots: HashMap::new(),
            transition_request: None,
            inv_cursor: HashMap::new(),
            effects: Vec::new(),
            effect_cursor: HashMap::new(),
            locations: Vec::new(),
            available: BTreeMap::new(),
            exiting: OBJECT_INVALID,
            last_opened_by: OBJECT_INVALID,
            last_used_by: OBJECT_INVALID,
            constant_defaults: BTreeMap::new(),
            repute: None,
            exptable: Vec::new(),
            pc_pressed: 0.0,
            left_modules: BTreeMap::new(),
            door_widths: HashMap::new(),
            plot: None,
            rng: 0x2545_F491_4F6C_DD1D,
            combat: crate::combat::CombatState::new(),
            spell: crate::force::SpellContext::default(),
            effect_expiry: HashMap::new(),
            game_over: false,
            conv_paused: false,
            conv_paused_at: None,
            hero: 1,
            interactive_dialogue: false,
            hold_levels: false,
            party_screen: false,
            party_request: None,
            explored: BTreeMap::new(),
            tutorial_seen: BTreeSet::new(),
            tutorial_requests: Vec::new(),
            plot_xp: 0,
            combat_queue: VecDeque::new(),
            bench: BTreeMap::new(),
            journal_order: Vec::new(),
            journal_file: None,
            steer: None,
            difficulty: 1,
            opened: None,
            last_disturbed_by: OBJECT_INVALID,
            disturbed_item: OBJECT_INVALID,
            last_closed_by: OBJECT_INVALID,
            filled: std::collections::HashSet::new(),
            conv: None,
            gold: 0,
            tokens: HashMap::new(),
            loaded_from_save: false,
            last_power: HashMap::new(),
            area_hits: Vec::new(),
            area_next: 0,
            area_started: false,
            conversation: None,
            trace: None,
            effect_spell: HashMap::new(),
            shape_walks: HashMap::new(),
            tables: HashMap::new(),
            item_templates: HashMap::new(),
        };
        let table =
            |w: &mut World, n: &str| TwoDa::parse(&w.inst.read(n, ResType::TWODA).ok()?).ok();
        w.repute = table(&mut w, "repute");
        w.plot = table(&mut w, "plot");
        if let Some(t) = table(&mut w, "exptable") {
            w.exptable = (0..t.row_count())
                .filter_map(|r| t.int(r, "xp").map(|v| v as i32))
                .collect();
        }
        let mut pc = Object::new(1, Kind::Creature, "player");
        pc.name = "Player".into();
        pc.hp = 30;
        w.objects.insert(1, pc);
        Ok(w)
    }

    /// A 2DA table from the install, read once.
    pub(crate) fn table(&mut self, name: &str) -> Option<Rc<TwoDa>> {
        let key = name.to_ascii_lowercase();
        if !self.tables.contains_key(&key) {
            let t = self
                .inst
                .read(&key, ResType::TWODA)
                .ok()
                .and_then(|b| TwoDa::parse(&b).ok())
                .map(Rc::new);
            self.tables.insert(key.clone(), t);
        }
        self.tables[&key].clone()
    }

    pub fn object(&self, id: ObjId) -> Option<&Object> {
        self.objects.get(&id)
    }

    pub fn object_mut(&mut self, id: ObjId) -> Option<&mut Object> {
        self.objects.get_mut(&id)
    }

    /// Id of an engine routine by name, from the game's own table.
    pub fn routine_id(&self, name: &str) -> Option<u16> {
        self.routines.by_name(name).map(|r| r.id)
    }

    /// Objects with this tag, lowest id first.
    pub fn by_tag(&self, tag: &str) -> Vec<ObjId> {
        // **Assumption:** scripts ask for the player with an empty tag. `k_pend_traskdl40` sends Trask to tag "".
        if tag.is_empty() {
            return vec![self.pc];
        }
        self.objects
            .values()
            .filter(|o| o.tag.eq_ignore_ascii_case(tag))
            .map(|o| o.id)
            .collect()
    }

    /// Create an object from a template (`CreateObject`), at `position`. `ty` is the template's resource type.
    pub(crate) fn spawn(
        &mut self,
        kind: Kind,
        template: &str,
        ty: ResType,
        position: [f32; 3],
    ) -> ObjId {
        let id = self.add(kind, template);
        self.fill_from_template(id, template, ty, &Struct::default());
        if let Some(o) = self.objects.get_mut(&id) {
            o.position = position;
        }
        id
    }

    /// Create an item from a UTI template, not yet carried by anyone.
    pub fn spawn_item(&mut self, template: &str) -> ObjId {
        self.spawn(Kind::Item, template, ResType(2025), [0.0; 3])
    }

    pub(crate) fn add(&mut self, kind: Kind, tag: &str) -> ObjId {
        let id = self.next_id;
        self.next_id += 1;
        self.objects.insert(id, Object::new(id, kind, tag));
        id
    }

    /// Text of a localised string: the talk table entry if it has a reference, else the first inline string.
    pub fn text(&self, l: &LocString) -> String {
        self.fill_tokens(self.raw_text(l))
    }

    /// Replace `<CUSTOMn>` with the value set by `SetCustomToken`.
    pub(crate) fn fill_tokens(&self, mut s: String) -> String {
        if s.contains("<CUSTOM") {
            for (n, v) in &self.tokens {
                s = s.replace(&format!("<CUSTOM{n}>"), v);
            }
        }
        s
    }

    fn raw_text(&self, l: &LocString) -> String {
        if l.strref >= 0 {
            if let Some(t) = self.tlk.as_ref().and_then(|t| t.get(l.strref as u32)) {
                return t;
            }
        }
        l.strings
            .first()
            .map(|(_, s)| s.clone())
            .unwrap_or_default()
    }

    /// Fill an object from a GFF template and its placement in the GIT.
    /// Half the width of a row of `genericdoors.2da`, from its model and read once.
    fn door_half_width(&mut self, row: i64) -> Option<f32> {
        if let Some(half) = self.door_widths.get(&row) {
            return *half;
        }
        let model = self
            .table("genericdoors")
            .and_then(|t| t.get(row.max(0) as usize, "modelname").map(str::to_string));
        let half = model.and_then(|m| {
            let mdl = self.inst.read(&m, ResType::MDL).ok()?;
            let mdx = self.inst.read(&m, ResType::MDX).unwrap_or_default();
            // The model's own `bbox` is 5 m for every door, so this measures its rest pose.
            let model = kotor_model::parse_model(&mdl, &mdx).ok()?;
            let half = kotor_model::to_obj(&model)
                .lines()
                .filter(|l| l.starts_with("v "))
                .filter_map(|l| l.split_whitespace().nth(1)?.parse::<f32>().ok())
                .fold(0.0, |most: f32, x| most.max(x.abs()));
            (half > 0.1).then_some(half)
        });
        self.door_widths.insert(row, half);
        half
    }

    pub(crate) fn fill_from_template(
        &mut self,
        id: ObjId,
        template: &str,
        ty: ResType,
        placement: &Struct,
    ) {
        let utx = self
            .inst
            .read(template, ty)
            .ok()
            .and_then(|b| Gff::parse(&b).ok());
        let mut name = String::new();
        let mut scripts = BTreeMap::new();
        let mut conversation = String::new();
        let mut tag = None;
        if let Some(g) = utx.as_ref().filter(|_| ty == ResType(2044)) {
            let on = g.root.int("Useable").is_some_and(|u| u != 0);
            if let Some(o) = self.objects.get_mut(&id) {
                o.useable = on;
            }
        }
        if let Some(g) = utx.as_ref().filter(|_| ty == ResType(2042)) {
            if let Some(half) = self.door_half_width(g.root.int("GenericType").unwrap_or(0)) {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.door_half_width = half;
                }
            }
        }
        if let Some(g) = &utx {
            let r = &g.root;
            if let Some(l) = r
                .loc("FirstName")
                .or_else(|| r.loc("LocName"))
                .or_else(|| r.loc("LocalizedName"))
            {
                name = self.text(l);
            }
            if let Some(l) = r.loc("LastName") {
                let last = self.text(l);
                if !last.is_empty() {
                    name = format!("{name} {last}");
                }
            }
            conversation = r.str("Conversation").unwrap_or_default().to_string();
            tag = r.str("Tag").map(str::to_string);
            for (label, v) in &r.fields {
                if let GffValue::ResRef(s) = v {
                    if !s.is_empty() && (label.starts_with("Script") || label.starts_with("On")) {
                        scripts.insert(label.clone(), s.clone());
                    }
                }
            }
        }
        let droppable = |e: &Struct| e.int("Dropable").is_some_and(|d| d != 0);
        let equip: Vec<(i32, String, bool)> = utx
            .as_ref()
            .and_then(|g| g.root.list("Equip_ItemList"))
            .unwrap_or_default()
            .iter()
            .filter_map(|e| {
                // The struct id is a bit mask with one bit per inventory slot.
                let r = e.str("EquippedRes")?;
                (e.id > 0 && !r.is_empty())
                    .then(|| (e.id.trailing_zeros() as i32, r.to_string(), droppable(e)))
            })
            .collect();
        // A placeable's `ItemList` waits until it is opened; a creature carries its own from the start.
        let carried: Vec<(String, bool)> = utx
            .as_ref()
            .filter(|_| ty == ResType(2027))
            .and_then(|g| g.root.list("ItemList"))
            .unwrap_or_default()
            .iter()
            .filter_map(|e| Some((e.str("InventoryRes")?.to_string(), droppable(e))))
            .filter(|(r, _)| !r.is_empty())
            .collect();
        let perception = utx
            .as_ref()
            .and_then(|g| g.root.int("PerceptionRange"))
            .and_then(|r| self.table("ranges")?.int(r as usize, "primaryrange"));
        let f = |l: &str| placement.float(l).unwrap_or(0.0) as f32;
        if let Some(o) = self.objects.get_mut(&id) {
            o.template = template.to_string();
            o.name = name;
            o.conversation = conversation;
            o.scripts = scripts;
            o.position = [
                f("XPosition") + f("X"),
                f("YPosition") + f("Y"),
                f("ZPosition") + f("Z"),
            ];
            if placement.float("XOrientation").is_some() {
                o.facing = f("YOrientation").atan2(f("XOrientation")).to_degrees();
            }
            if placement.float("Bearing").is_some() {
                o.door_axis = [f("Bearing").cos(), f("Bearing").sin()];
            }
            // A tag in the area beats the template's: every waypoint shares one template, as `end_jump_pc` shows.
            if placement.str("Tag").is_none_or(str::is_empty) {
                if let Some(t) = tag.filter(|t| !t.is_empty()) {
                    o.tag = t;
                }
            }
            if let Some(g) = &utx {
                creature_defaults(o, &g.root);
                o.hp = g.root.int("CurrentHitPoints").map_or(o.hp, |h| h as i32);
                o.locked = g.root.int("Locked").is_some_and(|l| l != 0);
                o.base_item = g.root.int("BaseItem").unwrap_or(0) as i32;
                o.stack = g.root.int("StackSize").unwrap_or(1).max(1) as i32;
                o.plot = g.root.int("Plot").is_some_and(|p| p != 0);
                o.min_one_hp = g.root.int("Min1HP").is_some_and(|m| m != 0);
            }
            if let Some(p) = perception {
                o.perception = p as f32;
            }
            // Links to other modules come from the placement, else the template.
            let link = |field: &str| {
                placement
                    .str(field)
                    .filter(|s| !s.is_empty())
                    .or_else(|| {
                        utx.as_ref()
                            .and_then(|g| g.root.str(field))
                            .filter(|s| !s.is_empty())
                    })
                    .unwrap_or_default()
                    .to_string()
            };
            o.link_module = link("LinkedToModule");
            o.link_to = link("LinkedTo");
            o.polygon = placement
                .list("Geometry")
                .unwrap_or_default()
                .iter()
                .map(|p| {
                    [
                        o.position[0] + p.float("PointX").unwrap_or(0.0) as f32,
                        o.position[1] + p.float("PointY").unwrap_or(0.0) as f32,
                    ]
                })
                .collect();
        }
        for (slot, res, drop) in equip {
            let item = self.spawn_item(&res);
            if let Some(i) = self.objects.get_mut(&item) {
                i.droppable = drop;
            }
            if let Some(o) = self.objects.get_mut(&id) {
                o.equipped.insert(slot, item);
            }
        }
        for (res, drop) in carried {
            let item = self.spawn_item(&res);
            if let Some(i) = self.objects.get_mut(&item) {
                i.droppable = drop;
            }
            if let Some(o) = self.objects.get_mut(&id) {
                o.inventory.push(item);
            }
        }
    }

    /// Load a module's area: the area and module objects, and every creature, door, placeable, waypoint and
    /// trigger of its GIT, with their event scripts, conversations and names.
    pub fn load_module(&mut self, module: &str) -> Result<()> {
        self.inst.load_module(module)?;
        // Modules have their own copies under the same names: `k_pend_sith_9` in `end_m01aa` turns a soldier's AI off, and
        // in `end_m01ab` it does not.
        self.programs.clear();
        self.item_templates.clear();
        let ifo = Gff::parse(&self.inst.read("module", ResType::IFO)?)?;
        let area_name = ifo
            .root
            .str("Mod_Entry_Area")
            .ok_or_else(|| Error::format("module.ifo has no Mod_Entry_Area"))?
            .to_string();
        let mid = self.add(Kind::Module, module);
        if let Some(m) = self.objects.get_mut(&mid) {
            for (label, v) in &ifo.root.fields {
                if let GffValue::ResRef(s) = v {
                    if !s.is_empty() && label.starts_with("Mod_On") {
                        m.scripts.insert(label.clone(), s.clone());
                    }
                }
            }
        }
        let are = Gff::parse(&self.inst.read(&area_name, ResType::ARE)?)?;
        let area = self.add(Kind::Area, &area_name);
        self.area = area;
        let area_label = are
            .root
            .loc("Name")
            .map(|l| self.text(l))
            .unwrap_or_default();
        if let Some(a) = self.objects.get_mut(&area) {
            a.name = area_label;
            for (label, v) in &are.root.fields {
                if let GffValue::ResRef(s) = v {
                    if !s.is_empty() && label.starts_with("On") {
                        a.scripts.insert(label.clone(), s.clone());
                    }
                }
            }
        }
        // The player starts at the module's entry point.
        let f = |l: &str| ifo.root.float(l).unwrap_or(0.0) as f32;
        let pc = self.pc;
        if let Some(p) = self.objects.get_mut(&pc) {
            p.position = [f("Mod_Entry_X"), f("Mod_Entry_Y"), f("Mod_Entry_Z")];
        }
        let git = Gff::parse(&self.inst.read(&area_name, ResType::GIT)?)?;
        let lists: [(&str, Kind, ResType); 5] = [
            ("Creature List", Kind::Creature, ResType(2027)),
            ("Door List", Kind::Door, ResType(2042)),
            ("Placeable List", Kind::Placeable, ResType(2044)),
            ("WaypointList", Kind::Waypoint, ResType(2058)),
            ("TriggerList", Kind::Trigger, ResType(2032)),
        ];
        for (label, kind, ty) in lists {
            for s in git.root.list(label).unwrap_or_default() {
                let tpl = s.str("TemplateResRef").unwrap_or_default().to_string();
                let tag = s.str("Tag").unwrap_or(&tpl).to_string();
                let id = self.add(kind, &tag);
                self.fill_from_template(id, &tpl, ty, s);
            }
        }
        self.module = module.to_string();
        self.build_nav(&area_name)?;
        let pos = self.objects.get(&pc).map_or([0.0; 3], |p| p.position);
        self.pc_mover = Mover::new(pos, std::f32::consts::FRAC_PI_2);
        Ok(())
    }

    fn program(&mut self, name: &str) -> Option<Rc<Program>> {
        let key = name.to_ascii_lowercase();
        if let Some(p) = self.programs.get(&key) {
            return p.clone();
        }
        let bytes = match self.inst.read(&key, ResType::NCS) {
            Ok(b) => b,
            // Only a script that is not there is remembered as missing. A file that could not be read this time may be
            // read the next, and says why.
            Err(kotor_core::Error::NotFound(_)) => {
                self.programs.insert(key, None);
                return None;
            }
            Err(e) => {
                self.errors.push(format!("script {key}: {e}"));
                return None;
            }
        };
        let p = disassemble(&bytes).ok().map(Rc::new);
        self.programs.insert(key, p.clone());
        p
    }

    /// Run a script by name with `caller` as OBJECT_SELF. Failures are recorded in `errors` and returned.
    pub fn run_script(
        &mut self,
        name: &str,
        caller: ObjId,
    ) -> std::result::Result<Outcome, String> {
        let Some(prog) = self.program(name) else {
            let e = format!("script {name} not found");
            self.errors.push(e.clone());
            return Err(e);
        };
        let routines = self.routines.clone();
        let vm = Vm::new(&prog, &routines);
        let mut ad = Adapter {
            world: self,
            script: name.to_ascii_lowercase(),
        };
        vm.run(caller, &mut ad).map_err(|e| self.note(name, e))
    }

    /// Run an already decoded program (tests and tools use this for hand-built scripts).
    pub fn run_program(
        &mut self,
        prog: &Program,
        caller: ObjId,
    ) -> std::result::Result<Outcome, String> {
        let routines = self.routines.clone();
        let vm = Vm::new(prog, &routines);
        let mut ad = Adapter {
            world: self,
            script: "<inline>".into(),
        };
        vm.run(caller, &mut ad)
            .map_err(|e| self.note("<inline>", e))
    }

    /// Replay a deferred block that `script` saved with `STORESTATE`.
    pub fn run_saved(
        &mut self,
        script: &str,
        state: &SavedState,
        caller: ObjId,
    ) -> std::result::Result<Outcome, String> {
        let Some(prog) = self.program(script) else {
            let e = format!("script {script} not found");
            self.errors.push(e.clone());
            return Err(e);
        };
        let routines = self.routines.clone();
        let vm = Vm::new(&prog, &routines);
        let mut ad = Adapter {
            world: self,
            script: script.to_ascii_lowercase(),
        };
        vm.run_state(state, caller, &mut ad)
            .map_err(|e| self.note(script, e))
    }

    fn note(&mut self, script: &str, e: VmError) -> String {
        let m = format!("{script}: {e}");
        self.errors.push(m.clone());
        m
    }

    /// Run a condition script: its integer result, or true when the script is empty or cannot be loaded
    /// (a missing condition does not block the dialogue).
    pub fn check(&mut self, script: &str, caller: ObjId) -> bool {
        if script.is_empty() {
            return true;
        }
        match self.run_script(script, caller) {
            Ok(out) => out.int_result().is_none_or(|v| v != 0),
            Err(_) => true,
        }
    }

    /// Queue an action on an object.
    pub fn queue(&mut self, id: ObjId, a: Action) {
        if let Some(o) = self.objects.get_mut(&id) {
            o.actions.push_back(a);
        }
    }

    /// Advance the clock by `dt`: run due delayed commands and step each object's action queue.
    pub fn tick(&mut self, dt: f32) {
        self.time += dt;
        let now = self.time;
        let (due, later): (Vec<Delayed>, Vec<Delayed>) = std::mem::take(&mut self.delayed)
            .into_iter()
            .partition(|d| d.due <= now);
        self.delayed = later;
        for d in due {
            let _ = self.run_saved(&d.script, &d.state, d.caller);
        }
        let ids: Vec<ObjId> = self.objects.keys().copied().collect();
        for id in ids {
            let mut budget = dt;
            // A queue that keeps adding to itself goes on next tick, and cannot hang this one.
            let mut left = ACTIONS_PER_TICK;
            while let Some(head) = self
                .objects
                .get_mut(&id)
                .and_then(|o| o.actions.front_mut())
            {
                if left == 0 {
                    break;
                }
                left -= 1;
                match head {
                    Action::Wait(left) => {
                        if *left > budget {
                            *left -= budget;
                            break;
                        }
                        budget -= *left;
                        if let Some(o) = self.objects.get_mut(&id) {
                            o.actions.pop_front();
                        }
                    }
                    Action::MoveTo { .. } => {
                        let from = self.objects.get(&id).map(|o| o.position);
                        if !self.step_move(id, budget) {
                            break;
                        }
                        budget = 0.0;
                        if let Some(o) = self.objects.get_mut(&id) {
                            o.actions.pop_front();
                            // Patrols in `tar_m02ac` send a creature to where it already stands and then do it again.
                            if from == Some(o.position) {
                                o.actions.push_front(Action::Wait(STAND_SECONDS));
                            }
                        }
                    }
                    _ => {
                        let a = self
                            .objects
                            .get_mut(&id)
                            .and_then(|o| o.actions.pop_front());
                        if let Some(a) = a {
                            self.perform(id, a);
                        }
                    }
                }
            }
        }
    }

    fn perform(&mut self, id: ObjId, a: Action) {
        match a {
            Action::Wait(_) | Action::MoveTo { .. } => {}
            Action::OpenDoor(d) => {
                self.open_door(d);
            }
            Action::CloseDoor(d) => self.close_door(d),
            Action::Jump(p) => self.teleport(id, p),
            Action::Attack(t) => self.perform_attack(id, t),
            Action::Cast {
                spell,
                target,
                cheat,
            } => {
                if let Err(e) = self.cast_spell(id, spell, target, cheat) {
                    // The round still ends, or the AI would never choose again.
                    self.combat.casters.insert(id);
                    self.events
                        .push(format!("{} cannot cast {spell}: {e}", self.describe(id)));
                }
            }
            Action::Recorded(what) => {
                match what.as_str() {
                    "ActionPauseConversation" => self.conv_paused = true,
                    "ActionResumeConversation" => self.conv_paused = false,
                    _ => {}
                }
                self.events.push(format!("{}: {what}", self.describe(id)));
            }
            Action::DoCommand { script, state } => {
                let _ = self.run_saved(&script, &state, id);
            }
            Action::StartConversation {
                target,
                dialogue,
                anywhere,
            } => {
                let far = match (self.objects.get(&id), self.objects.get(&target)) {
                    (Some(a), Some(b)) => {
                        a.kind == Kind::Creature
                            && crate::follow::dist2d(a.position, b.position)
                                > crate::play::TALK_RANGE
                    }
                    _ => false,
                };
                if self.interactive_dialogue && !anywhere && far {
                    // **Assumption:** a speaker runs up to the listener first, so the talk behind this waits for it.
                    if let Some(o) = self.objects.get_mut(&id) {
                        o.actions.push_front(Action::StartConversation {
                            target,
                            dialogue,
                            anywhere: true,
                        });
                        o.actions.push_front(Action::MoveTo {
                            target: Some(target),
                            to: [0.0; 3],
                            range: crate::play::TALK_RANGE,
                            run: true,
                            walker: None,
                        });
                    }
                    return;
                }
                let _ = self.converse(id, target, &dialogue, &mut crate::FirstReply);
            }
        }
    }

    /// Put an object somewhere at once; the player's walk is cancelled.
    pub fn teleport(&mut self, id: ObjId, pos: [f32; 3]) {
        if let Some(o) = self.objects.get_mut(&id) {
            o.position = pos;
        }
        if id == self.pc {
            self.pc_mover = Mover::new(pos, self.pc_mover.heading);
        }
    }

    /// Experience level for a total of experience points, from `exptable.2da`.
    pub fn level_for_xp(&self, xp: i32) -> i32 {
        self.exptable
            .iter()
            .take_while(|&&need| need <= xp)
            .count()
            .max(1) as i32
    }

    /// Text of a talk-table entry, empty when missing.
    pub fn strref_text(&self, strref: i32) -> String {
        if strref < 0 {
            return String::new();
        }
        self.fill_tokens(
            self.tlk
                .as_ref()
                .and_then(|t| t.get(strref as u32))
                .unwrap_or_default(),
        )
    }

    /// Experience a plot is worth (`plot.2da`, row labelled with the plot name, column `xp`).
    pub fn plot_xp(&self, plot: &str) -> i32 {
        let Some(t) = self.plot.as_ref() else {
            return 0;
        };
        (0..t.row_count())
            .find(|&r| {
                t.get(r, "label")
                    .is_some_and(|l| l.eq_ignore_ascii_case(plot))
            })
            .and_then(|r| t.int(r, "xp"))
            .unwrap_or(0) as i32
    }

    /// `GetIsEnemy`-style reputation of `target` as seen by `source`, 0..=100, from `repute.2da` (50 when unknown).
    pub fn reputation(&self, source: ObjId, target: ObjId) -> i32 {
        // Everyone is their own friend.
        if source == target && self.objects.contains_key(&source) {
            return 100;
        }
        let (Some(s), Some(t), Some(table)) = (
            self.objects.get(&source),
            self.objects.get(&target),
            self.repute.as_ref(),
        ) else {
            return 50;
        };
        // Rows are factions by id; each faction's column is named after its row label. There is no `player` column, so
        // how anyone sees the player comes from the player's row, read the other way. Without this no enemy fights back.
        let label = |f: i32| {
            table
                .get(f.max(0) as usize, "label")
                .map(str::to_ascii_lowercase)
        };
        let seen = |from: i32, of: i32| label(of).and_then(|l| table.int(from.max(0) as usize, &l));
        seen(s.faction, t.faction)
            .or_else(|| seen(t.faction, s.faction))
            .map_or(50, |v| v as i32)
    }

    pub(crate) fn describe(&self, id: ObjId) -> String {
        match self.objects.get(&id) {
            Some(o) if !o.name.is_empty() => format!("{} ({})", o.name, o.tag),
            Some(o) => o.tag.clone(),
            None => format!("#{id}"),
        }
    }

    /// Tick until nothing is scheduled and no object has queued work, or `limit` seconds have passed.
    /// Returns the seconds simulated.
    pub fn run_until_idle(&mut self, limit: f32) -> f32 {
        let start = self.time;
        while self.time - start < limit {
            let busy =
                !self.delayed.is_empty() || self.objects.values().any(|o| !o.actions.is_empty());
            if !busy {
                break;
            }
            self.tick(0.1);
        }
        self.time - start
    }

    /// The player talks to `target`: its dialogue event script runs (`ScriptDialogue`, usually the stock
    /// `k_hen_dialogue01` that hands over to the AI master script). If that script does not start a
    /// conversation by itself, the target's own conversation starts directly. Returns lines added.
    pub fn talk_to(&mut self, target: ObjId) -> usize {
        let before = self.transcript.len();
        let started = self.events.len();
        self.last_speaker = self.pc;
        let script = self
            .objects
            .get(&target)
            .and_then(|o| o.scripts.get("ScriptDialogue").cloned());
        if let Some(s) = script {
            let _ = self.run_script(&s, target);
            // Queued conversation actions run on the next ticks.
            for _ in 0..20 {
                self.tick(0.1);
            }
        }
        let began = self.events[started..]
            .iter()
            .any(|e| e.starts_with("conversation "));
        if !began {
            let _ = self.converse(target, self.pc, "", &mut crate::FirstReply);
        }
        self.transcript.len() - before
    }

    /// Tick (0.1 s steps) until an event starting with `prefix` has been recorded, or `limit` seconds pass.
    /// Returns whether it happened. Stops right after the tick that recorded it, so effects scheduled a
    /// moment later (for example `DelayCommand(0.2, ...)`) have not run yet.
    pub fn run_until_event(&mut self, prefix: &str, limit: f32) -> bool {
        let start = self.time;
        while self.time - start < limit {
            if self.events.iter().any(|e| e.starts_with(prefix)) {
                return true;
            }
            self.tick(0.1);
        }
        self.events.iter().any(|e| e.starts_with(prefix))
    }

    /// A creature's `ScriptSpawn`, once. This is where it gets its AI flags and listening patterns: `k_pend_sith_9`
    /// keeps the soldiers behind the first door from fighting until their cutscene turns them on.
    pub(crate) fn run_spawn_script(&mut self, id: ObjId) {
        if let Some(o) = self.objects.get_mut(&id) {
            if !o.spawned {
                o.spawned = true;
                self.run_event(id, "ScriptSpawn");
            }
        }
    }

    /// Run the area's `OnEnter` script with the player as the entering object, as when a module is entered.
    pub fn enter_area(&mut self) {
        let waiting: Vec<ObjId> = self
            .objects
            .values()
            .filter(|o| o.kind == Kind::Creature && !o.spawned)
            .map(|o| o.id)
            .collect();
        for id in waiting {
            self.run_spawn_script(id);
        }
        self.entering = self.pc;
        let area = self.area;
        let script = self
            .objects
            .get(&area)
            .and_then(|a| a.scripts.get("OnEnter").cloned());
        if let Some(s) = script {
            let _ = self.run_script(&s, area);
        }
    }
}
