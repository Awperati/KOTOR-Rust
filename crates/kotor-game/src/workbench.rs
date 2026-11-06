//! What the upgrade bench works on. `upgrade.2da` says which item is which upgrade and what kind of slot it goes in. An
//! item's own properties name the row they wait for in `UpgradeType`, so an item can be upgraded when it has such a property.
//!
//! **Read from the install:** all of that. **Assumed:** a kind's slots, from `upgrade.2da`'s types and the reference video
//! at 1:13:05 and 3:42:00: melee weapons take types 1 to 3, ranged 4 to 7, armour 8 and 9, and a lightsaber four crystals.
//! **Not built:** what a property does. The rules do not read item properties yet, so an upgrade only changes the list.

use crate::world::{ObjId, World};
use kotor_core::ResType;
use kotor_gff::Gff;

const ITEM_FILE: ResType = ResType(2025);
const CRYSTALS: [i32; 4] = [0; 4];
const MELEE: [i32; 3] = [1, 2, 3];
const RANGED: [i32; 4] = [4, 5, 6, 7];
const ARMOUR: [i32; 2] = [8, 9];

/// The four kinds of thing on the bench's first screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BenchKind {
    Lightsaber,
    Ranged,
    Melee,
    Armor,
}

impl BenchKind {
    /// The `upgrade.2da` type each slot takes.
    pub fn slots(self) -> &'static [i32] {
        match self {
            BenchKind::Lightsaber => &CRYSTALS,
            BenchKind::Ranged => &RANGED,
            BenchKind::Melee => &MELEE,
            BenchKind::Armor => &ARMOUR,
        }
    }
}

impl World {
    fn upgrade_rows(&mut self) -> Vec<(String, i32)> {
        let Some(t) = self.table("upgrade") else {
            return Vec::new();
        };
        (0..t.row_count())
            .map(|r| {
                (
                    t.get(r, "template").unwrap_or("").to_ascii_lowercase(),
                    t.int(r, "upgradetype").unwrap_or(-1) as i32,
                )
            })
            .collect()
    }

    fn item_file(&mut self, template: &str) -> Option<Gff> {
        Gff::parse(&self.inst.read(template, ITEM_FILE).ok()?).ok()
    }

    /// What kind of thing an item is on the bench, if it has properties that wait for an upgrade.
    pub fn bench_kind(&mut self, item: ObjId) -> Option<BenchKind> {
        let (template, row) = {
            let o = self.objects.get(&item)?;
            (o.template.clone(), o.base_item.max(0) as usize)
        };
        let file = self.item_file(&template)?;
        let waits = file
            .root
            .list("PropertiesList")
            .unwrap_or_default()
            .iter()
            .any(|p| p.int("UpgradeType").is_some());
        if !waits {
            return None;
        }
        let t = self.table("baseitems")?;
        let label = t.get(row, "label").unwrap_or("").to_ascii_lowercase();
        Some(if label.contains("saber") {
            BenchKind::Lightsaber
        } else if t.int(row, "rangedweapon").unwrap_or(0) != 0 {
            BenchKind::Ranged
        } else if t.int(row, "weaponwield").is_some() {
            BenchKind::Melee
        } else {
            BenchKind::Armor
        })
    }

    /// What a creature has of a kind, carried or worn.
    pub fn bench_items(&mut self, who: ObjId, kind: BenchKind) -> Vec<ObjId> {
        let mut all = self.carried(who);
        if let Some(o) = self.objects.get(&who) {
            all.extend(o.equipped.values().copied());
        }
        all.into_iter()
            .filter(|i| self.bench_kind(*i) == Some(kind))
            .collect()
    }

    /// The upgrades carried that go in a slot of this type.
    pub fn bench_choices(&mut self, who: ObjId, slot_type: i32) -> Vec<ObjId> {
        let rows = self.upgrade_rows();
        self.carried(who)
            .into_iter()
            .filter(|i| {
                let Some(o) = self.objects.get(i) else {
                    return false;
                };
                let template = o.template.to_ascii_lowercase();
                rows.iter()
                    .any(|(t, ty)| *t == template && *ty == slot_type)
            })
            .collect()
    }

    /// Put a carried upgrade in a slot of an item: what was there comes back to the bag. One of a stack is taken.
    pub fn install_upgrade(&mut self, who: ObjId, item: ObjId, slot: i32, upgrade: ObjId) -> bool {
        let Some(template) = self
            .objects
            .get(&upgrade)
            .map(|o| o.template.to_ascii_lowercase())
        else {
            return false;
        };
        let rows = self.upgrade_rows();
        let Some(row) = rows.iter().position(|(t, _)| *t == template) else {
            return false;
        };
        self.remove_upgrade(who, item, slot);
        let many = self.objects.get(&upgrade).is_some_and(|o| o.stack > 1);
        if many {
            if let Some(o) = self.objects.get_mut(&upgrade) {
                o.stack -= 1;
            }
        } else {
            if let Some(o) = self.objects.get_mut(&who) {
                o.inventory.retain(|i| *i != upgrade);
            }
            self.objects.remove(&upgrade);
        }
        if let Some(o) = self.objects.get_mut(&item) {
            o.upgrades.insert(slot, row as i32);
        }
        true
    }

    /// Take what is in a slot back into the bag.
    pub fn remove_upgrade(&mut self, who: ObjId, item: ObjId, slot: i32) {
        let row = self
            .objects
            .get_mut(&item)
            .and_then(|o| o.upgrades.remove(&slot));
        let Some(row) = row else { return };
        let rows = self.upgrade_rows();
        if let Some((template, _)) = rows.get(row.max(0) as usize) {
            let back = self.spawn_item(template);
            self.carry(who, back);
        }
    }

    /// The talk table's name of each property an item has working: those that wait for nothing, and those its upgrades
    /// have woken.
    pub fn item_properties(&mut self, item: ObjId) -> Vec<u32> {
        let Some((template, installed)) = self
            .objects
            .get(&item)
            .map(|o| (o.template.clone(), o.upgrades.clone()))
        else {
            return Vec::new();
        };
        let (Some(file), Some(names)) = (self.item_file(&template), self.table("itempropdef"))
        else {
            return Vec::new();
        };
        file.root
            .list("PropertiesList")
            .unwrap_or_default()
            .iter()
            .filter(|p| {
                p.int("UpgradeType")
                    .is_none_or(|row| installed.values().any(|r| i64::from(*r) == row))
            })
            .filter_map(|p| names.int(p.int("PropertyName")? as usize, "name"))
            .map(|n| n as u32)
            .collect()
    }

    /// The talk table's name of what is in row `row` of `upgrade.2da`, and its picture's name.
    pub fn upgrade_look(&mut self, row: i32) -> Option<(u32, String)> {
        let rows = self.upgrade_rows();
        let (template, _) = rows.get(row.max(0) as usize)?;
        let file = self.item_file(template)?;
        let name = file.root.loc("LocalizedName")?.strref as u32;
        let base = file.root.int("BaseItem")?.max(0) as usize;
        let class = self.table("baseitems")?;
        let class = class.get(base, "itemclass")?.to_ascii_lowercase();
        let variation = file.root.int("ModelVariation")?;
        Some((name, format!("i{class}_{variation:03}")))
    }

    /// The talk table's name of the upgrades of a type.
    pub fn upgrade_type_name(&mut self, slot_type: i32) -> Option<u32> {
        let rows = self.upgrade_rows();
        let row = rows.iter().position(|(_, ty)| *ty == slot_type)?;
        Some(self.upgrade_look(row as i32)?.0)
    }
}
