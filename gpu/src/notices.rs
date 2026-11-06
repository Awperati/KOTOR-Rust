//! Tutorial pop-ups and the icons that flash at the top of the HUD, as the reference video has them from 3:15. What set
//! off an icon is worked out by comparing what the party has with what it had.
//!
//! **Assumed:** an icon shows for `NOTICE_SECONDS`, which I did not measure. **Not built:** a pop-up over the game hides
//! the HUD under it, which the video keeps, and `Hostile_Creature_Encountered` does not pause the game as the video does.

use super::{Screen, Stage};

const STR_CONTINUE: u32 = 38623;
const STR_OK: u32 = 1580;
const NOTICE_SECONDS: f32 = 6.0;

/// Rows of `tutorial.2da`, by what they say they are for.
pub(super) const TUTORIAL_EQUIP_SLOT: i32 = 11;
pub(super) const TUTORIAL_MAP: i32 = 13;
const TUTORIAL_JOURNAL: i32 = 14;
const TUTORIAL_PARTY_DIES: i32 = 15;
pub(super) const TUTORIAL_LEVEL_ABILITIES: i32 = 16;
pub(super) const TUTORIAL_INVENTORY: i32 = 20;
const TUTORIAL_HOSTILE: i32 = 21;
const TUTORIAL_FLASH_JOURNAL: i32 = 22;
const TUTORIAL_FLASH_CREDITS: i32 = 23;
const TUTORIAL_FLASH_XP: i32 = 24;
const TUTORIAL_FLASH_DARK: i32 = 25;
const TUTORIAL_FLASH_LIGHT: i32 = 26;
const TUTORIAL_FLASH_GAINED: i32 = 27;
const TUTORIAL_FLASH_LOST: i32 = 28;
const TUTORIAL_ATTACK: i32 = 34;
pub(super) const TUTORIAL_LEVEL_SKILLS: i32 = 36;
pub(super) const TUTORIAL_LEVEL_FEATS: i32 = 37;
pub(super) const TUTORIAL_LEVEL_POWERS: i32 = 38;
pub(super) const TUTORIAL_PARTY: i32 = 41;

/// What the party had when last looked at.
#[derive(Clone, PartialEq)]
pub(super) struct Had {
    journal: Vec<(String, i32)>,
    gold: i32,
    plot_xp: i32,
    alignment: i32,
    items: i32,
}

impl Stage {
    /// Tutorial pop-ups come up, as the game's do. Off unless asked for: tests and the scripted player do not read them.
    pub fn enable_tutorials(&mut self, on: bool) {
        self.menus.tutorials_on = on;
    }

    /// This pop-up comes up next, if it has not been shown.
    pub(super) fn tutorial(&mut self, row: i32) {
        if self.menus.tutorials_on && self.menus.settings.tutorials {
            let pages = self.game.world.tutorial_once(row);
            self.menus.tutorials.extend(pages);
        }
    }

    fn party_had(&self) -> Had {
        let w = &self.game.world;
        let o = w.object(w.pc);
        let items = o.map_or(0, |o| {
            let held = o.inventory.iter().chain(o.equipped.values());
            held.filter_map(|i| w.object(*i))
                .map(|i| i.stack.max(1))
                .sum()
        });
        Had {
            journal: w.journal.iter().map(|(t, s)| (t.clone(), *s)).collect(),
            gold: w.gold,
            plot_xp: w.plot_xp,
            alignment: o.map_or(0, |o| o.alignment),
            items,
        }
    }

    /// What changed: a tag on the HUD to flash, and the pop-up that goes with it.
    fn flash(&mut self, tag: &'static str, row: i32) {
        let until = self.game.world.time + NOTICE_SECONDS;
        self.menus.notices.retain(|n| n.0 != tag);
        self.menus.notices.push((tag, until));
        self.tutorial(row);
    }

    /// Once a step: see what has changed, and bring up what is waiting.
    pub(in crate::stage) fn notices_tick(&mut self) {
        let asked = std::mem::take(&mut self.game.world.tutorial_requests);
        for row in asked {
            self.tutorial(row);
        }
        let now = self.party_had();
        let in_menu = self.menus.stack.last().is_some();
        if let Some(had) = self.menus.had.clone().filter(|h| *h != now) {
            if had.journal != now.journal {
                self.flash("LBL_JOURNAL", TUTORIAL_FLASH_JOURNAL);
                self.tutorial(TUTORIAL_JOURNAL);
            }
            if had.gold != now.gold {
                self.flash("LBL_CASH", TUTORIAL_FLASH_CREDITS);
            }
            if now.plot_xp > had.plot_xp {
                self.flash("LBL_PLOTXP", TUTORIAL_FLASH_XP);
            }
            if now.alignment > had.alignment {
                self.flash("LBL_LIGHTSHIFT", TUTORIAL_FLASH_LIGHT);
            } else if now.alignment < had.alignment {
                self.flash("LBL_DARKSHIFT", TUTORIAL_FLASH_DARK);
            }
            // On a screen, the player is the one moving things about.
            if !in_menu && now.items > had.items {
                self.flash("LBL_ITEMRCVD", TUTORIAL_FLASH_GAINED);
            } else if !in_menu && now.items < had.items {
                self.flash("LBL_ITEMLOST", TUTORIAL_FLASH_LOST);
            }
        }
        self.menus.had = Some(now);
        self.floats_tick();
        let w = &self.game.world;
        if self.menus.settings.auto_level
            && w.hold_levels
            && w.level_ready(w.pc)
            && !w.in_dialogue()
            && self.menus.stack.is_empty()
        {
            self.auto_level_up();
        }
        let time = self.game.world.time;
        self.menus.notices.retain(|n| n.1 > time);

        let w = &self.game.world;
        let lost = w
            .party
            .iter()
            .any(|id| w.object(*id).is_some_and(|o| o.dead));
        if lost {
            self.tutorial(TUTORIAL_PARTY_DIES);
        }
        if self.party_blocked() {
            self.tutorial(TUTORIAL_ATTACK);
        }
        if !self.menus.tutorials.is_empty()
            && self.top() != Some(Screen::Tutorial)
            && !self.game.world.in_dialogue()
        {
            self.menus.stack.push(Screen::Tutorial);
            self.menus.hover = None;
        }
    }

    /// An enemy is the target: its pop-up.
    pub(super) fn enemy_sighted(&mut self) {
        self.tutorial(TUTORIAL_HOSTILE);
    }

    /// The tags of the icons showing on the HUD.
    pub(in crate::stage) fn notices_view(&self) -> Vec<String> {
        self.menus.notices.iter().map(|n| n.0.to_string()).collect()
    }

    /// The words of the page showing, and what its button says.
    pub(super) fn tutorial_page(&mut self) -> kotor_ui::Notice {
        let page = self.menus.tutorials.front().copied().unwrap_or(0);
        let more = self.menus.tutorials.len() > 1;
        kotor_ui::Notice {
            text: self.ui.strref(page),
            button: self.ui.strref(if more { STR_CONTINUE } else { STR_OK }),
        }
    }

    /// The button on a pop-up, or Esc.
    pub(super) fn tutorial_next(&mut self) {
        self.menus.tutorials.pop_front();
        if self.menus.tutorials.is_empty() {
            self.menus.stack.pop();
        }
        self.menus.hover = None;
    }
}
