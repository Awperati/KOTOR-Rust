//! Who goes along: what the party selection screen needs.
//!
//! **Assumed, not compared with the game's code:** a member who is left behind is gone from the world and comes back as
//! their template says, so what they carried and the levels they gained are lost. The game keeps them; this does not yet.

use crate::host::PARTY_FACTION;
use crate::world::{Kind, ObjId, World};
use kotor_core::ResType;
use kotor_gff::Gff;

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
        let template = self.available.get(&slot)?.clone();
        let g = Gff::parse(&self.inst.read(&template, CREATURE_FILE).ok()?).ok()?;
        let name = g.root.loc("FirstName").map_or(template, |l| self.text(l));
        let classes = g.root.list("ClassList").unwrap_or_default();
        let level = classes
            .iter()
            .filter_map(|c| c.int("ClassLevel"))
            .sum::<i64>();
        let class = classes.first().and_then(|c| c.int("Class")).unwrap_or(0);
        Some((name, level as i32, self.class_name(class as i32)))
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
        Some(id)
    }

    /// The party's members become these slots. Then `script`, the one `ShowPartySelectionGUI` named, runs.
    pub fn choose_party(&mut self, slots: &[i32], script: &str) {
        for slot in self.party_slots() {
            if !slots.contains(&slot) {
                if let Some(id) = self.npc_slots.remove(&slot) {
                    self.party.retain(|p| *p != id);
                    self.followers.retain(|f, _| *f != id);
                    self.objects.remove(&id);
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
