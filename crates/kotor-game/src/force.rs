//! Force powers and effects: casting a power runs its impact script from `spells.2da`; the script builds effects
//! (`EffectDamage`, `EffectACIncrease`, ...) and applies them with a duration type and length.
//!
//! **Read from the install:** each power's Force point cost (`forcepoints`), range letter (`range`), and impact
//! script (`impactscript`). What a power does is whatever its script does with the routines below.
//!
//! **Modelled:** instant effects (damage, healing, death, Force point change) apply at once; temporary effects last
//! the seconds the script gave and then drop off; permanent ones stay. While active, `EffectACIncrease/Decrease`,
//! `EffectAttackIncrease/Decrease`, `EffectAbilityIncrease/Decrease`, `EffectSavingThrowIncrease/Decrease` and
//! `EffectSkillIncrease/Decrease` change the numbers the rules use, and the disabling effects (stun, paralysis, sleep,
//! knockdown, ...) stop a creature attacking. **Not modelled:** every other effect is kept (so scripts can find it)
//! but changes nothing: damage types, resistances and immunities, regeneration, haste and speed, invisibility,
//! shields, area effects, visual effects.
//!
//! **Assumed, not measured:** ranges are touch 2.25 m, short 10 m, medium 15 m, long 28 m (`ranges.2da`
//! `SpellRng*`), `P` is the caster itself; a spell's save DC is 10 + half the caster level + the Wisdom modifier;
//! a power can only be cast by a creature that knows it (unless forced) and has the Force points.

use crate::combat::ability_mod;
use crate::follow::dist2d;
use crate::world::{EffectRec, ObjId, World};
use kotor_script::OBJECT_INVALID;

pub const DURATION_INSTANT: i32 = 0;
pub const DURATION_TEMPORARY: i32 = 1;
pub const DURATION_PERMANENT: i32 = 2;

/// Effects that stop a creature acting while they last.
const DISABLING: &[&str] = &[
    "EffectStunned",
    "EffectParalyze",
    "EffectSleep",
    "EffectKnockdown",
    "EffectDroidStun",
    "EffectFrightened",
    "EffectHorrified",
    "EffectChoke",
    "EffectConfused",
    "EffectEntangle",
    "EffectForcePushed",
    "EffectCutSceneParalyze",
    "EffectCutSceneStunned",
    "EffectCutSceneHorrified",
    "EffectWhirlWind",
];

/// How far Force Push throws someone, and how long the throw and the fall last.
///
/// **Assumed, not measured:** the script gives only 0.1 s, so the engine must size these itself.
pub const PUSH_METRES: f32 = 3.0;
pub const PUSH_SLIDE_SECONDS: f32 = 0.5;
pub const PUSH_DOWN_SECONDS: f32 = 3.0;

/// A creature sliding from `from` to `to` after a push, begun at `start` (game seconds).
#[derive(Clone, Copy, Debug)]
pub struct Shove {
    pub who: ObjId,
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub start: f32,
}

/// The power being cast right now (`GetSpellId`, `GetSpellTargetObject`, `GetLastSpellCaster`).
#[derive(Clone, Copy, Debug)]
pub struct SpellContext {
    pub id: i32,
    pub caster: ObjId,
    pub target: ObjId,
}

impl Default for SpellContext {
    fn default() -> Self {
        SpellContext {
            id: -1,
            caster: OBJECT_INVALID,
            target: OBJECT_INVALID,
        }
    }
}

impl World {
    /// Sum of the first number (or, with `key`, the second number of effects whose first number is `key`) over the
    /// creature's active effects called `name`.
    pub fn effect_sum(&self, id: ObjId, name: &str, key: Option<f32>) -> i32 {
        let Some(o) = self.objects.get(&id) else {
            return 0;
        };
        o.effects
            .iter()
            .filter_map(|h| self.effects.get(h.wrapping_sub(1) as usize))
            .filter(|e| e.name == name)
            .map(|e| match key {
                // Saving-throw effects carry the kind of save first, then the amount.
                None => e
                    .nums
                    .get(usize::from(name.starts_with("EffectSavingThrow")))
                    .copied()
                    .unwrap_or(0.0) as i32,
                Some(k) if e.nums.first() == Some(&k) => {
                    e.nums.get(1).copied().unwrap_or(0.0) as i32
                }
                Some(_) => 0,
            })
            .sum()
    }

    /// Whether an active effect stops the creature acting.
    pub fn is_disabled(&self, id: ObjId) -> bool {
        self.objects.get(&id).is_some_and(|o| {
            o.effects
                .iter()
                .filter_map(|h| self.effects.get(h.wrapping_sub(1) as usize))
                .any(|e| DISABLING.contains(&e.name.as_str()))
        })
    }

    /// Apply an effect (and the effects linked to it) to `target`. Instant kinds act at once; others are attached for
    /// `seconds` (temporary) or for good (permanent).
    pub fn apply_effect(
        &mut self,
        creator: ObjId,
        target: ObjId,
        handle: u32,
        duration: i32,
        seconds: f32,
    ) {
        let Some(rec) = handle
            .checked_sub(1)
            .and_then(|i| self.effects.get(i as usize))
            .cloned()
        else {
            return;
        };
        if !self.objects.contains_key(&target) {
            return;
        }
        for l in &rec.links {
            self.apply_effect(creator, target, *l, duration, seconds);
        }
        let n0 = rec.nums.first().copied().unwrap_or(0.0) as i32;
        match rec.name.as_str() {
            "EffectLinkEffects" => {}
            "EffectDamage" => {
                self.hurt(creator, target, n0);
            }
            "EffectDeath" => {
                let hp = self.objects.get(&target).map_or(0, |o| o.hp);
                self.hurt(creator, target, hp.max(1));
            }
            "EffectHeal" => {
                if let Some(o) = self.objects.get_mut(&target) {
                    if !o.dead {
                        o.hp = (o.hp + n0).min(o.max_hp);
                    }
                }
            }
            "EffectHealForcePoints" => {
                if let Some(o) = self.objects.get_mut(&target) {
                    o.force = (o.force + n0).min(o.max_force);
                }
            }
            "EffectDamageForcePoints" => {
                if let Some(o) = self.objects.get_mut(&target) {
                    o.force = (o.force - n0).max(0);
                }
            }
            "EffectResurrection" => {
                if let Some(o) = self.objects.get_mut(&target) {
                    o.dead = false;
                    o.down = false;
                    o.hp = o.hp.max(1);
                }
            }
            _ => {
                let mut seconds = seconds;
                if rec.name == "EffectForcePushed" {
                    self.begin_shove(creator, target);
                    seconds = seconds.max(PUSH_DOWN_SECONDS);
                }
                let expires = (duration == DURATION_TEMPORARY).then_some(self.time + seconds);
                if duration == DURATION_INSTANT {
                    return;
                }
                if self.spell.id >= 0 {
                    self.effect_spell.insert(handle, self.spell.id);
                }
                if let Some(o) = self.objects.get_mut(&target) {
                    o.effects.push(handle);
                }
                if let Some(t) = expires {
                    self.effect_expiry.insert(handle, t);
                }
            }
        }
    }

    /// Throw `target` back from `from` along the floor, as far as the floor, walls, doors and other creatures allow.
    fn begin_shove(&mut self, from: ObjId, target: ObjId) {
        let (Some(a), Some(b)) = (
            self.objects.get(&from).map(|o| o.position),
            self.objects.get(&target).map(|o| o.position),
        ) else {
            return;
        };
        let Some(nav) = self.nav.as_ref() else {
            return;
        };
        let d = dist2d(a, b);
        let (dx, dy) = if d > 1e-3 {
            ((b[0] - a[0]) / d, (b[1] - a[1]) / d)
        } else {
            (1.0, 0.0)
        };
        let blockers: Vec<[f32; 2]> = self
            .objects
            .values()
            .filter(|o| o.kind == crate::world::Kind::Creature && !o.dead && o.id != target)
            .map(|o| [o.position[0], o.position[1]])
            .collect();
        let mut at = b;
        for i in 1..=(PUSH_METRES / 0.25) as usize {
            let s = i as f32 * 0.25;
            let (x, y) = (b[0] + dx * s, b[1] + dy * s);
            let Some(z) = nav.height_at(x, y, at[2] + 0.4) else {
                break;
            };
            let next = [x, y, z];
            let hit = nav.crosses_block(at, next)
                || blockers
                    .iter()
                    .any(|c| (c[0] - x).hypot(c[1] - y) < 2.0 * crate::follow::CREATURE_RADIUS);
            if hit {
                break;
            }
            at = next;
        }
        self.shoves.retain(|s| s.who != target);
        self.shoves.push(Shove {
            who: target,
            from: b,
            to: at,
            start: self.time,
        });
        if let Some(o) = self.objects.get_mut(&target) {
            o.facing = (-dy).atan2(-dx).to_degrees();
        }
    }

    /// Slide thrown creatures along their throw.
    pub(crate) fn update_shoves(&mut self) {
        let now = self.time;
        let shoves = std::mem::take(&mut self.shoves);
        for s in shoves {
            let t = ((now - s.start) / PUSH_SLIDE_SECONDS).clamp(0.0, 1.0);
            // Fast at first, slowing to a stop.
            let e = 1.0 - (1.0 - t).powi(2);
            let at = [0, 1, 2].map(|i| s.from[i] + (s.to[i] - s.from[i]) * e);
            if let Some(o) = self.objects.get_mut(&s.who) {
                o.position = at;
            }
            if s.who == self.pc {
                self.pc_mover.pos = at;
            }
            if t < 1.0 {
                self.shoves.push(s);
            }
        }
    }

    /// Drop temporary effects whose time has run out.
    pub(crate) fn expire_effects(&mut self) {
        let now = self.time;
        let gone: Vec<u32> = self
            .effect_expiry
            .iter()
            .filter(|(_, t)| **t <= now)
            .map(|(h, _)| *h)
            .collect();
        for h in gone {
            self.effect_expiry.remove(&h);
            for o in self.objects.values_mut() {
                o.effects.retain(|e| *e != h);
            }
        }
    }

    /// Force point cost and reach of a power, from `spells.2da` and `ranges.2da`.
    pub fn spell_cost_and_range(&mut self, spell: i32) -> Option<(i32, f32)> {
        let t = self.table("spells")?;
        let row = spell.max(0) as usize;
        if spell < 0 || row >= t.row_count() {
            return None;
        }
        let cost = t.int(row, "forcepoints").unwrap_or(0) as i32;
        let range = match t.get(row, "range").unwrap_or("P") {
            "T" => 2.25,
            "S" => 10.0,
            "M" => 15.0,
            "L" => 28.0,
            _ => 0.0,
        };
        Some((cost, range))
    }

    /// Cast a power: check it is known, affordable and in range, pay for it, and run its impact script with the
    /// spell context set. `cheat` skips those checks (scripts that give a power for free).
    pub fn cast_spell(
        &mut self,
        caster: ObjId,
        spell: i32,
        target: ObjId,
        cheat: bool,
    ) -> Result<(), String> {
        let (_, range) = self
            .spell_cost_and_range(spell)
            .ok_or_else(|| format!("no power {spell}"))?;
        let cost = self.power_cost(caster, spell).unwrap_or(0);
        let Some(c) = self.objects.get(&caster) else {
            return Err("no caster".into());
        };
        if c.dead || c.down {
            return Err("the caster cannot act".into());
        }
        let target = if range == 0.0 { caster } else { target };
        let Some(t) = self.objects.get(&target) else {
            return Err("no target".into());
        };
        if !cheat {
            if !c.spells.contains(&spell) {
                return Err(format!("power {spell} is not known"));
            }
            if c.force < cost {
                return Err(format!("{} Force points, {cost} needed", c.force));
            }
            if range > 0.0 && dist2d(c.position, t.position) > range {
                return Err("out of range".into());
            }
        }
        let script = self
            .table("spells")
            .and_then(|t| t.get(spell as usize, "impactscript").map(str::to_string))
            .filter(|s| !s.is_empty());
        if !cheat {
            if let Some(c) = self.objects.get_mut(&caster) {
                c.force -= cost;
            }
        }
        self.combat.casters.insert(caster);
        self.casts.push(caster);
        self.events.push(format!(
            "{} casts power {spell} on {}",
            self.describe(caster),
            self.describe(target)
        ));
        let saved = self.spell;
        self.spell = SpellContext {
            id: spell,
            caster,
            target,
        };
        let ran = script.is_some();
        if let Some(s) = script {
            let _ = self.run_script(&s, caster);
        }
        self.last_power.insert(caster, (spell, ran));
        self.spell = saved;
        // A hostile cast makes the target an attacker's business: the target is told it was attacked.
        if self.reputation(target, caster) <= 10 && target != caster {
            self.combat.last_attacker.insert(target, caster);
        }
        Ok(())
    }

    /// DC of the powers a caster casts.
    pub fn spell_save_dc(&self, caster: ObjId) -> i32 {
        self.objects.get(&caster).map_or(10, |o| {
            10 + o.level / 2 + ability_mod(self.ability_score(caster, 4))
        })
    }
}

impl EffectRec {
    pub fn new(name: &str) -> EffectRec {
        EffectRec {
            name: name.to_string(),
            args: Vec::new(),
            nums: Vec::new(),
            links: Vec::new(),
        }
    }
}
