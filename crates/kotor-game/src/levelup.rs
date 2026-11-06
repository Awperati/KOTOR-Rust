//! What the level-up screens choose from, and the choices the rules in `rules.rs` do not make yet. Tables: `feat.2da`,
//! `spells.2da`, `skills.2da`, `classes.2da` and `classpowergain.2da`.
//!
//! **Assumed, not compared with the game's code:** one attribute point on every level divisible by 4, as the reference
//! video shows at level 4 and not at level 2; a regular feat slot takes a feat of either list, since the video's Soldier,
//! with regular slots only, takes Conditioning from list 0; a raised Constitution pays back every level, and a raised
//! Intelligence one skill point for the level being taken.

use crate::combat::ability_mod;
use crate::world::{ObjId, World};
use kotor_tables::TwoDa;

/// One feat or Force power on a screen. Words are talk table numbers.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub id: i32,
    pub name: u32,
    pub description: u32,
    pub icon: String,
    pub owned: bool,
    pub can_take: bool,
}

/// Most steps of a chain a list's row shows; a longer chain goes on in the next row.
const CHAIN_WIDTH: usize = 3;
const POWER_USER_TYPE: i64 = 1;

/// Chains of rows: each starts where nothing leads to it and follows `next`.
fn chains(all: &[usize], next: impl Fn(usize) -> Option<usize>) -> Vec<Vec<usize>> {
    let followers: Vec<usize> = all.iter().filter_map(|r| next(*r)).collect();
    let mut seen = Vec::new();
    let mut out = Vec::new();
    let starts = all.iter().filter(|r| !followers.contains(r));
    for start in starts.chain(all.iter()).copied() {
        let mut chain = Vec::new();
        let mut at = Some(start);
        while let Some(r) = at.filter(|r| !seen.contains(r)) {
            seen.push(r);
            chain.push(r);
            at = next(r);
        }
        out.extend(chain.chunks(CHAIN_WIDTH).map(|c| c.to_vec()));
    }
    out
}

fn prerequisites(t: &TwoDa, row: usize) -> Vec<usize> {
    t.get(row, "prerequisites")
        .unwrap_or("")
        .split('_')
        .filter_map(|p| p.parse().ok())
        .collect()
}

impl World {
    /// Enough experience has come in for a level that has not been taken. Only so with `hold_levels`.
    pub fn level_ready(&self, id: ObjId) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|o| o.level < self.level_for_xp(o.xp))
    }

    /// Attribute points a level gives.
    pub fn ability_points_at(&self, level: i32) -> i32 {
        (level > 0 && level % 4 == 0) as i32
    }

    /// Force powers a class chooses at a level. None for a class without them.
    pub fn power_slots_at(&mut self, class: i32, level: i32) -> i32 {
        let Some(column) = self.known_powers_column(class) else {
            return 0;
        };
        self.table("classpowergain")
            .and_then(|t| t.int((level.max(1) - 1) as usize, &column))
            .unwrap_or(0) as i32
    }

    /// `classes.2da` names the class's column of `classpowergain.2da`.
    fn known_powers_column(&mut self, class: i32) -> Option<String> {
        let classes = self.table("classes")?;
        let v = classes.get(class.max(0) as usize, "spellknowntable")?;
        (!v.is_empty()).then(|| v.to_ascii_lowercase())
    }

    /// The column of `spells.2da` that holds the level a class may have a power from.
    fn power_level_column(&mut self, class: i32) -> Option<&'static str> {
        match self.known_powers_column(class)?.as_str() {
            "jgd" => Some("guardian"),
            "jcn" => Some("consular"),
            "jsn" => Some("sentinel"),
            _ => None,
        }
    }

    /// The feats a class has anything to do with, as chains from a feat to what it leads to.
    pub fn feat_choices(&mut self, id: ObjId) -> Vec<Vec<Choice>> {
        let Some(class) = self.objects.get(&id).map(|o| o.class) else {
            return Vec::new();
        };
        let (Some(col), Some(t)) = (self.feat_column(class, "list"), self.table("feat")) else {
            return Vec::new();
        };
        let listed: Vec<usize> = (0..t.row_count())
            .filter(|&r| {
                matches!(t.int(r, &col), Some(0 | 1 | 3))
                    && t.int(r, "name").is_some()
                    && !t.get(r, "label").unwrap_or("").starts_with("XXXX")
            })
            .collect();
        let next = |f: usize| {
            listed.iter().copied().find(|&r| {
                let (a, b) = (t.int(r, "prereqfeat1"), t.int(r, "prereqfeat2"));
                b == Some(f as i64) || (a == Some(f as i64) && b.is_none())
            })
        };
        let chains = chains(&listed, next);
        chains
            .iter()
            .map(|chain| chain.iter().map(|r| self.feat_choice(id, &t, *r)).collect())
            .collect()
    }

    fn feat_choice(&mut self, id: ObjId, t: &TwoDa, row: usize) -> Choice {
        let feat = row as i32;
        Choice {
            id: feat,
            name: t.int(row, "name").unwrap_or(0) as u32,
            description: t.int(row, "description").unwrap_or(0) as u32,
            icon: t.get(row, "icon").unwrap_or("").to_ascii_lowercase(),
            owned: self
                .objects
                .get(&id)
                .is_some_and(|o| o.feats.contains(&feat)),
            can_take: self.can_take_feat(id, feat, 1) || self.can_take_feat(id, feat, 0),
        }
    }

    /// Take a feat with a slot that holds it: the slot of its own list first, then the other.
    pub fn choose_feat(&mut self, id: ObjId, feat: i32) -> Result<(), String> {
        let regular_list = self.can_take_feat(id, feat, 1);
        if !regular_list && !self.can_take_feat(id, feat, 0) {
            return Err(format!("feat {feat} cannot be taken"));
        }
        let o = self.objects.get_mut(&id).ok_or("no such creature")?;
        let (reg, bon) = o.feat_slots;
        let bonus = match (regular_list, reg > 0, bon > 0) {
            (_, false, false) => return Err("no feat slot left".to_string()),
            (true, true, _) | (false, true, false) => false,
            _ => true,
        };
        if bonus {
            o.feat_slots.1 -= 1;
        } else {
            o.feat_slots.0 -= 1;
        }
        o.feats.insert(feat);
        self.pay_toughness(id, feat);
        Ok(())
    }

    /// The next feat the class's own order recommends, of those that can be taken.
    pub fn recommended_feat(&mut self, id: ObjId) -> Option<i32> {
        let class = self.objects.get(&id)?.class;
        let (col, t) = (self.feat_column(class, "recom")?, self.table("feat")?);
        let mut order: Vec<(i64, usize)> = (0..t.row_count())
            .filter_map(|r| Some((t.int(r, &col).filter(|n| *n > 0)?, r)))
            .collect();
        order.sort_unstable();
        let found = order.into_iter().map(|(_, r)| r as i32);
        found
            .into_iter()
            .find(|f| self.can_take_feat(id, *f, 1) || self.can_take_feat(id, *f, 0))
    }

    /// The Force powers a class can ever have, as chains from a power to what it leads to.
    pub fn power_choices(&mut self, id: ObjId) -> Vec<Vec<Choice>> {
        let Some(class) = self.objects.get(&id).map(|o| o.class) else {
            return Vec::new();
        };
        let (Some(col), Some(t)) = (self.power_level_column(class), self.table("spells")) else {
            return Vec::new();
        };
        let listed: Vec<usize> = (0..t.row_count())
            .filter(|&r| t.int(r, "usertype") == Some(POWER_USER_TYPE) && t.int(r, col).is_some())
            .collect();
        let next = |p: usize| {
            listed
                .iter()
                .copied()
                .find(|&r| prerequisites(&t, r).last() == Some(&p))
        };
        let chains = chains(&listed, next);
        chains
            .iter()
            .map(|chain| {
                chain
                    .iter()
                    .map(|r| self.power_choice(id, &t, *r))
                    .collect()
            })
            .collect()
    }

    fn power_choice(&mut self, id: ObjId, t: &TwoDa, row: usize) -> Choice {
        let power = row as i32;
        Choice {
            id: power,
            name: t.int(row, "name").unwrap_or(0) as u32,
            description: t.int(row, "spelldesc").unwrap_or(0) as u32,
            icon: t.get(row, "iconresref").unwrap_or("").to_ascii_lowercase(),
            owned: self
                .objects
                .get(&id)
                .is_some_and(|o| o.spells.contains(&power)),
            can_take: self.can_take_power(id, power),
        }
    }

    /// The class has the power, at this level, with the powers it needs and not this one.
    pub fn can_take_power(&mut self, id: ObjId, power: i32) -> bool {
        let Some((class, level, owned)) = self
            .objects
            .get(&id)
            .map(|o| (o.class, o.level, o.spells.clone()))
        else {
            return false;
        };
        let (Some(col), Some(t)) = (self.power_level_column(class), self.table("spells")) else {
            return false;
        };
        let row = power.max(0) as usize;
        power >= 0
            && !owned.contains(&power)
            && t.int(row, "usertype") == Some(POWER_USER_TYPE)
            && t.int(row, col).is_some_and(|l| l as i32 <= level)
            && prerequisites(&t, row)
                .iter()
                .all(|p| owned.contains(&(*p as i32)))
    }

    /// The next power `spells.2da` recommends for a creature on the light or the dark side.
    pub fn recommended_power(&mut self, id: ObjId) -> Option<i32> {
        let alignment = self.objects.get(&id)?.alignment;
        let t = self.table("spells")?;
        let col = if alignment >= 50 {
            "light_recom"
        } else {
            "dark_recom"
        };
        let mut order: Vec<(i64, usize)> = (0..t.row_count())
            .filter_map(|r| Some((t.int(r, col)?, r)))
            .collect();
        order.sort_unstable();
        let found = order.into_iter().map(|(_, r)| r as i32);
        found.into_iter().find(|p| self.can_take_power(id, *p))
    }

    /// The skills in the order the class recommends them.
    pub fn recommended_skills(&mut self, class: i32) -> Vec<usize> {
        let (Some(a), Some(t)) = (self.class_abbr(class), self.table("skills")) else {
            return Vec::new();
        };
        let column = format!("{a}_reco");
        let mut order: Vec<(i64, usize)> = (0..t.row_count())
            .filter_map(|r| Some((t.int(r, &column)?, r)))
            .collect();
        order.sort_unstable();
        order.into_iter().map(|(_, r)| r).collect()
    }

    /// The ability `classes.2da` calls the class's first, as a place in `abilities`.
    pub fn primary_ability(&mut self, class: i32) -> usize {
        let name = self
            .table("classes")
            .and_then(|t| Some(t.get(class.max(0) as usize, "primaryabil")?.to_string()));
        match name.as_deref() {
            Some("DEX") => 1,
            Some("CON") => 2,
            Some("INT") => 3,
            Some("WIS") => 4,
            Some("CHA") => 5,
            _ => 0,
        }
    }

    /// The words about a skill.
    pub fn skill_description(&mut self, skill: usize) -> u32 {
        self.table("skills")
            .and_then(|t| t.int(skill, "description"))
            .unwrap_or(0) as u32
    }

    /// Take a rank back, with its points.
    pub fn refund_skill_rank(&mut self, id: ObjId, skill: usize) {
        let Some((cost, _)) = self.skill_price(id, skill) else {
            return;
        };
        if let Some(o) = self.objects.get_mut(&id) {
            if let Some(rank) = o.skills.get_mut(skill).filter(|r| **r > 0) {
                *rank -= 1;
                o.skill_points += cost;
            }
        }
    }

    /// What raised abilities pay: Constitution for every level so far, Intelligence for this level's skill points.
    pub fn settle_abilities(&mut self, id: ObjId, before: [i32; 6]) {
        let Some(o) = self.objects.get_mut(&id) else {
            return;
        };
        let gained =
            |ability: usize| ability_mod(o.abilities[ability]) - ability_mod(before[ability]);
        let hp = gained(2) * o.level;
        let skill_points = gained(3);
        o.max_hp += hp;
        o.hp += hp;
        o.skill_points += skill_points;
    }
}
