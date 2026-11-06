//! `partyselection.gui`: who comes along. A script asks for it with `ShowPartySelectionGUI`, and the map's button opens it.
//! Nothing changes until OK, which asks first, as `confirm.gui` over the screen at 3:12:00. The reference video also has it
//! at 7:17:05. **Assumed:** a click shows someone and Add takes them: the video shows the screen, not the clicks.

use super::{Screen, Stage};
use kotor_game::{PartyRequest, PARTY_SIZE};
use kotor_ui::MenuView;

const STR_ADD: u32 = 38455;
const STR_REMOVE: u32 = 38456;
const STR_LEVEL: u32 = 32154;
const STR_SURE: u32 = 38328;
const STR_SOLO_ON: u32 = 37889;
const STR_SOLO_OFF: u32 = 37891;
const QUESTION_RISE: i32 = 10;
/// Who each slot is in the game, from the file's own `LBL_NA` pictures: these with a 2 are for those not on offer.
const PORTRAITS: [&str; 9] = [
    "po_pbastila",
    "po_pcanderous",
    "po_pcarth",
    "po_phk47",
    "po_pjolee",
    "po_pjuhani",
    "po_pmission",
    "po_pt3m3",
    "po_pzaalbar",
];

/// The party being chosen.
pub(super) struct Draft {
    chosen: Vec<i32>,
    /// The slot whose name is on show.
    shown: Option<i32>,
    request: Option<PartyRequest>,
    /// OK has asked whether this is the party wanted.
    asking: bool,
}

impl Stage {
    /// Open the screen, with the leader back as the character the player made.
    fn start_party(&mut self, request: Option<PartyRequest>) {
        let w = &mut self.game.world;
        w.switch_leader(w.hero);
        let mut chosen = w.party_slots();
        if let Some(r) = &request {
            for slot in r.forced.iter().filter(|s| w.available.contains_key(s)) {
                if !chosen.contains(slot) {
                    chosen.push(*slot);
                }
            }
        }
        self.menus.party = Some(Draft {
            chosen,
            shown: None,
            request,
            asking: false,
        });
        self.menus.stack.push(Screen::Party);
        self.menus.hover = None;
        self.tutorial(super::notices::TUTORIAL_PARTY);
    }

    /// The slot whose figure is on show.
    pub(super) fn party_figure_slot(&self) -> Option<i32> {
        self.menus.party.as_ref().and_then(|d| d.shown)
    }

    /// The party screen, from the map or a test.
    pub fn open_party(&mut self) {
        self.start_party(None);
    }

    /// A script has asked for the screen.
    pub(in crate::stage) fn take_party_request(&mut self) {
        if let Some(request) = self.game.world.party_request.take() {
            self.start_party(Some(request));
        }
    }

    pub(super) fn party_view_screen(&mut self) -> MenuView {
        let mut v = MenuView::new("partyselection");
        let Some((chosen, shown, forced, required, asking)) = self.menus.party.as_ref().map(|d| {
            let forced = d.request.as_ref().map_or(Vec::new(), |r| r.forced.clone());
            let required = d.request.is_some();
            (d.chosen.clone(), d.shown, forced, required, d.asking)
        }) else {
            return v;
        };
        for (slot, portrait) in PORTRAITS.iter().enumerate() {
            let n = slot as i32;
            let w = &self.game.world;
            let offered = w.available.contains_key(&n) && !w.unselectable.contains(&n);
            let (tag_char, tag_na, tag_button) = (
                format!("LBL_CHAR{slot}"),
                format!("LBL_NA{slot}"),
                format!("BTN_NPC{slot}"),
            );
            if offered {
                v.hide(&[&tag_na]);
                let own = self.game.world.party_portrait(n);
                v.fill(&tag_char, own.unwrap_or_else(|| portrait.to_string()));
                v.value(&tag_button, chosen.contains(&n) as i32, 1);
            } else {
                v.hide(&[&tag_char, &tag_button]);
            }
        }
        let places = PARTY_SIZE.saturating_sub(chosen.len());
        v.text("LBL_COUNT", places.to_string());
        if required {
            v.inert(&["BTN_BACK"]);
        }
        let word = if shown.is_some_and(|s| chosen.contains(&s)) {
            STR_REMOVE
        } else {
            STR_ADD
        };
        let word = self.ui.strref(word);
        v.text("BTN_ACCEPT", word);
        let can_change = shown.is_some_and(|s| {
            if chosen.contains(&s) {
                !forced.contains(&s)
            } else {
                places > 0
            }
        });
        if !can_change {
            v.inert(&["BTN_ACCEPT"]);
        }
        let about = shown.and_then(|s| self.game.world.party_candidate(s));
        match (shown, about) {
            (Some(_), Some((name, level, class))) => {
                v.holes.push("LBL_3D".to_string());
                v.text("LBL_NPC_NAME", name);
                let label = self.ui.strref(STR_LEVEL);
                let class = self.ui.strref(class);
                v.text("LBL_NPC_LEVEL", format!("{label} {level} {class}"));
            }
            _ => {
                v.hide(&["LBL_3D"]);
                v.text("LBL_NPC_NAME", "");
                v.text("LBL_NPC_LEVEL", "");
            }
        }
        if asking {
            let mut d = MenuView::new("confirm");
            d.text("LB_MESSAGE", self.ui.strref(STR_SURE));
            // The file's font runs off the box, and the video's much smaller letters fit on a line. Here it takes two.
            d.font("LB_MESSAGE", "fnt_d16x16");
            d.patches.entry("LB_MESSAGE".to_string()).or_default().rise = QUESTION_RISE;
            d.hover = self.menus.hover.clone();
            v.dialog = Some(Box::new(d));
        }
        v
    }

    /// The key for Solo Mode, and the button that shows while it is on: the game's question, then the change.
    /// **Assumed:** it asks. The talk table has the question at 37889 and 37891, and I have no frame of it.
    pub fn solo_key(&mut self) {
        if !self.menu_open() && !self.game.world.in_dialogue() {
            self.menus.stack.push(Screen::Solo);
            self.menus.hover = None;
        }
    }

    pub(super) fn solo_view(&mut self) -> MenuView {
        let mut v = MenuView::new("confirm");
        let question = if self.game.world.solo_mode {
            STR_SOLO_OFF
        } else {
            STR_SOLO_ON
        };
        v.text("LB_MESSAGE", self.ui.strref(question));
        v.font("LB_MESSAGE", "fnt_d16x16");
        v.patches.entry("LB_MESSAGE".to_string()).or_default().rise = QUESTION_RISE;
        v.over_scene = true;
        v
    }

    pub(super) fn solo_press(&mut self, tag: &str) {
        if tag == "BTN_OK" {
            let w = &mut self.game.world;
            w.solo_mode = !w.solo_mode;
        }
        self.menus.stack.pop();
        self.menus.hover = None;
    }

    fn close_party(&mut self, keep: bool) {
        let Some(d) = self.menus.party.take() else {
            return;
        };
        self.menus.stack.pop();
        let script = d.request.map_or(String::new(), |r| r.script);
        if keep {
            self.game.world.choose_party(&d.chosen, &script);
        }
    }

    pub(super) fn party_press(&mut self, tag: &str) {
        let asking = self.menus.party.as_ref().is_some_and(|d| d.asking);
        match (asking, tag) {
            (true, "BTN_OK") => {
                self.close_party(true);
                return;
            }
            (true, _) => {
                if let Some(d) = self.menus.party.as_mut() {
                    d.asking = false;
                }
                return;
            }
            _ => {}
        }
        match tag {
            "BTN_DONE" => {
                if let Some(d) = self.menus.party.as_mut() {
                    d.asking = true;
                }
                self.menus.hover = None;
            }
            "BTN_BACK" => {
                let required = self
                    .menus
                    .party
                    .as_ref()
                    .is_some_and(|d| d.request.is_some());
                if !required {
                    self.close_party(false);
                }
            }
            "BTN_ACCEPT" => {
                let Some(d) = self.menus.party.as_mut() else {
                    return;
                };
                let Some(slot) = d.shown else { return };
                let forced = d.request.as_ref().is_some_and(|r| r.forced.contains(&slot));
                if d.chosen.contains(&slot) {
                    if !forced {
                        d.chosen.retain(|s| *s != slot);
                    }
                } else if d.chosen.len() < PARTY_SIZE {
                    d.chosen.push(slot);
                }
            }
            _ => {
                let slot = tag.strip_prefix("BTN_NPC").and_then(|n| n.parse().ok());
                if let (Some(d), Some(slot)) = (self.menus.party.as_mut(), slot) {
                    d.shown = Some(slot);
                }
            }
        }
    }
}
