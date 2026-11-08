//! What worn items do beyond adding numbers (`docs/GEAR_CHECK.md`): resist or ignore a type of damage, be immune to an effect,
//! turn aside blaster bolts, regenerate, give feats and force resistance, and do something to whoever they hit.
//!
//! **Read from the install:** which property it is, its subtype and its amount (`itempropdef`, `iprp_damagetype`,
//! `iprp_resistcost`, `iprp_immuncost`, `iprp_immunity`, `iprp_onhit`). **Assumed, not compared with the game:**
//! - A damage type's bit is `1 << row` of `iprp_damagetype`, as `nwscript.nss` numbers `DAMAGE_TYPE_*`.
//! - Each point of bolt deflection turns aside 5% of ranged blows, at most 75%.
//! - Regeneration and Force point regeneration give their amount every 3 seconds.
//! - What a blow does on a hit (sleep, stun, paralysis, confusion, fear) is saved against with DC 14 (Fortitude for stun and
//!   paralysis, Will for the rest) and lasts 6 seconds. Slow, ability drain and the rest are left out.
//! - A property whose amount the tables do not give counts as 1.
use crate::gear::ItemProperty;
use crate::rules::Save;
use crate::world::{EffectRec, ObjId, World};
use std::collections::BTreeSet;

const BONUS_FEATS: i32 = 9;
const DAMAGE_IMMUNITY: i32 = 14;
const DAMAGE_RESIST: i32 = 17;
const IMMUNITY: i32 = 24;
const MAGIC_RESIST: i32 = 25;
const ON_HIT: i32 = 32;
const REGENERATION: i32 = 35;
const FORCE_REGENERATION: i32 = 54;
const BOLT_DEFLECT: i32 = 55;
/// Seconds between the points that regenerating items give.
const REGEN_SECONDS: f32 = 3.0;
const DEFLECT_PERCENT_EACH: i32 = 5;
const DEFLECT_MOST: i32 = 75;
const ON_HIT_DC: i32 = 14;
const ON_HIT_SECONDS: f32 = 6.0;

/// The `IMMUNITY_TYPE_*` of `nwscript.nss` for a row of `iprp_immunity`.
fn immunity_of_row(row: i32) -> Option<i32> {
    Some(match row {
        1 => 19,
        2 => 1,
        3 => 2,
        4 => 3,
        5 => 4,
        6 => 28,
        7 => 6,
        8 => 31,
        9 => 32,
        _ => return None,
    })
}

/// A property's amount: what the cost table says, and 1 if it says nothing.
fn amount(p: &ItemProperty) -> i32 {
    if p.flat != 0 {
        p.flat
    } else {
        1
    }
}

impl World {
    fn worn_props(&self, id: ObjId, kind: i32) -> Vec<ItemProperty> {
        self.gear(id)
            .into_iter()
            .map(|(_, p)| p)
            .filter(|p| p.kind == kind)
            .collect()
    }

    /// Flat damage the worn items take off a hit of this damage type (`DAMAGE_TYPE_*` bits).
    pub fn gear_damage_resist(&self, id: ObjId, kind: i32) -> i32 {
        self.worn_props(id, DAMAGE_RESIST)
            .iter()
            .filter(|p| (0..31).contains(&p.subtype) && kind & (1 << p.subtype) != 0)
            .map(amount)
            .sum()
    }

    /// Percent of a hit of this type that the worn items ignore.
    pub fn gear_damage_immunity(&self, id: ObjId, kind: i32) -> i32 {
        self.worn_props(id, DAMAGE_IMMUNITY)
            .iter()
            .filter(|p| (0..31).contains(&p.subtype) && kind & (1 << p.subtype) != 0)
            .map(amount)
            .sum()
    }

    /// Whether a worn item makes someone immune to an `IMMUNITY_TYPE_*`.
    pub fn gear_immune_to(&self, id: ObjId, kind: i32) -> bool {
        self.worn_props(id, IMMUNITY)
            .iter()
            .any(|p| immunity_of_row(p.subtype) == Some(kind))
    }

    /// The percent of ranged blows the worn items turn aside.
    pub fn gear_deflect_percent(&self, id: ObjId) -> i32 {
        (self
            .worn_props(id, BOLT_DEFLECT)
            .iter()
            .map(amount)
            .sum::<i32>()
            * DEFLECT_PERCENT_EACH)
            .min(DEFLECT_MOST)
    }

    /// What the worn items add to force resistance.
    pub fn gear_magic_resist(&self, id: ObjId) -> i32 {
        self.worn_props(id, MAGIC_RESIST).iter().map(amount).sum()
    }

    /// The feats the worn items give.
    pub(crate) fn gear_bonus_feats(&self, id: ObjId) -> BTreeSet<i32> {
        self.worn_props(id, BONUS_FEATS)
            .iter()
            .map(|p| p.subtype)
            .collect()
    }

    /// Hit points and Force points the worn items give back every `REGEN_SECONDS`.
    pub fn gear_regeneration(&self, id: ObjId) -> (i32, i32) {
        (
            self.worn_props(id, REGENERATION).iter().map(amount).sum(),
            self.worn_props(id, FORCE_REGENERATION)
                .iter()
                .map(amount)
                .sum(),
        )
    }

    /// Once a frame: the feats worn items give, and what they regenerate, for the party.
    pub(crate) fn update_gear(&mut self, dt: f32) {
        let party: Vec<ObjId> = std::iter::once(self.pc)
            .chain(self.party.iter().copied())
            .collect();
        for &id in &party {
            let want = self.gear_bonus_feats(id);
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let gone: Vec<i32> = o.gear_feats.difference(&want).copied().collect();
            for f in gone {
                o.gear_feats.remove(&f);
                o.feats.remove(&f);
            }
            for f in want {
                if o.feats.insert(f) {
                    o.gear_feats.insert(f);
                }
            }
        }
        self.gear_regen += dt;
        if self.gear_regen < REGEN_SECONDS {
            return;
        }
        self.gear_regen -= REGEN_SECONDS;
        for id in party {
            let (hp, fp) = self.gear_regeneration(id);
            if let Some(o) = self.objects.get_mut(&id) {
                if !o.dead && !o.down {
                    if hp > 0 {
                        o.hp = (o.hp + hp).min(o.max_hp.max(o.hp));
                    }
                    if fp > 0 {
                        o.force = (o.force + fp).min(o.max_force.max(o.force));
                    }
                }
            }
        }
    }

    /// What the weapon in `slot` does to someone it hits: each property that puts an effect on the target.
    pub fn gear_on_hit(&mut self, attacker: ObjId, target: ObjId, slot: i32) {
        let rows: Vec<i32> = self
            .attacking(attacker, slot)
            .into_iter()
            .filter(|p| p.kind == ON_HIT)
            .map(|p| p.subtype)
            .collect();
        for row in rows {
            let (effect, save) = match row {
                0 => ("EffectSleep", Save::Will),
                1 => ("EffectStunned", Save::Fortitude),
                2 => ("EffectParalyze", Save::Fortitude),
                3 => ("EffectConfused", Save::Will),
                4 => ("EffectFrightened", Save::Will),
                _ => continue,
            };
            if self.saving_throw(target, save, ON_HIT_DC).passed {
                continue;
            }
            self.effects.push(EffectRec::new(effect));
            let handle = self.effects.len() as u32;
            self.apply_effect(
                attacker,
                target,
                handle,
                crate::force::DURATION_TEMPORARY,
                ON_HIT_SECONDS,
            );
        }
    }
}
