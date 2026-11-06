//! Feats the second game rebuilt, offered at level up as the owner asked. Their names, words and pictures stay in the
//! owner's own copy of that game and are read from it at run time, so without that copy they are simply not offered.
//! What each one does is written here, from those words.
//!
//! Rows go after the first game's in `feat.2da`, so nothing it has moves. A name's or a description's number is the second
//! game's plus `kotor_core::SECOND_GAME_STRREF`, which is how the screens know whose talk table to ask.
//!
//! Left out on purpose: Superior Two-Weapon Fighting (nothing attacks with the left hand yet), and what the words say about
//! deflecting bolts and about the +6 for hitting someone with a ranged weapon in melee: neither is in this game's rules.
//!
//! **Assumed:** short range is 5 m. The second game's words do not give it. Superior Weapon Focus belongs to its prestige
//! classes, which this game has not got, so here it is for any Jedi with Weapon Focus and Weapon Specialization: Lightsaber,
//! at levels 12, 15 and 18. Nothing in the second game gives those levels. A class it grants a feat to only chooses it here.

use crate::{ObjId, World};
use kotor_archive::Install;
use kotor_core::ResType;
use kotor_tables::TwoDa;

/// What stands for a feat this one needs.
#[derive(Clone, Copy)]
enum Needs {
    Nothing,
    /// A row of the first game's table, by its label.
    Label(&'static str),
    /// A place in `PORTS`.
    Port(usize),
}

struct Port {
    label: &'static str,
    level: i32,
    needs: [Needs; 2],
    /// For the Jedi classes alone, whatever the second game's lists say.
    jedi_only: bool,
}

const fn port(label: &'static str, level: i32, needs: [Needs; 2]) -> Port {
    Port {
        label,
        level,
        needs,
        jedi_only: false,
    }
}

const fn jedi(label: &'static str, level: i32, needs: [Needs; 2]) -> Port {
    Port {
        label,
        level,
        needs,
        jedi_only: true,
    }
}

const PRECISE: usize = 0;
const CLOSE: usize = 5;
const FINESSE_SABER: usize = 7;
const FINESSE_MELEE: usize = 8;
const SUPERIOR: usize = 9;

const NONE: [Needs; 2] = [Needs::Nothing, Needs::Nothing];

/// In the order a chain is read: the screens follow the last feat a row needs.
const PORTS: [Port; 12] = [
    port("PRECISE_SHOT_I", 4, NONE),
    port("PRECISE_SHOT_II", 8, [Needs::Port(0), Needs::Nothing]),
    port("PRECISE_SHOT_III", 12, [Needs::Port(0), Needs::Port(1)]),
    port("PRECISE_SHOT_IV", 16, [Needs::Port(1), Needs::Port(2)]),
    port("PRECISE_SHOT_V", 20, [Needs::Port(2), Needs::Port(3)]),
    port("CLOSE_COMBAT", 4, NONE),
    port("IMPROVED_CLOSE_COMBAT", 8, [Needs::Port(5), Needs::Nothing]),
    port("FINESSE_LIGHTSABERS", 0, NONE),
    port("FINESSE_MELEE_WEAPONS", 0, NONE),
    jedi(
        "SUPER_WEAPON_FOCUS_LIGHTSABER_I",
        12,
        [
            Needs::Label("WEAPON_FOCUS_LIGHTSABER"),
            Needs::Label("WEAPON_SPEC_LIGHTSABER"),
        ],
    ),
    jedi(
        "SUPER_WEAPON_FOCUS_LIGHTSABER_II",
        15,
        [Needs::Port(9), Needs::Nothing],
    ),
    jedi(
        "SUPER_WEAPON_FOCUS_LIGHTSABER_III",
        18,
        [Needs::Port(9), Needs::Port(10)],
    ),
];

/// More damage with a ranged weapon, by how far along Precise Shot a creature is.
const PRECISE_DAMAGE: [i32; 6] = [0, 1, 2, 4, 6, 8];
/// Metres.
const SHORT_RANGE: f32 = 5.0;
const CLASSES: [&str; 8] = ["sol", "sct", "scd", "jgd", "jcn", "jsn", "drx", "drc"];
const JEDI: [&str; 3] = ["jgd", "jcn", "jsn"];

/// Add the ported rows to the first game's table. Returns the first of them, or `None` with no copy of the second game.
pub(crate) fn add_second_game_feats(t: &mut TwoDa) -> Option<usize> {
    let mut inst = Install::open(kotor_core::second_game_dir()?).ok()?;
    let theirs = TwoDa::parse(&inst.read("feat", ResType::TWODA).ok()?).ok()?;
    add_rows(t, &theirs)
}

fn add_rows(t: &mut TwoDa, theirs: &TwoDa) -> Option<usize> {
    let row_called = |table: &TwoDa, label: &str| {
        (0..table.row_count()).find(|r| table.get(*r, "label") == Some(label))
    };
    let first = t.row_count();
    let mut rows = Vec::new();
    for p in &PORTS {
        let at = row_called(theirs, p.label)?;
        let shifted = |c: &str| {
            let n = theirs.int(at, c)? as u64 + u64::from(kotor_core::SECOND_GAME_STRREF);
            Some(n.to_string())
        };
        let mut cells = vec![
            ("label".to_string(), format!("K2_{}", p.label)),
            ("name".to_string(), shifted("name")?),
            ("description".to_string(), shifted("description")?),
            ("icon".to_string(), theirs.get(at, "icon")?.to_string()),
        ];
        if p.level > 0 {
            cells.push(("mincharlevel".to_string(), p.level.to_string()));
        }
        for (column, need) in ["prereqfeat1", "prereqfeat2"].into_iter().zip(p.needs) {
            let row = match need {
                Needs::Nothing => continue,
                Needs::Label(l) => row_called(t, l)?,
                Needs::Port(n) => first + n,
            };
            cells.push((column.to_string(), row.to_string()));
        }
        let mut named = Vec::new();
        for class in CLASSES {
            let column = format!("{class}_list");
            // 3 is granted in the second game's order, and 4 is not offered.
            let list = match (p.jedi_only, theirs.int(at, &column)) {
                (true, _) if JEDI.contains(&class) => 1,
                (true, _) | (_, None) | (_, Some(4)) => 4,
                (false, Some(0)) => 0,
                _ => 1,
            };
            named.push((column, list.to_string()));
        }
        let mut row = vec!["****".to_string(); t.columns.len()];
        for (column, value) in cells.iter().chain(&named) {
            if let Some(c) = t.columns.iter().position(|n| n == column) {
                row[c].clone_from(value);
            }
        }
        rows.push(row);
    }
    for row in rows {
        let cells: Vec<&str> = row.iter().map(String::as_str).collect();
        t.push_row(&cells);
    }
    Some(first)
}

impl World {
    /// How far along a run of ported feats a creature is: 0 with none of them, 1 with the first, and so on.
    fn ported_rank(&mut self, id: ObjId, from: usize, count: usize) -> usize {
        self.table("feat");
        let (Some(first), Some(o)) = (self.second_game_feats, self.objects.get(&id)) else {
            return 0;
        };
        (0..count)
            .rev()
            .find(|n| o.feats.contains(&((first + from + n) as i32)))
            .map_or(0, |n| n + 1)
    }

    /// More damage with a ranged weapon: Precise Shot.
    pub(crate) fn precise_shot_damage(&mut self, id: ObjId) -> i32 {
        PRECISE_DAMAGE[self.ported_rank(id, PRECISE, 5)]
    }

    /// More to hit with a ranged weapon at short range: Close Combat.
    pub(crate) fn close_combat_bonus(&mut self, id: ObjId, target: ObjId) -> i32 {
        if !self.hand_weapon(id, 4).is_some_and(|w| w.ranged) {
            return 0;
        }
        let (Some(a), Some(b)) = (self.objects.get(&id), self.objects.get(&target)) else {
            return 0;
        };
        let (dx, dy) = (a.position[0] - b.position[0], a.position[1] - b.position[1]);
        if dx.hypot(dy) > SHORT_RANGE {
            return 0;
        }
        self.ported_rank(id, CLOSE, 2) as i32
    }

    /// Whether what is in a slot is a lightsaber.
    fn holds_lightsaber(&mut self, id: ObjId, slot: i32) -> bool {
        let base = self
            .objects
            .get(&id)
            .and_then(|o| o.equipped.get(&slot))
            .and_then(|w| self.objects.get(w))
            .map(|w| w.base_item);
        let Some((base, t)) = base.zip(self.table("baseitems")) else {
            return false;
        };
        self.hand_weapon(id, slot).is_some()
            && t.get(base.max(0) as usize, "label")
                .is_some_and(|l| l.contains("Lightsaber"))
    }

    /// Whether a melee weapon in a slot may use Dexterity to hit: Finesse.
    pub(crate) fn has_finesse(&mut self, id: ObjId, slot: i32) -> bool {
        if self.hand_weapon(id, slot).is_none_or(|w| w.ranged) {
            return false;
        }
        let row = if self.holds_lightsaber(id, slot) {
            FINESSE_SABER
        } else {
            FINESSE_MELEE
        };
        self.ported_rank(id, row, 1) > 0
    }

    /// More to hit with a lightsaber: Superior Weapon Focus.
    pub(crate) fn superior_focus_bonus(&mut self, id: ObjId, slot: i32) -> i32 {
        if !self.holds_lightsaber(id, slot) {
            return 0;
        }
        self.ported_rank(id, SUPERIOR, 3) as i32
    }

    /// The rows of the feats the second game rebuilt, in `feat.2da`, if its copy was there to read. For a test.
    pub fn ported_feat_rows(&mut self) -> Option<std::ops::Range<i32>> {
        self.table("feat");
        let first = self.second_game_feats? as i32;
        Some(first..first + PORTS.len() as i32)
    }
}

// Just what a test needs of the rules above and of the module.
impl World {
    /// `close_combat_bonus` for a test.
    pub fn close_combat_for_test(&mut self, id: ObjId, target: ObjId) -> i32 {
        self.close_combat_bonus(id, target)
    }
}
