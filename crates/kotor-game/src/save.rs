//! Saving and loading the world as a SAV archive (an ERF with `savenfo`, `globalvars`, `partytable` and `objects`
//! resources, each a GFF).
//!
//! **This is this engine's own layout, not the original game's save format**: the field names follow the original
//! where I know them (`savenfo`, `globalvars`, `partytable` are the original's resource names), but the contents were
//! not checked against a real save, so a save made here is not meant to be opened by the original game, and the
//! original's saves are not read. **Not saved (assumption):** `DelayCommand` blocks still pending, objects' action
//! queues, conversations in progress, and `Effect` or location handles held by scripts. A load starts the module
//! fresh, applies the saved state over it and does not run the area's enter scripts again.

use crate::world::{DoorState, Kind, ObjId, World};
use kotor_archive::{write_archive, Erf, ErfKind, NewResource, Source};
use kotor_core::{Error, ResType, Result};
use kotor_gff::{Gff, Struct, Value};
use std::collections::HashMap;
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

impl World {
    fn keys(&self) -> HashMap<ObjId, Key> {
        let mut seen: HashMap<(u8, String, String), usize> = HashMap::new();
        let mut out = HashMap::new();
        for o in self.objects.values() {
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
        let nfo = gff(
            "NFO ",
            vec![
                ("SaveVersion", Value::Int(SAVE_VERSION)),
                ("Engine", s("kotor-rs")),
                ("LASTMODULE", s(&self.module)),
                ("AREANAME", s(&area)),
                ("PCNAME", s(&pc_name)),
                ("SAVEGAMENAME", s(save_name)),
                ("TIMEPLAYED", Value::Float(self.time)),
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
                        st(vec![("Tag", s(tag)), ("State", Value::Int(*state))])
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
        write_archive(
            ErfKind::Sav,
            &[
                NewResource::new("savenfo", rt(), nfo),
                NewResource::new("globalvars", rt(), globals),
                NewResource::new("partytable", rt(), party),
                NewResource::new("objects", rt(), objects),
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
            ("IsPlayer", Value::Byte((o.id == self.pc) as u8)),
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
            ("XP", Value::Int(o.xp)),
            ("Faction", Value::Int(o.faction)),
            ("Race", Value::Int(o.race)),
            ("Subrace", Value::Int(o.subrace)),
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
                        ("Name", s(e.map_or("", |e| &e.name))),
                        (
                            "Args",
                            list_of(e.map(|e| e.args.clone()).unwrap_or_default(), |a| {
                                st(vec![("Value", s(&a))])
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
        std::fs::write(path, self.save_bytes(save_name)?)?;
        Ok(())
    }

    /// Read a save from `path` (see [`World::load_save`]).
    pub fn load_from(&mut self, path: &Path) -> Result<()> {
        self.load_save(&std::fs::read(path)?)
    }

    /// Replace the world's state with a save made by [`World::save_bytes`]: the module is loaded fresh, then the
    /// saved state is applied over it. Errors leave the world partly changed.
    pub fn load_save(&mut self, bytes: &[u8]) -> Result<()> {
        let erf = Erf::open(Source::memory(bytes.to_vec()))?;
        let read = |name: &str| -> Result<Gff> { Gff::parse(&erf.get(name, rt())?) };
        let nfo = read("savenfo")?;
        if nfo.root.int("SaveVersion") != Some(SAVE_VERSION as i64) {
            return Err(Error::format("unsupported save version"));
        }
        let module = nfo.root.str("LASTMODULE").unwrap_or_default().to_string();
        let time = nfo.root.float("TIMEPLAYED").unwrap_or(0.0) as f32;
        let globals = read("globalvars")?;
        let party = read("partytable")?;
        let objects = read("objects")?;

        // A fresh start: only the player object survives, then the module is built again.
        let pc = self.pc;
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
        self.party.clear();
        self.npc_slots.clear();
        self.inv_cursor.clear();
        self.effects.clear();
        self.load_module(&module)?;
        self.time = time;

        // Match saved objects to the fresh ones by (kind, tag, template, nth); spawn the rest; drop the unmatched.
        let fresh: HashMap<Key, ObjId> = self
            .keys()
            .into_iter()
            .filter(|(id, _)| *id != pc)
            .map(|(id, k)| (k, id))
            .collect();
        let saved = objects.root.list("Objects").unwrap_or_default();
        let key_of = |e: &Struct| -> Key {
            (
                e.int("Kind").unwrap_or(255) as u8,
                e.str("Tag").unwrap_or("").to_ascii_lowercase(),
                e.str("Template").unwrap_or("").to_ascii_lowercase(),
                e.int("Nth").unwrap_or(0) as usize,
            )
        };
        let mut map: HashMap<Key, ObjId> = HashMap::new();
        let mut wanted: std::collections::HashSet<ObjId> = std::collections::HashSet::new();
        wanted.insert(pc);
        for e in saved {
            let key = key_of(e);
            if e.int("IsPlayer") == Some(1) {
                map.insert(key, pc);
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
        for e in saved {
            let Some(id) = resolve(e) else { continue };
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
                self.effects.push(crate::world::EffectRec {
                    name: f.str("Name").unwrap_or("").to_string(),
                    args: f
                        .list("Args")
                        .unwrap_or_default()
                        .iter()
                        .map(|a| a.str("Value").unwrap_or("").to_string())
                        .collect(),
                });
                effects.push(self.effects.len() as u32);
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

        // Globals, journal, party.
        self.global_numbers = globals
            .root
            .list("NumberList")
            .unwrap_or_default()
            .iter()
            .filter_map(|x| Some((x.str("Name")?.to_string(), x.int("Value")? as i32)))
            .collect();
        self.global_booleans = globals
            .root
            .list("BooleanList")
            .unwrap_or_default()
            .iter()
            .filter_map(|x| Some((x.str("Name")?.to_string(), x.int("Value")? != 0)))
            .collect();
        let p = &party.root;
        self.journal = p
            .list("PT_JOURNAL")
            .unwrap_or_default()
            .iter()
            .filter_map(|x| Some((x.str("Tag")?.to_string(), x.int("State")? as i32)))
            .collect();
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
        if let Some(r) = p.int("PT_RNG") {
            self.rng = r as u64;
        }

        // Rebuild what depends on the loaded state: doors in the navigation mesh, and the player's mover.
        let open_doors: Vec<[f32; 3]> = self
            .objects
            .values()
            .filter(|o| o.kind == Kind::Door && o.door == DoorState::Open)
            .map(|o| o.position)
            .collect();
        if let Some(n) = self.nav.as_mut() {
            for pos in open_doors {
                n.unblock_disc(pos, crate::DOOR_BLOCK_RADIUS);
            }
        }
        let pos = match p.get("PT_PLAYER_POS") {
            Some(Value::Vector(v)) => *v,
            _ => self.object(pc).map_or([0.0; 3], |o| o.position),
        };
        let heading = p.float("PT_PLAYER_HEAD").unwrap_or(0.0) as f32;
        self.pc_mover = kotor_walkmesh::Mover::new(pos, heading);
        if let Some(o) = self.objects.get_mut(&pc) {
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
                "{} name={:?} pos=({:.3},{:.3},{:.3}) face={:.2} hp={} dead={} party={} locked={} lvl={} open={} door={:?} base={} xp={} fac={}/{}/{} spells={:?} feats={:?} inv={inv:?} eq={eq:?} ln={ln:?} lb={lb:?} fx={fx:?}",
                name(&o.id),
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
