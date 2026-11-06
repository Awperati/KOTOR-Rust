//! Character rules from the game's tables: saving throws, skills, feats, hit and Force points, level-up, experience
//! for kills and the knocked-out state.
//!
//! **Read from the install:** class save progression (`classes.2da` `savingthrowtable`, columns `fortsave` `refsave`
//! `willsave` by level), class skills and skill point base (`skills.2da` `<class>_class`, `classes.2da`
//! `skillpointbase`), the key ability and whether untrained use is allowed (`skills.2da` `keyability`, `untrained`),
//! feat lists per class (`feat.2da` `<class>_list` / `_granted`: 3 is granted at that level, 1 and 0 are the two
//! lists feats are chosen from, 4 is not available to the class), feats per level (`featgain.2da` `<class>_reg` /
//! `_bon`), hit die and Force die (`classes.2da`), experience levels (`exptable.2da`).
//!
//! **Assumed, not measured from the game:** the d20 save and skill check (d20 + bonus against a DC, natural 1 fails
//! and natural 20 succeeds a save); first level gives four times the skill points of later levels; a class skill rank
//! costs 1 point and a cross-class rank 2, ranks capped at level + 3 (cross-class half that); the first feat list is
//! the regular slots (`_reg`) and the zero list the bonus slots (`_bon`); hit points rise by the whole hit die plus the
//! Constitution modifier, which the reference video's sheet measures; Force points
//! likewise from `forcedie`; kill experience is `ChallengeRating` x 25, paid in full to each of the living party; a party member
//! reduced to 0 hit points is knocked out and stands up with 1 hit point when the fight is over (the player and
//! party members never die from damage); the game is over when everyone in the party is down at once.

use crate::combat::ability_mod;
use crate::world::{Kind, ObjId, World};

/// The death sequence: the fall plays this long with the camera held, then the screen fades to black over the next
/// seconds, then the game-over screen. **Assumed**, not measured: the original's timings are not in the reference video.
pub const DEATH_FALL_SECONDS: f32 = 3.0;
pub const DEATH_FADE_SECONDS: f32 = 2.0;

/// Where the death sequence has got to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DeathPhase {
    /// The party falls; nothing moves.
    Falling,
    /// Fading to black: 0 to 1.
    Fading(f32),
    /// Black, and the game-over screen is due.
    Over,
}

pub const XP_PER_CHALLENGE_RATING: f32 = 25.0;
pub const FIRST_LEVEL_SKILL_MULTIPLIER: i32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Save {
    Fortitude,
    Reflex,
    Will,
}

impl Save {
    fn column(self) -> &'static str {
        match self {
            Save::Fortitude => "fortsave",
            Save::Reflex => "refsave",
            Save::Will => "willsave",
        }
    }
    fn ability(self) -> usize {
        match self {
            Save::Fortitude => 2,
            Save::Reflex => 1,
            Save::Will => 4,
        }
    }
}

/// A d20 check against a difficulty class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Check {
    pub roll: i32,
    pub bonus: i32,
    pub dc: i32,
    pub passed: bool,
}

/// A save: natural 1 fails, natural 20 succeeds, otherwise roll + bonus must reach the DC.
pub fn save_passes(roll: i32, bonus: i32, dc: i32) -> bool {
    roll == 20 || (roll != 1 && roll + bonus >= dc)
}

/// A skill check: roll + bonus must reach the DC (no automatic results).
pub fn skill_passes(roll: i32, bonus: i32, dc: i32) -> bool {
    roll + bonus >= dc
}

/// What a level-up gave.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LevelUp {
    pub level: i32,
    pub hp: i32,
    pub force: i32,
    pub skill_points: i32,
    pub regular_feats: i32,
    pub bonus_feats: i32,
    pub granted_feats: Vec<i32>,
}

fn abbr(class_row: &str) -> String {
    class_row.to_ascii_lowercase()
}

/// `TOUGHNESS`, `IMPROVED_TOUGHNESS` and `MASTER_TOUGHNESS` in `feat.2da`. Their words: one more hit point a level,
/// counted back to the first.
const TOUGHNESS_FEATS: [i32; 3] = [84, 123, 124];

impl World {
    /// A creature's ability score including effects (ability increase and decrease).
    pub fn ability_score(&self, id: ObjId, ability: usize) -> i32 {
        let Some(o) = self.objects.get(&id) else {
            return 10;
        };
        o.abilities[ability]
            + self.gear_ability(id, ability)
            + self.effect_sum(id, "EffectAbilityIncrease", Some(ability as f32))
            - self.effect_sum(id, "EffectAbilityDecrease", Some(ability as f32))
    }

    /// Class save value at a level (`cls_st_*` tables, first row = level 1).
    pub fn class_save(&mut self, class: i32, level: i32, save: Save) -> i32 {
        let Some(classes) = self.table("classes") else {
            return 0;
        };
        let Some(name) = classes.get(class.max(0) as usize, "savingthrowtable") else {
            return 0;
        };
        let name = name.to_ascii_lowercase();
        self.table(&name)
            .and_then(|t| t.int((level.max(1) - 1) as usize, save.column()))
            .unwrap_or(0) as i32
    }

    /// Saving throw bonus: class value + ability modifier + effects.
    pub fn save_bonus(&mut self, id: ObjId, save: Save) -> i32 {
        let Some((class, level)) = self.objects.get(&id).map(|o| (o.class, o.level)) else {
            return 0;
        };
        self.class_save(class, level, save)
            + ability_mod(self.ability_score(id, save.ability()))
            + self.gear_save(id, save)
            + self.effect_sum(id, "EffectSavingThrowIncrease", None)
            - self.effect_sum(id, "EffectSavingThrowDecrease", None)
    }

    pub fn saving_throw(&mut self, id: ObjId, save: Save, dc: i32) -> Check {
        let roll = self.roll(20);
        let bonus = self.save_bonus(id, save);
        Check {
            roll,
            bonus,
            dc,
            passed: save_passes(roll, bonus, dc),
        }
    }

    pub(crate) fn class_abbr(&mut self, class: i32) -> Option<String> {
        let classes = self.table("classes")?;
        Some(abbr(classes.get(class.max(0) as usize, "skillstable")?))
    }

    pub fn is_class_skill(&mut self, class: i32, skill: usize) -> bool {
        let (Some(a), Some(t)) = (self.class_abbr(class), self.table("skills")) else {
            return false;
        };
        t.int(skill, &format!("{a}_class")).unwrap_or(0) != 0
    }

    pub fn skill_ranks(&self, id: ObjId, skill: usize) -> i32 {
        self.objects
            .get(&id)
            .and_then(|o| o.skills.get(skill).copied())
            .unwrap_or(0)
    }

    /// What a skill has besides its ranks: its key ability's modifier and effects.
    pub fn skill_modifier(&mut self, id: ObjId, skill: usize) -> i32 {
        let key = match self
            .table("skills")
            .as_ref()
            .and_then(|t| t.get(skill, "keyability").map(str::to_string))
            .as_deref()
        {
            Some("STR") => 0,
            Some("DEX") => 1,
            Some("CON") => 2,
            Some("WIS") => 4,
            Some("CHA") => 5,
            _ => 3,
        };
        ability_mod(self.ability_score(id, key))
            + self.gear_skill(id, skill)
            + self.effect_sum(id, "EffectSkillIncrease", Some(skill as f32))
            - self.effect_sum(id, "EffectSkillDecrease", Some(skill as f32))
    }

    /// d20 + ranks + key ability modifier + effects against `dc`. A skill that cannot be used untrained fails
    /// without a roll when the creature has no ranks.
    pub fn skill_check(&mut self, id: ObjId, skill: usize, dc: i32) -> Check {
        let ranks = self.skill_ranks(id, skill);
        let Some(t) = self.table("skills") else {
            return Check {
                roll: 0,
                bonus: 0,
                dc,
                passed: false,
            };
        };
        let untrained = t.int(skill, "untrained").unwrap_or(0) != 0;
        let bonus = ranks + self.skill_modifier(id, skill);
        if ranks == 0 && !untrained {
            return Check {
                roll: 0,
                bonus,
                dc,
                passed: false,
            };
        }
        let roll = self.roll(20);
        Check {
            roll,
            bonus,
            dc,
            passed: skill_passes(roll, bonus, dc),
        }
    }

    /// Skill points a level gives. **Measured from the video:** a Soldier, `skillpointbase` 2, with Intelligence 12 has 8 at
    /// 0:50, and 1 a level at 1:23:30: 3 ranks of a cross-class skill and 4 of a class skill by level 3 is 10 points, 8 + 1 + 1.
    /// **Assumed:** how that goes for the rest. At level 1, half the base and the modifier, times 4. After, half of the two
    /// together, rounded down. At least 1.
    pub fn skill_points_at(&mut self, class: i32, intelligence: i32, level: i32) -> i32 {
        let base = self
            .table("classes")
            .and_then(|t| t.int(class.max(0) as usize, "skillpointbase"))
            .unwrap_or(0) as i32;
        let modifier = ability_mod(intelligence);
        if level <= 1 {
            (base / 2 + modifier).max(1) * FIRST_LEVEL_SKILL_MULTIPLIER
        } else {
            ((base + modifier) / 2).max(1)
        }
    }

    /// What a rank of a skill costs a creature, and the most ranks it can have. Class skills cost 1, cross-class 2.
    pub fn skill_price(&mut self, id: ObjId, skill: usize) -> Option<(i32, i32)> {
        let (class, level) = self.objects.get(&id).map(|o| (o.class, o.level))?;
        Some(if self.is_class_skill(class, skill) {
            (1, level + 3)
        } else {
            (2, (level + 3) / 2)
        })
    }

    /// Raise a skill by one rank, spending points.
    pub fn buy_skill_rank(&mut self, id: ObjId, skill: usize) -> Result<(), String> {
        let Some((level, points)) = self.objects.get(&id).map(|o| (o.level, o.skill_points)) else {
            return Err("no such creature".into());
        };
        let count = self.table("skills").map_or(0, |t| t.row_count());
        if skill >= count {
            return Err(format!("no skill {skill}"));
        }
        let Some((cost, cap)) = self.skill_price(id, skill) else {
            return Err("no such creature".into());
        };
        let ranks = self.skill_ranks(id, skill);
        if ranks >= cap {
            return Err(format!("rank {ranks} is the most at level {level}"));
        }
        if points < cost {
            return Err(format!("{points} points left, {cost} needed"));
        }
        let o = self.objects.get_mut(&id).expect("checked");
        if o.skills.len() <= skill {
            o.skills.resize(skill + 1, 0);
        }
        o.skills[skill] += 1;
        o.skill_points -= cost;
        Ok(())
    }

    pub(crate) fn feat_column(&mut self, class: i32, suffix: &str) -> Option<String> {
        let classes = self.table("classes")?;
        let a = abbr(classes.get(class.max(0) as usize, "featstable")?);
        Some(format!("{a}_{suffix}"))
    }

    /// Feats a class grants automatically at `level` or below (list 3 with a grant level).
    pub fn feats_granted(&mut self, class: i32, level: i32) -> Vec<i32> {
        let (Some(list), Some(granted), Some(t)) = (
            self.feat_column(class, "list"),
            self.feat_column(class, "granted"),
            self.table("feat"),
        ) else {
            return Vec::new();
        };
        (0..t.row_count())
            .filter(|&r| {
                t.int(r, &list) == Some(3)
                    && t.int(r, &granted)
                        .is_some_and(|g| g >= 1 && g as i32 <= level)
            })
            .map(|r| r as i32)
            .collect()
    }

    /// Feats the class grants at exactly `level`.
    fn feats_granted_at(&mut self, class: i32, level: i32) -> Vec<i32> {
        let now = self.feats_granted(class, level);
        let before = if level > 1 {
            self.feats_granted(class, level - 1)
        } else {
            Vec::new()
        };
        now.into_iter().filter(|f| !before.contains(f)).collect()
    }

    /// (regular, bonus) feat choices a class gives at a level (`featgain.2da`).
    pub fn feat_slots_at(&mut self, class: i32, level: i32) -> (i32, i32) {
        let (Some(reg), Some(bon), Some(t)) = (
            self.feat_column(class, "reg"),
            self.feat_column(class, "bon"),
            self.table("featgain"),
        ) else {
            return (0, 0);
        };
        let row = (level.max(1) - 1) as usize;
        (
            t.int(row, &reg).unwrap_or(0) as i32,
            t.int(row, &bon).unwrap_or(0) as i32,
        )
    }

    /// Whether a creature may choose a feat from list `list` (1 regular, 0 bonus): the class offers it there,
    /// the creature lacks it, and the level, ability and prerequisite-feat requirements in `feat.2da` hold.
    pub fn can_take_feat(&mut self, id: ObjId, feat: i32, list: i32) -> bool {
        let Some((class, level, owned)) = self
            .objects
            .get(&id)
            .map(|o| (o.class, o.level, o.feats.clone()))
        else {
            return false;
        };
        let (Some(col), Some(t)) = (self.feat_column(class, "list"), self.table("feat")) else {
            return false;
        };
        let row = feat.max(0) as usize;
        if feat < 0 || row >= t.row_count() || owned.contains(&feat) {
            return false;
        }
        if t.int(row, &col) != Some(list as i64) {
            return false;
        }
        if t.int(row, "mincharlevel").unwrap_or(0) as i32 > level {
            return false;
        }
        for (c, a) in [("minstr", 0), ("mindex", 1), ("minint", 3), ("minwis", 4)] {
            if t.int(row, c).unwrap_or(0) as i32 > self.ability_score(id, a) {
                return false;
            }
        }
        ["prereqfeat1", "prereqfeat2"]
            .iter()
            .filter_map(|c| t.int(row, c))
            .all(|p| owned.contains(&(p as i32)))
    }

    /// Choose a feat with one of the creature's unspent feat slots.
    pub fn take_feat(&mut self, id: ObjId, feat: i32, bonus_list: bool) -> Result<(), String> {
        let Some((reg, bon)) = self.objects.get(&id).map(|o| o.feat_slots) else {
            return Err("no such creature".into());
        };
        if (if bonus_list { bon } else { reg }) < 1 {
            return Err("no feat slot left".into());
        }
        if !self.can_take_feat(id, feat, if bonus_list { 0 } else { 1 }) {
            return Err(format!("feat {feat} cannot be taken"));
        }
        let o = self.objects.get_mut(&id).expect("checked");
        o.feats.insert(feat);
        if bonus_list {
            o.feat_slots.1 -= 1;
        } else {
            o.feat_slots.0 -= 1;
        }
        Ok(())
    }

    fn die_of(&mut self, class: i32, column: &str) -> i32 {
        self.table("classes")
            .and_then(|t| t.int(class.max(0) as usize, column))
            .unwrap_or(0) as i32
    }

    /// Hit points a level adds: the whole hit die,
    /// plus the Constitution modifier, at least 1.
    pub fn hp_for_level(&mut self, class: i32, constitution: i32, level: i32) -> i32 {
        // Every level pays the whole die. Measured: the reference video's Soldier has Constitution 14 and Toughness, and
        // 26 hit points at level 2. That is (10 + 2 + 1) x 2. Half a die would give 22.
        let _ = level;
        (self.die_of(class, "hitdie") + ability_mod(constitution)).max(1)
    }

    /// Force points a level adds, the same way from `forcedie` (none for a class without one).
    pub fn force_for_level(&mut self, class: i32, level: i32) -> i32 {
        let die = self.die_of(class, "forcedie");
        if die == 0 {
            0
        } else if level <= 1 {
            die
        } else {
            die / 2 + 1
        }
    }

    /// The hit points a creature's Toughness feats add each level.
    fn toughness_of(&self, id: ObjId) -> i32 {
        self.objects.get(&id).map_or(0, |o| {
            TOUGHNESS_FEATS
                .iter()
                .filter(|f| o.feats.contains(f))
                .count() as i32
        })
    }

    /// A Toughness feat just taken pays for the levels so far.
    pub(crate) fn pay_toughness(&mut self, id: ObjId, feat: i32) {
        if TOUGHNESS_FEATS.contains(&feat) {
            if let Some(o) = self.objects.get_mut(&id) {
                o.max_hp += o.level;
                o.hp += o.level;
            }
        }
    }

    /// Raise a creature one level: hit and Force points, skill points, feat slots, and the feats the class grants.
    pub fn level_up(&mut self, id: ObjId) -> Option<LevelUp> {
        let (class, level) = self.objects.get(&id).map(|o| (o.class, o.level))?;
        let level = level + 1;
        let con = self.ability_score(id, 2);
        let int = self.ability_score(id, 3);
        let up = LevelUp {
            level,
            hp: self.hp_for_level(class, con, level),
            force: if self.is_early(id) {
                self.early_points_a_level(id)
            } else {
                self.force_for_level(class, level)
            },
            skill_points: self.skill_points_at(class, int, level),
            regular_feats: self.feat_slots_at(class, level).0,
            bonus_feats: self.feat_slots_at(class, level).1,
            granted_feats: self.feats_granted_at(class, level),
        };
        let tough = self.toughness_of(id);
        let o = self.objects.get_mut(&id)?;
        o.level = level;
        // A level heals whoever takes it: the reference video's bar stands at 82% before the level at about 650 s, and
        // the sheet reads 26/26 after it.
        o.max_hp += up.hp + tough;
        o.hp = o.max_hp;
        o.max_force += up.force;
        o.force += up.force;
        o.skill_points += up.skill_points;
        o.feat_slots.0 += up.regular_feats;
        o.feat_slots.1 += up.bonus_feats;
        o.feats.extend(up.granted_feats.iter().copied());
        Some(up)
    }

    /// Give experience; level up for every threshold crossed, unless the player's are held for the screen. Returns the
    /// level-ups.
    pub fn grant_xp(&mut self, id: ObjId, amount: i32) -> Vec<LevelUp> {
        let (hold, pc) = (self.hold_levels, self.pc);
        let Some(o) = self.objects.get_mut(&id) else {
            return Vec::new();
        };
        o.xp += amount;
        if hold && (id == pc || self.party.contains(&id)) {
            return Vec::new();
        }
        let xp = o.xp;
        let target = self.level_for_xp(xp);
        let mut out = Vec::new();
        while self.objects.get(&id).is_some_and(|o| o.level < target) {
            match self.level_up(id) {
                Some(u) => out.push(u),
                None => break,
            }
        }
        out
    }

    /// Experience a kill is worth: the creature's challenge rating x 25.
    pub fn kill_xp(&self, victim: ObjId) -> i32 {
        self.objects.get(&victim).map_or(0, |o| {
            (o.challenge_rating * XP_PER_CHALLENGE_RATING).round() as i32
        })
    }

    /// The player and party that are standing (not down, not dead).
    pub fn standing_party(&self) -> Vec<ObjId> {
        std::iter::once(self.pc)
            .chain(self.party.iter().copied())
            .filter(|id| {
                self.objects
                    .get(id)
                    .is_some_and(|o| !o.dead && !o.down && o.kind == Kind::Creature)
            })
            .collect()
    }

    /// Pay a kill's experience to each of the standing party. The reference video's total on the Endar Spire is only
    /// reached with whole payments, see `docs/REFERENCE.md`.
    pub(crate) fn award_kill(&mut self, victim: ObjId) {
        let xp = self.kill_xp(victim);
        if xp <= 0 {
            return;
        }
        for id in self.standing_party() {
            let ups = self.grant_xp(id, xp);
            for u in ups {
                self.events
                    .push(format!("{} reaches level {}", self.describe(id), u.level));
            }
        }
    }

    /// Whether the party and player are the only side still fighting alive: nobody hostile is attacking them or
    /// being attacked by them.
    pub(crate) fn fight_is_over(&self) -> bool {
        let mine = |id: &ObjId| *id == self.pc || self.party.contains(id);
        let alive = |id: &ObjId| self.objects.get(id).is_some_and(|o| !o.dead && !o.down);
        // **Assumption:** a fight lasts while an enemy still notices someone who stands, as well as while blows are aimed.
        // Without this a knocked-out hero stood up the moment the foe's aim was cleared, to be hit again (GAPS row 5).
        !self
            .combat
            .target
            .iter()
            .any(|(a, t)| alive(a) && alive(t) && (mine(a) != mine(t)))
            && !self.combat.perceived.iter().any(|(a, b)| {
                !mine(a) && mine(b) && alive(a) && alive(b) && self.reputation(*a, *b) <= 10
            })
    }

    /// The death sequence's phase, if the whole party has fallen.
    pub fn death_phase(&self) -> Option<DeathPhase> {
        self.game_over.then(|| {
            let t = self.death_clock;
            if t < DEATH_FALL_SECONDS {
                DeathPhase::Falling
            } else if t < DEATH_FALL_SECONDS + DEATH_FADE_SECONDS {
                DeathPhase::Fading((t - DEATH_FALL_SECONDS) / DEATH_FADE_SECONDS)
            } else {
                DeathPhase::Over
            }
        })
    }

    /// A step with the whole party down: input is dead, the fight is called off, and only the clock runs.
    pub(crate) fn step_dead(&mut self, dt: f32) {
        self.time += dt;
        self.death_clock += dt;
        self.steer = None;
        self.roll_request = false;
        self.pending = None;
        self.pc_mover.stop();
        self.close_container();
        self.conv = None;
        self.combat.target.clear();
        self.combat.perceived.clear();
    }

    /// Stand knocked-out party members up with 1 hit point once the fight is over; flag the game over when the whole
    /// party is down.
    pub(crate) fn update_knockouts(&mut self) {
        let group: Vec<ObjId> = std::iter::once(self.pc)
            .chain(self.party.iter().copied())
            .collect();
        let down: Vec<ObjId> = group
            .iter()
            .copied()
            .filter(|id| self.objects.get(id).is_some_and(|o| o.down))
            .collect();
        if down.is_empty() {
            return;
        }
        if self.standing_party().is_empty() {
            if !self.game_over {
                self.game_over = true;
                self.death_clock = 0.0;
                self.events
                    .push("game over: the whole party is down".into());
            }
            return;
        }
        if self.fight_is_over() {
            for id in down {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.down = false;
                    o.hp = 1;
                }
                self.events
                    .push(format!("{} gets back up", self.describe(id)));
            }
        }
    }
}
