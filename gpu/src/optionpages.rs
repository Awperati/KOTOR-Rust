//! `optfeedback.gui` and `optautopause.gui`, from the pause menu or the main menu's options, and the words beside them.
//! What each box says comes from the talk table. **Assumed:** which boxes start ticked: the video pauses for an enemy, and
//! the others I could not see. Mine Sighted is greyed: there are no mines to sight.

use super::{Screen, Settings, Stage};
use kotor_ui::{Hit, MenuView, Row};

/// A row of the Feedback list: its name and words. Controller Vibration has no use with a mouse.
const FEEDBACK: [(u32, u32); 5] = [
    (42279, 42286),
    (42280, 42287),
    (42281, 42288),
    (42283, 42289),
    (42285, 42291),
];
/// A box on the Auto-Pause page, and its words.
const AUTOPAUSE: [(&str, u32); 6] = [
    ("CB_ENDROUND", 42445),
    ("CB_ENEMYSIGHTED", 42446),
    ("CB_MINESIGHTED", 49117),
    ("CB_PARTYKILLED", 42447),
    ("CB_ACTIONMENU", 48416),
    ("CB_TRIGGERS", 48214),
];
/// Words for the boxes on the Gameplay page that do something new.
const GAMEPLAY_WORDS: [(&str, u32); 2] = [("CB_LEVELUP", 42266), ("CB_DISABLEMOVE", 42484)];

fn feedback_flag(s: &mut Settings, row: usize) -> Option<&mut bool> {
    Some(match row {
        0 => &mut s.hide_unequippable,
        1 => &mut s.tutorials,
        2 => &mut s.subtitles,
        3 => &mut s.minimap,
        4 => &mut s.floating_numbers,
        _ => return None,
    })
}

fn autopause_flag<'a>(s: &'a mut Settings, tag: &str) -> Option<&'a mut bool> {
    Some(match tag {
        "CB_ENDROUND" => &mut s.pause_round,
        "CB_ENEMYSIGHTED" => &mut s.pause_enemy,
        "CB_PARTYKILLED" => &mut s.pause_party,
        "CB_ACTIONMENU" => &mut s.pause_action,
        "CB_TRIGGERS" => &mut s.pause_target,
        _ => return None,
    })
}

impl Stage {
    /// The words of what the cursor last rested on, in a page's list for them.
    fn option_words(&mut self, v: &mut MenuView, list: &str) {
        match self.menus.option_words {
            Some(words) => {
                let text = self.ui.strref(words);
                v.text(list, text);
            }
            None => v.hide(&[list]),
        }
    }

    pub(super) fn gameplay_words(&mut self, v: &mut MenuView) {
        self.option_words(v, "LB_DESC");
    }

    pub(super) fn feedback_view(&mut self) -> MenuView {
        let mut v = MenuView::new("optfeedback");
        let mut settings = self.menus.settings;
        let rows = FEEDBACK
            .iter()
            .enumerate()
            .map(|(n, (name, _))| Row {
                text: self.ui.strref(*name),
                selected: feedback_flag(&mut settings, n).is_some_and(|f| *f),
                ..Row::default()
            })
            .collect();
        v.lists.insert("LB_OPTIONS".to_string(), (rows, 0));
        self.option_words(&mut v, "LB_DESC");
        v
    }

    pub(super) fn autopause_view(&mut self) -> MenuView {
        let mut v = MenuView::new("optautopause");
        let mut settings = self.menus.settings;
        for (tag, _) in AUTOPAUSE {
            let on = autopause_flag(&mut settings, tag).is_some_and(|f| *f);
            v.value(tag, on as i32, 1);
        }
        v.inert(&["CB_MINESIGHTED"]);
        self.option_words(&mut v, "LB_DETAILS");
        v
    }

    /// The cursor moved over an option page: what is under it says its words.
    pub(super) fn option_hover(&mut self, screen: Screen) {
        let words = match (screen, &self.menus.hover) {
            (Screen::Feedback, Some(Hit::Row(_, row))) => FEEDBACK.get(*row).map(|f| f.1),
            (Screen::AutoPause, Some(Hit::Button(tag))) => {
                AUTOPAUSE.iter().find(|a| a.0 == tag).map(|a| a.1)
            }
            (Screen::Gameplay, Some(Hit::Button(tag))) => {
                GAMEPLAY_WORDS.iter().find(|a| a.0 == tag).map(|a| a.1)
            }
            _ => None,
        };
        if words.is_some() {
            self.menus.option_words = words;
        }
    }

    /// A click on a row of the Feedback list.
    pub(super) fn feedback_row(&mut self, row: usize) {
        if let Some(flag) = feedback_flag(&mut self.menus.settings, row) {
            *flag = !*flag;
        }
        self.settings_changed();
    }

    pub(super) fn option_page_press(&mut self, screen: Screen, tag: &str) {
        match tag {
            "BTN_BACK" => {
                self.menus.option_words = None;
                self.menus.stack.pop();
            }
            "BTN_DEFAULT" => {
                let d = Settings::default();
                let s = &mut self.menus.settings;
                if screen == Screen::Feedback {
                    s.hide_unequippable = d.hide_unequippable;
                    s.tutorials = d.tutorials;
                    s.subtitles = d.subtitles;
                    s.minimap = d.minimap;
                    s.floating_numbers = d.floating_numbers;
                } else {
                    s.pause_round = d.pause_round;
                    s.pause_enemy = d.pause_enemy;
                    s.pause_party = d.pause_party;
                    s.pause_action = d.pause_action;
                    s.pause_target = d.pause_target;
                }
                self.settings_changed();
            }
            _ => {
                if let Some(flag) = autopause_flag(&mut self.menus.settings, tag) {
                    *flag = !*flag;
                    self.settings_changed();
                }
            }
        }
    }
}
