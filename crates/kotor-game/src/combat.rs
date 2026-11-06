//! Combat: perception, heartbeats and 3-second rounds of d20 attack rolls, driven by the game's own scripts.
//!
//! The rules are the d20 system the game's data is built for. **What is checked against the install:** weapon dice
//! and critical values come from `baseitems.2da` (`numdice`, `dietoroll`, `critthreat`, `crithitmult`), base attack
//! bonus from the class's attack table (`classes.2da` `attackbonustable`), armor class from the equipped items'
//! `baseitems.2da` `baseac`, ability scores, natural AC and hit points from the creature's UTC, perception range from
//! `ranges.2da`, and who is an enemy from `repute.2da`.
//!
//! **Assumed, not measured from the game** (each is one constant or one line, listed in `docs/COMBAT.md`):
//! - a round is 3 s and a creature makes one attack per round, resolved at the end of the round, attackers in id order;
//! - attack roll = d20 + base attack bonus + Strength modifier (melee) or Dexterity modifier (ranged) against
//!   defense 10 + armor AC + natural AC + Dexterity modifier; a natural 1 misses and a natural 20 hits;
//! - a roll inside the weapon's threat range (`critthreat`, 1 = natural 20 only, 2 = 19-20, ...) that also hits
//!   threatens a critical, confirmed by a second attack roll; a confirmed critical multiplies damage by `crithitmult`;
//! - damage = weapon dice + Strength modifier (melee only), at least 1 on a hit; unarmed is 1d3;
//! - melee reach is 2.0 m and a ranged weapon fires from 12 m;
//! - a creature dies at 0 hit points;
//! - a heartbeat is every 6 s; a creature notices an enemy (`repute.2da` value 10 or less) that comes within its
//!   perception range, along a line the floor runs under the whole way, so walls and closed doors hide things.

use crate::follow::{dist2d, slide_around};
use crate::world::{Kind, ObjId, World};
use kotor_script::OBJECT_INVALID;
use kotor_walkmesh::Mover;
use std::collections::{BTreeMap, BTreeSet};

pub const ROUND_SECONDS: f32 = 3.0;
pub const HEARTBEAT_SECONDS: f32 = 6.0;
pub const MELEE_REACH: f32 = 2.0;
pub const RANGED_REACH: f32 = 12.0;
/// How often perception is checked.
const PERCEPTION_SECONDS: f32 = 0.25;

/// The ability scores in file order.
pub const STR: usize = 0;
pub const DEX: usize = 1;

/// Ability modifier of a score: (score - 10) / 2, rounded down.
/// Seconds from the start of a scripted attack to its blow.
const SWING_SECONDS: f32 = 0.6;

pub fn ability_mod(score: i32) -> i32 {
    (score - 10).div_euclid(2)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Miss,
    Hit,
    Critical,
}

/// Decide an attack from the d20 roll, a second roll that confirms a threatened critical, the attack bonus, the
/// defense, and the weapon's threat range (`threat` 1 = natural 20 only, 2 = 19-20, ...).
pub fn resolve_attack(roll: i32, confirm: i32, bonus: i32, defense: i32, threat: i32) -> Outcome {
    let hit = |r: i32| r == 20 || (r != 1 && r + bonus >= defense);
    if !hit(roll) {
        return Outcome::Miss;
    }
    if roll > 20 - threat.max(1) && hit(confirm) {
        Outcome::Critical
    } else {
        Outcome::Hit
    }
}

/// Damage of a successful attack: the dice total plus the ability modifier, times the multiplier on a critical,
/// never less than 1.
pub fn damage(dice_total: i32, ability_mod: i32, outcome: Outcome, multiplier: i32) -> i32 {
    let base = (dice_total + ability_mod).max(1);
    match outcome {
        Outcome::Critical => base * multiplier.max(1),
        _ => base,
    }
}

/// What a weapon does, read from `baseitems.2da`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weapon {
    pub dice: i32,
    pub sides: i32,
    pub threat: i32,
    pub multiplier: i32,
    pub ranged: bool,
}

impl Weapon {
    pub const UNARMED: Weapon = Weapon {
        dice: 1,
        sides: 3,
        threat: 1,
        multiplier: 2,
        ranged: false,
    };
}

/// One resolved attack.
#[derive(Clone, Debug, PartialEq)]
pub struct AttackLog {
    pub round: u32,
    pub attacker: ObjId,
    pub target: ObjId,
    pub roll: i32,
    pub bonus: i32,
    pub defense: i32,
    pub outcome: Outcome,
    pub damage: i32,
    pub target_hp: i32,
}

/// Everything combat remembers between steps.
#[derive(Clone, Debug, Default)]
pub struct CombatState {
    /// Who used a power since the last round ended.
    pub(crate) casters: BTreeSet<ObjId>,
    /// Blows a script has decided on, still in the air: when, who, whom and how hard.
    swings: Vec<(f32, ObjId, ObjId, i32)>,
    /// Who is attacking whom (set by `ActionAttack`, cleared when the target dies or the actions are cleared).
    pub target: BTreeMap<ObjId, ObjId>,
    /// (watcher, watched) pairs that have already been noticed.
    pub perceived: BTreeSet<(ObjId, ObjId)>,
    pub last_attacker: BTreeMap<ObjId, ObjId>,
    pub last_damager: BTreeMap<ObjId, ObjId>,
    pub last_hostile: BTreeMap<ObjId, ObjId>,
    /// The result of each attacker's last attack, as the script constants (1 hit, 2 critical, 4 miss).
    pub last_result: BTreeMap<ObjId, i32>,
    pub last_killer: ObjId,
    pub last_perceived: ObjId,
    pub round: u32,
    pub log: Vec<AttackLog>,
    round_clock: f32,
    heartbeat_clock: f32,
    perception_clock: f32,
    chasers: BTreeMap<ObjId, Mover>,
    replan_in: f32,
}

impl CombatState {
    pub fn new() -> CombatState {
        CombatState {
            last_killer: OBJECT_INVALID,
            last_perceived: OBJECT_INVALID,
            ..CombatState::default()
        }
    }

    pub fn in_combat(&self, id: ObjId) -> bool {
        self.target.contains_key(&id) || self.target.values().any(|t| *t == id)
    }
}

impl World {
    /// The `multiplier` of the row of `difficultyopt.2da` the game is on.
    pub fn difficulty_multiplier(&mut self) -> f32 {
        let row = self.difficulty.max(0) as usize;
        self.table("difficultyopt")
            .and_then(|t| t.get(row, "multiplier")?.parse().ok())
            .unwrap_or(1.0)
    }

    /// A roll of a die with `sides` sides, from the world's random stream.
    pub fn roll(&mut self, sides: i32) -> i32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng % sides.max(1) as u64) as i32 + 1
    }

    /// Restart the random stream from `seed` (a zero seed is replaced, since the generator cannot start from 0).
    pub fn set_rng_seed(&mut self, seed: u64) {
        self.rng = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    }

    /// The weapon in a creature's right-hand slot (4), from `baseitems.2da`.
    pub fn weapon_of(&mut self, id: ObjId) -> Weapon {
        self.hand_weapon(id, 4).unwrap_or(Weapon::UNARMED)
    }

    /// The weapon in a slot, if what is there is one.
    pub fn hand_weapon(&mut self, id: ObjId, slot: i32) -> Option<Weapon> {
        let base = self
            .objects
            .get(&id)
            .and_then(|o| o.equipped.get(&slot))
            .and_then(|w| self.objects.get(w))
            .map(|w| w.base_item)?;
        let t = self.table("baseitems")?;
        let row = base.max(0) as usize;
        let g = |c: &str| t.int(row, c).unwrap_or(0) as i32;
        if g("numdice") == 0 || g("dietoroll") == 0 {
            return None;
        }
        Some(Weapon {
            dice: g("numdice"),
            sides: g("dietoroll"),
            threat: g("critthreat").max(1),
            multiplier: g("crithitmult").max(1),
            ranged: g("rangedweapon") > 0,
        })
    }

    /// What is added to the d20 with the weapon in a slot (the right hand's empty fist too): base attack bonus,
    /// Strength in melee or Dexterity at range, and effects.
    pub fn attack_bonus_of(&mut self, id: ObjId, slot: i32) -> i32 {
        let ranged = self.hand_weapon(id, slot).is_some_and(|w| w.ranged);
        let ability = if ranged { DEX } else { STR };
        self.base_attack_of(id)
            + ability_mod(self.ability_score(id, ability))
            + self.effect_sum(id, "EffectAttackIncrease", None)
            - self.effect_sum(id, "EffectAttackDecrease", None)
    }

    /// Least and most damage of a hit that is not a critical with the weapon in a slot. An empty right hand fights
    /// with its fist; an empty left hand does not fight. As in `resolve`, only melee adds Strength.
    pub fn damage_range_of(&mut self, id: ObjId, slot: i32) -> Option<(i32, i32)> {
        let w = self.hand_weapon(id, slot).or_else(|| {
            let fist = slot == 4 && !self.objects.get(&id)?.equipped.contains_key(&4);
            fist.then_some(Weapon::UNARMED)
        })?;
        let add = if w.ranged {
            0
        } else {
            ability_mod(self.ability_score(id, STR))
        };
        Some((
            damage(w.dice, add, Outcome::Hit, 1),
            damage(w.dice * w.sides, add, Outcome::Hit, 1),
        ))
    }

    /// Armor class: 10 plus the armor items' `baseac` plus natural AC plus Dexterity modifier.
    pub fn defense_of(&mut self, id: ObjId) -> i32 {
        let Some(o) = self.objects.get(&id) else {
            return 10;
        };
        let natural = o.natural_ac;
        let dex = self.ability_score(id, DEX);
        let items: Vec<i32> = o
            .equipped
            .values()
            .filter_map(|i| self.objects.get(i))
            .map(|i| i.base_item)
            .collect();
        let armor: i32 = match self.table("baseitems") {
            Some(t) => items
                .iter()
                .map(|b| t.int((*b).max(0) as usize, "baseac").unwrap_or(0) as i32)
                .sum(),
            None => 0,
        };
        10 + armor + natural + ability_mod(dex) + self.effect_sum(id, "EffectACIncrease", None)
            - self.effect_sum(id, "EffectACDecrease", None)
    }

    /// Base attack bonus: the class's attack-table value at the creature's level.
    pub fn base_attack_of(&mut self, id: ObjId) -> i32 {
        let Some((class, level)) = self.objects.get(&id).map(|o| (o.class, o.level)) else {
            return 0;
        };
        let Some(classes) = self.table("classes") else {
            return 0;
        };
        let Some(name) = classes.get(class.max(0) as usize, "attackbonustable") else {
            return 0;
        };
        let name = name.to_ascii_lowercase();
        self.table(&name)
            .and_then(|t| t.int((level.max(1) - 1) as usize, "bab"))
            .unwrap_or(0) as i32
    }

    /// Start attacking `target` (what `ActionAttack` does).
    pub fn attack(&mut self, attacker: ObjId, target: ObjId) {
        if attacker != target
            && self.objects.get(&target).is_some_and(|t| !t.dead)
            && self.objects.get(&attacker).is_some_and(|a| !a.dead)
        {
            self.combat.target.insert(attacker, target);
            self.combat.last_hostile.insert(attacker, target);
        }
    }

    /// Stop attacking (what `ClearAllActions` and `CancelCombat` do).
    pub fn stop_attacking(&mut self, id: ObjId) {
        self.combat.target.remove(&id);
        self.combat.chasers.remove(&id);
    }

    fn reach_of(&mut self, id: ObjId) -> f32 {
        if self.weapon_of(id).ranged {
            RANGED_REACH
        } else {
            MELEE_REACH
        }
    }

    /// Resolve one attack and everything that follows from it: damage, scripts, death.
    fn resolve(&mut self, attacker: ObjId, target: ObjId) {
        let weapon = self.weapon_of(attacker);
        let str_mod = ability_mod(self.ability_score(attacker, STR));
        let bonus = self.attack_bonus_of(attacker, 4);
        let defense = self.defense_of(target);
        let (roll, confirm) = (self.roll(20), self.roll(20));
        let outcome = resolve_attack(roll, confirm, bonus, defense, weapon.threat);
        let mut dealt = 0;
        if outcome != Outcome::Miss {
            let dice: i32 = (0..weapon.dice).map(|_| self.roll(weapon.sides)).sum();
            let mod_ = if weapon.ranged { 0 } else { str_mod };
            dealt = damage(dice, mod_, outcome, weapon.multiplier);
            // **Assumption:** difficulty scales what the party takes, by the `multiplier` of `difficultyopt.2da`.
            if target == self.pc || self.party.contains(&target) {
                dealt = ((dealt as f32 * self.difficulty_multiplier()).round() as i32).max(1);
            }
        }
        let fell = self.hurt(attacker, target, dealt);
        let hp_after = self.objects.get(&target).map_or(0, |t| t.hp);
        self.combat.log.push(AttackLog {
            round: self.combat.round,
            attacker,
            target,
            roll,
            bonus,
            defense,
            outcome,
            damage: dealt,
            target_hp: hp_after,
        });
        self.events.push(format!(
            "{} attacks {}: d20 {roll} + {bonus} vs {defense}: {outcome:?}, {dealt} damage",
            self.describe(attacker),
            self.describe(target),
        ));
        self.combat.last_attacker.insert(target, attacker);
        self.combat.last_hostile.insert(attacker, target);
        self.combat.last_result.insert(
            attacker,
            match outcome {
                Outcome::Hit => 1,
                Outcome::Critical => 2,
                Outcome::Miss => 4,
            },
        );
        if dealt > 0 {
            self.combat.last_damager.insert(target, attacker);
        }
        if !fell {
            self.run_event(target, "ScriptAttacked");
            if dealt > 0 {
                self.run_event(target, "ScriptDamaged");
            }
        }
    }

    /// A round a script has decided in advance: it looks like an attack and deals what it says, and a death it causes
    /// runs as usual. `k_pend_cut1_1` waits for such a death before its conversation goes on.
    pub(crate) fn scripted_attack(
        &mut self,
        attacker: ObjId,
        target: ObjId,
        result: i32,
        amount: i32,
    ) {
        if !self.objects.contains_key(&target) {
            return;
        }
        let outcome = match result {
            1 | 3 => Outcome::Hit,
            2 => Outcome::Critical,
            _ => Outcome::Miss,
        };
        // The blow lands partway through the swing: `k_pend_cut2_2` lifts the target's one hit point after it starts.
        // **Assumption:** clip timing is not known here, so every swing lands after the same time.
        self.combat
            .swings
            .push((self.time + SWING_SECONDS, attacker, target, amount));
        let target_hp = self.objects.get(&target).map_or(0, |t| t.hp);
        self.combat.log.push(AttackLog {
            round: self.combat.round,
            attacker,
            target,
            roll: 0,
            bonus: 0,
            defense: 0,
            outcome,
            damage: amount,
            target_hp,
        });
    }

    /// Take `amount` hit points off `target`. At 0 a party member is knocked out and anyone else dies (their
    /// death script runs and the party earns the kill's experience). Returns whether the target fell.
    pub fn hurt(&mut self, source: ObjId, target: ObjId, amount: i32) -> bool {
        if amount <= 0 {
            return false;
        }
        let party_side = target == self.pc || self.party.contains(&target);
        let Some(t) = self.objects.get_mut(&target) else {
            return false;
        };
        if t.dead || t.down {
            return false;
        }
        t.hp -= amount;
        if t.min_one_hp {
            t.hp = t.hp.max(1);
        }
        if t.hp > 0 {
            return false;
        }
        t.hp = 0;
        if party_side {
            t.down = true;
        } else {
            t.dead = true;
        }
        self.combat
            .target
            .retain(|a, t| *a != target && *t != target);
        self.combat.chasers.remove(&target);
        if party_side {
            self.events
                .push(format!("{} is knocked out", self.describe(target)));
        } else {
            self.combat.last_killer = source;
            self.events.push(format!("{} dies", self.describe(target)));
            self.leave_loot(target);
            self.run_event(target, "ScriptDeath");
            if source == self.pc || self.party.contains(&source) {
                self.award_kill(target);
            }
        }
        true
    }

    /// Run one of an object's event scripts, if it has one.
    pub(crate) fn run_event(&mut self, id: ObjId, field: &str) {
        let script = self
            .objects
            .get(&id)
            .and_then(|o| o.scripts.get(field).cloned());
        if let Some(s) = script {
            let _ = self.run_script(&s, id);
        }
    }

    /// Advance combat by `dt` seconds: heartbeats, perception, movement towards targets, and rounds.
    pub(crate) fn update_combat(&mut self, dt: f32) {
        let now = self.time;
        let (due, later) = std::mem::take(&mut self.combat.swings)
            .into_iter()
            .partition::<Vec<_>, _>(|s| s.0 <= now);
        self.combat.swings = later;
        for (_, attacker, target, amount) in due {
            self.hurt(attacker, target, amount);
        }
        self.combat.heartbeat_clock += dt;
        while self.combat.heartbeat_clock >= HEARTBEAT_SECONDS {
            self.combat.heartbeat_clock -= HEARTBEAT_SECONDS;
            let ids: Vec<ObjId> = self
                .objects
                .values()
                .filter(|o| o.kind == Kind::Creature && !o.dead && o.id != self.pc)
                .map(|o| o.id)
                .collect();
            for id in ids {
                if self.objects.get(&id).is_some_and(|o| !o.dead) {
                    self.run_event(id, "ScriptHeartbeat");
                }
            }
        }
        self.combat.perception_clock += dt;
        if self.combat.perception_clock >= PERCEPTION_SECONDS {
            self.combat.perception_clock = 0.0;
            self.update_perception();
        }
        self.chase(dt);
        self.combat.round_clock += dt;
        while self.combat.round_clock >= ROUND_SECONDS {
            self.combat.round_clock -= ROUND_SECONDS;
            self.combat.round += 1;
            self.run_round();
        }
    }

    /// Whether a line between two places stays over walkable, unblocked floor.
    ///
    /// **Assumption:** the walkmesh stands in for sight, since walls and closed doors end it. Pits and railings block
    /// this line though the game may see across them.
    pub(crate) fn clear_line(&self, a: [f32; 3], b: [f32; 3]) -> bool {
        let Some(nav) = self.nav.as_ref() else {
            return true;
        };
        let len = dist2d(a, b);
        let steps = (len / 0.75).ceil().max(1.0) as usize;
        let top = a[2].max(b[2]) + 1.5;
        (1..steps).all(|i| {
            let t = i as f32 / steps as f32;
            nav.walkable_at(a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, top)
                .is_some()
        })
    }

    fn update_perception(&mut self) {
        let who: Vec<(ObjId, [f32; 3], f32)> = self
            .objects
            .values()
            .filter(|o| o.kind == Kind::Creature && !o.dead)
            .map(|o| (o.id, o.position, o.perception))
            .collect();
        for &(a, apos, range) in &who {
            for &(b, bpos, _) in &who {
                if a == b {
                    continue;
                }
                let known = self.combat.perceived.contains(&(a, b));
                let enemy = self.reputation(a, b) <= 10;
                let near =
                    dist2d(apos, bpos) <= range && (known || enemy) && self.clear_line(apos, bpos);
                if near && !known && enemy {
                    self.combat.perceived.insert((a, b));
                    self.combat.last_perceived = b;
                    self.run_event(a, "ScriptOnNotice");
                } else if !near && known {
                    self.combat.perceived.remove(&(a, b));
                }
            }
        }
        let alive: BTreeSet<ObjId> = who.iter().map(|w| w.0).collect();
        self.combat
            .perceived
            .retain(|(a, b)| alive.contains(a) && alive.contains(b));
    }

    /// Walk attackers that are out of reach towards their targets.
    fn chase(&mut self, dt: f32) {
        self.combat.replan_in -= dt;
        let replan = self.combat.replan_in <= 0.0;
        if replan {
            self.combat.replan_in = crate::follow::REPLAN_SECONDS;
        }
        let pairs: Vec<(ObjId, ObjId)> = self.combat.target.iter().map(|(a, t)| (*a, *t)).collect();
        for (a, t) in pairs {
            let (Some(apos), Some(tpos)) = (
                self.objects.get(&a).map(|o| o.position),
                self.objects.get(&t).map(|o| o.position),
            ) else {
                self.combat.target.remove(&a);
                continue;
            };
            let reach = self.reach_of(a);
            if dist2d(apos, tpos) <= reach * 0.9 {
                self.combat.chasers.remove(&a);
                if a == self.pc {
                    self.pc_mover.stop();
                }
                continue;
            }
            if a == self.pc {
                if replan && !self.pc_mover.has_route() {
                    if let Some(r) = self.nav.as_ref().and_then(|n| n.find_path(apos, tpos)) {
                        self.pc_mover
                            .follow(crate::follow::trim_route(r, reach * 0.8));
                    }
                }
                continue;
            }
            let Some(nav) = self.nav.as_ref() else {
                continue;
            };
            let m = self
                .combat
                .chasers
                .entry(a)
                .or_insert_with(|| Mover::new(apos, 0.0));
            m.pos = apos;
            if replan || !m.has_route() {
                if let Some(r) = nav.find_path(apos, tpos) {
                    m.follow(crate::follow::trim_route(r, reach * 0.8));
                }
            }
            if m.has_route() {
                m.step(nav, &self.move_params, dt);
            }
            let mut to = m.pos;
            if to != apos {
                let blockers: Vec<[f32; 2]> = self
                    .objects
                    .values()
                    .filter(|o| o.kind == Kind::Creature && !o.dead && o.id != a && o.id != t)
                    .map(|o| [o.position[0], o.position[1]])
                    .collect();
                to = slide_around(nav, apos, to, &blockers);
                m.pos = to;
            }
            if let Some(o) = self.objects.get_mut(&a) {
                o.position = to;
            }
        }
    }

    fn run_round(&mut self) {
        let pairs: Vec<(ObjId, ObjId)> = self.combat.target.iter().map(|(a, t)| (*a, *t)).collect();
        // A power takes up the round too, and the AI picks its next move when a round ends. Without this a Jedi who casts
        // never acts again, and the duel in `end_cut04` never finishes.
        let mut involved: BTreeSet<ObjId> = std::mem::take(&mut self.combat.casters);
        for (a, t) in pairs {
            let alive = |w: &World, id: ObjId| w.objects.get(&id).is_some_and(|o| !o.dead);
            if !alive(self, a) || !alive(self, t) {
                self.combat.target.remove(&a);
                continue;
            }
            // The attacker may have been killed earlier in this round by someone else's attack.
            if self.combat.target.get(&a) != Some(&t) {
                continue;
            }
            if self.is_disabled(a) {
                continue;
            }
            let reach = self.reach_of(a);
            let (ap, tp) = (self.objects[&a].position, self.objects[&t].position);
            if dist2d(ap, tp) > reach {
                continue;
            }
            involved.insert(a);
            involved.insert(t);
            self.resolve(a, t);
        }
        for id in involved {
            if self.objects.get(&id).is_some_and(|o| !o.dead) {
                self.run_event(id, "ScriptEndRound");
            }
        }
    }
}

impl World {
    /// Perform an `Action::Attack` from an object's queue.
    pub(crate) fn perform_attack(&mut self, id: ObjId, target: ObjId) {
        self.attack(id, target);
    }
}
