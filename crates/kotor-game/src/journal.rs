//! The journal: `global.jrl` says what each quest and state are called, and `World::journal` where the party has got to.
//!
//! **Assumed, not compared with the game's code:** a quest whose state's entry has `End` set is completed; a state the
//! file does not have shows the quest with no words; conversations only move a quest forward; and a quest moves to the
//! front of "order received" when its state changes, as the reference video at 51:30 has the newest first.

use crate::world::World;
use kotor_core::ResType;
use kotor_gff::Gff;
use std::rc::Rc;

const JOURNAL_FILE: ResType = ResType(2056);

/// One quest the party has heard of, for the journal screen.
#[derive(Clone, Debug, PartialEq)]
pub struct Quest {
    pub tag: String,
    pub name: String,
    /// The words of the state it has got to.
    pub text: String,
    /// The talk table's name for its planet.
    pub planet: Option<u32>,
    pub priority: i32,
    pub completed: bool,
    /// 0 is the newest.
    pub received: usize,
}

impl World {
    /// Set a quest's state and put it first among those received.
    pub(crate) fn set_journal(&mut self, tag: &str, state: i32) {
        self.journal.insert(tag.to_string(), state);
        self.journal_order.retain(|t| t != tag);
        self.journal_order.insert(0, tag.to_string());
    }

    fn journal_file(&mut self) -> Option<Rc<Gff>> {
        if self.journal_file.is_none() {
            let g = self.inst.read("global", JOURNAL_FILE).ok()?;
            self.journal_file = Some(Rc::new(Gff::parse(&g).ok()?));
        }
        self.journal_file.clone()
    }

    /// Every quest's tag in the file.
    pub fn quest_tags(&mut self) -> Vec<String> {
        let Some(file) = self.journal_file() else {
            return Vec::new();
        };
        let categories = file.root.list("Categories").unwrap_or_default();
        let tags = categories.iter().filter_map(|c| c.str("Tag"));
        tags.map(|t| t.to_string()).collect()
    }

    /// The quests the party has heard of, newest first. A tag the file does not have is left out.
    pub fn quests(&mut self) -> Vec<Quest> {
        let Some(file) = self.journal_file() else {
            return Vec::new();
        };
        let planets = self.table("planetary");
        let categories = file.root.list("Categories").unwrap_or_default();
        let received = |tag: &str| {
            self.journal_order
                .iter()
                .position(|t| t.eq_ignore_ascii_case(tag))
                .unwrap_or(usize::MAX)
        };
        let mut out: Vec<Quest> = categories
            .iter()
            .filter_map(|c| {
                let tag = c.str("Tag")?;
                let state = self
                    .journal
                    .iter()
                    .find(|(t, _)| t.eq_ignore_ascii_case(tag))
                    .map(|(_, s)| *s)?;
                let entries = c.list("EntryList").unwrap_or_default();
                let entry = entries.iter().find(|e| e.int("ID") == Some(state as i64));
                let planet = c.int("PlanetID").filter(|p| *p >= 0).and_then(|p| {
                    planets
                        .as_ref()
                        .and_then(|t| t.int(p as usize, "name"))
                        .map(|n| n as u32)
                });
                Some(Quest {
                    tag: tag.to_string(),
                    name: c.loc("Name").map_or(String::new(), |l| self.text(l)),
                    text: entry
                        .and_then(|e| e.loc("Text"))
                        .map_or(String::new(), |l| self.text(l)),
                    planet,
                    priority: c.int("Priority").unwrap_or(0) as i32,
                    completed: entry.and_then(|e| e.int("End")).is_some_and(|n| n != 0),
                    received: received(tag),
                })
            })
            .collect();
        // The ones the player has killed: a line each, newest, finished.
        for (tag, name) in self.lost_quests.iter().rev() {
            out.push(Quest {
                tag: format!("law_{tag}"),
                name: format!("Lost: {name}"),
                text: format!("You killed {name}. Whatever {name} had for you will not be done."),
                planet: None,
                priority: 0,
                completed: true,
                received: 0,
            });
        }
        out.sort_by_key(|q| q.received);
        out
    }
}
