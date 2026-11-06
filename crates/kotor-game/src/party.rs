//! Who goes along: what the party selection screen needs.
//!
//! A member who is left behind is gone from the world, and what they were is kept in `World::bench` until they come back.
//! **Assumed, not compared with the game's code:** they gain no experience meanwhile; an item is kept as its template and
//! stack; a member left behind at 0 hit points comes back with 1.

use crate::host::PARTY_FACTION;
use crate::world::{Kind, ObjId, World};
use kotor_core::ResType;
use kotor_gff::Gff;
use std::collections::BTreeSet;

/// Most members beside the leader.
pub const PARTY_SIZE: usize = 2;
const CREATURE_FILE: ResType = ResType(2027);

/// A script asked for the party screen with `ShowPartySelectionGUI`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PartyRequest {
    /// What runs when the screen closes.
    pub script: String,
    /// The slots that have to come along.
    pub forced: Vec<i32>,
}

/// What a member left behind keeps.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BenchedMember {
    pub class: i32,
    pub level: i32,
    pub xp: i32,
    pub alignment: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub force: i32,
    pub max_force: i32,
    pub abilities: [i32; 6],
    pub skills: Vec<i32>,
    pub skill_points: i32,
    pub feats: BTreeSet<i32>,
    pub feat_slots: (i32, i32),
    pub spells: BTreeSet<i32>,
    /// Template and stack of what is carried.
    pub carried: Vec<(String, i32)>,
    /// Slot, template and stack of what is worn.
    pub worn: Vec<(i32, String, i32)>,
}

impl World {
    /// The slots of the party's members now.
    pub fn party_slots(&self) -> Vec<i32> {
        let mut slots: Vec<i32> = self
            .npc_slots
            .iter()
            .filter(|(_, id)| self.party.contains(id))
            .map(|(slot, _)| *slot)
            .collect();
        slots.sort_unstable();
        slots
    }

    /// The talk table's name for a class.
    fn class_name(&mut self, class: i32) -> u32 {
        self.table("classes")
            .and_then(|t| t.int(class.max(0) as usize, "name"))
            .unwrap_or(0) as u32
    }

    /// The name, level and class's name of whoever a slot offers: the creature if it is here, else its template.
    pub fn party_candidate(&mut self, slot: i32) -> Option<(String, i32, u32)> {
        if let Some(o) = self
            .npc_slots
            .get(&slot)
            .and_then(|id| self.objects.get(id))
        {
            let (name, level, class) = (o.name.clone(), o.level, o.class);
            return Some((name, level, self.class_name(class)));
        }
        let kept = self.bench.get(&slot).map(|b| (b.level, b.class));
        let template = self.available.get(&slot)?.clone();
        let g = Gff::parse(&self.inst.read(&template, CREATURE_FILE).ok()?).ok()?;
        let name = g.root.loc("FirstName").map_or(template, |l| self.text(l));
        let classes = g.root.list("ClassList").unwrap_or_default();
        let level = classes
            .iter()
            .filter_map(|c| c.int("ClassLevel"))
            .sum::<i64>();
        let class = classes.first().and_then(|c| c.int("Class")).unwrap_or(0);
        let (level, class) = kept.unwrap_or((level as i32, class as i32));
        Some((name, level, self.class_name(class)))
    }

    /// Bring a slot's creature to the party, beside the leader.
    fn join_slot(&mut self, slot: i32) -> Option<ObjId> {
        let template = self.available.get(&slot)?.clone();
        let (at, facing) = self.object(self.pc).map(|o| (o.position, o.facing))?;
        let id = self.spawn(Kind::Creature, &template, CREATURE_FILE, at);
        self.party.push(id);
        self.npc_slots.insert(slot, id);
        if let Some(o) = self.object_mut(id) {
            o.party_member = true;
            o.faction = PARTY_FACTION;
            o.facing = facing;
        }
        self.run_spawn_script(id);
        self.restore_benched(slot, id);
        Some(id)
    }

    /// Send a member away, and keep what they were.
    fn bench_member(&mut self, slot: i32, id: ObjId) {
        self.party.retain(|p| *p != id);
        self.followers.retain(|f, _| *f != id);
        let Some(o) = self.objects.remove(&id) else {
            return;
        };
        let mut item = |item: &ObjId| {
            let i = self.objects.remove(item)?;
            Some((i.template, i.stack))
        };
        let carried = o.inventory.iter().filter_map(&mut item).collect();
        let worn = o.equipped.iter().filter_map(|(slot, id)| {
            let (template, stack) = item(id)?;
            Some((*slot, template, stack))
        });
        let worn = worn.collect();
        self.bench.insert(
            slot,
            BenchedMember {
                class: o.class,
                level: o.level,
                xp: o.xp,
                alignment: o.alignment,
                hp: o.hp.max(1),
                max_hp: o.max_hp,
                force: o.force,
                max_force: o.max_force,
                abilities: o.abilities,
                skills: o.skills,
                skill_points: o.skill_points,
                feats: o.feats,
                feat_slots: o.feat_slots,
                spells: o.spells,
                carried,
                worn,
            },
        );
    }

    /// A creature just made for a slot becomes who was left behind there, in place of what its template and spawn
    /// script gave it.
    pub(crate) fn restore_benched(&mut self, slot: i32, id: ObjId) {
        let Some(b) = self.bench.remove(&slot) else {
            return;
        };
        let given: Vec<ObjId> = self.objects.get(&id).map_or(Vec::new(), |o| {
            o.inventory
                .iter()
                .chain(o.equipped.values())
                .copied()
                .collect()
        });
        for item in given {
            self.objects.remove(&item);
        }
        let mut one = |template: &str, stack: i32| {
            let item = self.spawn_item(template);
            if let Some(i) = self.object_mut(item) {
                i.stack = stack;
            }
            item
        };
        let inventory = b.carried.iter().map(|(t, n)| one(t, *n)).collect();
        let equipped = b.worn.iter().map(|(s, t, n)| (*s, one(t, *n))).collect();
        let Some(o) = self.object_mut(id) else {
            return;
        };
        o.inventory = inventory;
        o.equipped = equipped;
        o.class = b.class;
        o.level = b.level;
        o.xp = b.xp;
        o.alignment = b.alignment;
        o.hp = b.hp;
        o.max_hp = b.max_hp;
        o.force = b.force;
        o.max_force = b.max_force;
        o.abilities = b.abilities;
        o.skills = b.skills;
        o.skill_points = b.skill_points;
        o.feats = b.feats;
        o.feat_slots = b.feat_slots;
        o.spells = b.spells;
    }

    /// The party's members become these slots. Then `script`, the one `ShowPartySelectionGUI` named, runs.
    pub fn choose_party(&mut self, slots: &[i32], script: &str) {
        for slot in self.party_slots() {
            if !slots.contains(&slot) {
                if let Some(id) = self.npc_slots.remove(&slot) {
                    self.bench_member(slot, id);
                }
            }
        }
        for slot in slots {
            if !self.npc_slots.contains_key(slot) {
                self.join_slot(*slot);
            }
        }
        if !script.is_empty() {
            let pc = self.pc;
            let _ = self.run_script(script, pc);
        }
    }
}
