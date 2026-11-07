//! Character creation as data: a [`CharacterSpec`] (class, portrait, body size, attribute scores, skill ranks, feats)
//! is checked against the game's tables and turned into the player character, replacing the placeholder object.
//!
//! **Read from the install:** which classes the player may take (`classes.2da` `playerclass`), which portraits a
//! player may take (`portraits.2da` `forpc`), the portrait's gender, race and body and head choices
//! (`portraits.2da` `sex`, `race`, `appearancenumber`, `appearance_s`, `appearance_l`), the body model and texture
//! and head (`appearance.2da` `modela`, `texa`, `normalhead`; `heads.2da` `head`), race ability adjustments
//! (`racialtypes.2da`), and every rule in `rules.rs` (skill points, feat slots, hit and Force points).
//!
//! **Assumed:** attributes start at 8 and are bought from a pool of 30 points at 1 point per step up to 14, 2 per step
//! for 15 and 16, 3 per step for 17 and 18, none above 18. The three player classes' recommended scores in
//! `classes.2da` each cost exactly 30 points under this schedule, which is the evidence for it (tested). Which of
//! `appearance_s`, `appearancenumber` and `appearance_l` is the small, medium and large body is a guess from their
//! order (91, 92, 93 for the first portrait).

use crate::combat::ability_mod;
use crate::world::{ObjId, World};

pub const ATTRIBUTE_BASE: i32 = 8;
pub const ATTRIBUTE_MAX: i32 = 18;
pub const POINT_POOL: i32 = 30;
const NAME_TRIES: usize = 30;
const NAME_END_CHANCE: f32 = 0.45;
pub const ABILITY_NAMES: [&str; 6] = [
    "Strength",
    "Dexterity",
    "Constitution",
    "Intelligence",
    "Wisdom",
    "Charisma",
];

/// Points to buy a score starting from 8 (`None` above 18 or below 8).
pub fn attribute_cost(score: i32) -> Option<i32> {
    if !(ATTRIBUTE_BASE..=ATTRIBUTE_MAX).contains(&score) {
        return None;
    }
    Some(
        (ATTRIBUTE_BASE + 1..=score)
            .map(|s| match s {
                ..=14 => 1,
                15 | 16 => 2,
                _ => 3,
            })
            .sum(),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodySize {
    Small,
    Medium,
    Large,
}

impl BodySize {
    /// **From the reference video at 0:24:** each class has its own frame: `classes.2da` row 0 Soldier, 1 Scout, 2 Scoundrel.
    pub fn of_class(class: i32) -> BodySize {
        match class {
            0 => BodySize::Large,
            1 => BodySize::Medium,
            _ => BodySize::Small,
        }
    }
}

/// `humanm.ltr`, `humanf.ltr` and `humanl.ltr`: what letters tend to follow what. Each row is cumulative.
struct NameLetters {
    count: usize,
    numbers: Vec<f32>,
}

/// Where a name is in the word: which of a row's three lists to use.
const LTR_START: usize = 0;
const LTR_MIDDLE: usize = 1;
const LTR_END: usize = 2;
const LTR_HEADER: usize = 9;
const LTR_ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz'-";

impl NameLetters {
    fn parse(bytes: &[u8]) -> Option<NameLetters> {
        let count = *bytes.get(LTR_HEADER - 1)? as usize;
        let want = 3 * count * (1 + count + count * count);
        let body = bytes.get(LTR_HEADER..LTR_HEADER + want * 4)?;
        let numbers = body
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect();
        Some(NameLetters { count, numbers })
    }

    /// The letter a number between 0 and 1 falls on, given the letters before it: `None` when nothing follows them.
    fn letter(&self, before: &[usize], place: usize, roll: f32) -> Option<usize> {
        let n = self.count;
        let row = match before {
            [] => place * n,
            [a] => 3 * n + (a * 3 + place) * n,
            [a, b] => 3 * n + 3 * n * n + ((a * n + b) * 3 + place) * n,
            _ => return None,
        };
        let list = self.numbers.get(row..row + n)?;
        list.iter().position(|p| *p > 0.0 && roll <= *p)
    }
}

/// How the player was made to look, beyond the head and the body: each slider by name, in per cent. None of them is set: the
/// game's own look. Kept with the character, and only on the player. See `docs/CHARGEN_PLAN.md`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CharacterLook {
    pub sliders: std::collections::BTreeMap<String, i32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CharacterSpec {
    pub name: String,
    /// `classes.2da` row: 0 Soldier, 1 Scout, 2 Scoundrel.
    pub class: i32,
    /// `portraits.2da` row.
    pub portrait: i32,
    pub body: BodySize,
    /// Final scores in file order (Str, Dex, Con, Int, Wis, Cha), before race adjustments.
    pub abilities: [i32; 6],
    /// (skill row, ranks to buy).
    pub skills: Vec<(usize, i32)>,
    /// Feats to choose with the level-1 slots, regular list first.
    pub feats: Vec<i32>,
    pub look: CharacterLook,
}

impl World {
    /// The player's underwear: the id of a variant, empty for the standard paint. Chosen in character creation.
    pub fn set_player_underwear(&mut self, id: &str) {
        let pc = self.pc;
        if let Some(o) = self.objects.get_mut(&pc) {
            o.underwear = id.to_string();
        }
    }

    /// The class's recommended scores (`classes.2da` `str`..`cha`).
    pub fn recommended_abilities(&mut self, class: i32) -> Option<[i32; 6]> {
        let t = self.table("classes")?;
        let row = class.max(0) as usize;
        let mut out = [0; 6];
        for (i, c) in ["str", "dex", "con", "int", "wis", "cha"]
            .iter()
            .enumerate()
        {
            out[i] = t.int(row, c)? as i32;
        }
        Some(out)
    }

    /// The portraits on offer for a gender (`portraits.2da` `sex`), in the table's order.
    pub fn player_portraits(&mut self, gender: i32) -> Vec<i32> {
        let Some(t) = self.table("portraits") else {
            return Vec::new();
        };
        (0..t.row_count())
            .filter(|r| t.int(*r, "forpc") == Some(1) && t.int(*r, "sex") == Some(gender as i64))
            .map(|r| r as i32)
            .collect()
    }

    /// The picture a creature's file names: `PortraitId`, a row of `portraits.2da`. Row 0 is blank, and then it is the
    /// row of the body it has. **Assumed:** that finding never gives a player's head, which only a file can name.
    pub(crate) fn creature_portrait(&mut self, utc: &kotor_gff::Struct) -> String {
        let id = utc.int("PortraitId").unwrap_or(0).max(0) as usize;
        let body = utc.int("Appearance_Type");
        let Some(t) = self.table("portraits") else {
            return String::new();
        };
        let name = |row: usize| t.get(row, "baseresref").unwrap_or("").to_ascii_lowercase();
        if !name(id).is_empty() {
            return name(id);
        }
        let has_body = |row: &usize| {
            t.int(*row, "forpc") != Some(1)
                && ["appearancenumber", "appearance_s", "appearance_l"]
                    .iter()
                    .any(|c| body.is_some() && t.int(*row, c) == body)
        };
        (0..t.row_count())
            .find(has_body)
            .map_or(String::new(), name)
    }

    /// The `appearance.2da` row a portrait has in a body size.
    pub fn portrait_appearance(&mut self, portrait: i32, body: BodySize) -> Option<i32> {
        let t = self.table("portraits")?;
        let column = match body {
            BodySize::Small => "appearance_s",
            BodySize::Medium => "appearancenumber",
            BodySize::Large => "appearance_l",
        };
        t.int(portrait.max(0) as usize, column).map(|a| a as i32)
    }

    fn name_part(&mut self, file: &str) -> Option<String> {
        let bytes = self.inst.read(file, kotor_core::ResType(2036)).ok()?;
        let letters = NameLetters::parse(&bytes)?;
        let mut roll = || self.roll(10_000) as f32 / 10_000.0;
        // **Assumed:** a name has at least four letters, and more than nine are cut short.
        for _ in 0..NAME_TRIES {
            let mut name = Vec::new();
            for len in 0..3 {
                match letters.letter(&name[..len], LTR_START, roll()) {
                    Some(l) => name.push(l),
                    None => break,
                }
            }
            if name.len() < 3 {
                continue;
            }
            loop {
                let last = [name[name.len() - 2], name[name.len() - 1]];
                let place = if name.len() >= 4 && roll() < NAME_END_CHANCE || name.len() >= 9 {
                    LTR_END
                } else {
                    LTR_MIDDLE
                };
                let Some(l) = letters.letter(&last, place, roll()) else {
                    break;
                };
                name.push(l);
                if place == LTR_END {
                    let mut out: String = name
                        .iter()
                        .map(|l| *LTR_ALPHABET.get(*l).unwrap_or(&b'a') as char)
                        .collect();
                    if let Some(first) = out.get_mut(0..1) {
                        first.make_ascii_uppercase();
                    }
                    return Some(out);
                }
            }
        }
        None
    }

    /// A name from the game's letter tables, for the Random Name button.
    pub fn random_name(&mut self, gender: i32) -> String {
        let first = self.name_part(if gender == 1 { "humanf" } else { "humanm" });
        let last = self.name_part("humanl");
        match (first, last) {
            (Some(a), Some(b)) => format!("{a} {b}"),
            (Some(a), None) | (None, Some(a)) => a,
            _ => String::new(),
        }
    }

    /// Build the player character from a spec; the placeholder player object becomes the character.
    pub fn create_character(&mut self, spec: &CharacterSpec) -> Result<ObjId, String> {
        let classes = self.table("classes").ok_or("no classes.2da")?;
        let row = spec.class.max(0) as usize;
        if spec.class < 0
            || row >= classes.row_count()
            || classes.int(row, "playerclass") != Some(1)
        {
            return Err(format!("class {} is not a player class", spec.class));
        }
        let portraits = self.table("portraits").ok_or("no portraits.2da")?;
        let prow = spec.portrait.max(0) as usize;
        if spec.portrait < 0
            || prow >= portraits.row_count()
            || portraits.int(prow, "forpc") != Some(1)
        {
            return Err(format!("portrait {} is not for the player", spec.portrait));
        }
        let mut spent = 0;
        for (i, s) in spec.abilities.iter().enumerate() {
            spent += attribute_cost(*s).ok_or_else(|| {
                format!(
                    "{} {s} is outside {ATTRIBUTE_BASE}..={ATTRIBUTE_MAX}",
                    ABILITY_NAMES[i]
                )
            })?;
        }
        if spent > POINT_POOL {
            return Err(format!(
                "{spent} attribute points spent, {POINT_POOL} available"
            ));
        }
        // Race adjustments (human has none).
        let race = portraits.int(prow, "race").unwrap_or(6) as usize;
        let races = self.table("racialtypes").ok_or("no racialtypes.2da")?;
        let mut abilities = spec.abilities;
        for (i, c) in [
            "stradjust",
            "dexadjust",
            "conadjust",
            "intadjust",
            "wisadjust",
            "chaadjust",
        ]
        .iter()
        .enumerate()
        {
            abilities[i] += races.int(race, c).unwrap_or(0) as i32;
        }
        // Body and head from the portrait row.
        let body_row = match spec.body {
            BodySize::Small => portraits.int(prow, "appearance_s"),
            BodySize::Medium => portraits.int(prow, "appearancenumber"),
            BodySize::Large => portraits.int(prow, "appearance_l"),
        }
        .ok_or("the portrait names no body")? as i32;
        let head_row = self
            .table("appearance")
            .and_then(|t| t.int(body_row.max(0) as usize, "normalhead"))
            .unwrap_or(0) as i32;
        let portrait = portraits.get(prow, "baseresref").unwrap_or("").to_string();
        let gender = portraits.int(prow, "sex").unwrap_or(0) as i32;

        let pc = self.pc;
        {
            let o = self.objects.get_mut(&pc).ok_or("no player object")?;
            o.name = spec.name.clone();
            o.class = spec.class;
            o.level = 1;
            o.xp = 0;
            o.abilities = abilities;
            o.appearance = body_row;
            o.head = head_row;
            o.gender = gender;
            o.portrait = portrait;
            o.look = spec.look.clone();
            o.race = race as i32;
            o.skills.clear();
            o.feats.clear();
            o.spells.clear();
            o.effects.clear();
            o.dead = false;
            o.down = false;
        }
        let con = self.ability_score(pc, 2);
        let int = self.ability_score(pc, 3);
        let hp = self.hp_for_level(spec.class, con, 1);
        let force = self.force_for_level(spec.class, 1);
        let points = self.skill_points_at(spec.class, int, 1);
        let (reg, bon) = self.feat_slots_at(spec.class, 1);
        let granted = self.feats_granted(spec.class, 1);
        {
            let o = self.objects.get_mut(&pc).expect("checked");
            o.max_hp = hp;
            o.hp = hp;
            o.max_force = force;
            o.force = force;
            o.skill_points = points;
            o.feat_slots = (reg, bon);
            o.feats.extend(granted);
        }
        self.start_early(pc);
        for (skill, ranks) in &spec.skills {
            for _ in 0..*ranks {
                self.buy_skill_rank(pc, *skill)?;
            }
        }
        for f in &spec.feats {
            self.choose_feat(pc, *f)
                .map_err(|e| format!("feat {f}: {e}"))?;
        }
        self.pc_mover.pos = self.objects[&pc].position;
        Ok(pc)
    }

    /// The class's recommended build for a portrait: its recommended scores, medium body, no skills or feats chosen.
    pub fn recommended_character(
        &mut self,
        name: &str,
        class: i32,
        portrait: i32,
    ) -> Option<CharacterSpec> {
        Some(CharacterSpec {
            name: name.to_string(),
            class,
            portrait,
            body: BodySize::Medium,
            abilities: self.recommended_abilities(class)?,
            skills: Vec::new(),
            feats: Vec::new(),
            look: CharacterLook::default(),
        })
    }

    /// A printed character sheet.
    pub fn character_sheet(&mut self, id: ObjId) -> String {
        use std::fmt::Write;
        let Some(o) = self.objects.get(&id).cloned() else {
            return "no such creature\n".into();
        };
        let class = self
            .table("classes")
            .and_then(|t| t.get(o.class.max(0) as usize, "label").map(str::to_string))
            .unwrap_or_default();
        let (fort, refl, will) = (
            self.save_bonus(id, crate::rules::Save::Fortitude),
            self.save_bonus(id, crate::rules::Save::Reflex),
            self.save_bonus(id, crate::rules::Save::Will),
        );
        let (bab, ac) = (self.base_attack_of(id), self.defense_of(id));
        let weapon = self.weapon_of(id);
        let mut s = String::new();
        let _ = writeln!(
            s,
            "{} - {} level {} {}",
            o.name,
            if o.gender == 1 { "female" } else { "male" },
            o.level,
            class
        );
        let _ = writeln!(
            s,
            "XP {}   HP {}/{}   Force {}/{}   Defense {}   Base attack +{}",
            o.xp, o.hp, o.max_hp, o.force, o.max_force, ac, bab
        );
        let _ = writeln!(s, "Saves: Fortitude +{fort}  Reflex +{refl}  Will +{will}");
        let _ = writeln!(s, "Attributes:");
        for (i, name) in ABILITY_NAMES.iter().enumerate() {
            let score = self.ability_score(id, i);
            let _ = writeln!(s, "  {name:<13}{score:>3} ({:+})", ability_mod(score));
        }
        let _ = writeln!(
            s,
            "Weapon: {}d{} threat {}x{}{}",
            weapon.dice,
            weapon.sides,
            21 - weapon.threat.max(1),
            weapon.multiplier,
            if weapon.ranged { " ranged" } else { "" }
        );
        let skills = self.table("skills");
        let _ = writeln!(s, "Skills ({} points unspent):", o.skill_points);
        if let Some(t) = &skills {
            for row in 0..t.row_count() {
                let ranks = o.skills.get(row).copied().unwrap_or(0);
                if ranks > 0 {
                    let _ = writeln!(s, "  {:<14}{ranks:>2}", t.get(row, "label").unwrap_or("?"));
                }
            }
        }
        let feats = self.table("feat");
        let _ = writeln!(
            s,
            "Feats ({} regular and {} bonus choices unspent):",
            o.feat_slots.0, o.feat_slots.1
        );
        if let Some(t) = &feats {
            for f in &o.feats {
                let _ = writeln!(s, "  {}", t.get(*f as usize, "label").unwrap_or("?"));
            }
        }
        let body = self.table("appearance");
        let heads = self.table("heads");
        let model = |col: &str| {
            body.as_ref()
                .and_then(|t| t.get(o.appearance.max(0) as usize, col).map(str::to_string))
                .unwrap_or_default()
        };
        let head = heads
            .as_ref()
            .and_then(|t| t.get(o.head.max(0) as usize, "head").map(str::to_string))
            .unwrap_or_default();
        let _ = writeln!(
            s,
            "Look: appearance {} body {} texture {} head {} portrait {}",
            o.appearance,
            model("modela"),
            model("texa"),
            head,
            o.portrait
        );
        s
    }
}
