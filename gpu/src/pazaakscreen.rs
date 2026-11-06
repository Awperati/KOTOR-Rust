//! Pazaak's three screens, from the stock files: `pazaakwager.gui`, `pazaaksetup.gui` and `pazaakgame.gui`. These only show
//! what they are told and say what was pressed: the rules are the engine's and not built, so nothing in a game opens them.
//! **Not compared:** I found no game of Pazaak in the reference video. **Assumed:** a card is chosen with a click and goes
//! over with Add Card, and which picture goes with which kind of card is read from the names of the file's own pictures.

use super::{Screen, Stage};
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

/// A card as the screens draw it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PazaakCard {
    /// From the main deck.
    Main(i32),
    Plus(i32),
    Minus(i32),
    /// Either way: its sign is flipped in the hand, and this is what it would play as.
    Either(i32),
}

impl PazaakCard {
    fn picture(self) -> &'static str {
        match self {
            PazaakCard::Main(_) => PICTURE_MAIN,
            PazaakCard::Plus(_) => PICTURE_PLUS,
            PazaakCard::Minus(_) => PICTURE_MINUS,
            PazaakCard::Either(_) => PICTURE_EITHER,
        }
    }

    fn words(self) -> String {
        match self {
            PazaakCard::Main(n) => n.to_string(),
            PazaakCard::Plus(n) => format!("+{n}"),
            PazaakCard::Minus(n) => format!("-{n}"),
            PazaakCard::Either(n) => format!("{n:+}"),
        }
    }
}

/// One side of the table.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PazaakSide {
    /// What it has played, in order.
    pub played: Vec<PazaakCard>,
    pub total: i32,
    /// Sets won.
    pub wins: usize,
    /// The player's hand as itself. The opponent's shows only its backs, so only how many matters there.
    pub hand: Vec<PazaakCard>,
    pub to_play: bool,
}

/// What the screens show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PazaakView {
    Wager {
        value: i32,
        maximum: i32,
    },
    /// How many of each card there are, by the file's column and row, and those chosen so far.
    Setup {
        available: [[i32; 6]; 3],
        chosen: Vec<PazaakCard>,
    },
    Table {
        player: Box<PazaakSide>,
        opponent: Box<PazaakSide>,
        /// A row of the talk table, in a box with an OK: "You win the set."
        message: Option<u32>,
    },
}

/// What was pressed. The rules answer with the next view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PazaakMove {
    WagerLess,
    WagerMore,
    Wager,
    Quit,
    /// Add Card, with this column and row of the available cards.
    AddCard(usize, usize),
    /// A chosen card goes back.
    RemoveCard(usize),
    Play,
    EndTurn,
    Stand,
    PlayHand(usize),
    FlipHand(usize),
    /// The OK of a message.
    Understood,
}

#[derive(Default)]
pub(super) struct Pazaak {
    view: Option<PazaakView>,
    moves: Vec<PazaakMove>,
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
    /// The screens come up, or go on to another view.
    pub fn show_pazaak(&mut self, view: PazaakView) {
        self.menus.pazaak.view = Some(view);
        self.menus.hover = None;
        if self.menus.stack.last() != Some(&Screen::Pazaak) {
            self.menus.stack.push(Screen::Pazaak);
        }
    }

    /// The screens go.
    pub fn close_pazaak(&mut self) {
        self.menus.pazaak = Pazaak::default();
        if self.menus.stack.last() == Some(&Screen::Pazaak) {
            self.menus.stack.pop();
        }
        self.menus.hover = None;
    }

    /// What has been pressed since last asked.
    pub fn take_pazaak_moves(&mut self) -> Vec<PazaakMove> {
        std::mem::take(&mut self.menus.pazaak.moves)
    }

    pub(super) fn pazaak_view(&mut self) -> MenuView {
        match self.menus.pazaak.view.clone() {
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
                v.text(&words, available_card(column, row).words());
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
                    v.fill(&button, card.picture());
                    v.text(&words, card.words());
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
                        v.fill(&button, card.picture());
                        v.text(&words, card.words());
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
                    v.fill(&button, card.picture());
                    v.text(&words, card.words());
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
        let setup = matches!(self.menus.pazaak.view, Some(PazaakView::Setup { .. }));
        let table = matches!(self.menus.pazaak.view, Some(PazaakView::Table { .. }));
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
            self.menus.pazaak.moves.push(done);
        }
    }

    /// Esc is Quit where there is one. In a game the rules decide what leaving costs, so it is theirs to answer.
    pub(super) fn pazaak_escape(&mut self) {
        self.menus.pazaak.moves.push(PazaakMove::Quit);
    }
}
