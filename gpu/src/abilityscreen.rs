//! `abilities.gui` and `messages.gui`, the two tabs of the party's screens: what the leader can do, and what has been said
//! and fought. Words and pictures are the tables' and the talk table's.

use super::{Screen, Stage};
use kotor_game::{Choice, LineKind, Outcome};
use kotor_ui::{Hit, MenuView, Row};

const TAB_BUTTONS: [&str; 3] = ["BTN_SKILLS", "BTN_POWERS", "BTN_FEATS"];
const SKILLS_TAB: usize = 0;
const POWERS_TAB: usize = 1;
const NUMBERS: [&str; 6] = [
    "LBL_SKILLRANK",
    "LBL_RANKVAL",
    "LBL_BONUS",
    "LBL_BONUSVAL",
    "LBL_TOTAL",
    "LBL_TOTALVAL",
];
const STR_SKILL_RANK: u32 = 1579;
const STR_BONUS: u32 = 32129;
const STR_SHOW_DIALOG: u32 = 42143;
const STR_SHOW_FEEDBACK: u32 = 42142;
const STR_HIT: u32 = 42133;
const STR_MISS: u32 = 42134;
const STR_CRITICAL: u32 = 1390;
const STR_DAMAGE: u32 = 31385;
const MESSAGES_GUI: &str = "messages";
const DIALOG_LIST: &str = "LB_DIALOG";
const FEEDBACK_LIST: &str = "LB_MESSAGES";

impl Stage {
    /// The tab that is up and its rows: a member with no powers has no such tab.
    fn ability_rows(&mut self) -> (usize, Vec<Choice>) {
        let w = &mut self.game.world;
        let pc = w.pc;
        let tab = match self.menus.abilities_tab {
            POWERS_TAB if w.owned_powers(pc).is_empty() => SKILLS_TAB,
            tab => tab,
        };
        let rows = match tab {
            SKILLS_TAB => w.skill_choices(),
            POWERS_TAB => w.owned_powers(pc),
            _ => w.owned_feats(pc),
        };
        (tab, rows)
    }

    pub(super) fn abilities_view(&mut self) -> MenuView {
        let mut v = MenuView::new("abilities");
        self.change_buttons(&mut v);
        v.fill("LBL_PORTRAIT", self.portrait());
        let (tab, rows) = self.ability_rows();
        let pc = self.game.world.pc;
        if self.game.world.owned_powers(pc).is_empty() {
            v.inert(&["BTN_POWERS"]);
        }
        v.lit(TAB_BUTTONS[tab]);
        let selected = self.menus.selected;
        let listed = rows
            .iter()
            .enumerate()
            .map(|(n, c)| Row {
                text: self.ui.strref(c.name),
                icon: Some(c.icon.clone()),
                selected: selected == Some(n),
                ..Row::default()
            })
            .collect();
        v.lists
            .insert("LB_ABILITY".to_string(), (listed, self.menus.first));
        let under = match self.menus.hover {
            Some(Hit::Row(_, n)) => Some(n),
            _ => None,
        };
        let Some(shown) = under.or(selected).and_then(|n| rows.get(n)) else {
            v.hide(&NUMBERS);
            v.text("LBL_NAME", "");
            return v;
        };
        v.text("LBL_NAME", self.ui.strref(shown.name));
        v.text("LB_DESC", self.ui.strref(shown.description));
        match tab {
            SKILLS_TAB => {
                let w = &mut self.game.world;
                let skill = shown.id as usize;
                let (ranks, bonus) = (w.skill_ranks(pc, skill), w.skill_modifier(pc, skill));
                v.text("LBL_SKILLRANK", self.ui.strref(STR_SKILL_RANK));
                v.text("LBL_RANKVAL", ranks.to_string());
                v.text("LBL_BONUS", self.ui.strref(STR_BONUS));
                v.text("LBL_BONUSVAL", format!("{bonus:+}"));
                v.text("LBL_TOTALVAL", (ranks + bonus).to_string());
            }
            POWERS_TAB => {
                let cost = self.game.world.spell_cost_and_range(shown.id).map(|c| c.0);
                v.text("LBL_RANKVAL", cost.unwrap_or(0).to_string());
                v.hide(&["LBL_BONUS", "LBL_BONUSVAL", "LBL_TOTAL", "LBL_TOTALVAL"]);
            }
            _ => v.hide(&NUMBERS),
        }
        v
    }

    pub(super) fn messages_view(&mut self) -> MenuView {
        let mut v = MenuView::new(MESSAGES_GUI);
        let (shown, other, button) = if self.menus.messages_feedback {
            (FEEDBACK_LIST, DIALOG_LIST, STR_SHOW_DIALOG)
        } else {
            (DIALOG_LIST, FEEDBACK_LIST, STR_SHOW_FEEDBACK)
        };
        v.hide(&[other]);
        v.text("BTN_SHOW", self.ui.strref(button));
        v.text(shown, self.messages_words());
        let list = v.patches.entry(shown.to_string()).or_default();
        list.hidden = false;
        list.from_end = Some(self.menus.message_back);
        v
    }

    /// What the Messages tab has to show: the conversations, or the attacks the party had a part in.
    pub fn messages_words(&mut self) -> String {
        let lines: Vec<String> = if self.menus.messages_feedback {
            let w = &self.game.world;
            let ours = |id| id == w.pc || w.party.contains(&id);
            let log: Vec<_> = w
                .combat
                .log
                .iter()
                .filter(|a| ours(a.attacker) || ours(a.target))
                .cloned()
                .collect();
            log.iter()
                .map(|a| {
                    let (outcome, damage) = match a.outcome {
                        Outcome::Miss => (STR_MISS, false),
                        Outcome::Hit => (STR_HIT, true),
                        Outcome::Critical => (STR_CRITICAL, true),
                    };
                    let words = format!(
                        "{} > {}: {}, {} + {} / {}",
                        self.named(a.attacker),
                        self.named(a.target),
                        self.ui.strref(outcome),
                        a.roll,
                        a.bonus,
                        a.defense,
                    );
                    if damage {
                        format!("{words}. {}: {}", self.ui.strref(STR_DAMAGE), a.damage)
                    } else {
                        words
                    }
                })
                .collect()
        } else {
            let player = self.named(self.game.world.pc);
            let said = &self.game.world.transcript;
            said.iter()
                .map(|l| match (l.kind, l.speaker.is_empty()) {
                    (LineKind::Player, _) => format!("{player}: {}", l.text),
                    (LineKind::Npc, true) => l.text.clone(),
                    (LineKind::Npc, false) => format!("{}: {}", l.speaker, l.text),
                })
                .collect()
        };
        lines.join("\n")
    }

    fn named(&self, id: kotor_game::ObjId) -> String {
        self.game
            .world
            .object(id)
            .map_or(String::new(), |o| o.name.clone())
    }

    /// The wheel over the Messages tab: up goes back through what has been said.
    pub(super) fn messages_wheel(&mut self, lines: f32) {
        let words = self.messages_words();
        let list = if self.menus.messages_feedback {
            FEEDBACK_LIST
        } else {
            DIALOG_LIST
        };
        let (total, fits) = self.ui.menu_text_lines(MESSAGES_GUI, list, &words);
        let last = total.saturating_sub(fits) as i32;
        self.menus.message_back =
            (self.menus.message_back as i32 + lines.signum() as i32).clamp(0, last) as usize;
    }

    pub(super) fn ability_press(&mut self, tag: &str) {
        match tag {
            "BTN_EXIT" => {
                self.menus.stack.pop();
            }
            "BTN_SHOW" => {
                self.menus.messages_feedback = !self.menus.messages_feedback;
                self.menus.message_back = 0;
            }
            "BTN_CHANGE1" => self.change_member(0),
            "BTN_CHANGE2" => self.change_member(1),
            _ => {
                let pc = self.game.world.pc;
                let greyed =
                    tag == TAB_BUTTONS[POWERS_TAB] && self.game.world.owned_powers(pc).is_empty();
                if let Some(n) = TAB_BUTTONS.iter().position(|t| *t == tag && !greyed) {
                    self.menus.abilities_tab = n;
                    self.menus.selected = Some(0);
                    self.menus.first = 0;
                }
            }
        }
    }

    /// The tabs' own screens, for keys.
    pub fn open_abilities(&mut self) {
        self.open_items(Screen::Abilities);
    }

    pub fn open_messages(&mut self) {
        self.open_items(Screen::Messages);
    }
}
