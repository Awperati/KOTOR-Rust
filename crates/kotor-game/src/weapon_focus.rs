//! Weapon Focus: +1 to the attack roll with the kind of weapon the feat names.
//!
//! **Read from the install:** the feat numbers (`feat.2da` rows `WEAPON_FOCUS_BLASTER` 32, `_BLASTER_RIFLE` 33,
//! `_HEAVY_WEAPONS` 35, `_LIGHTSABER` 36, `_MELEE_WEAPONS` 37) and the item's `weaponwield` and `weaponmattype` in
//! `baseitems.2da`. **Assumed:** the weapon each feat covers (a one-handed blaster is a blaster, wield 4; a rifle,
//! carbine or bowcaster is a blaster rifle, wield 5; a repeating blaster is a heavy weapon, wield 6; a lightsaber is
//! material 1; everything else wielded in the hands is melee), and that the bonus is +1, as in the tabletop rules the
//! game follows.

use crate::world::{ObjId, World};

const FOCUS_BLASTER: i32 = 32;
const FOCUS_BLASTER_RIFLE: i32 = 33;
const FOCUS_HEAVY_WEAPONS: i32 = 35;
const FOCUS_LIGHTSABER: i32 = 36;
const FOCUS_MELEE_WEAPONS: i32 = 37;

/// The Weapon Focus feat that covers an item of this `weaponwield` and `weaponmattype`, if any.
fn focus_feat(wield: i32, material: i32) -> Option<i32> {
    match (wield, material) {
        (1..=3, 1) => Some(FOCUS_LIGHTSABER),
        (1..=3, _) => Some(FOCUS_MELEE_WEAPONS),
        (4, _) => Some(FOCUS_BLASTER),
        (5, _) => Some(FOCUS_BLASTER_RIFLE),
        (6, _) => Some(FOCUS_HEAVY_WEAPONS),
        _ => None,
    }
}

impl World {
    /// What Weapon Focus adds to the attack with the weapon in `slot`: 1 with the feat for that kind of weapon, else 0.
    pub(crate) fn weapon_focus_bonus(&mut self, id: ObjId, slot: i32) -> i32 {
        let Some(base) = self
            .objects
            .get(&id)
            .and_then(|o| o.equipped.get(&slot))
            .and_then(|w| self.objects.get(w))
            .map(|w| w.base_item.max(0) as usize)
        else {
            return 0;
        };
        let Some(t) = self.table("baseitems") else {
            return 0;
        };
        let wield = t.int(base, "weaponwield").unwrap_or(0) as i32;
        let material = t.int(base, "weaponmattype").unwrap_or(0) as i32;
        let has = focus_feat(wield, material)
            .is_some_and(|f| self.objects.get(&id).is_some_and(|o| o.feats.contains(&f)));
        has as i32
    }
}
