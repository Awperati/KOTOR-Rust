//! What a character sheet shows, from the rules, and using an item.

use crate::combat::ability_mod;
use crate::rules::Save;
use crate::world::{ObjId, World};

/// One creature's numbers, for the character screen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sheet {
    pub name: String,
    pub class: String,
    pub level: i32,
    pub xp: i32,
    /// Experience the next level needs. The same as `xp` at the top of the table.
    pub next_level_xp: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub force: i32,
    pub max_force: i32,
    pub defense: i32,
    /// Strength, Dexterity, Constitution, Intelligence, Wisdom, Charisma: score and modifier.
    pub abilities: [(i32, i32); 6],
    pub fortitude: i32,
    pub reflex: i32,
    pub will: i32,
    /// 0 dark to 100 light.
    pub alignment: i32,
}

/// Why an item cannot be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CannotUse {
    /// It has no power to use.
    NoPower,
    Failed,
}

impl World {
    pub fn sheet(&mut self, id: ObjId) -> Option<Sheet> {
        let o = self.objects.get(&id)?.clone();
        let class = self.table("classes").and_then(|t| {
            let row = o.class.max(0) as usize;
            let strref = t.int(row, "name")? as i32;
            let name = self.strref_text(strref);
            Some(if name.is_empty() {
                t.get(row, "label")?.to_string()
            } else {
                name
            })
        });
        let mut abilities = [(0, 0); 6];
        for (i, a) in abilities.iter_mut().enumerate() {
            let score = self.ability_score(id, i);
            *a = (score, ability_mod(score));
        }
        let next_level_xp = self
            .exptable
            .iter()
            .copied()
            .find(|need| *need > o.xp)
            .unwrap_or(o.xp);
        Some(Sheet {
            name: o.name.clone(),
            class: class.unwrap_or_default(),
            level: o.level,
            xp: o.xp,
            next_level_xp,
            hp: o.hp,
            max_hp: o.max_hp,
            force: o.force,
            max_force: o.max_force,
            defense: self.defense_of(id),
            abilities,
            fortitude: self.save_bonus(id, Save::Fortitude),
            reflex: self.save_bonus(id, Save::Reflex),
            will: self.save_bonus(id, Save::Will),
            alignment: o.alignment,
        })
    }

    /// Use an item a creature carries: the power in its `CastSpell` property runs on the creature through its row of
    /// `spells.2da` and impact script, as for a Force power, and one comes off the stack.
    ///
    /// **Measured:** a medpack has property 10 (`CastSpell` in `itempropdef.2da`) with `Subtype` 64, and row 64 of
    /// `spells.2da` is `ITEM_ABILITY_MED_PACK`, script `k_sup_healing`. **Assumed:** using an item takes no time and has
    /// no feat or skill check.
    pub fn use_item(&mut self, who: ObjId, item: ObjId) -> Result<(), CannotUse> {
        let spell = self.item_view(item).and_then(|v| v.power);
        let spell = spell.ok_or(CannotUse::NoPower)?;
        self.cast_spell(who, spell, who, true)
            .map_err(|_| CannotUse::Failed)?;
        let last = self.objects.get(&item).is_some_and(|o| o.stack <= 1);
        if last {
            if let Some(o) = self.objects.get_mut(&who) {
                o.inventory.retain(|i| *i != item);
            }
            self.objects.remove(&item);
        } else if let Some(o) = self.objects.get_mut(&item) {
            o.stack -= 1;
        }
        Ok(())
    }
}
