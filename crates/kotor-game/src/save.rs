//! Saving and loading the world as a SAV archive (an ERF with `savenfo`, `globalvars`, `partytable` and `objects`
//! resources, each a GFF).
//!
//! **This is this engine's own layout, not the original game's save format**: the field names follow the original
//! where I know them (`savenfo`, `globalvars`, `partytable` are the original's resource names), but the contents were
//! not checked against a real save, so a save made here is not meant to be opened by the original game, and the
//! original's saves are not read. **Not saved (assumption):** `DelayCommand` blocks still pending, objects' action
//! queues, conversations in progress, who is fighting whom, and `Effect` or location handles held by scripts. A load
//! starts the module fresh, applies the saved state over it and does not run the area's enter scripts again.
//!
//! A module left behind keeps its objects the same way, in `World::left_modules` and in the save's `modules` resource,
//! and a return visit applies them over a fresh load. **Assumption:** what was waiting on `DelayCommand` or in an action
//! queue when the player left is lost, as in a save.

use crate::party::BenchedMember;
use crate::world::{DoorState, Kind, ObjId, World};
use kotor_archive::{write_archive, Erf, ErfKind, NewResource, Source};
use kotor_core::{Error, ResType, Result};
use kotor_gff::{Gff, Struct, Value};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// The save format version this engine writes and reads.
pub const SAVE_VERSION: i32 = 1;

fn rt() -> ResType {
    // Resource type 0 (`res`) is what the original uses for these entries.
    ResType(0)
}

fn kind_code(k: Kind) -> u8 {
    match k {
        Kind::Module => 0,
        Kind::Area => 1,
        Kind::Creature => 2,
        Kind::Door => 3,
        Kind::Placeable => 4,
        Kind::Waypoint => 5,
        Kind::Trigger => 6,
        Kind::Item => 7,
        Kind::Store => 8,
    }
}

fn kind_from(c: i64) -> Option<Kind> {
    Some(match c {
        0 => Kind::Module,
        1 => Kind::Area,
        2 => Kind::Creature,
        3 => Kind::Door,
        4 => Kind::Placeable,
        5 => Kind::Waypoint,
        6 => Kind::Trigger,
        7 => Kind::Item,
        8 => Kind::Store,
        _ => return None,
    })
}

fn st(fields: Vec<(&str, Value)>) -> Struct {
    Struct {
        id: 0,
        fields: fields
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    }
}

fn s(v: &str) -> Value {
    Value::String(v.to_string())
}

fn list_of<T>(items: impl IntoIterator<Item = T>, f: impl Fn(T) -> Struct) -> Value {
    Value::List(items.into_iter().map(f).collect())
}

fn gff(file_type: &str, fields: Vec<(&str, Value)>) -> Result<Vec<u8>> {
    Gff {
        file_type: file_type.to_string(),
        root: st(fields),
    }
    .to_bytes()
}

/// Identity of an object that survives a reload: kind, tag, template and which of that sort it is in id order.
type Key = (u8, String, String, usize);

fn key_of(e: &Struct) -> Key {
    (
        e.int("Kind").unwrap_or(255) as u8,
        e.str("Tag").unwrap_or("").to_ascii_lowercase(),
        e.str("Template").unwrap_or("").to_ascii_lowercase(),
        e.int("Nth").unwrap_or(0) as usize,
    )
}

/// What a save screen shows about a save, without loading it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SaveInfo {
    pub name: String,
    pub module: String,
    pub area: String,
    pub pc_name: String,
    /// Seconds played.
    pub seconds: f32,
    /// The party's portrait textures, the player first.
    pub portraits: Vec<String>,
}

/// Read the header of a save file. Fails on anything this engine did not write.
pub fn read_save_info(path: &Path) -> Result<SaveInfo> {
    let erf = Erf::open(Source::file(path))?;
    let g = Gff::parse(&erf.get("savenfo", rt())?)?;
    let r = &g.root;
    if r.int("SaveVersion") != Some(SAVE_VERSION as i64) {
        return Err(Error::format("unsupported save version"));
    }
    let text = |l: &str| r.str(l).unwrap_or_default().to_string();
    Ok(SaveInfo {
        name: text("SAVEGAMENAME"),
        module: text("LASTMODULE"),
        area: text("AREANAME"),
        pc_name: text("PCNAME"),
        seconds: r.float("TIMEPLAYED").unwrap_or(0.0) as f32,
        portraits: r
            .list("PORTRAITS")
            .unwrap_or_default()
            .iter()
            .filter_map(|p| p.str("Portrait").map(str::to_string))
            .collect(),
    })
}

/// Cells as the lengths of their runs, the first of unseen ones.
fn cell_runs(cells: &[bool]) -> String {
    let mut runs = Vec::new();
    let (mut value, mut length) = (false, 0);
    for cell in cells {
        if *cell != value {
            runs.push(length.to_string());
            (value, length) = (*cell, 0);
        }
        length += 1;
    }
    runs.push(length.to_string());
    runs.join(" ")
}

fn cells_of_runs(runs: &str) -> Vec<bool> {
    let mut cells = Vec::new();
    for (n, run) in runs.split(' ').enumerate() {
        let length: usize = run.parse().unwrap_or(0);
        cells.resize(cells.len() + length, n % 2 == 1);
    }
    cells
}

fn value_list<'a>(values: impl IntoIterator<Item = &'a i32>) -> Value {
    list_of(values, |v| st(vec![("Value", Value::Int(*v))]))
}

fn values_of(s: &Struct, label: &str) -> impl Iterator<Item = i32> {
    let list = s.list(label).unwrap_or_default();
    let values: Vec<i32> = list
        .iter()
        .filter_map(|x| x.int("Value").map(|v| v as i32))
        .collect();
    values.into_iter()
}

fn bench_struct(slot: i32, b: &BenchedMember) -> Struct {
    st(vec![
        ("Slot", Value::Int(slot)),
        ("Class", Value::Int(b.class)),
        ("Level", Value::Int(b.level)),
        ("XP", Value::Int(b.xp)),
        ("Alignment", Value::Int(b.alignment)),
        ("HP", Value::Int(b.hp)),
        ("MaxHP", Value::Int(b.max_hp)),
        ("Force", Value::Int(b.force)),
        ("MaxForce", Value::Int(b.max_force)),
        ("SkillPoints", Value::Int(b.skill_points)),
        ("FeatRegular", Value::Int(b.feat_slots.0)),
        ("FeatBonus", Value::Int(b.feat_slots.1)),
        ("Abilities", value_list(&b.abilities)),
        ("Skills", value_list(&b.skills)),
        ("Feats", value_list(&b.feats)),
        ("Spells", value_list(&b.spells)),
        (
            "Carried",
            list_of(&b.carried, |(t, n)| {
                st(vec![("Template", s(t)), ("Stack", Value::Int(*n))])
            }),
        ),
        (
            "Worn",
            list_of(&b.worn, |(slot, t, n)| {
                st(vec![
                    ("Slot", Value::Int(*slot)),
                    ("Template", s(t)),
                    ("Stack", Value::Int(*n)),
                ])
            }),
        ),
    ])
}

fn bench_from(e: &Struct) -> BenchedMember {
    let int = |l: &str| e.int(l).unwrap_or(0) as i32;
    let mut abilities = [10; 6];
    for (to, from) in abilities.iter_mut().zip(values_of(e, "Abilities")) {
        *to = from;
    }
    BenchedMember {
        class: int("Class"),
        level: int("Level"),
        xp: int("XP"),
        alignment: int("Alignment"),
        hp: int("HP"),
        max_hp: int("MaxHP"),
        force: int("Force"),
        max_force: int("MaxForce"),
        abilities,
        skills: values_of(e, "Skills").collect(),
        skill_points: int("SkillPoints"),
        feats: values_of(e, "Feats").collect(),
        feat_slots: (int("FeatRegular"), int("FeatBonus")),
        spells: values_of(e, "Spells").collect(),
        carried: e
            .list("Carried")
            .unwrap_or_default()
            .iter()
            .filter_map(|x| Some((x.str("Template")?.to_string(), x.int("Stack")? as i32)))
            .collect(),
        worn: e
            .list("Worn")
            .unwrap_or_default()
            .iter()
            .filter_map(|x| {
                Some((
                    x.int("Slot")? as i32,
                    x.str("Template")?.to_string(),
                    x.int("Stack")? as i32,
                ))
            })
            .collect(),
    }
}

impl World {
    fn keys(&self) -> HashMap<ObjId, Key> {
        self.keys_without(&HashSet::new())
    }

    /// As [`World::keys`], counting only the objects not in `out_of_it`: the party is in one module and then another.
    fn keys_without(&self, out_of_it: &HashSet<ObjId>) -> HashMap<ObjId, Key> {
        let mut seen: HashMap<(u8, String, String), usize> = HashMap::new();
        let mut out = HashMap::new();
        for o in self.objects.values().filter(|o| !out_of_it.contains(&o.id)) {
            let base = (
                kind_code(o.kind),
                o.tag.to_ascii_lowercase(),
                o.template.to_ascii_lowercase(),
            );
            let n = seen.entry(base.clone()).or_default();
            out.insert(o.id, (base.0, base.1, base.2, *n));
            *n += 1;
        }
        out
    }

    /// Everything of the module the player is leaving, apart from `keep`: the party and what it carries.
    pub(crate) fn module_snapshot(&self, keep: &HashSet<ObjId>) -> Vec<Struct> {
        let keys = self.keys_without(keep);
        let key_struct = |id: ObjId| -> Struct {
            let k = keys.get(&id);
            st(vec![
                ("Kind", Value::Byte(k.map_or(255, |k| k.0))),
                ("Tag", s(k.map_or("", |k| &k.1))),
                ("Template", s(k.map_or("", |k| &k.2))),
                ("Nth", Value::Dword(k.map_or(0, |k| k.3 as u32))),
            ])
        };
        self.objects
            .values()
            .filter(|o| !keep.contains(&o.id) && !matches!(o.kind, Kind::Module | Kind::Area))
            .map(|o| self.object_struct(o, &key_struct))
            .collect()
    }

    /// Put a module that was left behind back as it was, over its fresh load.
    pub(crate) fn restore_module(&mut self, saved: &[Struct], keep: &HashSet<ObjId>) {
        let keys = self.keys_without(keep);
        self.apply_objects(saved, keep, &keys);
        self.sync_door_lines();
    }

    /// Doors in the navigation mesh, from their state.
    fn sync_door_lines(&mut self) {
        let shut: Vec<([f32; 3], [f32; 3])> = self
            .objects
            .values()
            .filter(|o| {
                o.kind == Kind::Door && (!o.open || matches!(o.door, DoorState::Opening(_)))
            })
            .map(crate::play::door_line)
            .collect();
        if let Some(n) = self.nav.as_mut() {
            n.clear_lines();
            for (a, b) in shut {
                n.block_line(a, b);
            }
        }
    }

    /// The game as SAV archive bytes.
    pub fn save_bytes(&self, save_name: &str) -> Result<Vec<u8>> {
        let pc_name = self
            .object(self.pc)
            .map(|o| o.name.clone())
            .unwrap_or_default();
        let area = self
            .object(self.area)
            .map(|o| o.name.clone())
            .unwrap_or_default();
        let portraits: Vec<String> = std::iter::once(self.pc)
            .chain(self.party.iter().copied().filter(|id| *id != self.pc))
            .filter_map(|id| self.object(id))
            .take(3)
            .map(|o| o.portrait.clone())
            .collect();
        let nfo = gff(
            "NFO ",
            vec![
                (
                    "PORTRAITS",
                    list_of(portraits.iter(), |p| st(vec![("Portrait", s(p))])),
                ),
                ("SaveVersion", Value::Int(SAVE_VERSION)),
                ("Engine", s("kotor-rs")),
                ("LASTMODULE", s(&self.module)),
                ("AREANAME", s(&area)),
                ("PCNAME", s(&pc_name)),
                ("SAVEGAMENAME", s(save_name)),
                ("TIMEPLAYED", Value::Float(self.time)),
                ("PartyAIStyle", Value::Int(self.party_ai_style)),
            ],
        )?;
        let mut numbers: Vec<_> = self.global_numbers.iter().collect();
        numbers.sort();
        let mut bools: Vec<_> = self.global_booleans.iter().collect();
        bools.sort();
        let globals = gff(
            "GVT ",
            vec![
                (
                    "NumberList",
                    list_of(numbers, |(k, v)| {
                        st(vec![("Name", s(k)), ("Value", Value::Int(*v))])
                    }),
                ),
                ("SoloMode", Value::Byte(self.solo_mode as u8)),
                ("RestrictMode", Value::Byte(self.restrict_mode as u8)),
                (
                    "Unselectable",
                    list_of(self.unselectable.iter(), |n| {
                        st(vec![("Row", Value::Int(*n))])
                    }),
                ),
                (
                    "ReputationList",
                    list_of(self.reputation_shifts.iter(), |((from, of), by)| {
                        st(vec![
                            ("From", Value::Int(*from)),
                            ("Of", Value::Int(*of)),
                            ("By", Value::Int(*by)),
                        ])
                    }),
                ),
                (
                    "PlanetList",
                    list_of(self.planet_lines.values(), |p| {
                        st(vec![
                            ("Row", Value::Int(p.row)),
                            ("Available", Value::Byte(p.available as u8)),
                            ("Selectable", Value::Byte(p.selectable as u8)),
                        ])
                    }),
                ),
                (
                    "BooleanList",
                    list_of(bools, |(k, v)| {
                        st(vec![("Name", s(k)), ("Value", Value::Byte(*v as u8))])
                    }),
                ),
            ],
        )?;
        let keys = self.keys();
        let key_struct = |id: ObjId| -> Struct {
            let k = keys.get(&id);
            st(vec![
                ("Kind", Value::Byte(k.map_or(255, |k| k.0))),
                ("Tag", s(k.map_or("", |k| &k.1))),
                ("Template", s(k.map_or("", |k| &k.2))),
                ("Nth", Value::Dword(k.map_or(0, |k| k.3 as u32))),
            ])
        };
        let mut slots: Vec<_> = self.npc_slots.iter().collect();
        slots.sort();
        let party = gff(
            "PT  ",
            vec![
                ("PT_PCNAME", s(&pc_name)),
                (
                    "PT_XP_POOL",
                    Value::Int(self.object(self.pc).map_or(0, |o| o.xp)),
                ),
                (
                    "PT_MEMBERS",
                    list_of(self.party.iter(), |id| key_struct(*id)),
                ),
                (
                    "PT_NPC_SLOTS",
                    list_of(slots, |(slot, id)| {
                        let mut k = key_struct(*id);
                        k.set("Slot", Value::Int(*slot));
                        k
                    }),
                ),
                (
                    "PT_AVAIL_NPCS",
                    list_of(self.available.iter(), |(slot, t)| {
                        st(vec![("Slot", Value::Int(*slot)), ("Template", s(t))])
                    }),
                ),
                (
                    "PT_JOURNAL",
                    list_of(self.journal.iter(), |(tag, state)| {
                        let order = self.journal_order.iter().position(|t| t == tag);
                        st(vec![
                            ("Tag", s(tag)),
                            ("State", Value::Int(*state)),
                            ("Order", Value::Int(order.map_or(i32::MAX, |o| o as i32))),
                        ])
                    }),
                ),
                ("PT_TUTORIALS", value_list(&self.tutorial_seen)),
                ("PT_PLOT_XP", Value::Int(self.plot_xp)),
                (
                    "PT_EXPLORED",
                    list_of(self.explored.iter(), |(map, cells)| {
                        st(vec![("Map", s(map)), ("Runs", s(&cell_runs(cells)))])
                    }),
                ),
                (
                    "PT_BENCH",
                    list_of(self.bench.iter(), |(slot, b)| bench_struct(*slot, b)),
                ),
                ("PT_GOLD", Value::Int(self.gold)),
                ("RS_EARLY_FORCE", Value::Int(self.early_force as i32)),
                (
                    "RS_ENCOUNTERS",
                    Value::String(
                        self.spent_encounters
                            .iter()
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(" "),
                    ),
                ),
                (
                    "PT_TOKENS",
                    list_of(self.tokens.iter(), |(n, t)| {
                        st(vec![("Number", Value::Int(*n)), ("Text", s(t))])
                    }),
                ),
                ("PT_PLAYER_POS", Value::Vector(self.pc_mover.pos)),
                ("PT_PLAYER_HEAD", Value::Float(self.pc_mover.heading)),
                ("PT_RNG", Value::Int64(self.rng as i64)),
            ],
        )?;
        let objects = gff(
            "OBJ ",
            vec![(
                "Objects",
                Value::List(
                    self.objects
                        .values()
                        .filter(|o| !matches!(o.kind, Kind::Module | Kind::Area))
                        .map(|o| self.object_struct(o, &key_struct))
                        .collect(),
                ),
            )],
        )?;
        let modules = gff(
            "MOD ",
            vec![(
                "Modules",
                list_of(self.left_modules.iter(), |(module, rows)| {
                    st(vec![
                        ("Module", s(module)),
                        ("Objects", Value::List(rows.clone())),
                    ])
                }),
            )],
        )?;
        write_archive(
            ErfKind::Sav,
            &[
                NewResource::new("savenfo", rt(), nfo),
                NewResource::new("globalvars", rt(), globals),
                NewResource::new("partytable", rt(), party),
                NewResource::new("objects", rt(), objects),
                NewResource::new("modules", rt(), modules),
            ],
        )
    }

    fn object_struct(
        &self,
        o: &crate::world::Object,
        key_struct: &dyn Fn(ObjId) -> Struct,
    ) -> Struct {
        let (door_code, door_t) = match o.door {
            DoorState::Closed => (0u8, 0.0),
            DoorState::Opening(t) => (1, t),
            DoorState::Open => (2, 0.0),
            DoorState::Closing(t) => (3, t),
        };
        let ints = |v: &std::collections::BTreeSet<i32>| {
            list_of(v.iter(), |i| st(vec![("Value", Value::Int(*i))]))
        };
        let mut nums: Vec<_> = o.local_numbers.iter().collect();
        nums.sort();
        let mut bools: Vec<_> = o.local_booleans.iter().collect();
        bools.sort();
        let mut k = key_struct(o.id);
        for (name, v) in [
            ("IsPlayer", Value::Byte((o.id == self.hero) as u8)),
            ("IsLeader", Value::Byte((o.id == self.pc) as u8)),
            ("MinOneHP", Value::Byte(o.min_one_hp as u8)),
            ("Name", s(&o.name)),
            ("Position", Value::Vector(o.position)),
            ("Facing", Value::Float(o.facing)),
            ("HP", Value::Int(o.hp)),
            ("Dead", Value::Byte(o.dead as u8)),
            ("PartyMember", Value::Byte(o.party_member as u8)),
            ("Locked", Value::Byte(o.locked as u8)),
            ("Level", Value::Int(o.level)),
            ("Open", Value::Byte(o.open as u8)),
            ("DoorState", Value::Byte(door_code)),
            ("DoorTime", Value::Float(door_t)),
            ("BaseItem", Value::Int(o.base_item)),
            ("Filled", Value::Byte(self.filled.contains(&o.id) as u8)),
            ("Droppable", Value::Byte(o.droppable as u8)),
            ("Conversation", s(&o.conversation)),
            (
                "Scripts",
                list_of(o.scripts.iter(), |(field, script)| {
                    st(vec![("Field", s(field)), ("Script", s(script))])
                }),
            ),
            ("NaturalAC", Value::Int(o.natural_ac)),
            ("Perception", Value::Float(o.perception)),
            ("Challenge", Value::Float(o.challenge_rating)),
            (
                "Upgrades",
                list_of(o.upgrades.iter(), |(slot, row)| {
                    st(vec![("Slot", Value::Int(*slot)), ("Row", Value::Int(*row))])
                }),
            ),
            ("AIStyle", Value::Int(o.ai_style)),
            ("NonEquippable", Value::Byte(o.non_equippable as u8)),
            ("MapNote", Value::Int(o.map_note.map_or(-1, i32::from))),
            ("XP", Value::Int(o.xp)),
            ("Faction", Value::Int(o.faction)),
            ("Race", Value::Int(o.race)),
            ("Subrace", Value::Int(o.subrace)),
            ("Down", Value::Byte(o.down as u8)),
            ("Stack", Value::Int(o.stack)),
            ("PlotFlag", Value::Byte(o.plot as u8)),
            ("Alignment", Value::Int(o.alignment)),
            ("Class", Value::Int(o.class)),
            ("MaxHP", Value::Int(o.max_hp)),
            (
                "Abilities",
                list_of(o.abilities.iter(), |a| st(vec![("Value", Value::Int(*a))])),
            ),
            ("Appearance", Value::Int(o.appearance)),
            ("Head", Value::Int(o.head)),
            ("Gender", Value::Int(o.gender)),
            ("Portrait", s(&o.portrait)),
            ("Force", Value::Int(o.force)),
            ("MaxForce", Value::Int(o.max_force)),
            ("SkillPoints", Value::Int(o.skill_points)),
            ("FeatRegular", Value::Int(o.feat_slots.0)),
            ("FeatBonus", Value::Int(o.feat_slots.1)),
            (
                "Skills",
                list_of(o.skills.iter(), |r| st(vec![("Value", Value::Int(*r))])),
            ),
            ("Spells", ints(&o.spells)),
            ("Feats", ints(&o.feats)),
            (
                "Inventory",
                list_of(o.inventory.iter(), |id| key_struct(*id)),
            ),
            (
                "Equipped",
                list_of(o.equipped.iter(), |(slot, id)| {
                    let mut e = key_struct(*id);
                    e.set("Slot", Value::Int(*slot));
                    e
                }),
            ),
            (
                "LocalNumbers",
                list_of(nums, |(i, v)| {
                    st(vec![("Index", Value::Int(*i)), ("Value", Value::Int(*v))])
                }),
            ),
            (
                "LocalBooleans",
                list_of(bools, |(i, v)| {
                    st(vec![
                        ("Index", Value::Int(*i)),
                        ("Value", Value::Byte(*v as u8)),
                    ])
                }),
            ),
            (
                "Effects",
                list_of(o.effects.iter(), |h| {
                    let e = self.effects.get(h.wrapping_sub(1) as usize);
                    st(vec![
                        (
                            "Expires",
                            Value::Float(self.effect_expiry.get(h).copied().unwrap_or(-1.0)),
                        ),
                        (
                            "Spell",
                            Value::Int(self.effect_spell.get(h).copied().unwrap_or(-1)),
                        ),
                        ("Name", s(e.map_or("", |e| &e.name))),
                        (
                            "Args",
                            list_of(e.map(|e| e.args.clone()).unwrap_or_default(), |a| {
                                st(vec![("Value", s(&a))])
                            }),
                        ),
                        (
                            "Nums",
                            list_of(e.map(|e| e.nums.clone()).unwrap_or_default(), |n| {
                                st(vec![("Value", Value::Float(n))])
                            }),
                        ),
                    ])
                }),
            ),
        ] {
            k.set(name, v);
        }
        k
    }

    /// Match saved objects to the fresh ones of a module just loaded and give them their saved state. Whatever is in `keep`
    /// is left alone. Returns who each saved key is now, and the saved leader.
    fn apply_objects(
        &mut self,
        saved: &[Struct],
        keep: &HashSet<ObjId>,
        keys: &HashMap<ObjId, Key>,
    ) -> (HashMap<Key, ObjId>, ObjId) {
        // Match saved objects to the fresh ones by (kind, tag, template, nth); spawn the rest; drop the unmatched.
        let fresh: HashMap<Key, ObjId> = keys
            .iter()
            .filter(|(id, _)| !keep.contains(id))
            .map(|(id, k)| (k.clone(), *id))
            .collect();
        let mut map: HashMap<Key, ObjId> = HashMap::new();
        let mut wanted: HashSet<ObjId> = HashSet::new();
        wanted.extend(keep.iter().copied());
        for e in saved {
            let key = key_of(e);
            if e.int("IsPlayer") == Some(1) {
                map.insert(key, self.hero);
                continue;
            }
            let id = match fresh.get(&key) {
                Some(id) => *id,
                None => {
                    let Some(kind) = kind_from(key.0 as i64) else {
                        continue;
                    };
                    let ty = match kind {
                        Kind::Creature => ResType(2027),
                        Kind::Door => ResType(2042),
                        Kind::Placeable => ResType(2044),
                        Kind::Waypoint => ResType(2058),
                        Kind::Trigger => ResType(2032),
                        Kind::Store => crate::store::STORE_FILE,
                        _ => ResType(2025),
                    };
                    let id = self.spawn(kind, &key.2, ty, [0.0; 3]);
                    if let Some(o) = self.objects.get_mut(&id) {
                        o.tag = e.str("Tag").unwrap_or("").to_string();
                    }
                    id
                }
            };
            map.insert(key, id);
            wanted.insert(id);
        }
        self.objects
            .retain(|id, o| wanted.contains(id) || matches!(o.kind, Kind::Module | Kind::Area));
        let resolve = |e: &Struct| map.get(&key_of(e)).copied();
        let mut leader = self.pc;
        for e in saved {
            let Some(id) = resolve(e) else { continue };
            if e.int("IsLeader") == Some(1) {
                leader = id;
            }
            let inventory: Vec<ObjId> = e
                .list("Inventory")
                .unwrap_or_default()
                .iter()
                .filter_map(resolve)
                .collect();
            let equipped = e
                .list("Equipped")
                .unwrap_or_default()
                .iter()
                .filter_map(|x| Some((x.int("Slot")? as i32, resolve(x)?)))
                .collect();
            let mut effects = Vec::new();
            for f in e.list("Effects").unwrap_or_default() {
                let mut rec = crate::world::EffectRec::new(f.str("Name").unwrap_or(""));
                rec.args = f
                    .list("Args")
                    .unwrap_or_default()
                    .iter()
                    .map(|a| a.str("Value").unwrap_or("").to_string())
                    .collect();
                rec.nums = f
                    .list("Nums")
                    .unwrap_or_default()
                    .iter()
                    .map(|a| a.float("Value").unwrap_or(0.0) as f32)
                    .collect();
                self.effects.push(rec);
                let handle = self.effects.len() as u32;
                effects.push(handle);
                let expires = f.float("Expires").unwrap_or(-1.0) as f32;
                if expires >= 0.0 {
                    self.effect_expiry.insert(handle, expires);
                }
                if let Some(spell) = f.int("Spell").filter(|v| *v >= 0) {
                    self.effect_spell.insert(handle, spell as i32);
                }
            }
            if e.int("Filled").is_some_and(|f| f != 0) {
                self.filled.insert(id);
            }
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            if let Some(Value::Vector(p)) = e.get("Position") {
                o.position = *p;
            }
            let flag = |l: &str| e.int(l).unwrap_or(0) != 0;
            o.name = e.str("Name").unwrap_or("").to_string();
            o.facing = e.float("Facing").unwrap_or(0.0) as f32;
            o.hp = e.int("HP").unwrap_or(0) as i32;
            o.dead = flag("Dead");
            o.party_member = flag("PartyMember");
            o.spawned = true;
            o.min_one_hp = flag("MinOneHP");
            o.locked = flag("Locked");
            o.level = e.int("Level").unwrap_or(1) as i32;
            o.open = flag("Open");
            let t = e.float("DoorTime").unwrap_or(0.0) as f32;
            o.door = match e.int("DoorState").unwrap_or(0) {
                1 => DoorState::Opening(t),
                2 => DoorState::Open,
                3 => DoorState::Closing(t),
                _ => DoorState::Closed,
            };
            o.base_item = e.int("BaseItem").unwrap_or(0) as i32;
            // An object from another module has no template to read here, so these come from the save too.
            o.template = e.str("Template").unwrap_or("").to_string();
            o.conversation = e.str("Conversation").unwrap_or("").to_string();
            o.scripts = e
                .list("Scripts")
                .unwrap_or_default()
                .iter()
                .filter_map(|x| Some((x.str("Field")?.to_string(), x.str("Script")?.to_string())))
                .collect();
            o.ai_style = e.int("AIStyle").unwrap_or(0) as i32;
            o.non_equippable = e.int("NonEquippable").unwrap_or(0) != 0;
            o.map_note = match e.int("MapNote").unwrap_or(-1) {
                -1 => None,
                on => Some(on != 0),
            };
            o.natural_ac = e.int("NaturalAC").unwrap_or(0) as i32;
            o.perception = e.float("Perception").unwrap_or(20.0) as f32;
            o.challenge_rating = e.float("Challenge").unwrap_or(1.0) as f32;
            o.upgrades = e
                .list("Upgrades")
                .unwrap_or_default()
                .iter()
                .filter_map(|x| Some((x.int("Slot")? as i32, x.int("Row")? as i32)))
                .collect();
            o.xp = e.int("XP").unwrap_or(0) as i32;
            o.faction = e.int("Faction").unwrap_or(0) as i32;
            o.race = e.int("Race").unwrap_or(0) as i32;
            o.subrace = e.int("Subrace").unwrap_or(0) as i32;
            let ints = |l: &str| -> std::collections::BTreeSet<i32> {
                e.list(l)
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|x| Some(x.int("Value")? as i32))
                    .collect()
            };
            o.down = flag("Down");
            o.droppable = flag("Droppable");
            o.stack = e.int("Stack").unwrap_or(1) as i32;
            o.plot = flag("PlotFlag");
            o.alignment = e.int("Alignment").unwrap_or(50) as i32;
            o.class = e.int("Class").unwrap_or(0) as i32;
            o.max_hp = e.int("MaxHP").unwrap_or(o.hp as i64) as i32;
            let scores: Vec<i32> = e
                .list("Abilities")
                .unwrap_or_default()
                .iter()
                .map(|x| x.int("Value").unwrap_or(10) as i32)
                .collect();
            if scores.len() == 6 {
                o.abilities.copy_from_slice(&scores);
            }
            o.appearance = e.int("Appearance").unwrap_or(0) as i32;
            o.head = e.int("Head").unwrap_or(0) as i32;
            o.gender = e.int("Gender").unwrap_or(0) as i32;
            o.portrait = e.str("Portrait").unwrap_or("").to_string();
            o.force = e.int("Force").unwrap_or(0) as i32;
            o.max_force = e.int("MaxForce").unwrap_or(0) as i32;
            o.skill_points = e.int("SkillPoints").unwrap_or(0) as i32;
            o.feat_slots = (
                e.int("FeatRegular").unwrap_or(0) as i32,
                e.int("FeatBonus").unwrap_or(0) as i32,
            );
            o.skills = e
                .list("Skills")
                .unwrap_or_default()
                .iter()
                .map(|x| x.int("Value").unwrap_or(0) as i32)
                .collect();
            o.spells = ints("Spells");
            o.feats = ints("Feats");
            o.inventory = inventory;
            o.equipped = equipped;
            o.effects = effects;
            o.local_numbers = e
                .list("LocalNumbers")
                .unwrap_or_default()
                .iter()
                .filter_map(|x| Some((x.int("Index")? as i32, x.int("Value")? as i32)))
                .collect();
            o.local_booleans = e
                .list("LocalBooleans")
                .unwrap_or_default()
                .iter()
                .filter_map(|x| Some((x.int("Index")? as i32, x.int("Value")? != 0)))
                .collect();
            o.actions.clear();
        }
        // A save from before creatures had their portraits read has them blank.
        let blank: Vec<(ObjId, String)> = self
            .objects
            .values()
            .filter(|o| o.kind == Kind::Creature && o.portrait.is_empty() && !o.template.is_empty())
            .map(|o| (o.id, o.template.clone()))
            .collect();
        for (id, template) in blank {
            let file = self
                .inst
                .read(&template, kotor_core::ResType(2027))
                .ok()
                .and_then(|b| kotor_gff::Gff::parse(&b).ok());
            if let Some(file) = file {
                let portrait = self.creature_portrait(&file.root);
                if let Some(o) = self.objects.get_mut(&id) {
                    o.portrait = portrait;
                }
            }
        }
        (map, leader)
    }

    /// Write a save to `path`. Refuses a path inside the game's install folder (the install is read-only).
    pub fn save_to(&self, path: &Path, save_name: &str) -> Result<()> {
        let dir = self
            .inst
            .dir()
            .canonicalize()
            .unwrap_or_else(|_| self.inst.dir().to_path_buf());
        let parent = path.parent().unwrap_or(Path::new("."));
        let parent = parent
            .canonicalize()
            .unwrap_or_else(|_| parent.to_path_buf());
        if parent.starts_with(&dir) {
            return Err(Error::format(
                "refusing to write inside the game's install folder",
            ));
        }
        // A crash while writing must not cost the save that is being replaced.
        let part = path.with_extension("part");
        std::fs::write(&part, self.save_bytes(save_name)?)?;
        std::fs::rename(&part, path)?;
        Ok(())
    }

    /// Read a save from `path` (see [`World::load_save`]).
    pub fn load_from(&mut self, path: &Path) -> Result<()> {
        self.load_save(&std::fs::read(path)?)
    }

    /// Replace the world's state with a save made by [`World::save_bytes`]: the module is loaded fresh, then the
    /// saved state is applied over it, whatever module the world was in. A file that cannot be read, or that names a
    /// module the install does not have, leaves the world as it was.
    pub fn load_save(&mut self, bytes: &[u8]) -> Result<()> {
        let erf = Erf::open(Source::memory(bytes.to_vec()))?;
        let read = |name: &str| -> Result<Gff> { Gff::parse(&erf.get(name, rt())?) };
        let nfo = read("savenfo")?;
        if nfo.root.int("SaveVersion") != Some(SAVE_VERSION as i64) {
            return Err(Error::format("unsupported save version"));
        }
        let module = nfo.root.str("LASTMODULE").unwrap_or_default().to_string();
        let time = nfo.root.float("TIMEPLAYED").unwrap_or(0.0) as f32;
        let party_ai_style = nfo.root.int("PartyAIStyle").unwrap_or(0) as i32;
        let globals = read("globalvars")?;
        let party = read("partytable")?;
        let objects = read("objects")?;
        let modules = self.inst.dir().join("modules");
        if !["mod", "rim"]
            .iter()
            .any(|x| modules.join(format!("{module}.{x}")).is_file())
        {
            return Err(Error::NotFound(format!("module {module}")));
        }

        // A fresh start: only the character the player made survives, then the module is built again.
        let pc = self.hero;
        let mut player = self
            .objects
            .remove(&pc)
            .ok_or_else(|| Error::format("the world has no player"))?;
        self.objects.clear();
        player.actions.clear();
        player.effects.clear();
        self.objects.insert(pc, player);
        self.delayed.clear();
        self.transcript.clear();
        self.followers.clear();
        self.inside.clear();
        self.pending = None;
        self.transition_request = None;
        self.in_conversation = false;
        self.conv = None;
        self.conversation = None;
        self.opened = None;
        self.filled.clear();
        self.steer = None;
        self.combat = crate::combat::CombatState::new();
        self.game_over = false;
        self.last_power.clear();
        self.effect_expiry.clear();
        self.effect_spell.clear();
        self.effect_cursor.clear();
        self.party.clear();
        self.npc_slots.clear();
        self.inv_cursor.clear();
        self.effects.clear();
        self.load_module(&module)?;
        self.time = time;
        self.party_ai_style = party_ai_style;

        let keys = self.keys();
        let keep: HashSet<ObjId> = std::iter::once(pc).collect();
        let saved = objects.root.list("Objects").unwrap_or_default();
        let (map, mut leader) = self.apply_objects(saved, &keep, &keys);
        if !saved.iter().any(|e| e.int("IsLeader") == Some(1)) {
            leader = pc;
        }
        let resolve = |e: &Struct| map.get(&key_of(e)).copied();
        // Older saves have no `modules`: nothing is remembered of the modules left behind.
        self.left_modules = read("modules")
            .ok()
            .map(|m| {
                m.root
                    .list("Modules")
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|x| {
                        Some((x.str("Module")?.to_string(), x.list("Objects")?.to_vec()))
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Globals, journal, party.
        self.solo_mode = globals.root.int("SoloMode").unwrap_or(0) != 0;
        self.restrict_mode = globals.root.int("RestrictMode").unwrap_or(0) != 0;
        self.unselectable = globals
            .root
            .list("Unselectable")
            .unwrap_or_default()
            .iter()
            .filter_map(|x| Some(x.int("Row")? as i32))
            .collect();
        self.reputation_shifts = globals
            .root
            .list("ReputationList")
            .unwrap_or_default()
            .iter()
            .filter_map(|x| {
                let key = (x.int("From")? as i32, x.int("Of")? as i32);
                Some((key, x.int("By")? as i32))
            })
            .collect();
        self.planet_lines = globals
            .root
            .list("PlanetList")
            .unwrap_or_default()
            .iter()
            .filter_map(|x| {
                let row = x.int("Row")? as i32;
                Some((
                    row,
                    crate::galaxy::PlanetLine {
                        row,
                        available: x.int("Available")? != 0,
                        selectable: x.int("Selectable")? != 0,
                    },
                ))
            })
            .collect();
        self.galaxy = None;
        self.global_numbers = globals
            .root
            .list("NumberList")
            .unwrap_or_default()
            .iter()
            .filter_map(|x| Some((x.str("Name")?.to_ascii_uppercase(), x.int("Value")? as i32)))
            .collect();
        self.global_booleans = globals
            .root
            .list("BooleanList")
            .unwrap_or_default()
            .iter()
            .filter_map(|x| Some((x.str("Name")?.to_ascii_uppercase(), x.int("Value")? != 0)))
            .collect();
        let p = &party.root;
        self.journal = p
            .list("PT_JOURNAL")
            .unwrap_or_default()
            .iter()
            .filter_map(|x| Some((x.str("Tag")?.to_string(), x.int("State")? as i32)))
            .collect();
        let mut order: Vec<(i64, String)> = p
            .list("PT_JOURNAL")
            .unwrap_or_default()
            .iter()
            .filter_map(|x| Some((x.int("Order").unwrap_or(0), x.str("Tag")?.to_string())))
            .collect();
        order.sort();
        self.journal_order = order.into_iter().map(|o| o.1).collect();
        self.party = p
            .list("PT_MEMBERS")
            .unwrap_or_default()
            .iter()
            .filter_map(resolve)
            .collect();
        self.npc_slots = p
            .list("PT_NPC_SLOTS")
            .unwrap_or_default()
            .iter()
            .filter_map(|x| Some((x.int("Slot")? as i32, resolve(x)?)))
            .collect();
        self.available = p
            .list("PT_AVAIL_NPCS")
            .unwrap_or_default()
            .iter()
            .filter_map(|x| Some((x.int("Slot")? as i32, x.str("Template")?.to_string())))
            .collect();
        self.tutorial_seen = values_of(p, "PT_TUTORIALS").collect();
        self.plot_xp = p.int("PT_PLOT_XP").unwrap_or(0) as i32;
        self.explored = p
            .list("PT_EXPLORED")
            .unwrap_or_default()
            .iter()
            .filter_map(|x| Some((x.str("Map")?.to_string(), cells_of_runs(x.str("Runs")?))))
            .collect();
        self.bench = p
            .list("PT_BENCH")
            .unwrap_or_default()
            .iter()
            .filter_map(|b| Some((b.int("Slot")? as i32, bench_from(b))))
            .collect();
        self.gold = p.int("PT_GOLD").unwrap_or(0) as i32;
        // A save from before this has none: such a game began with the rule off.
        self.early_force = p.int("RS_EARLY_FORCE").unwrap_or(0) != 0;
        self.spent_encounters = p
            .str("RS_ENCOUNTERS")
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_string)
            .collect();
        self.tokens = p
            .list("PT_TOKENS")
            .unwrap_or_default()
            .iter()
            .filter_map(|x| Some((x.int("Number")? as i32, x.str("Text")?.to_string())))
            .collect();
        self.loaded_from_save = true;
        if let Some(r) = p.int("PT_RNG") {
            self.rng = r as u64;
        }

        // Rebuild what depends on the loaded state: doors in the navigation mesh, and the player's mover.
        self.sync_door_lines();
        let pos = match p.get("PT_PLAYER_POS") {
            Some(Value::Vector(v)) => *v,
            _ => self.object(leader).map_or([0.0; 3], |o| o.position),
        };
        let heading = p.float("PT_PLAYER_HEAD").unwrap_or(0.0) as f32;
        self.pc_mover = kotor_walkmesh::Mover::new(pos, heading);
        self.remember_items();
        self.pc = leader;
        if let Some(o) = self.objects.get_mut(&leader) {
            o.position = pos;
        }
        Ok(())
    }

    /// A text description of the saved-state parts of the world that does not depend on object ids, for comparing
    /// a world with a reloaded copy of itself.
    pub fn state_digest(&self) -> String {
        use std::fmt::Write;
        let keys = self.keys();
        let name = |id: &ObjId| {
            keys.get(id).map_or("?".to_string(), |k| {
                format!("{}:{}:{}:{}", k.0, k.1, k.2, k.3)
            })
        };
        let mut out = String::new();
        let _ = writeln!(out, "module {} time {:.3}", self.module, self.time);
        let mut g: Vec<_> = self.global_numbers.iter().collect();
        g.sort();
        let _ = writeln!(out, "numbers {g:?}");
        let mut b: Vec<_> = self.global_booleans.iter().collect();
        b.sort();
        let _ = writeln!(out, "booleans {b:?}");
        let _ = writeln!(out, "journal {:?}", self.journal);
        let _ = writeln!(out, "bench {:?}", self.bench);
        let _ = writeln!(out, "explored {:?}", self.explored);
        let party: Vec<String> = self.party.iter().map(&name).collect();
        let _ = writeln!(out, "party {party:?}");
        let mut slots: Vec<_> = self
            .npc_slots
            .iter()
            .map(|(s, id)| (*s, name(id)))
            .collect();
        slots.sort();
        let _ = writeln!(out, "slots {slots:?} available {:?}", self.available);
        let mut rows: Vec<String> = Vec::new();
        for o in self
            .objects
            .values()
            .filter(|o| !matches!(o.kind, Kind::Module | Kind::Area))
        {
            let inv: Vec<String> = o.inventory.iter().map(&name).collect();
            let eq: Vec<(i32, String)> = o.equipped.iter().map(|(s, i)| (*s, name(i))).collect();
            let mut ln: Vec<_> = o.local_numbers.iter().collect();
            ln.sort();
            let mut lb: Vec<_> = o.local_booleans.iter().collect();
            lb.sort();
            let fx: Vec<String> = o
                .effects
                .iter()
                .filter_map(|h| self.effects.get(h.wrapping_sub(1) as usize))
                .map(|e| format!("{}{:?}", e.name, e.args))
                .collect();
            rows.push(format!(
                "{} filled={} drop={} stack={} name={:?} pos=({:.3},{:.3},{:.3}) face={:.2} hp={} dead={} party={} locked={} lvl={} open={} door={:?} base={} xp={} fac={}/{}/{} spells={:?} feats={:?} inv={inv:?} eq={eq:?} ln={ln:?} lb={lb:?} fx={fx:?}",
                name(&o.id),
                self.filled.contains(&o.id),
                o.droppable,
                o.stack,
                o.name,
                o.position[0],
                o.position[1],
                o.position[2],
                o.facing,
                o.hp,
                o.dead,
                o.party_member,
                o.locked,
                o.level,
                o.open,
                o.door,
                o.base_item,
                o.xp,
                o.faction,
                o.race,
                o.subrace,
                o.spells,
                o.feats,
            ));
        }
        rows.sort();
        for r in rows {
            let _ = writeln!(out, "{r}");
        }
        out
    }
}
