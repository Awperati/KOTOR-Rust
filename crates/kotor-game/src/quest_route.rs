//! Where to go next for a quest: the door, the lift or the person, for the compass's hidden marker.
//!
//! Two sources. An index derived from the install says who moves each journal state on, and which door leads where; it is built
//! on the first run, off the game's thread, into the local cache (never the repo: it comes from the game's files). The hand table
//! `docs/quest_routes.tsv` beats it, for what the files do not say.
//!
//! **The rule, ours and not the game's:** the next state of a quest is the smallest one above its current state that something
//! sets, or the one after if that one has nothing to point at from here. A branching quest where that is wrong gets a row in the
//! table.
//!
//! **Assumed:** routine 367 is `AddJournalQuestEntry` and 509 `StartNewModule`, with their string and number the nearest constants
//! before the call. A script that computes them is not seen.

use crate::world::{Kind, Object, World};
use kotor_archive::Install;
use kotor_core::ResType;
use kotor_gff::dlg::{Dialog, Node};
use kotor_gff::{Gff, Struct, Value};
use kotor_script::{disassemble, Op};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

const TABLE_TEXT: &str = include_str!("../../../docs/quest_routes.tsv");
const JOURNAL_FILE: ResType = ResType(2056);
const ROUTINE_JOURNAL: u16 = 367;
const ROUTINE_NEW_MODULE: u16 = 509;
/// How far before a call its constants are looked for.
const ARGUMENTS_WITHIN: usize = 10;
const INDEX_VERSION: u32 = 1;
/// Where another planet is reached from.
const THE_SHIP: &str = "ebo_m12aa";
/// On the ship: the galaxy map, and the way out onto the planet it stands on.
const GALAXY_MAP: &str = "GalaxyMap";
const SHIP_EXIT: &str = "AreaTransition";
const CUT_SCENE_MODULES: &str = "stunt_";
/// Planets by the start of their modules' names: the row of `planetary.2da`, and our name for the label.
const PLANETS: [(&str, i32, &str); 9] = [
    ("end_", 0, "the Endar Spire"),
    ("tar_", 1, "Taris"),
    ("dan", 3, "Dantooine"),
    ("tat_", 4, "Tatooine"),
    ("kas_", 5, "Kashyyyk"),
    ("man", 6, "Manaan"),
    ("korr_", 7, "Korriban"),
    ("unk_", 9, "the Unknown World"),
    ("sta_", 10, "the Star Forge"),
];

fn planet_of(module: &str) -> Option<(i32, &'static str)> {
    PLANETS
        .iter()
        .find(|(start, ..)| module.starts_with(start))
        .map(|(_, row, name)| (*row, *name))
}

/// What the player does at a step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum StepKind {
    Door,
    Elevator,
    Talk,
    Item,
    Area,
    Kill,
    /// On the ship, for a quest on another planet: fly there.
    GalaxyMap,
}

impl StepKind {
    pub fn name(self) -> &'static str {
        match self {
            StepKind::Door => "door",
            StepKind::Elevator => "elevator",
            StepKind::Talk => "talk",
            StepKind::Item => "item",
            StepKind::Area => "area",
            StepKind::Kill => "kill",
            StepKind::GalaxyMap => "galaxy_map",
        }
    }

    fn of_name(s: &str) -> Option<StepKind> {
        [
            StepKind::Door,
            StepKind::Elevator,
            StepKind::Talk,
            StepKind::Item,
            StepKind::Area,
            StepKind::Kill,
            StepKind::GalaxyMap,
        ]
        .into_iter()
        .find(|k| k.name().eq_ignore_ascii_case(s))
    }
}

/// The next thing to do for a quest, from where the player is now.
#[derive(Clone, Debug, PartialEq)]
pub struct NextStep {
    /// Stable while the step stays the same: `quest/state/kind/tag`.
    pub id: String,
    pub kind: StepKind,
    pub position: [f32; 3],
    pub label: String,
    /// The module the quest's target is in, which may be several exits away.
    pub final_module: String,
}

#[derive(Clone, Debug, PartialEq)]
struct Link {
    tag: String,
    kind: StepKind,
    to: String,
}

#[derive(Clone, Debug, PartialEq)]
struct Target {
    module: String,
    /// Empty: only the module is known.
    tag: String,
    kind: StepKind,
    label: String,
}

/// What the install says about quests and exits. Plain data, tags and numbers, with no text of the game's.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct QuestIndex {
    links: BTreeMap<String, Vec<Link>>,
    setters: BTreeMap<String, BTreeMap<i32, Vec<Target>>>,
    ends: BTreeMap<String, BTreeSet<i32>>,
}

impl QuestIndex {
    fn add_link(&mut self, module: &str, tag: &str, kind: StepKind, to: &str) {
        let to = to.to_ascii_lowercase();
        // A cut-scene's stage is not a place to go to.
        if to.starts_with(CUT_SCENE_MODULES) || to == module {
            return;
        }
        let v = self.links.entry(module.to_string()).or_default();
        if !v
            .iter()
            .any(|l| l.tag.eq_ignore_ascii_case(tag) && l.to == to)
        {
            v.push(Link {
                tag: tag.to_string(),
                kind,
                to,
            });
        }
    }

    fn add_setter(&mut self, quest: &str, state: i32, module: &str, tag: &str, kind: StepKind) {
        let v = self
            .setters
            .entry(quest.to_ascii_lowercase())
            .or_default()
            .entry(state)
            .or_default();
        if !v
            .iter()
            .any(|t| t.module == module && t.tag.eq_ignore_ascii_case(tag))
        {
            v.push(Target {
                module: module.to_string(),
                tag: tag.to_string(),
                kind,
                label: String::new(),
            });
        }
    }

    /// The exit out of `from` on a shortest way to `to`, and how many exits that way has.
    fn first_exit(&self, from: &str, to: &str) -> Option<(&Link, usize)> {
        let mut seen = BTreeSet::from([from]);
        let mut queue: VecDeque<(&str, Option<&Link>, usize)> = VecDeque::from([(from, None, 0)]);
        while let Some((module, first, hops)) = queue.pop_front() {
            for l in self.links.get(module).into_iter().flatten() {
                let first = first.or(Some(l));
                if l.to == to {
                    return first.map(|f| (f, hops + 1));
                }
                if seen.insert(l.to.as_str()) {
                    queue.push_back((l.to.as_str(), first, hops + 1));
                }
            }
        }
        None
    }

    /// One line for each fact, for the cache.
    pub fn to_text(&self, fingerprint: u64) -> String {
        let mut out = format!("quest-index\t{INDEX_VERSION}\t{fingerprint}\n");
        for (module, links) in &self.links {
            for l in links {
                out += &format!("L\t{module}\t{}\t{}\t{}\n", l.tag, l.kind.name(), l.to);
            }
        }
        for (quest, states) in &self.setters {
            for (state, targets) in states {
                for t in targets {
                    out += &format!(
                        "S\t{quest}\t{state}\t{}\t{}\t{}\n",
                        t.module,
                        t.tag,
                        t.kind.name()
                    );
                }
            }
        }
        for (quest, states) in &self.ends {
            for state in states {
                out += &format!("E\t{quest}\t{state}\n");
            }
        }
        out
    }

    /// Back from `to_text`; `None` if it was written for another install or by another version.
    pub fn from_text(text: &str, fingerprint: u64) -> Option<QuestIndex> {
        let mut lines = text.lines();
        let head: Vec<&str> = lines.next()?.split('\t').collect();
        if head
            != [
                "quest-index",
                &INDEX_VERSION.to_string(),
                &fingerprint.to_string(),
            ]
        {
            return None;
        }
        let mut i = QuestIndex::default();
        for line in lines {
            let f: Vec<&str> = line.split('\t').collect();
            match f.as_slice() {
                ["L", module, tag, kind, to] => {
                    i.add_link(module, tag, StepKind::of_name(kind)?, to)
                }
                ["S", quest, state, module, tag, kind] => i.add_setter(
                    quest,
                    state.parse().ok()?,
                    module,
                    tag,
                    StepKind::of_name(kind)?,
                ),
                ["E", quest, state] => {
                    i.ends
                        .entry(quest.to_string())
                        .or_default()
                        .insert(state.parse().ok()?);
                }
                _ => return None,
            }
        }
        Some(i)
    }
}

/// One row of `docs/quest_routes.tsv`: while the quest is in this state, go here.
#[derive(Clone, Debug, PartialEq)]
pub struct RouteRow {
    pub quest: String,
    pub state: i32,
    pub module: String,
    pub kind: StepKind,
    pub tag: String,
    pub label: String,
}

/// The rows of `docs/quest_routes.tsv`. A line that does not read is an error here, so that a test finds it.
pub fn parse_routes(text: &str) -> Result<Vec<RouteRow>, String> {
    let mut rows = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        let [quest, state, module, kind, tag, label] = f.as_slice() else {
            return Err(format!("line {}: wants 6 columns, has {}", n + 1, f.len()));
        };
        rows.push(RouteRow {
            quest: quest.to_ascii_lowercase(),
            state: state
                .parse()
                .map_err(|_| format!("line {}: state {state}", n + 1))?,
            module: module.to_ascii_lowercase(),
            kind: StepKind::of_name(kind).ok_or(format!("line {}: kind {kind}", n + 1))?,
            tag: tag.to_string(),
            label: label.to_string(),
        });
    }
    Ok(rows)
}

fn route_rows() -> &'static [RouteRow] {
    static ROWS: OnceLock<Vec<RouteRow>> = OnceLock::new();
    ROWS.get_or_init(|| parse_routes(TABLE_TEXT).unwrap_or_default())
}

/// The nearest string and number given to the call at `at`, as a script pushes its arguments last to first.
fn arguments(ops: &[&Op], at: usize) -> (Option<String>, Option<i32>) {
    let (mut text, mut number) = (None, None);
    for op in ops[at.saturating_sub(ARGUMENTS_WITHIN)..at].iter().rev() {
        match op {
            Op::ConstS(s) if text.is_none() => text = Some(s.clone()),
            Op::ConstI(n) if number.is_none() => number = Some(*n),
            _ => {}
        }
    }
    (text, number)
}

/// What a script does that a route cares about.
#[derive(Default)]
struct ScriptFacts {
    journal: Vec<(String, i32)>,
    goes_to: Vec<String>,
}

fn script_facts(bytes: &[u8]) -> ScriptFacts {
    let mut facts = ScriptFacts::default();
    let Ok(program) = disassemble(bytes) else {
        return facts;
    };
    let ops: Vec<&Op> = program.instructions.iter().map(|i| &i.op).collect();
    for (at, op) in ops.iter().enumerate() {
        let Op::Action { routine, .. } = op else {
            continue;
        };
        if *routine == ROUTINE_JOURNAL {
            if let (Some(q), Some(n)) = arguments(&ops, at) {
                facts.journal.push((q, n));
            }
        } else if *routine == ROUTINE_NEW_MODULE {
            if let (Some(m), _) = arguments(&ops, at) {
                if !m.is_empty() {
                    facts.goes_to.push(m);
                }
            }
        }
    }
    facts
}

/// The state a node writes to the journal, if any.
fn node_quest(n: &Node) -> Option<(String, i32)> {
    let quest = n.extra.iter().find_map(|(l, v)| match (l.as_str(), v) {
        ("Quest", Value::String(q)) if !q.is_empty() => Some(q.clone()),
        _ => None,
    })?;
    let state = n.extra.iter().find_map(|(l, v)| match (l.as_str(), v) {
        ("QuestEntry", Value::Dword(i)) => Some(*i as i32),
        _ => None,
    })?;
    Some((quest, state))
}

/// What a script field of an object means for whoever looks for that object.
fn script_user_kind(kind: Kind, field: &str) -> Option<StepKind> {
    let f = field.to_ascii_lowercase();
    match kind {
        Kind::Creature => f.contains("death").then_some(StepKind::Kill),
        Kind::Placeable => Some(StepKind::Item),
        Kind::Door => Some(StepKind::Door),
        Kind::Trigger => Some(StepKind::Area),
        _ => None,
    }
}

fn looks_like_a_lift(tag: &str, template: &str) -> bool {
    [tag, template].iter().any(|s| {
        let s = s.to_ascii_lowercase();
        s.contains("elev") || s.contains("lift")
    })
}

/// What a way out that a script opens is to the player: someone to talk to stays that, a console is a lift.
fn scripted_exit(kind: StepKind, tag: &str) -> StepKind {
    match kind {
        _ if looks_like_a_lift(tag, "") => StepKind::Elevator,
        StepKind::Item => StepKind::Elevator,
        k => k,
    }
}

fn scan_module(inst: &mut Install, module: &str, index: &mut QuestIndex) -> Option<()> {
    let resources: Vec<(String, ResType)> = inst
        .overlay_resources()
        .map(|(r, t, _)| (r.to_string().to_ascii_lowercase(), t))
        .collect();
    let ifo = Gff::parse(&inst.read("module", ResType::IFO).ok()?).ok()?;
    let area = ifo.root.str("Mod_Entry_Area")?.to_string();
    let git = Gff::parse(&inst.read(&area, ResType::GIT).ok()?).ok()?;
    // Who owns each conversation, and who runs each script.
    let mut owners: BTreeMap<String, Vec<(String, StepKind)>> = BTreeMap::new();
    let mut users: BTreeMap<String, Vec<(String, StepKind)>> = BTreeMap::new();
    let lists: [(&str, Kind, ResType); 4] = [
        ("Creature List", Kind::Creature, ResType(2027)),
        ("Door List", Kind::Door, ResType(2042)),
        ("Placeable List", Kind::Placeable, ResType(2044)),
        ("TriggerList", Kind::Trigger, ResType(2032)),
    ];
    for (label, kind, ty) in lists {
        for s in git.root.list(label).unwrap_or_default() {
            let tpl = s.str("TemplateResRef").unwrap_or_default().to_string();
            let utx = inst.read(&tpl, ty).ok().and_then(|b| Gff::parse(&b).ok());
            let from: [Option<&Struct>; 2] = [Some(s), utx.as_ref().map(|g| &g.root)];
            let field = |name: &str| {
                from.iter()
                    .flatten()
                    .find_map(|x| x.str(name).filter(|v| !v.is_empty()))
                    .unwrap_or_default()
                    .to_string()
            };
            let tag = field("Tag");
            let to = field("LinkedToModule");
            if !to.is_empty() {
                let k = if looks_like_a_lift(&tag, &tpl) {
                    StepKind::Elevator
                } else if kind == Kind::Door {
                    StepKind::Door
                } else {
                    StepKind::Area
                };
                index.add_link(module, &tag, k, &to);
            }
            let talk = field("Conversation").to_ascii_lowercase();
            if !talk.is_empty() {
                let k = if kind == Kind::Creature {
                    StepKind::Talk
                } else {
                    StepKind::Item
                };
                owners.entry(talk).or_default().push((tag.clone(), k));
            }
            for x in from.iter().flatten() {
                for (name, v) in &x.fields {
                    let Value::ResRef(script) = v else {
                        continue;
                    };
                    if script.is_empty() || !(name.starts_with("On") || name.starts_with("Script"))
                    {
                        continue;
                    }
                    if let Some(k) = script_user_kind(kind, name) {
                        users
                            .entry(script.to_string().to_ascii_lowercase())
                            .or_default()
                            .push((tag.clone(), k));
                    }
                }
            }
        }
    }
    // What each of the module's own scripts does.
    let mut facts: BTreeMap<String, ScriptFacts> = BTreeMap::new();
    for (name, ty) in &resources {
        if *ty == ResType::NCS {
            if let Ok(b) = inst.read(name, ResType::NCS) {
                facts.insert(name.clone(), script_facts(&b));
            }
        }
    }
    let mut set_by_a_node: BTreeSet<String> = BTreeSet::new();
    for (name, ty) in &resources {
        if *ty != ResType::DLG {
            continue;
        }
        let Some(d) = inst
            .read(name, ResType::DLG)
            .ok()
            .and_then(|b| Gff::parse(&b).ok())
            .and_then(|g| Dialog::from_gff(&g).ok())
        else {
            continue;
        };
        let nobody = vec![(String::new(), StepKind::Area)];
        let who = owners.get(name).unwrap_or(&nobody);
        for n in d.entries.iter().chain(&d.replies) {
            for (tag, kind) in who {
                if let Some((q, state)) = node_quest(n) {
                    index.add_setter(&q, state, module, tag, *kind);
                }
                let script = n.script.as_deref().unwrap_or_default().to_ascii_lowercase();
                if let Some(f) = facts.get(&script) {
                    set_by_a_node.insert(script.clone());
                    for (q, state) in &f.journal {
                        index.add_setter(q, *state, module, tag, *kind);
                    }
                    if !tag.is_empty() {
                        for to in &f.goes_to {
                            index.add_link(module, tag, scripted_exit(*kind, tag), to);
                        }
                    }
                }
            }
        }
    }
    for (script, f) in &facts {
        let who = users.get(script);
        for (tag, kind) in who.into_iter().flatten() {
            for (q, state) in &f.journal {
                index.add_setter(q, *state, module, tag, *kind);
            }
            for to in &f.goes_to {
                index.add_link(module, tag, scripted_exit(*kind, tag), to);
            }
        }
        if who.is_none() && !set_by_a_node.contains(script) {
            for (q, state) in &f.journal {
                index.add_setter(q, *state, module, "", StepKind::Area);
            }
        }
    }
    Some(())
}

fn module_names(dir: &Path) -> Vec<String> {
    let mut names = BTreeSet::new();
    for e in std::fs::read_dir(dir.join("modules"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let n = e.file_name().to_string_lossy().to_ascii_lowercase();
        let stem = n.strip_suffix(".rim").or_else(|| n.strip_suffix(".mod"));
        if let Some(s) = stem.filter(|s| !s.ends_with("_s")) {
            names.insert(s.to_string());
        }
    }
    names.into_iter().collect()
}

/// Read the install's modules and journal for the facts. A module that does not read is left out.
pub fn scan_install(dir: &Path) -> QuestIndex {
    let mut index = QuestIndex::default();
    for module in module_names(dir) {
        // Its own install, so that one module's files do not hide another's.
        let Ok(mut inst) = Install::open(dir) else {
            return index;
        };
        if inst.load_module(&module).is_ok() {
            let _ = scan_module(&mut inst, &module, &mut index);
        }
    }
    if let Some(g) = Install::open(dir)
        .ok()
        .and_then(|mut i| i.read("global", JOURNAL_FILE).ok())
        .and_then(|b| Gff::parse(&b).ok())
    {
        for c in g.root.list("Categories").unwrap_or_default() {
            let Some(tag) = c.str("Tag") else {
                continue;
            };
            for e in c.list("EntryList").unwrap_or_default() {
                if e.int("End").is_some_and(|n| n != 0) {
                    if let Some(id) = e.int("ID") {
                        index
                            .ends
                            .entry(tag.to_ascii_lowercase())
                            .or_default()
                            .insert(id as i32);
                    }
                }
            }
        }
    }
    index
}

/// Changes when the install's modules do.
fn fingerprint(dir: &Path) -> u64 {
    let size = |p: PathBuf| std::fs::metadata(p).map_or(0, |m| m.len());
    let mut total = size(dir.join("chitin.key"));
    for e in std::fs::read_dir(dir.join("modules"))
        .into_iter()
        .flatten()
        .flatten()
    {
        total = total
            .wrapping_mul(31)
            .wrapping_add(e.metadata().map_or(0, |m| m.len()));
    }
    total
}

/// The index from `cache` if it fits this install, else scanned and written there.
pub fn load_or_scan(dir: &Path, cache: Option<&Path>) -> QuestIndex {
    let print = fingerprint(dir);
    if let Some(i) = cache
        .and_then(|c| std::fs::read_to_string(c).ok())
        .and_then(|t| QuestIndex::from_text(&t, print))
    {
        return i;
    }
    let index = scan_install(dir);
    if let Some(c) = cache {
        if let Some(parent) = c.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(c, index.to_text(print));
    }
    index
}

fn cache_file() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(base).join("kotor-rs").join("quest_index.tsv"))
}

static INDEX: OnceLock<QuestIndex> = OnceLock::new();
static STARTED: AtomicBool = AtomicBool::new(false);

/// Make the index now and wait for it, with `cache` as its file or none. For tests and tools.
pub fn prepare_index(dir: &Path, cache: Option<&Path>) -> &'static QuestIndex {
    STARTED.store(true, Ordering::SeqCst);
    INDEX.get_or_init(|| load_or_scan(dir, cache))
}

/// The index if it is ready. The first call starts it on a thread of its own, so that a first run does not stop the game.
fn index_for(dir: &Path) -> Option<&'static QuestIndex> {
    if let Some(i) = INDEX.get() {
        return Some(i);
    }
    if !STARTED.swap(true, Ordering::SeqCst) {
        let dir = dir.to_path_buf();
        std::thread::spawn(move || {
            INDEX.get_or_init(|| load_or_scan(&dir, cache_file().as_deref()));
        });
    }
    None
}

/// The middle of a trigger's shape, or where the object stands.
fn where_it_is(o: &Object) -> [f32; 3] {
    if o.polygon.is_empty() {
        return o.position;
    }
    let n = o.polygon.len() as f32;
    let (x, y) = o
        .polygon
        .iter()
        .fold((0.0, 0.0), |(x, y), p| (x + p[0], y + p[1]));
    [x / n, y / n, o.position[2]]
}

impl World {
    /// The nearest live object with this tag; a body does not count.
    fn route_object(&self, tag: &str) -> Option<&Object> {
        if tag.is_empty() {
            return None;
        }
        let at = self.object(self.pc)?.position;
        self.by_tag(tag)
            .into_iter()
            .filter_map(|id| self.object(id))
            .filter(|o| o.kind != Kind::Creature || o.hp > 0)
            .min_by(|a, b| {
                let d = |o: &Object| (o.position[0] - at[0]).hypot(o.position[1] - at[1]);
                d(a).total_cmp(&d(b))
            })
    }

    /// What to do next for `quest` from here: the person or thing if it is in this module, else the first exit on the way there.
    ///
    /// `None`: the quest is finished, the party has not heard of it, nothing is known of its next state, or the index is still being
    /// built on the first run. Cheap enough for each frame; the position is read live.
    pub fn next_step(&self, quest: &str) -> Option<NextStep> {
        let index = index_for(self.inst.dir())?;
        let quest = quest.to_ascii_lowercase();
        let state = self
            .journal
            .iter()
            .find(|(t, _)| t.eq_ignore_ascii_case(&quest))
            .map(|(_, s)| *s)?;
        if index.ends.get(&quest).is_some_and(|e| e.contains(&state)) {
            return None;
        }
        let rows: Vec<Target> = route_rows()
            .iter()
            .filter(|r| r.quest == quest && r.state == state)
            .map(|r| Target {
                module: r.module.clone(),
                tag: r.tag.clone(),
                kind: r.kind,
                label: r.label.clone(),
            })
            .collect();
        if let Some(step) = self.step_to(index, &quest, state, &rows) {
            return Some(step);
        }
        // A state with nothing to point at from here gives way to the one after: the next that has something says where to go.
        index
            .setters
            .get(&quest)?
            .range(state + 1..)
            .find_map(|(_, targets)| self.step_to(index, &quest, state, targets))
    }

    fn step_to(
        &self,
        index: &QuestIndex,
        quest: &str,
        state: i32,
        targets: &[Target],
    ) -> Option<NextStep> {
        let here = self.module.to_ascii_lowercase();
        let step =
            |kind: StepKind, tag: &str, o: &Object, label: String, final_module: &str| NextStep {
                id: format!(
                    "{quest}/{state}/{}/{}",
                    kind.name(),
                    tag.to_ascii_lowercase()
                ),
                kind,
                position: where_it_is(o),
                label,
                final_module: final_module.to_string(),
            };
        // Here first, and of those one with a person or thing named.
        for t in targets.iter().filter(|t| t.module == here) {
            if let Some(o) = self.route_object(&t.tag) {
                let label = target_label(t, o);
                return Some(step(t.kind, &t.tag, o, label, &t.module));
            }
        }
        // Else the first exit of the shortest way; a target with a name beats one with only a module.
        let best = targets
            .iter()
            .filter(|t| t.module != here)
            .filter_map(|t| Some((t, index.first_exit(&here, &t.module)?)))
            .min_by_key(|(t, (_, hops))| (t.tag.is_empty(), *hops));
        if let Some((t, (exit, _))) = best {
            let o = self.exit_object(exit)?;
            return Some(step(
                exit.kind,
                &exit.tag,
                o,
                exit_label(exit, o),
                &t.module,
            ));
        }
        // Another planet: back to the ship, and on it the galaxy map.
        let t = targets.iter().find(|t| t.module != here)?;
        if here == THE_SHIP {
            return self.leave_the_ship(t, |kind, tag, o, label| {
                step(kind, tag, o, label, &t.module)
            });
        }
        let (exit, _) = index.first_exit(&here, THE_SHIP)?;
        let o = self.exit_object(exit)?;
        Some(step(
            exit.kind,
            &exit.tag,
            o,
            exit_label(exit, o),
            &t.module,
        ))
    }

    /// On the ship, for `t` elsewhere: out of the hatch if the ship stands on `t`'s planet, else to the galaxy map.
    ///
    /// **Assumed:** the ship stands on the planet last chosen on the map. With none chosen, as after a load, it is the map.
    fn leave_the_ship(
        &self,
        t: &Target,
        step: impl Fn(StepKind, &str, &Object, String) -> NextStep,
    ) -> Option<NextStep> {
        let planet = planet_of(&t.module);
        if planet.is_some_and(|(row, _)| row == self.selected_planet) {
            if let Some(o) = self.route_object(SHIP_EXIT) {
                return Some(step(
                    StepKind::Area,
                    SHIP_EXIT,
                    o,
                    "Leave the ship".to_string(),
                ));
            }
        }
        let o = self.route_object(GALAXY_MAP)?;
        let label = match planet {
            Some((_, name)) => format!("Fly to {name} on the galaxy map"),
            None => "Use the galaxy map".to_string(),
        };
        Some(step(StepKind::GalaxyMap, GALAXY_MAP, o, label))
    }

    /// The object of an exit: of its tag, and the one that leads there if several have that tag.
    fn exit_object(&self, exit: &Link) -> Option<&Object> {
        self.by_tag(&exit.tag)
            .into_iter()
            .filter_map(|id| self.object(id))
            .find(|o| o.link_module.eq_ignore_ascii_case(&exit.to))
            .or_else(|| self.route_object(&exit.tag))
    }
}

fn target_label(t: &Target, o: &Object) -> String {
    if !t.label.is_empty() {
        return t.label.clone();
    }
    let name = if o.name.is_empty() { &o.tag } else { &o.name };
    match t.kind {
        StepKind::Talk => format!("Talk to {name}"),
        StepKind::Kill => format!("Defeat {name}"),
        StepKind::Item => format!("Find {name}"),
        StepKind::Door => format!("Go to {name}"),
        StepKind::Elevator => "Take the elevator".to_string(),
        StepKind::Area => "Go here".to_string(),
        StepKind::GalaxyMap => "Use the galaxy map".to_string(),
    }
}

fn exit_label(exit: &Link, o: &Object) -> String {
    match exit.kind {
        StepKind::Elevator => "Take the elevator".to_string(),
        StepKind::Door if !o.name.is_empty() => format!("Go through {}", o.name),
        StepKind::Door => "Go through this door".to_string(),
        _ => "Leave this way".to_string(),
    }
}
