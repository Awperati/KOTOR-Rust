//! Carrying and wearing items: which slot takes what, stacks, credits, and what a screen shows about an item.
//!
//! **Read from `baseitems.2da`:** `equipableslots` is a mask of slot bits, `stacking` the largest stack, `label` names
//! the credits row. **Assumed, not compared with the game:** every `reqfeat` column of a row must be known (armour rows
//! list the lighter armour's feat as well, so "any of them" would let light armour's feat wear heavy armour);
//! `droidorhuman` 1 is for humans and 2 for droids; a weapon that fits only the right hand takes both hands.

use crate::world::{Kind, ObjId, World};
use kotor_core::ResType;
use kotor_gff::Gff;
use std::rc::Rc;

pub const SLOT_HEAD: i32 = 0;
pub const SLOT_BODY: i32 = 1;
pub const SLOT_HANDS: i32 = 3;
pub const SLOT_RIGHT_HAND: i32 = 4;
pub const SLOT_LEFT_HAND: i32 = 5;
pub const SLOT_LEFT_ARM: i32 = 7;
pub const SLOT_RIGHT_ARM: i32 = 8;
pub const SLOT_IMPLANT: i32 = 9;
pub const SLOT_BELT: i32 = 10;
/// The slots of a creature, in the order an item looks for a free one.
pub const SLOTS: [i32; 9] = [
    SLOT_BODY,
    SLOT_RIGHT_HAND,
    SLOT_LEFT_HAND,
    SLOT_HEAD,
    SLOT_HANDS,
    SLOT_LEFT_ARM,
    SLOT_RIGHT_ARM,
    SLOT_IMPLANT,
    SLOT_BELT,
];
const REQUIRED_FEATS: [&str; 5] = ["reqfeat0", "reqfeat1", "reqfeat2", "reqfeat3", "reqfeat4"];
const DROID_RACE: i32 = 5;

/// What the UTI gives that the object does not keep.
#[derive(Debug, Default)]
pub(crate) struct ItemTemplate {
    description: String,
    model_variation: i64,
    texture_variation: i64,
    /// The row of `spells.2da` its `CastSpell` property runs.
    power: Option<i32>,
    pub(crate) properties: Vec<crate::gear::ItemProperty>,
}

/// What a screen shows about an item.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemView {
    pub id: ObjId,
    pub name: String,
    pub description: String,
    /// Texture names to try, best first: `i<itemclass>_<number>`, then the row's `defaulticon`.
    pub icons: Vec<String>,
    pub stack: i32,
    /// Bits of the slots it fits.
    pub slots: u32,
    /// The row of `spells.2da` it runs when used, if it can be used.
    pub power: Option<i32>,
}

/// Two weapons, or one in the right hand that takes both: why an item cannot go on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CannotEquip {
    /// It is not something to wear or hold.
    NotWearable,
    /// That slot does not take it.
    WrongSlot,
    /// A feat is missing, or it is for droids.
    Prerequisites,
    /// It is for the other side, in the game's words at 1512.
    Alignment,
    /// The right hand holds a weapon that takes both.
    BothHandsFull,
}

impl World {
    pub(crate) fn item_template(&mut self, template: &str) -> Rc<ItemTemplate> {
        let key = template.to_ascii_lowercase();
        if let Some(t) = self.item_templates.get(&key) {
            return t.clone();
        }
        let mut out = ItemTemplate::default();
        if let Some(g) = self
            .inst
            .read(&key, ResType(2025))
            .ok()
            .and_then(|b| Gff::parse(&b).ok())
        {
            out.description = g
                .root
                .loc("DescIdentified")
                .or_else(|| g.root.loc("Description"))
                .map(|l| self.text(l))
                .unwrap_or_default();
            out.model_variation = g.root.int("ModelVariation").unwrap_or(0);
            out.texture_variation = g.root.int("TextureVar").unwrap_or(0);
            // Property 10 is `CastSpell` in `itempropdef.2da`.
            out.power = g
                .root
                .list("PropertiesList")
                .unwrap_or_default()
                .iter()
                .find(|p| p.int("PropertyName") == Some(10))
                .and_then(|p| p.int("Subtype"))
                .map(|s| s as i32);
            out.properties =
                self.read_properties(g.root.list("PropertiesList").unwrap_or_default());
        }
        let out = Rc::new(out);
        self.item_templates.insert(key, out.clone());
        out
    }

    /// The bits of the slots an item fits, from `baseitems.2da`.
    pub fn slots_of(&mut self, item: ObjId) -> u32 {
        let Some(row) = self.objects.get(&item).map(|o| o.base_item.max(0) as usize) else {
            return 0;
        };
        self.table("baseitems")
            .and_then(|t| {
                let v = t.get(row, "equipableslots")?;
                u32::from_str_radix(v.trim_start_matches("0x"), 16).ok()
            })
            .unwrap_or(0)
    }

    /// What a screen shows about an item.
    pub fn item_view(&mut self, item: ObjId) -> Option<ItemView> {
        let o = self.objects.get(&item)?;
        let (template, base, stack) = (o.template.clone(), o.base_item.max(0) as usize, o.stack);
        let name = if o.name.is_empty() {
            template.clone()
        } else {
            o.name.clone()
        };
        let t = self.item_template(&template);
        let table = self.table("baseitems");
        let cell = |c: &str| {
            table
                .as_ref()
                .and_then(|x| x.get(base, c))
                .unwrap_or("")
                .to_ascii_lowercase()
        };
        // Measured over the BIFs' 557 item templates: 430 have a picture by `ModelVariation` and 97, the armour, by
        // `TextureVar`. `defaulticon` found no picture for any of the other 30.
        let class = cell("itemclass");
        let mut icons = Vec::new();
        for n in [t.model_variation, t.texture_variation] {
            if n > 0 && !class.is_empty() {
                icons.push(format!("i{class}_{n:03}"));
            }
        }
        let fallback = cell("defaulticon");
        if !fallback.is_empty() {
            icons.push(fallback);
        }
        let slots = self.slots_of(item);
        Some(ItemView {
            id: item,
            name,
            description: t.description.clone(),
            icons,
            stack,
            slots,
            power: t.power,
        })
    }

    /// Put a creature from a template at a place, as `CreateObject` does.
    pub fn spawn_creature(&mut self, template: &str, at: [f32; 3]) -> ObjId {
        self.spawn(Kind::Creature, template, ResType(2027), at)
    }

    /// What a creature carries and does not wear.
    pub fn carried(&self, who: ObjId) -> Vec<ObjId> {
        let Some(o) = self.objects.get(&who) else {
            return Vec::new();
        };
        o.inventory
            .iter()
            .copied()
            .filter(|i| !o.equipped.values().any(|e| e == i))
            .collect()
    }

    /// Whether a creature may wear or hold an item at all.
    pub fn can_wear(&mut self, who: ObjId, item: ObjId) -> Result<(), CannotEquip> {
        if self.slots_of(item) == 0 {
            return Err(CannotEquip::NotWearable);
        }
        if self.objects.get(&item).is_some_and(|o| o.non_equippable) {
            return Err(CannotEquip::Prerequisites);
        }
        let row = self
            .objects
            .get(&item)
            .map_or(0, |o| o.base_item.max(0) as usize);
        let Some(table) = self.table("baseitems") else {
            return Err(CannotEquip::NotWearable);
        };
        let Some(o) = self.objects.get(&who) else {
            return Err(CannotEquip::NotWearable);
        };
        let wrong_body = match table.int(row, "droidorhuman") {
            Some(1) => o.race == DROID_RACE,
            Some(2) => o.race != DROID_RACE,
            _ => false,
        };
        let has_feats = REQUIRED_FEATS
            .iter()
            .filter_map(|c| table.int(row, c))
            .all(|f| o.feats.contains(&(f as i32)));
        if wrong_body || !has_feats {
            return Err(CannotEquip::Prerequisites);
        }
        let template = self.objects.get(&item).map(|o| o.template.clone());
        if let Some(template) = template {
            self.item_template(&template);
        }
        match self.passes_item_limits(who, item) {
            (false, _) => Err(CannotEquip::Alignment),
            (_, false) => Err(CannotEquip::Prerequisites),
            _ => Ok(()),
        }
    }

    /// Put an item on: in `slot`, or the first free slot it fits, or else the first it fits. Whatever was there goes
    /// back to the carried items. One comes off a stack. Returns the slot.
    pub fn equip(
        &mut self,
        who: ObjId,
        item: ObjId,
        slot: Option<i32>,
    ) -> Result<i32, CannotEquip> {
        self.can_wear(who, item)?;
        let mask = self.slots_of(item);
        let fits = |s: i32| mask & (1 << s) != 0;
        let Some(o) = self.objects.get(&who) else {
            return Err(CannotEquip::NotWearable);
        };
        let target = match slot {
            Some(s) if fits(s) => s,
            Some(_) => return Err(CannotEquip::WrongSlot),
            None => SLOTS
                .iter()
                .copied()
                .filter(|s| fits(*s))
                .find(|s| !o.equipped.contains_key(s))
                .or_else(|| SLOTS.iter().copied().find(|s| fits(*s)))
                .ok_or(CannotEquip::WrongSlot)?,
        };
        let right = o.equipped.get(&SLOT_RIGHT_HAND).copied();
        if target == SLOT_LEFT_HAND && right.is_some_and(|r| self.takes_both_hands(r)) {
            return Err(CannotEquip::BothHandsFull);
        }
        let worn = self.take_one_off(who, item);
        let old = self
            .objects
            .get_mut(&who)
            .and_then(|o| o.equipped.insert(target, worn));
        if let Some(old) = old {
            self.carry(who, old);
        }
        if target == SLOT_RIGHT_HAND && self.takes_both_hands(worn) {
            self.unequip(who, SLOT_LEFT_HAND);
        }
        Ok(target)
    }

    fn takes_both_hands(&mut self, item: ObjId) -> bool {
        let mask = self.slots_of(item);
        mask & (1 << SLOT_RIGHT_HAND) != 0 && mask & (1 << SLOT_LEFT_HAND) == 0
    }

    /// Out of the carried items: one from a stack, or the item itself.
    fn take_one_off(&mut self, who: ObjId, item: ObjId) -> ObjId {
        let (stack, template) = self
            .objects
            .get(&item)
            .map_or((1, String::new()), |o| (o.stack, o.template.clone()));
        if stack > 1 {
            if let Some(o) = self.objects.get_mut(&item) {
                o.stack -= 1;
            }
            let one = self.spawn_item(&template);
            if let Some(o) = self.objects.get_mut(&one) {
                o.stack = 1;
            }
            return one;
        }
        if let Some(o) = self.objects.get_mut(&who) {
            o.inventory.retain(|i| *i != item);
        }
        item
    }

    /// Take off what is in a slot. Returns whether there was anything.
    pub fn unequip(&mut self, who: ObjId, slot: i32) -> bool {
        let Some(item) = self
            .objects
            .get_mut(&who)
            .and_then(|o| o.equipped.remove(&slot))
        else {
            return false;
        };
        self.carry(who, item);
        true
    }

    /// [`World::carry`], for an item new to whoever gets it: the module hears of it. An item that joins a stack goes, and
    /// `k_ptar_acquire` and its like are told of the stack.
    pub(crate) fn carry_acquired(&mut self, who: ObjId, item: ObjId) {
        let template = self.objects.get(&item).map(|o| o.template.clone());
        self.carry(who, item);
        let held = if self.objects.contains_key(&item) {
            Some(item)
        } else {
            self.carried(who)
                .into_iter()
                .find(|i| self.objects.get(i).map(|o| &o.template) == template.as_ref())
        };
        if let Some(held) = held.filter(|_| who == self.pc || self.party.contains(&who)) {
            self.items_acquired.push(held);
        }
    }

    /// Give a creature an item: credits become the party's, and an item joins a stack of its own kind if there is
    /// room.
    pub fn carry(&mut self, who: ObjId, item: ObjId) {
        let Some(o) = self.objects.get(&item) else {
            return;
        };
        let (row, stack, template) = (
            o.base_item.max(0) as usize,
            o.stack.max(1),
            o.template.clone(),
        );
        let table = self.table("baseitems");
        let cell = |c: &str| table.as_ref().and_then(|t| t.get(row, c));
        if cell("label") == Some("Credits") {
            self.gold += stack;
            self.objects.remove(&item);
            return;
        }
        let most = table
            .as_ref()
            .and_then(|t| t.int(row, "stacking"))
            .unwrap_or(1) as i32;
        let same = self.carried(who).into_iter().find(|i| {
            *i != item
                && self.objects.get(i).is_some_and(|c| {
                    c.kind == Kind::Item
                        && c.template.eq_ignore_ascii_case(&template)
                        && c.stack + stack <= most
                })
        });
        match same {
            Some(s) => {
                if let Some(c) = self.objects.get_mut(&s) {
                    c.stack += stack;
                }
                self.objects.remove(&item);
            }
            None => {
                if let Some(o) = self.objects.get_mut(&who) {
                    if !o.inventory.contains(&item) {
                        o.inventory.push(item);
                    }
                }
            }
        }
    }
}
