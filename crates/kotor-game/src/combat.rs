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
/// `STANDARD_FACTION_NEUTRAL`.
const NEUTRAL_FACTION: i32 = 5;
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
    /// When each creature and placeable beats next, on `heartbeat_clock`.
    heartbeat_due: BTreeMap<ObjId, f32>,
    module_heartbeat_clock: f32,
    /// How many heartbeat scripts have run, for a test of how they are spread.
    pub heartbeats_run: u64,
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
    /// Strength in melee or Dexterity at range, what the gear gives, and effects.
    pub fn attack_bonus_of(&mut self, id: ObjId, slot: i32) -> i32 {
        let ranged = self.hand_weapon(id, slot).is_some_and(|w| w.ranged);
        let ability = if ranged { DEX } else { STR };
        let mut score = self.ability_score(id, ability);
        if self.has_finesse(id, slot) {
            score = score.max(self.ability_score(id, DEX));
        }
        self.base_attack_of(id)
            + ability_mod(score)
            + self.superior_focus_bonus(id, slot)
            + self.gear_attack(id, slot)
            + self.weapon_focus_bonus(id, slot)
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
        let (_, dice) = self.gear_damage(id, slot);
        let add = self.flat_damage(id, slot, w.ranged);
        let (least, most) = dice.iter().fold((0, 0), |(l, m), (count, sides)| {
            (l + count, m + count * sides)
        });
        Some((
            damage(w.dice, add, Outcome::Hit, 1) + least,
            damage(w.dice * w.sides, add, Outcome::Hit, 1) + most,
        ))
    }

    /// What is added to a hit's damage whatever the dice say: the gear's flat damage, Weapon Specialization, and
    /// Strength in melee.
    pub(crate) fn flat_damage(&mut self, id: ObjId, slot: i32, ranged: bool) -> i32 {
        self.gear_damage(id, slot).0
            + self.weapon_spec_bonus(id, slot)
            + if ranged {
                self.precise_shot_damage(id)
            } else {
                ability_mod(self.ability_score(id, STR))
            }
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
        10 + armor
            + natural
            + ability_mod(dex)
            + self.gear_defense(id)
            + self.effect_sum(id, "EffectACIncrease", None)
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
            && self
                .objects
                .get(&target)
                .is_some_and(|t| !t.dead && !t.down)
            && self
                .objects
                .get(&attacker)
                .is_some_and(|a| !a.dead && !a.down)
            && !(self.on_player_side(attacker) && self.on_player_side(target))
        {
            self.combat.target.insert(attacker, target);
            self.combat.last_hostile.insert(attacker, target);
        }
    }

    /// The player or one of the party: they never turn on each other (bug 21, Trask shooting the player).
    fn on_player_side(&self, id: ObjId) -> bool {
        id == self.pc
            || self.party.contains(&id)
            || self.objects.get(&id).is_some_and(|o| o.party_member)
    }

    /// Stop attacking (what `ClearAllActions` and `CancelCombat` do).
    pub fn stop_attacking(&mut self, id: ObjId) {
        self.combat.target.remove(&id);
        self.combat.chasers.remove(&id);
    }

    /// `SurrenderToEnemies`: the fight with `id` is over on both sides, and it is no longer anyone's enemy. The duel ring's
    /// fighters end on this (`k_ptar_dueler_ud`).
    ///
    /// **Assumption:** it goes back to the neutral faction, as the fighters started.
    pub fn surrender(&mut self, id: ObjId) {
        self.stop_attacking(id);
        let foes: Vec<ObjId> = self
            .combat
            .target
            .iter()
            .filter(|(_, t)| **t == id)
            .map(|(a, _)| *a)
            .collect();
        for foe in foes {
            self.stop_attacking(foe);
        }
        if let Some(o) = self.objects.get_mut(&id) {
            o.faction = NEUTRAL_FACTION;
        }
    }

    fn reach_of(&mut self, id: ObjId) -> f32 {
        if self.weapon_of(id).ranged {
            RANGED_REACH
        } else {
            MELEE_REACH
        }
    }

    /// A creature with an empty right hand and a ranged weapon in its bag takes the best of them up before it fights, and
    /// keeps it at any distance: it shoots point blank rather than punch. The arena's duelists carry blasters and wield
    /// nothing until a script says so.
    ///
    /// **Assumption:** melee only when it has no ranged weapon, or a script asks for it while its hand is empty.
    pub(crate) fn ready_ranged(&mut self, id: ObjId) {
        let Some(o) = self.objects.get(&id) else {
            return;
        };
        if o.equipped.contains_key(&4) {
            return;
        }
        let bag = o.inventory.clone();
        let best = self.best_in_bag(&bag, true);
        if let (Some(item), Some(o)) = (best, self.objects.get_mut(&id)) {
            o.equipped.insert(4, item);
        }
    }

    /// The weapon of the highest average damage among `items`, ranged or melee as asked.
    pub(crate) fn best_in_bag(&mut self, items: &[ObjId], ranged: bool) -> Option<ObjId> {
        let withbase: Vec<(ObjId, i32)> = items
            .iter()
            .filter_map(|i| Some((*i, self.objects.get(i)?.base_item)))
            .collect();
        let t = self.table("baseitems")?;
        let mut best: Option<(f32, ObjId)> = None;
        for (item, base) in withbase {
            let row = base.max(0) as usize;
            let (dice, sides) = (
                t.int(row, "numdice").unwrap_or(0) as f32,
                t.int(row, "dietoroll").unwrap_or(0) as f32,
            );
            let is_ranged = t.int(row, "rangedweapon").unwrap_or(0) > 0;
            let avg = dice * (sides + 1.0) / 2.0;
            if dice > 0.0 && is_ranged == ranged && best.is_none_or(|b| avg > b.0) {
                best = Some((avg, item));
            }
        }
        best.map(|b| b.1)
    }

    /// Resolve one attack and everything that follows from it: damage, scripts, death.
    fn resolve(&mut self, attacker: ObjId, target: ObjId) {
        self.ready_ranged(attacker);
        // What the player queued goes first: a power instead of the attack, or a feat that changes it.
        let queued = (attacker == self.pc)
            .then(|| self.combat_queue.pop_front())
            .flatten();
        if let Some(crate::combat_actions::CombatAction::Power(spell)) = queued {
            if self.cast_spell(attacker, spell, target, false).is_ok() {
                return;
            }
        }
        let boost = match queued {
            Some(crate::combat_actions::CombatAction::Feat(f)) => {
                crate::combat_actions::feat_boost(f)
            }
            _ => Default::default(),
        };
        if boost.defense > 0 {
            let mut lower = crate::world::EffectRec::new("EffectACDecrease");
            lower.nums = vec![boost.defense as f32];
            self.effects.push(lower);
            let handle = self.effects.len() as u32;
            let seconds = ROUND_SECONDS + crate::combat_actions::DEFENSE_SECONDS;
            self.apply_effect(
                attacker,
                attacker,
                handle,
                crate::force::DURATION_TEMPORARY,
                seconds,
            );
        }
        self.resolve_with(attacker, target, boost);
        if boost.extra_attack && self.objects.get(&target).is_some_and(|t| !t.dead) {
            self.resolve_with(attacker, target, boost);
        }
        // The other hand swings too, in the same round.
        let standing = self
            .objects
            .get(&target)
            .is_some_and(|t| !t.dead && !t.down);
        if let (Some(slot), true) = (self.off_hand(attacker), standing) {
            let (_, penalty) = self.two_weapon_penalties(attacker);
            self.resolve_hand(attacker, target, Default::default(), slot, penalty);
        }
    }

    /// The slot of the second weapon's attack, if there is one: a weapon in the left hand beside one in the right, or
    /// the other blade of a double-bladed weapon (which swings from the right hand's slot).
    fn off_hand(&mut self, id: ObjId) -> Option<i32> {
        self.hand_weapon(id, 4)?;
        if self.hand_weapon(id, 5).is_some() {
            return Some(5);
        }
        self.has_double_blades(id).then_some(4)
    }

    /// Whether the right hand holds a double-bladed weapon: `Double` in the item's `label` in `baseitems.2da`
    /// (the double-bladed sword and lightsaber and the vibro double-blade).
    fn has_double_blades(&mut self, id: ObjId) -> bool {
        let base = self
            .objects
            .get(&id)
            .and_then(|o| o.equipped.get(&4))
            .and_then(|i| self.objects.get(i))
            .map(|i| i.base_item.max(0) as usize);
        let (Some(base), Some(t)) = (base, self.table("baseitems")) else {
            return false;
        };
        t.get(base, "label").is_some_and(|l| l.contains("Double"))
    }

    /// The attack penalty of the main hand and the off hand when two weapons (or both blades of one) are used: -6 and
    /// -10, and with the feat Two-Weapon Fighting -6 and -6, Advanced -4 and -4, Mastery -2 and -2. The words of the
    /// feats in the install say so, and the second game's Superior Two-Weapon Fighting takes more off Mastery's: see
    /// `k2feats.rs`. **Not built:** the "balanced" off-hand weapon that takes 2 more off the main hand.
    pub(crate) fn two_weapon_penalties(&self, id: ObjId) -> (i32, i32) {
        let has = |f: i32| self.objects.get(&id).is_some_and(|o| o.feats.contains(&f));
        if has(85) {
            let (main, off) = self.superior_two_weapon_eases(id);
            (-2 + main, -2 + off)
        } else if has(9) {
            (-4, -4)
        } else if has(3) {
            (-6, -6)
        } else {
            (-6, -10)
        }
    }

    /// The `DAMAGE_TYPE_*` bits of the weapon in the right hand: its `damageflags` column, bludgeoning for a fist.
    fn weapon_damage_type(&mut self, id: ObjId, slot: i32) -> i32 {
        let base = self
            .objects
            .get(&id)
            .and_then(|o| o.equipped.get(&slot))
            .and_then(|i| self.objects.get(i))
            .map(|i| i.base_item);
        let Some(base) = base else { return 1 };
        self.table("baseitems")
            .and_then(|t| t.int(base.max(0) as usize, "damageflags"))
            .map_or(crate::force::UNIVERSAL, |v| v as i32)
            .max(1)
    }

    /// `EffectAssuredHit` is on: a blow cannot miss.
    fn has_assured_hit(&self, id: ObjId) -> bool {
        self.objects.get(&id).is_some_and(|o| {
            o.effects.iter().any(|h| {
                self.effects
                    .get(h.wrapping_sub(1) as usize)
                    .is_some_and(|e| e.name == "EffectAssuredHit")
            })
        })
    }

    /// The main hand's attack.
    fn resolve_with(
        &mut self,
        attacker: ObjId,
        target: ObjId,
        boost: crate::combat_actions::FeatBoost,
    ) {
        let penalty = if self.off_hand(attacker).is_some() {
            self.two_weapon_penalties(attacker).0
        } else {
            0
        };
        self.resolve_hand(attacker, target, boost, 4, penalty);
    }

    /// One attack with the weapon in `slot` (4 right hand, 5 left), with `penalty` added to the roll.
    fn resolve_hand(
        &mut self,
        attacker: ObjId,
        target: ObjId,
        boost: crate::combat_actions::FeatBoost,
        slot: i32,
        penalty: i32,
    ) {
        let weapon = self.hand_weapon(attacker, slot).unwrap_or(Weapon::UNARMED);
        let bonus = self.attack_bonus_of(attacker, slot)
            + penalty
            + boost.to_hit
            + self.close_combat_bonus(attacker, target);
        let defense = self.defense_of(target);
        let (roll, confirm) = (self.roll(20), self.roll(20));
        let threat = weapon.threat * boost.threat.max(1) * self.threat_factor(attacker, slot);
        let mut outcome = resolve_attack(roll, confirm, bonus, defense, threat);
        if outcome == Outcome::Miss && self.has_assured_hit(attacker) {
            outcome = Outcome::Hit;
        }
        let mut dealt = 0;
        if outcome != Outcome::Miss {
            let dice: i32 = (0..weapon.dice).map(|_| self.roll(weapon.sides)).sum();
            let mod_ = self.flat_damage(attacker, slot, weapon.ranged);
            let more = self.roll_gear_damage(attacker, slot, target, outcome == Outcome::Critical);
            dealt = damage(dice, mod_, outcome, weapon.multiplier) + boost.damage + more;
            // The weapon's own damage type (`damageflags` of `baseitems.2da`) meets the target's resistances.
            let kind = self.weapon_damage_type(attacker, slot);
            dealt = self.adjust_damage(target, dealt, kind);
            // **Assumption:** difficulty scales what the party takes, by the `multiplier` of `difficultyopt.2da`.
            if target == self.pc || self.party.contains(&target) {
                dealt = ((dealt as f32 * self.difficulty_multiplier()).round() as i32).max(1);
            }
        }
        let fell = self.hurt(attacker, target, dealt);
        if let (Some(ability), true, false) = (boost.stun_ability, outcome != Outcome::Miss, fell) {
            let level = self.objects.get(&attacker).map_or(1, |o| o.level);
            let dc = level + ability_mod(self.ability_score(attacker, ability));
            if !self
                .saving_throw(target, crate::rules::Save::Fortitude, dc)
                .passed
            {
                self.effects
                    .push(crate::world::EffectRec::new("EffectStunned"));
                let handle = self.effects.len() as u32;
                let seconds = crate::combat_actions::STUN_SECONDS;
                self.apply_effect(
                    attacker,
                    target,
                    handle,
                    crate::force::DURATION_TEMPORARY,
                    seconds,
                );
            }
        }
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
        // **Assumption:** the plot flag means no harm at all. `tar03_turret` in `tar_m03aa` has it, and took blows till it fell.
        if t.dead || t.down || t.plot {
            return false;
        }
        t.hp -= amount;
        if t.min_one_hp {
            t.hp = t.hp.max(1);
        }
        if t.hp > 0 {
            self.say_from_soundset(target, &crate::soundset::PAIN_SLOTS);
            return false;
        }
        t.hp = 0;
        self.say_from_soundset(target, &[crate::soundset::DEAD_SLOT]);
        let Some(t) = self.objects.get_mut(&target) else {
            return false;
        };
        if party_side {
            t.down = true;
        } else {
            t.dead = true;
            self.died_at.insert(target, self.time);
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
        // Each creature and placeable beats every `HEARTBEAT_SECONDS`, each at its own moment within them. All at once, in one
        // frame, cost 11 ms (the opening module's two dozen scripts) and showed as a stutter.
        self.combat.heartbeat_clock += dt;
        let beating: Vec<(ObjId, bool)> = self
            .objects
            .values()
            .filter(|o| match o.kind {
                Kind::Creature => !o.dead && o.id != self.pc,
                Kind::Placeable => o.scripts.contains_key("OnHeartbeat"),
                _ => false,
            })
            .map(|o| (o.id, o.kind == Kind::Creature))
            .collect();
        let clock = self.combat.heartbeat_clock;
        let mut due: Vec<(ObjId, bool)> = Vec::new();
        for (id, creature) in beating {
            let next = self
                .combat
                .heartbeat_due
                .entry(id)
                .or_insert_with(|| clock + heartbeat_phase(id));
            if clock >= *next {
                *next += HEARTBEAT_SECONDS;
                if *next <= clock {
                    *next = clock + HEARTBEAT_SECONDS;
                }
                due.push((id, creature));
            }
        }
        self.combat.heartbeats_run += due.len() as u64;
        self.timed("heartbeats", |w| {
            for (id, creature) in due {
                if creature && w.objects.get(&id).is_some_and(|o| o.dead) {
                    continue;
                }
                w.run_event(
                    id,
                    if creature {
                        "ScriptHeartbeat"
                    } else {
                        "OnHeartbeat"
                    },
                );
            }
        });
        self.combat.module_heartbeat_clock += dt;
        while self.combat.module_heartbeat_clock >= HEARTBEAT_SECONDS {
            self.combat.module_heartbeat_clock -= HEARTBEAT_SECONDS;
            if self.minigame.is_none() {
                self.run_module_script("Mod_OnHeartbeat");
            }
        }
        self.combat.perception_clock += dt;
        if self.combat.perception_clock >= PERCEPTION_SECONDS {
            self.combat.perception_clock = 0.0;
            self.timed("perception", |w| w.update_perception());
        }
        self.timed("chase", |w| w.chase(dt));
        self.combat.round_clock += dt;
        while self.combat.round_clock >= ROUND_SECONDS {
            self.combat.round_clock -= ROUND_SECONDS;
            self.combat.round += 1;
            self.timed("round", |w| w.run_round());
        }
    }

    /// [`World::clear_line`] for tests.
    pub fn can_see_along(&self, a: [f32; 3], b: [f32; 3]) -> bool {
        self.clear_line(a, b)
    }

    /// Whether one can see from `a` to `b`: no closed door and no wall in between, at eye height, and no long stretch
    /// without floor under the line.
    ///
    /// **Assumption:** floor missing for under 2.5 m is an obstacle that can be looked over, a table or a crate (bug 25),
    /// where it used to end sight. A wall is any upright face of the area's walkmesh that is not floor. Pits and
    /// railings longer than that still block sight, though the game may see across them.
    pub(crate) fn clear_line(&self, a: [f32; 3], b: [f32; 3]) -> bool {
        const EYE: f32 = 1.5;
        const LOOK_OVER: f32 = 2.5;
        let Some(nav) = self.nav.as_ref() else {
            return true;
        };
        if nav.crosses_block(a, b) {
            return false;
        }
        if nav.sight_blocked([a[0], a[1], a[2] + EYE], [b[0], b[1], b[2] + EYE]) {
            return false;
        }
        let len = dist2d(a, b);
        let step = 0.75;
        let steps = (len / step).ceil().max(1.0) as usize;
        let top = a[2].max(b[2]) + 1.5;
        let mut gap = 0.0;
        for i in 1..steps {
            let t = i as f32 / steps as f32;
            if nav
                .walkable_at(a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, top)
                .is_some()
            {
                gap = 0.0;
            } else {
                gap += len / steps as f32;
                if gap > LOOK_OVER {
                    return false;
                }
            }
        }
        true
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
                // A sneaking player is noticed only by a watcher who passes its Awareness check.
                let near = near && (known || b != self.pc || self.stealth_lets_notice(a));
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
            if self.nav.is_none() {
                continue;
            }
            let moving = self.combat.chasers.get(&a).is_some_and(|m| m.has_route());
            let fresh = if replan || !moving {
                self.route_through_doors(apos, tpos)
            } else {
                None
            };
            self.open_doors_near(apos);
            let nav = self.nav.as_ref().expect("checked above");
            let m = self
                .combat
                .chasers
                .entry(a)
                .or_insert_with(|| Mover::new(apos, 0.0));
            m.pos = apos;
            if let Some(r) = fresh {
                m.follow(crate::follow::trim_route(r, reach * 0.8));
            }
            if m.has_route() {
                // A fighter runs at a foe.
                let params = kotor_walkmesh::MoveParams {
                    speed: crate::RUN_SPEED,
                    ..self.move_params
                };
                m.step(nav, &params, dt);
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

    /// Party members who have no target join the leader's: they fight what the player fights (bug 21).
    ///
    /// **Assumption:** the aggressive party AI style does this. Members with a target of their own keep it. Defensive
    /// joins only a foe that already fights the party, passive never (`SetPartyAIStyle`).
    fn assist_leader(&mut self) {
        let style = self.party_ai_style;
        // Passive: the party does not start or keep fights, whatever the leader does.
        if style >= 2 {
            for id in self.party.clone() {
                self.stop_attacking(id);
            }
            return;
        }
        let Some(foe) = self.combat.target.get(&self.pc).copied() else {
            return;
        };
        if self.reputation(self.pc, foe) > 10 {
            return;
        }
        // Defensive: they join only against a foe that is fighting the party already.
        let mine = |w: &World, id: ObjId| id == w.pc || w.party.contains(&id);
        let on_us = self.combat.target.get(&foe).is_some_and(|t| mine(self, *t));
        if style == 1 && !on_us {
            return;
        }
        for id in self.party.clone() {
            if !self.combat.target.contains_key(&id) {
                self.attack(id, foe);
            }
        }
    }

    /// **Assumption:** a creature not on the player's side with under a third of its hit points left, and a medpac in its
    /// bag, uses it instead of attacking that round (GAPS row 8). The game's own scripts never did.
    fn enemy_heals(&mut self, id: ObjId) -> bool {
        if self.on_player_side(id) {
            return false;
        }
        let Some(o) = self.objects.get(&id) else {
            return false;
        };
        if o.hp * 3 >= o.max_hp {
            return false;
        }
        let pack = o.inventory.iter().copied().find(|i| {
            self.objects
                .get(i)
                .is_some_and(|x| x.template.starts_with("g_i_medeqpmnt"))
        });
        let Some(pack) = pack else {
            return false;
        };
        self.use_item(id, pack).is_ok()
    }

    fn run_round(&mut self) {
        self.assist_leader();
        let pairs: Vec<(ObjId, ObjId)> = self.combat.target.iter().map(|(a, t)| (*a, *t)).collect();
        // A power takes up the round too, and the AI picks its next move when a round ends. Without this a Jedi who casts
        // never acts again, and the duel in `end_cut04` never finishes.
        let mut involved: BTreeSet<ObjId> = std::mem::take(&mut self.combat.casters);
        for (a, t) in pairs {
            // Someone knocked out is out of the fight as the dead are: a downed hero was hit over and over, then stood up to be hit again.
            let alive =
                |w: &World, id: ObjId| w.objects.get(&id).is_some_and(|o| !o.dead && !o.down);
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
            // An enemy low on hit points spends a round on a medpac it carries, as the player would.
            if self.enemy_heals(a) {
                involved.insert(a);
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

/// Where in the six seconds a thing's heartbeat falls: spread by its id, so that they do not all run in one frame.
fn heartbeat_phase(id: ObjId) -> f32 {
    ((id as f32 * 0.618_034) % 1.0) * HEARTBEAT_SECONDS
}
