//! Pazaak's three screens, from the stock files: `pazaakwager.gui`, `pazaaksetup.gui` and `pazaakgame.gui`. They show
//! `World::pazaak_view` and answer with `World::pazaak_move`: the rules are `kotor_game::pazaak`.
//! **Not compared:** I found no game of Pazaak in the reference video. **Assumed:** a card is chosen with a click and goes
//! over with Add Card, and which picture goes with which kind of card is read from the names of the file's own pictures.

use super::Stage;
use kotor_game::{PazaakCard, PazaakMove, PazaakSide, PazaakView};
use kotor_ui::{MenuView, Notice};

const WAGER_GUI: &str = "pazaakwager";
const SETUP_GUI: &str = "pazaaksetup";
const GAME_GUI: &str = "pazaakgame";
const STR_MAXIMUM: u32 = 32321;
const STR_OK: u32 = 1580;
const PICTURE_MAIN: &str = "lbl_cardstand";
const PICTURE_PLUS: &str = "lbl_cardmpos";
const PICTURE_MINUS: &str = "lbl_cardmneg";
const PICTURE_EITHER: &str = "lbl_cardrarem";
const PICTURE_BACK: &str = "lbl_cardback";
const MARK_WON: &str = "lbl_winmark02";
const MARK_NOT_YET: &str = "lbl_winmark01";
/// Cards a side deck has, and a table each side, and sets to win.
const SIDE_DECK: usize = 10;
const TABLE_CARDS: usize = 9;
const HAND_CARDS: usize = 4;
const WINS: usize = 3;

fn picture(card: PazaakCard) -> &'static str {
    match card {
        PazaakCard::Main(_) => PICTURE_MAIN,
        PazaakCard::Plus(_) => PICTURE_PLUS,
        PazaakCard::Minus(_) => PICTURE_MINUS,
        PazaakCard::Either(_) => PICTURE_EITHER,
    }
}

fn card_words(card: PazaakCard) -> String {
    match card {
        PazaakCard::Main(n) => n.to_string(),
        PazaakCard::Plus(n) => format!("+{n}"),
        PazaakCard::Minus(n) => format!("-{n}"),
        PazaakCard::Either(n) => format!("{n:+}"),
    }
}

#[derive(Default)]
pub(super) struct Pazaak {
    /// The available card with a click on it, by column and row.
    picked: Option<(usize, usize)>,
}

/// The card a place in the setup's columns stands for.
fn available_card(column: usize, row: usize) -> PazaakCard {
    let n = row as i32 + 1;
    match column {
        0 => PazaakCard::Plus(n),
        1 => PazaakCard::Minus(n),
        _ => PazaakCard::Either(n),
    }
}

impl Stage {
    /// A pilot has nobody to play: for it the game is gone again, as it was before there was a game.
    pub(in crate::stage) fn take_pazaak_request(&mut self) {
        if self.pilot.is_some() {
            self.game.world.pazaak = None;
        }
    }

    pub(super) fn pazaak_view(&mut self) -> MenuView {
        match self.game.world.pazaak_view() {
            Some(PazaakView::Wager { value, maximum }) => self.pazaak_wager_view(value, maximum),
            Some(PazaakView::Setup { available, chosen }) => {
                self.pazaak_setup_view(available, &chosen)
            }
            Some(PazaakView::Table {
                player,
                opponent,
                message,
            }) => self.pazaak_table_view(&player, &opponent, message),
            None => MenuView::new(GAME_GUI),
        }
    }

    fn pazaak_wager_view(&mut self, value: i32, maximum: i32) -> MenuView {
        let mut v = MenuView::new(WAGER_GUI);
        v.over_scene = true;
        v.text("LBL_WAGERVAL", value.to_string());
        let words = self.ui.strref(STR_MAXIMUM);
        v.text("LBL_MAXIMUM", format!("{words} {maximum}"));
        if value <= 0 {
            v.inert(&["BTN_WAGER"]);
        }
        v
    }

    fn pazaak_setup_view(&mut self, available: [[i32; 6]; 3], chosen: &[PazaakCard]) -> MenuView {
        let mut v = MenuView::new(SETUP_GUI);
        let picked = self.menus.pazaak.picked;
        for (column, rows) in available.iter().enumerate() {
            for (row, count) in rows.iter().enumerate() {
                let (button, words, number) = (
                    format!("BTN_AVAIL{column}{row}"),
                    format!("LBL_AVAIL{column}{row}"),
                    format!("LBL_AVAILNUM{column}{row}"),
                );
                if *count <= 0 {
                    v.hide(&[button.as_str(), words.as_str(), number.as_str()]);
                    continue;
                }
                v.text(&words, card_words(available_card(column, row)));
                v.text(&number, count.to_string());
                if picked == Some((column, row)) {
                    v.lit(&words);
                }
            }
        }
        for n in 0..SIDE_DECK {
            let (button, words) = (format!("BTN_CHOSEN{n}"), format!("LBL_CHOSEN{n}"));
            match chosen.get(n) {
                Some(card) => {
                    v.fill(&button, picture(*card));
                    v.text(&words, card_words(*card));
                }
                None => v.fill(&button, PICTURE_BACK),
            }
        }
        if picked.is_none() || chosen.len() >= SIDE_DECK {
            v.inert(&["BTN_YTEXT"]);
        }
        if chosen.len() < SIDE_DECK {
            v.inert(&["BTN_ATEXT"]);
        }
        v
    }

    fn pazaak_table_view(
        &mut self,
        player: &PazaakSide,
        opponent: &PazaakSide,
        message: Option<u32>,
    ) -> MenuView {
        let mut v = MenuView::new(GAME_GUI);
        for (side, who) in [("PLR", player), ("NPC", opponent)] {
            for n in 0..TABLE_CARDS {
                let (button, words) = (format!("BTN_{side}{n}"), format!("LBL_{side}{n}"));
                match who.played.get(n) {
                    Some(card) => {
                        v.fill(&button, picture(*card));
                        v.text(&words, card_words(*card));
                    }
                    None => v.hide(&[button.as_str(), words.as_str()]),
                }
            }
            for n in 0..WINS {
                let mark = if n < who.wins { MARK_WON } else { MARK_NOT_YET };
                v.fill(&format!("LBL_{side}SCORE{n}"), mark);
            }
            v.text(&format!("LBL_{side}TOTAL"), who.total.to_string());
            if !who.to_play {
                v.hide(&[format!("LBL_{side}TURN").as_str()]);
            }
        }
        for n in 0..HAND_CARDS {
            let (button, words, flip) = (
                format!("BTN_PLRSIDE{n}"),
                format!("LBL_PLRSIDE{n}"),
                format!("BTN_FLIP{n}"),
            );
            match player.hand.get(n) {
                Some(card) => {
                    v.fill(&button, picture(*card));
                    v.text(&words, card_words(*card));
                    if !matches!(card, PazaakCard::Either(_)) {
                        v.hide(&[flip.as_str()]);
                    }
                }
                None => v.hide(&[button.as_str(), words.as_str(), flip.as_str()]),
            }
            let (back, gap) = (format!("BTN_NPCSIDE{n}"), format!("LBL_NPCSIDE{n}"));
            v.hide(&[gap.as_str()]);
            if n >= opponent.hand.len() {
                v.hide(&[back.as_str()]);
            }
        }
        if !player.to_play {
            v.inert(&["BTN_XTEXT", "BTN_YTEXT"]);
        }
        if let Some(words) = message {
            v.notice = Some(Notice {
                text: self.ui.strref(words),
                button: self.ui.strref(STR_OK),
            });
        }
        v
    }

    pub(super) fn pazaak_press(&mut self, tag: &str) {
        let view = self.game.world.pazaak_view();
        let setup = matches!(view, Some(PazaakView::Setup { .. }));
        let table = matches!(view, Some(PazaakView::Table { .. }));
        let number = |prefix: &str| tag.strip_prefix(prefix)?.parse::<usize>().ok();
        let done = match tag {
            "BTN_LESS" => Some(PazaakMove::WagerLess),
            "BTN_MORE" => Some(PazaakMove::WagerMore),
            "BTN_WAGER" => Some(PazaakMove::Wager),
            "BTN_QUIT" => Some(PazaakMove::Quit),
            "BTN_ATEXT" => Some(PazaakMove::Play),
            "BTN_OK" => Some(PazaakMove::Understood),
            "BTN_XTEXT" if table => Some(PazaakMove::EndTurn),
            "BTN_YTEXT" if table => Some(PazaakMove::Stand),
            "BTN_YTEXT" if setup => self
                .menus
                .pazaak
                .picked
                .map(|(column, row)| PazaakMove::AddCard(column, row)),
            _ => number("BTN_CHOSEN")
                .map(PazaakMove::RemoveCard)
                .or_else(|| number("BTN_PLRSIDE").map(PazaakMove::PlayHand))
                .or_else(|| number("BTN_FLIP").map(PazaakMove::FlipHand)),
        };
        if let Some(place) = number("BTN_AVAIL") {
            self.menus.pazaak.picked = Some((place / 10, place % 10));
        }
        if let Some(done) = done {
            self.pazaak_move(done);
        }
    }

    fn pazaak_move(&mut self, m: PazaakMove) {
        self.game.world.pazaak_move(m);
        self.menus.hover = None;
        if self.game.world.pazaak.is_none() {
            self.menus.pazaak = Pazaak::default();
        }
    }

    /// Esc is Quit where there is one. At the table it does nothing: the game has to be played out.
    pub(super) fn pazaak_escape(&mut self) {
        self.pazaak_move(PazaakMove::Quit);
    }
}
