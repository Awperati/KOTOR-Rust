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

/// `DAMAGE_TYPE_UNIVERSAL`.
pub const UNIVERSAL: i32 = 8;
pub const DURATION_INSTANT: i32 = 0;
pub const DURATION_TEMPORARY: i32 = 1;
pub const DURATION_PERMANENT: i32 = 2;

/// The `IMMUNITY_TYPE_*` (from the install's `nwscript.nss`) that guards against an effect, if any.
/// **Assumed:** droid stun counts as stun, horror as fear, choke as paralysis, a push as knockdown.
fn immunity_type(effect: &str) -> Option<i32> {
    Some(match effect {
        "EffectPoison" => 2,
        "EffectFrightened" | "EffectHorrified" | "EffectCutSceneHorrified" => 4,
        "EffectParalyze" | "EffectChoke" | "EffectCutSceneParalyze" => 6,
        "EffectMovementSpeedDecrease" => 24,
        "EffectEntangle" => 10,
        "EffectStunned" | "EffectDroidStun" | "EffectCutSceneStunned" => 12,
        "EffectSleep" => 13,
        "EffectConfused" => 16,
        "EffectAbilityDecrease" => 19,
        "EffectAttackDecrease" => 20,
        "EffectDamageDecrease" => 21,
        "EffectDamageImmunityDecrease" => 22,
        "EffectACDecrease" => 23,
        "EffectSavingThrowDecrease" => 25,
        "EffectForceResistanceDecrease" => 26,
        "EffectSkillDecrease" => 27,
        "EffectKnockdown" | "EffectForcePushed" => 28,
        "EffectDeath" => 32,
        _ => return None,
    })
}

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
/// How far a power throws, by its `spells.2da` row: Force Push 8 m, Force Wave 10 m, another 6 m (owner, 7 Nov: "much further
/// than now", which was 3 m). **Assumed**, as are the flight's speed and the three seconds down.
pub fn push_metres(spell: i32) -> f32 {
    match spell {
        23 => 8.0,
        26 => 10.0,
        _ => 6.0,
    }
}
/// How far a power throws when this caster casts it: the power's own reach (Force Push 8 m, Force Wave 10 m), and 0.1 m more
/// for each level of the caster over the first, 2 m more at the most. A first-level caster throws the power's own reach.
/// **Assumed:** the owner gave the reach of each power, not of each level.
pub fn push_reach(spell: i32, caster_level: i32) -> f32 {
    push_metres(spell) + (0.1 * (caster_level - 1).max(0) as f32).min(PUSH_LEVEL_MOST_METRES)
}
/// The most the caster's level adds to a throw.
pub const PUSH_LEVEL_MOST_METRES: f32 = 2.0;
/// A throw takes this long and this much longer for each metre.
pub const PUSH_SLIDE_SECONDS: f32 = 0.3;
pub const PUSH_SECONDS_PER_METRE: f32 = 0.07;
pub const PUSH_DOWN_SECONDS: f32 = 3.0;
/// How high a thrown body rises for each metre of the throw, and at the most. 8 m lifts it 0.48 m. **Assumed.**
pub const PUSH_LIFT_PER_METRE: f32 = 0.06;
pub const PUSH_LIFT_MOST: f32 = 0.6;
/// In the open the body is in the air for this part of the throw, then lands and skids to a stop.
pub const PUSH_AIR_PART: f32 = 0.65;
/// Against a wall the body hits it at speed and bounces back this far (half the throw at the most), over this long.
pub const PUSH_REBOUND_METRES: f32 = 0.35;
pub const PUSH_REBOUND_SECONDS: f32 = 0.25;
/// The `getupdead` clip plays this much faster than its 3.00 s, so a body is up in 2 s.
pub const GET_UP_RATE: f32 = 1.5;
/// A body that was knocked down cannot act for this long after the knockdown ends: until it is on its feet. Measured: in
/// `getupdead` the head is at nine tenths of its standing height 2.4 s in, which is 1.6 s at [`GET_UP_RATE`].
pub const GET_UP_READY_SECONDS: f32 = 1.6;

/// A creature sliding from `from` to `to` after a push, begun at `start` (game seconds).
#[derive(Clone, Copy, Debug)]
pub struct Shove {
    pub who: ObjId,
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub start: f32,
    /// How long the flight takes.
    pub seconds: f32,
    /// It ends against a wall, a closed door or the edge of the floor, and not in open air.
    pub wall: bool,
    /// Who threw it.
    pub by: ObjId,
    /// It went over an edge and falls: how far, in metres, or `FALL_VOID` into nothing.
    pub fall: Option<f32>,
    /// How long the flight takes until it lands or hits the wall. Against a wall the bounce back follows, to `back`.
    pub hit: f32,
    /// Where a body that hit a wall comes to rest after bouncing off it. `to` where it did not.
    pub back: [f32; 3],
    /// How high the body rises in its flight.
    pub lift: f32,
    /// The wall has been hit and the landing heard.
    pub landed: bool,
}

impl Shove {
    /// Where the body is `since` seconds into the throw: in the air, then on the floor. Against a wall it goes at speed until it
    /// hits, still off the floor, and drops back from it. In the open it slows to a stop, landing part way.
    pub fn at(&self, since: f32) -> [f32; 3] {
        let lerp = |a: [f32; 3], b: [f32; 3], e: f32| [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * e);
        if self.fall.is_some() {
            // Over an edge: it slows along the way, and the height goes late as a fall speeds up.
            let t = (since / self.seconds).clamp(0.0, 1.0);
            let mut at = lerp(self.from, self.to, 1.0 - (1.0 - t).powi(2));
            at[2] = self.from[2] + (self.to[2] - self.from[2]) * t * t;
            return at;
        }
        if self.wall {
            let u = (since / self.hit).clamp(0.0, 1.0);
            if u < 1.0 {
                // Nearly as fast at the wall as at the start, and still rising off the floor when it gets there.
                let mut at = lerp(self.from, self.to, u * (1.2 - 0.2 * u));
                at[2] += self.lift * (std::f32::consts::PI * 0.8 * u).sin();
                return at;
            }
            let v = ((since - self.hit) / (self.seconds - self.hit).max(1e-3)).clamp(0.0, 1.0);
            let top = self.lift * (std::f32::consts::PI * 0.8).sin();
            let mut at = lerp(self.to, self.back, 1.0 - (1.0 - v).powi(2));
            // It drops from where it struck to the floor.
            at[2] += top * (1.0 - v * v);
            return at;
        }
        let t = (since / self.seconds).clamp(0.0, 1.0);
        let mut at = lerp(self.from, self.to, 1.0 - (1.0 - t).powi(2));
        if t < PUSH_AIR_PART {
            at[2] += self.lift * (std::f32::consts::PI * t / PUSH_AIR_PART).sin();
        }
        at
    }
}

/// How far a creature falls when there is no floor under the edge it was thrown over: it is lost.
pub const FALL_VOID: f32 = f32::INFINITY;
/// A fall does 1d6 for each this many metres, 10 dice at the most. **Assumed:** the game's files give no rule.
pub const FALL_METRES_PER_DIE: f32 = 3.0;
/// The rails over nothing of `docs/rails.tsv`: module, x, y, radius. A line that is not four such cells is a note.
pub(crate) fn rails_from_table() -> Vec<(String, f32, f32, f32)> {
    include_str!("../../../docs/rails.tsv")
        .lines()
        .filter_map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            let num = |i: usize| c.get(i)?.trim().parse::<f32>().ok();
            Some((c.first()?.trim().to_string(), num(1)?, num(2)?, num(3)?))
        })
        .collect()
}

/// A fall of less than this hurts no one: a step down.
const FALL_SAFE_METRES: f32 = 2.0;
/// A drop of more than this, between one quarter metre and the next, is the edge of a level: a step is not.
const EDGE_DROP: f32 = 1.0;
/// Seconds a fall takes for each metre, on top of the slide, and at the most.
const FALL_SECONDS_PER_METRE: f32 = 0.08;
const FALL_MOST_SECONDS: f32 = 1.6;
/// How far down the void is taken, so that the body is out of sight.
const VOID_DEPTH: f32 = 40.0;

/// A thrown creature has come down: where, and whether on a wall first (a smack, and then the floor).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thud {
    pub who: ObjId,
    pub wall: bool,
}

/// The power being cast right now (`GetSpellId`, `GetSpellTargetObject`, `GetLastSpellCaster`).
#[derive(Clone, Copy, Debug, PartialEq)]
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

    /// What is left of `amount` of damage of type `kind` (a set of `DAMAGE_TYPE_*` bits) after the target's resistance,
    /// reduction and immunity.
    ///
    /// **Assumed, not measured:** `EffectDamageResistance(type, amount)` takes `amount` off a hit of a matching type and
    /// ignores its limit; `EffectDamageReduction(amount)` takes it off any type; immunity is a percentage per type,
    /// raised and lowered by `EffectDamageImmunityIncrease` and `Decrease`, and kept between 0 and 100. Universal damage
    /// (type 8) passes resistance and immunity, as in the d20 game.
    pub fn adjust_damage(&self, target: ObjId, amount: i32, kind: i32) -> i32 {
        if kind & UNIVERSAL != 0 || amount <= 0 {
            return amount.max(0);
        }
        let Some(o) = self.objects.get(&target) else {
            return amount;
        };
        let (mut flat, mut immune) = (0, 0);
        for e in o
            .effects
            .iter()
            .filter_map(|h| self.effects.get(h.wrapping_sub(1) as usize))
        {
            let num = |i: usize| e.nums.get(i).copied().unwrap_or(0.0) as i32;
            match e.name.as_str() {
                "EffectDamageResistance" if num(0) & kind != 0 => flat += num(1),
                "EffectDamageReduction" => flat += num(0),
                "EffectDamageImmunityIncrease" if num(0) & kind != 0 => immune += num(1),
                "EffectDamageImmunityDecrease" if num(0) & kind != 0 => immune -= num(1),
                _ => {}
            }
        }
        let left = (amount - flat).max(0);
        left * (100 - immune.clamp(0, 100)) / 100
    }

    /// Whether the creature has been knocked off its feet and lies there: a push or a knockdown is on it.
    pub fn is_knocked_down(&self, id: ObjId) -> bool {
        self.objects.get(&id).is_some_and(|o| {
            o.effects
                .iter()
                .filter_map(|h| self.effects.get(h.wrapping_sub(1) as usize))
                .any(|e| matches!(e.name.as_str(), "EffectForcePushed" | "EffectKnockdown"))
        })
    }

    /// Whether an active effect stops the creature acting.
    pub fn is_disabled(&self, id: ObjId) -> bool {
        // Getting up after a knockdown: not on its feet yet.
        if self.rising.get(&id).is_some_and(|t| self.time < *t) {
            return true;
        }
        self.objects.get(&id).is_some_and(|o| {
            o.effects
                .iter()
                .filter_map(|h| self.effects.get(h.wrapping_sub(1) as usize))
                .any(|e| DISABLING.contains(&e.name.as_str()))
        })
    }

    /// Whether `target` carries an `EffectImmunity` for `kind` (an `IMMUNITY_TYPE_*`).
    pub fn is_immune(&self, target: ObjId, kind: i32) -> bool {
        self.objects.get(&target).is_some_and(|o| {
            o.effects
                .iter()
                .filter_map(|h| self.effects.get(h.wrapping_sub(1) as usize))
                .any(|e| {
                    e.name == "EffectImmunity" && e.nums.first().is_some_and(|n| *n as i32 == kind)
                })
        })
    }

    /// Whether `target` is immune to the effect `handle` or any effect linked to it (`GetIsLinkImmune`).
    pub fn is_link_immune(&self, target: ObjId, handle: u32) -> bool {
        let Some(rec) = handle
            .checked_sub(1)
            .and_then(|i| self.effects.get(i as usize))
        else {
            return false;
        };
        immunity_type(&rec.name).is_some_and(|k| self.is_immune(target, k))
            || rec.links.iter().any(|l| self.is_link_immune(target, *l))
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
        if immunity_type(&rec.name).is_some_and(|k| self.is_immune(target, k)) {
            return;
        }
        let n0 = rec.nums.first().copied().unwrap_or(0.0) as i32;
        match rec.name.as_str() {
            "EffectLinkEffects" => {}
            "EffectDamage" => {
                // `EffectDamage(amount, type, power)`: the type is a set of `DAMAGE_TYPE_*` bits.
                let kind = rec.nums.get(1).copied().unwrap_or(UNIVERSAL as f32) as i32;
                let amount = self.adjust_damage(target, n0, kind);
                let amount = self.absorb_with_shields(target, amount, kind);
                self.hurt(creator, target, amount);
            }
            "EffectDeath" => {
                let hp = self.objects.get(&target).map_or(0, |o| o.hp);
                self.hurt(creator, target, hp.max(1));
            }
            "EffectHeal" => {
                if let Some(o) = self.objects.get_mut(&target) {
                    if !o.dead {
                        let n0 = if self.heal_scale < 1.0 {
                            ((n0 as f32 * self.heal_scale).round() as i32).max(1)
                        } else {
                            n0
                        };
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
                if rec.name == "EffectForceShield" {
                    self.arm_shield(handle);
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

    /// A force shield goes on: its row in `forceshields.2da` gives what it takes in (`damageflags`, the `DAMAGE_TYPE_*` bits)
    /// and how much (`amount`). The record keeps `[row, what is left, bits]`.
    ///
    /// **Assumed:** `amount` is the shield's whole store: it takes that much damage of those types, from any number of hits,
    /// and is gone when it is spent or its time (200 s from the item's script) runs out. The table's `resistance` (300 on
    /// every row) and `vulnerflags` (ion damage) are not used.
    fn arm_shield(&mut self, handle: u32) {
        let Some(row) = self
            .effects
            .get(handle.wrapping_sub(1) as usize)
            .and_then(|e| e.nums.first())
            .map(|n| *n as usize)
        else {
            return;
        };
        let Some(t) = self.table("forceshields") else {
            return;
        };
        let amount = t.int(row, "amount").unwrap_or(0) as f32;
        let bits = t.int(row, "damageflags").unwrap_or(0) as f32;
        if let Some(e) = self.effects.get_mut(handle.wrapping_sub(1) as usize) {
            e.nums = vec![row as f32, amount, bits];
        }
    }

    /// An effect's record by its handle (the tests set a shield's store through it).
    pub fn effect_record_mut(&mut self, handle: u32) -> Option<&mut crate::world::EffectRec> {
        self.effects.get_mut(handle.wrapping_sub(1) as usize)
    }

    /// What is left of a hit of `kind` after the force shields on the target take what they can. A shield that is spent goes.
    pub fn absorb_with_shields(&mut self, target: ObjId, amount: i32, kind: i32) -> i32 {
        if amount <= 0 || kind & UNIVERSAL != 0 {
            return amount.max(0);
        }
        let held: Vec<u32> = self
            .objects
            .get(&target)
            .map(|o| o.effects.clone())
            .unwrap_or_default();
        let mut left = amount;
        for h in held {
            let Some(e) = self.effects.get_mut(h.wrapping_sub(1) as usize) else {
                continue;
            };
            if e.name != "EffectForceShield" || e.nums.len() < 3 || e.nums[2] as i32 & kind == 0 {
                continue;
            }
            let store = e.nums[1] as i32;
            let take = store.min(left);
            e.nums[1] = (store - take) as f32;
            left -= take;
            if store - take <= 0 {
                self.effect_expiry.remove(&h);
                self.effect_spell.remove(&h);
                if let Some(o) = self.objects.get_mut(&target) {
                    o.effects.retain(|x| *x != h);
                }
            }
            if left == 0 {
                break;
            }
        }
        left
    }

    /// Throw `target` back from `from` along the floor, as far as the floor, walls, doors and other creatures allow.
    fn begin_shove(&mut self, from: ObjId, target: ObjId) {
        let (Some(a), Some(b)) = (
            self.objects.get(&from).map(|o| o.position),
            self.objects.get(&target).map(|o| o.position),
        ) else {
            return;
        };
        // A body stays where it fell.
        if self.objects.get(&target).is_some_and(|o| o.dead) {
            return;
        }
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
        let level = self.objects.get(&from).map_or(1, |o| o.level);
        let metres = push_reach(self.spell.id, level);
        let mut wall = false;
        // Past a low barrier, in the air: the last place on the floor, and where the flight has got to.
        let mut air: Option<[f32; 3]> = None;
        let mut over = [b[0], b[1]];
        let mut prev = [b[0], b[1]];
        // The player keeps the invisible wall at the edge. Anyone else goes over a barrier that is only an edge of the floor:
        // no wall of the walkmesh stands there to stop a body, and no door.
        let over_edges = target != self.pc;
        for i in 1..=(metres / 0.25) as usize {
            let s = i as f32 * 0.25;
            let (x, y) = (b[0] + dx * s, b[1] + dy * s);
            let level = air.map_or(at[2], |l| l[2]);
            let found = nav.height_at(x, y, level + 0.4);
            // No floor, or a drop of more than a step, is an edge.
            if found.is_none_or(|z| z < level - EDGE_DROP) {
                // A ledge over a floor below, or an edge that `docs/rails.tsv` lists as a rail over nothing.
                let drop_below = found.is_some() || nav.locate(x, y, level - 1.0).is_some();
                let open = over_edges
                    && (drop_below || self.is_rail_over_nothing(x, y))
                    && !nav.crosses_block([prev[0], prev[1], level], [x, y, level])
                    && !nav.sight_blocked([prev[0], prev[1], level + 0.6], [x, y, level + 0.6]);
                if open {
                    air.get_or_insert(at);
                    over = [x, y];
                    prev = [x, y];
                    continue;
                }
                if found.is_none() {
                    wall = true;
                    break;
                }
            }
            let Some(z) = found else {
                wall = true;
                break;
            };
            let next = [x, y, z];
            let blocked = nav.crosses_block(at, next);
            let hit = blocked
                || blockers
                    .iter()
                    .any(|c| (c[0] - x).hypot(c[1] - y) < 2.0 * crate::follow::CREATURE_RADIUS);
            if hit {
                wall = blocked;
                break;
            }
            // Floor again across the gap: it lands there, with no fall.
            air = None;
            at = next;
            prev = [x, y];
        }
        // Over the edge and still in the air at the end of the push: it falls to the floor below, or into nothing.
        let mut fall = None;
        if let Some(last) = air {
            match nav.locate(over[0], over[1], last[2] - 0.3) {
                Some((_, z)) => {
                    fall = Some(last[2] - z);
                    at = [over[0], over[1], z];
                }
                None => {
                    fall = Some(FALL_VOID);
                    at = [over[0], over[1], last[2] - VOID_DEPTH];
                }
            }
        }
        self.shoves.retain(|s| s.who != target);
        let length = dist2d(b, at);
        let drop = fall.map_or(0.0, |f| {
            (if f.is_finite() { f } else { VOID_DEPTH }) * FALL_SECONDS_PER_METRE
        });
        let hit =
            PUSH_SLIDE_SECONDS + PUSH_SECONDS_PER_METRE * length + drop.min(FALL_MOST_SECONDS);
        // Off a wall it bounces back along its own way, onto the floor it crossed.
        let mut back = at;
        let mut seconds = hit;
        if wall && fall.is_none() {
            let r = PUSH_REBOUND_METRES.min(length * 0.5);
            let (x, y) = (at[0] - dx * r, at[1] - dy * r);
            if let Some(z) = nav.height_at(x, y, at[2] + 0.4) {
                back = [x, y, z];
            }
            seconds += PUSH_REBOUND_SECONDS;
        }
        self.shoves.push(Shove {
            who: target,
            from: b,
            to: at,
            start: self.time,
            seconds,
            wall,
            by: from,
            fall,
            hit,
            back,
            lift: (PUSH_LIFT_PER_METRE * length).min(PUSH_LIFT_MOST),
            landed: false,
        });
        if let Some(o) = self.objects.get_mut(&target) {
            o.facing = (-dy).atan2(-dx).to_degrees();
        }
    }

    /// Slide thrown creatures along their throw.
    pub(crate) fn update_shoves(&mut self) {
        let now = self.time;
        let shoves = std::mem::take(&mut self.shoves);
        for mut s in shoves {
            let since = now - s.start;
            let at = s.at(since);
            if let Some(o) = self.objects.get_mut(&s.who) {
                o.position = at;
            }
            if s.who == self.pc {
                self.pc_mover.pos = at;
            }
            // The landing is heard when the body hits the wall, or comes to rest.
            if !s.landed && (since >= s.seconds || (s.wall && since >= s.hit)) {
                s.landed = true;
                self.thuds.push(Thud {
                    who: s.who,
                    wall: s.wall,
                });
            }
            if since < s.seconds {
                self.shoves.push(s);
            } else if let Some(height) = s.fall {
                self.land_after_fall(s.by, s.who, height);
            }
        }
    }

    /// Whether `docs/rails.tsv` lists this place in this module as a rail over nothing, where a pushed creature goes over and is lost.
    fn is_rail_over_nothing(&self, x: f32, y: f32) -> bool {
        self.rails
            .iter()
            .any(|(m, cx, cy, r)| *m == self.module && (cx - x).hypot(cy - y) <= *r)
    }

    /// An edge of the floor in a module that is only a low barrier over nothing, as the rows of `docs/rails.tsv` say.
    pub fn add_rail(&mut self, module: &str, x: f32, y: f32, radius: f32) {
        self.rails.push((module.to_string(), x, y, radius));
    }

    /// A fall of `height` metres, as a test has it.
    pub fn fall_for_test(&mut self, by: ObjId, who: ObjId, height: f32) {
        self.land_after_fall(by, who, height);
    }

    /// A creature thrown over an edge has come down. Into nothing, it is lost. Onto a floor below, it takes 1d6 for each
    /// `FALL_METRES_PER_DIE` of the fall, 10 dice at the most, and gets up where it landed.
    fn land_after_fall(&mut self, by: ObjId, who: ObjId, height: f32) {
        if height < FALL_SAFE_METRES {
            return;
        }
        let hp = self.objects.get(&who).map_or(0, |o| o.hp);
        let damage = if height.is_finite() {
            let dice = ((height / FALL_METRES_PER_DIE).ceil() as i32).clamp(1, 10);
            (0..dice).map(|_| self.roll(6)).sum()
        } else {
            hp.max(1)
        };
        self.hurt(by, who, damage);
        self.events.push(format!(
            "{} {}",
            self.describe(who),
            if height.is_finite() {
                "falls"
            } else {
                "falls out of sight"
            }
        ));
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
        self.rising.retain(|_, t| *t > now);
        for h in gone {
            self.effect_expiry.remove(&h);
            let knock = self
                .effects
                .get(h.wrapping_sub(1) as usize)
                .is_some_and(|e| {
                    matches!(e.name.as_str(), "EffectForcePushed" | "EffectKnockdown")
                });
            let mut rise = Vec::new();
            for o in self.objects.values_mut() {
                if knock && !o.dead && o.effects.contains(&h) {
                    rise.push(o.id);
                }
                o.effects.retain(|e| *e != h);
            }
            // A knockdown is over: the body gets up, and cannot act until it is on its feet.
            for id in rise {
                if !self.is_knocked_down(id) {
                    self.rising.insert(id, now + GET_UP_READY_SECONDS);
                }
            }
        }
    }

    /// Whether the creature is getting up after a knockdown, and cannot act until it stands.
    pub fn is_getting_up(&self, id: ObjId) -> bool {
        self.rising.get(&id).is_some_and(|t| self.time < *t)
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
        // The player's own on themselves: under a stronger power of its line the power's script puts nothing on, so nothing is
        // paid. Measured for powers on the caster only: what a power on an enemy does under another is not looked at.
        if !cheat && caster == self.pc && target == caster {
            if let Some(on) = self.stronger_on(target, spell) {
                return Err(format!("power {on}, a stronger one, is on"));
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
        self.casts.push((caster, spell));
        self.events.push(format!(
            "{} casts power {spell} on {}",
            self.describe(caster),
            self.describe(target)
        ));
        self.last_power.insert(caster, (spell, script.is_some()));
        // A thrown grenade is in the air first: its power happens where it lands (`flight.rs`).
        if script.is_some() && self.is_throw(spell) {
            self.start_flight(caster, spell, target);
        } else {
            self.burst(caster, spell, target);
        }
        Ok(())
    }

    /// Whether this power is on this creature now.
    pub fn power_active(&self, who: ObjId, spell: i32) -> bool {
        self.objects.get(&who).is_some_and(|o| {
            o.effects
                .iter()
                .any(|h| self.effect_spell.get(h) == Some(&spell))
        })
    }

    /// This power stops, on this creature and the party it may have reached, with nothing paid back.
    pub fn end_power(&mut self, who: ObjId, spell: i32) {
        let on: Vec<ObjId> = std::iter::once(who)
            .chain(self.party.iter().copied())
            .collect();
        for id in on {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let spells = &self.effect_spell;
            let mine = |h: &u32| spells.get(h) == Some(&spell);
            for h in o.effects.iter().filter(|h| mine(h)) {
                self.effect_expiry.remove(h);
            }
            o.effects.retain(|h| !mine(h));
        }
    }

    /// The power's script runs, and a hostile one makes its target an attacker's business.
    pub(crate) fn burst(&mut self, caster: ObjId, spell: i32, target: ObjId) {
        let script = self
            .table("spells")
            .and_then(|t| t.get(spell as usize, "impactscript").map(str::to_string))
            .filter(|s| !s.is_empty());
        let saved = self.spell;
        self.spell = SpellContext {
            id: spell,
            caster,
            target,
        };
        if let Some(s) = script {
            let _ = self.run_script(&s, caster);
        }
        self.spell = saved;
        // A hostile cast makes the target an attacker's business: the target is told it was attacked.
        if self.reputation(target, caster) <= 10 && target != caster {
            self.combat.last_attacker.insert(target, caster);
        }
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
