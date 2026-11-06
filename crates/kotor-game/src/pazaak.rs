//! Pazaak: a wager, a side deck, then sets to 20, the first to win three taking the wager. `PlayPazaak` starts a game and
//! its end script reads `GetLastPazaakResult`. The screens are the viewer's: they show `World::pazaak_view` and answer with
//! `World::pazaak_move`.
//!
//! **Read from the install:** the opponent's deck and the default side deck from `pazaakdecks.2da`, the words of the stock
//! screens, and the side deck cards as items `g_i_pazcard_001` to `018`. **Assumed, from knowing the game and not from the
//! video, where I found no game:** a main deck of four each of 1 to 10 for each side, shuffled each set; a card drawn at
//! the start of each turn; one hand card a turn; over 20 at the end of a turn loses the set; nine cards without going over
//! wins it; hands are dealt once and last the game; the loser of a set starts the next. The wager goes up and down by
//! `WAGER_STEP`. Leaving before the first set runs nothing and costs nothing. The opponent's play is mine.

use crate::world::{ObjId, World};

const TARGET: i32 = 20;
const SETS_TO_WIN: i32 = 3;
const TABLE_CARDS: usize = 9;
const HAND_CARDS: usize = 4;
const SIDE_DECK: usize = 10;
const MAIN_COPIES: usize = 4;
const WAGER_STEP: i32 = 10;
/// The opponent stands from here.
const STAND_FROM: i32 = 18;
const CARD_ITEM: &str = "g_i_pazcard_";
const DECK_COLUMNS: usize = 10;
const DEFAULT_DECK_ROW: usize = 0;
const STR_YOU_WIN_SET: u32 = 32334;
const STR_THEY_WIN_SET: u32 = 32335;
const STR_YOU_WIN_GAME: u32 = 32336;
const STR_THEY_WIN_GAME: u32 = 32337;
const STR_TIED_SET: u32 = 32338;
/// What `GetLastPazaakResult` answers. **Assumed:** true for a win, as its name and the end scripts suggest.
pub(crate) const RESULT_LOST: i32 = 0;
const RESULT_WON: i32 = 1;

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
    fn value(self) -> i32 {
        match self {
            PazaakCard::Main(n) | PazaakCard::Plus(n) | PazaakCard::Either(n) => n,
            PazaakCard::Minus(n) => -n,
        }
    }

    /// As `pazaakdecks.2da` writes one: `+3`, `-3` or `*3`.
    fn parse(words: &str) -> Option<PazaakCard> {
        let n: i32 = words.get(1..)?.parse().ok()?;
        match words.as_bytes().first()? {
            b'+' => Some(PazaakCard::Plus(n)),
            b'-' => Some(PazaakCard::Minus(n)),
            b'*' => Some(PazaakCard::Either(n)),
            _ => None,
        }
    }

    /// The side deck screen's place for it: column and row.
    fn place(self) -> Option<(usize, usize)> {
        let (column, n) = match self {
            PazaakCard::Plus(n) => (0, n),
            PazaakCard::Minus(n) => (1, n),
            PazaakCard::Either(n) => (2, n.abs()),
            PazaakCard::Main(_) => return None,
        };
        (1..=6).contains(&n).then_some((column, n as usize - 1))
    }

    fn at_place(column: usize, row: usize) -> PazaakCard {
        let n = row as i32 + 1;
        match column {
            0 => PazaakCard::Plus(n),
            1 => PazaakCard::Minus(n),
            _ => PazaakCard::Either(n),
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

/// What was pressed.
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    Wager,
    Setup,
    Table,
}

/// How a message goes on when it is answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum After {
    NextSet { player_first: bool },
    Finish { won: bool },
}

/// A side's own part of a set.
#[derive(Clone, Debug, Default)]
struct Seat {
    deck: Vec<i32>,
    stood: bool,
}

/// How a game stands. It knows nothing of the world: what is random comes from `roll`.
#[derive(Clone, Debug)]
pub struct PazaakGame {
    pub end_script: String,
    pub opponent_object: Option<ObjId>,
    part: Part,
    wager: i32,
    maximum: i32,
    /// How many of each card the player owns.
    owned: [[i32; 6]; 3],
    chosen: Vec<PazaakCard>,
    opponent_deck: Vec<PazaakCard>,
    player: PazaakSide,
    opponent: PazaakSide,
    player_seat: Seat,
    opponent_seat: Seat,
    /// A hand card has been played this turn.
    hand_used: bool,
    message: Option<u32>,
    after: Option<After>,
    /// The game is over: won or not. Quit leaves it `None` with `left` set.
    pub result: Option<bool>,
    pub left: bool,
}

type Roll<'a> = &'a mut dyn FnMut(usize) -> usize;

fn shuffled_main(roll: Roll) -> Vec<i32> {
    let mut deck: Vec<i32> = (1..=10)
        .flat_map(|n| std::iter::repeat_n(n, MAIN_COPIES))
        .collect();
    for i in (1..deck.len()).rev() {
        deck.swap(i, roll(i + 1));
    }
    deck
}

fn deal_hand(from: &[PazaakCard], roll: Roll) -> Vec<PazaakCard> {
    let mut left = from.to_vec();
    let mut hand = Vec::new();
    while hand.len() < HAND_CARDS && !left.is_empty() {
        hand.push(left.remove(roll(left.len())));
    }
    hand
}

fn play_card(side: &mut PazaakSide, card: PazaakCard) {
    side.total += card.value();
    side.played.push(card);
}

impl PazaakGame {
    pub fn new(
        end_script: &str,
        opponent_object: Option<ObjId>,
        maximum: i32,
        owned: [[i32; 6]; 3],
        opponent_deck: Vec<PazaakCard>,
    ) -> PazaakGame {
        let maximum = maximum.max(0);
        PazaakGame {
            end_script: end_script.to_string(),
            opponent_object,
            part: Part::Wager,
            wager: WAGER_STEP.min(maximum),
            maximum,
            owned,
            chosen: Vec::new(),
            opponent_deck,
            player: PazaakSide::default(),
            opponent: PazaakSide::default(),
            player_seat: Seat::default(),
            opponent_seat: Seat::default(),
            hand_used: false,
            message: None,
            after: None,
            result: None,
            left: false,
        }
    }

    pub fn wager(&self) -> i32 {
        self.wager
    }

    pub fn view(&self) -> PazaakView {
        match self.part {
            Part::Wager => PazaakView::Wager {
                value: self.wager,
                maximum: self.maximum,
            },
            Part::Setup => {
                let mut available = self.owned;
                for card in &self.chosen {
                    if let Some((column, row)) = card.place() {
                        available[column][row] -= 1;
                    }
                }
                PazaakView::Setup {
                    available,
                    chosen: self.chosen.clone(),
                }
            }
            Part::Table => PazaakView::Table {
                player: Box::new(self.player.clone()),
                opponent: Box::new(self.opponent.clone()),
                message: self.message,
            },
        }
    }

    pub fn apply(&mut self, m: PazaakMove, roll: Roll) {
        match (self.part, m) {
            (Part::Wager, PazaakMove::WagerLess) => {
                self.wager = (self.wager - WAGER_STEP).max(self.maximum.min(1));
            }
            (Part::Wager, PazaakMove::WagerMore) => {
                self.wager = (self.wager + WAGER_STEP).min(self.maximum);
            }
            (Part::Wager, PazaakMove::Wager) if self.wager > 0 => self.part = Part::Setup,
            (Part::Wager | Part::Setup, PazaakMove::Quit) => self.left = true,
            (Part::Setup, PazaakMove::AddCard(column, row)) => self.add_card(column, row),
            (Part::Setup, PazaakMove::RemoveCard(n)) if n < self.chosen.len() => {
                self.chosen.remove(n);
            }
            (Part::Setup, PazaakMove::Play) if self.chosen.len() == SIDE_DECK => {
                self.part = Part::Table;
                self.player.hand = deal_hand(&self.chosen, &mut *roll);
                self.opponent.hand = deal_hand(&self.opponent_deck, &mut *roll);
                self.start_set(true, roll);
            }
            (Part::Table, PazaakMove::Understood) if self.message.is_some() => {
                self.answer_message(roll)
            }
            (Part::Table, _) if self.message.is_some() || !self.player.to_play => {}
            (Part::Table, PazaakMove::FlipHand(n)) => {
                if let Some(PazaakCard::Either(v)) = self.player.hand.get_mut(n) {
                    *v = -*v;
                }
            }
            (Part::Table, PazaakMove::PlayHand(n))
                if !self.hand_used && n < self.player.hand.len() =>
            {
                let card = self.player.hand.remove(n);
                play_card(&mut self.player, card);
                self.hand_used = true;
                if self.player.played.len() >= TABLE_CARDS && self.player.total <= TARGET {
                    self.end_set(Some(true));
                }
            }
            (Part::Table, PazaakMove::EndTurn) => self.player_turn_over(),
            (Part::Table, PazaakMove::Stand) => {
                self.player_seat.stood = true;
                self.player_turn_over();
            }
            _ => {}
        }
    }

    fn add_card(&mut self, column: usize, row: usize) {
        if self.chosen.len() >= SIDE_DECK || column >= 3 || row >= 6 {
            return;
        }
        let card = PazaakCard::at_place(column, row);
        let used = self
            .chosen
            .iter()
            .filter(|c| c.place() == card.place())
            .count();
        if (used as i32) < self.owned[column][row] {
            self.chosen.push(card);
        }
    }

    fn start_set(&mut self, player_first: bool, roll: Roll) {
        for side in [&mut self.player, &mut self.opponent] {
            side.played.clear();
            side.total = 0;
            side.to_play = false;
        }
        self.player_seat = Seat {
            deck: shuffled_main(&mut *roll),
            stood: false,
        };
        self.opponent_seat = Seat {
            deck: shuffled_main(&mut *roll),
            stood: false,
        };
        if player_first {
            self.begin_player_turn();
        } else {
            self.opponent_turn();
        }
    }

    fn begin_player_turn(&mut self) {
        self.opponent.to_play = false;
        self.player.to_play = true;
        self.hand_used = false;
        if let Some(n) = self.player_seat.deck.pop() {
            play_card(&mut self.player, PazaakCard::Main(n));
        }
        if self.player.played.len() >= TABLE_CARDS && self.player.total <= TARGET {
            self.end_set(Some(true));
        }
    }

    fn player_turn_over(&mut self) {
        if self.player.total > TARGET {
            return self.end_set(Some(false));
        }
        self.player.to_play = false;
        if self.player_seat.stood && self.opponent_seat.stood {
            return self.compare_totals();
        }
        self.opponent_turn();
    }

    /// The opponent's whole turn, and more if the player has stood.
    fn opponent_turn(&mut self) {
        loop {
            if !self.opponent_seat.stood {
                if let Some(n) = self.opponent_seat.deck.pop() {
                    play_card(&mut self.opponent, PazaakCard::Main(n));
                }
                self.opponent_choose();
                if self.opponent.total > TARGET {
                    return self.end_set(Some(true));
                }
                if self.opponent.played.len() >= TABLE_CARDS {
                    return self.end_set(Some(false));
                }
            }
            if !self.player_seat.stood {
                return self.begin_player_turn();
            }
            if self.opponent_seat.stood {
                return self.compare_totals();
            }
        }
    }

    /// The opponent's choice: a hand card that makes exactly 20 or saves it, and then to stand or not.
    fn opponent_choose(&mut self) {
        let total = self.opponent.total;
        let helps = |card: &PazaakCard| {
            let values = match card {
                PazaakCard::Either(n) => vec![*n, -*n],
                other => vec![other.value()],
            };
            values
                .into_iter()
                .find(|v| total + v == TARGET || (total > TARGET && total + v <= TARGET))
        };
        let pick = self.opponent.hand.iter().position(|c| helps(c).is_some());
        if let Some(i) = pick {
            let card = self.opponent.hand.remove(i);
            let value = helps(&card).unwrap_or(card.value());
            let shown = match card {
                PazaakCard::Either(_) => PazaakCard::Either(value),
                other => other,
            };
            play_card(&mut self.opponent, shown);
        }
        let ahead = self.player_seat.stood && self.opponent.total > self.player.total;
        if self.opponent.total <= TARGET && (self.opponent.total >= STAND_FROM || ahead) {
            self.opponent_seat.stood = true;
        }
    }

    fn compare_totals(&mut self) {
        let (mine, theirs) = (self.player.total, self.opponent.total);
        self.end_set(if mine == theirs {
            None
        } else {
            Some(mine > theirs)
        });
    }

    fn end_set(&mut self, player_won: Option<bool>) {
        self.player.to_play = false;
        self.opponent.to_play = false;
        match player_won {
            Some(true) => {
                self.player.wins += 1;
                self.message = Some(STR_YOU_WIN_SET);
                self.after = Some(After::NextSet {
                    player_first: false,
                });
            }
            Some(false) => {
                self.opponent.wins += 1;
                self.message = Some(STR_THEY_WIN_SET);
                self.after = Some(After::NextSet { player_first: true });
            }
            None => {
                self.message = Some(STR_TIED_SET);
                self.after = Some(After::NextSet { player_first: true });
            }
        }
    }

    fn answer_message(&mut self, roll: Roll) {
        self.message = None;
        match self.after.take() {
            Some(After::NextSet { player_first }) => {
                if self.player.wins as i32 >= SETS_TO_WIN {
                    self.message = Some(STR_YOU_WIN_GAME);
                    self.after = Some(After::Finish { won: true });
                } else if self.opponent.wins as i32 >= SETS_TO_WIN {
                    self.message = Some(STR_THEY_WIN_GAME);
                    self.after = Some(After::Finish { won: false });
                } else {
                    self.start_set(player_first, roll);
                }
            }
            Some(After::Finish { won }) => self.result = Some(won),
            None => {}
        }
    }
}

impl World {
    /// `PlayPazaak`.
    pub fn start_pazaak(
        &mut self,
        deck: i32,
        end_script: &str,
        max_wager: i32,
        opponent: Option<ObjId>,
    ) {
        let table = self.table("pazaakdecks");
        let row_cards = |row: usize| -> Vec<PazaakCard> {
            let Some(t) = &table else {
                return Vec::new();
            };
            (0..DECK_COLUMNS)
                .filter_map(|c| PazaakCard::parse(t.get(row, &format!("card{c}"))?))
                .collect()
        };
        let opponent_deck = row_cards(deck.max(0) as usize);
        let mut owned = [[0; 6]; 3];
        for card in row_cards(DEFAULT_DECK_ROW) {
            if let Some((column, row)) = card.place() {
                owned[column][row] += 1;
            }
        }
        for item in self.carried(self.pc) {
            let Some(o) = self.object(item) else { continue };
            let number = o.template.to_ascii_lowercase();
            let number = number
                .strip_prefix(CARD_ITEM)
                .and_then(|n| n.parse::<usize>().ok());
            if let Some(n) = number.filter(|n| (1..=18).contains(n)) {
                owned[(n - 1) / 6][(n - 1) % 6] += o.stack;
            }
        }
        let maximum = max_wager.min(self.gold);
        self.pazaak = Some(PazaakGame::new(
            end_script,
            opponent,
            maximum,
            owned,
            opponent_deck,
        ));
    }

    pub fn pazaak_view(&self) -> Option<PazaakView> {
        self.pazaak.as_ref().map(PazaakGame::view)
    }

    /// A press on the screens. A game that has ended pays, and its end script runs.
    pub fn pazaak_move(&mut self, m: PazaakMove) {
        let Some(mut game) = self.pazaak.take() else {
            return;
        };
        let mut roll = |n: usize| self.roll(n as i32) as usize - 1;
        game.apply(m, &mut roll);
        if game.left {
            return;
        }
        let Some(won) = game.result else {
            self.pazaak = Some(game);
            return;
        };
        self.gold += if won { game.wager() } else { -game.wager() };
        self.gold = self.gold.max(0);
        self.last_pazaak_result = if won { RESULT_WON } else { RESULT_LOST };
        let caller = game.opponent_object.unwrap_or(self.pc);
        if !game.end_script.is_empty() {
            let _ = self.run_script(&game.end_script, caller);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plain roll: enough to deal and shuffle, and the same each time.
    fn roller() -> impl FnMut(usize) -> usize {
        let mut x = 7usize;
        move |n| {
            x = x.wrapping_mul(31).wrapping_add(11);
            x % n.max(1)
        }
    }

    fn game() -> PazaakGame {
        let mut owned = [[0; 6]; 3];
        owned[0] = [2, 2, 2, 2, 2, 0];
        PazaakGame::new("", None, 100, owned, vec![PazaakCard::Plus(1); 10])
    }

    fn at_table(g: &mut PazaakGame, r: &mut impl FnMut(usize) -> usize) {
        g.apply(PazaakMove::Wager, r);
        for (column, row) in [
            (0, 0),
            (0, 0),
            (0, 1),
            (0, 1),
            (0, 2),
            (0, 2),
            (0, 3),
            (0, 3),
            (0, 4),
            (0, 4),
        ] {
            g.apply(PazaakMove::AddCard(column, row), r);
        }
        g.apply(PazaakMove::Play, r);
    }

    #[test]
    fn the_wager_goes_by_steps_inside_its_limits_and_needs_something() {
        let mut r = roller();
        let mut g = game();
        assert_eq!(g.wager(), WAGER_STEP);
        g.apply(PazaakMove::WagerLess, &mut r);
        assert_eq!(g.wager(), 1, "not to nothing");
        for _ in 0..30 {
            g.apply(PazaakMove::WagerMore, &mut r);
        }
        assert_eq!(g.wager(), 100);
        let mut broke = PazaakGame::new("", None, 0, [[0; 6]; 3], Vec::new());
        broke.apply(PazaakMove::Wager, &mut r);
        assert!(matches!(broke.view(), PazaakView::Wager { value: 0, .. }));
    }

    #[test]
    fn a_side_deck_takes_only_what_is_owned_and_plays_at_ten() {
        let mut r = roller();
        let mut g = game();
        g.apply(PazaakMove::Wager, &mut r);
        g.apply(PazaakMove::AddCard(1, 0), &mut r);
        let PazaakView::Setup { chosen, .. } = g.view() else {
            panic!()
        };
        assert!(chosen.is_empty(), "none of those owned");
        g.apply(PazaakMove::AddCard(0, 0), &mut r);
        g.apply(PazaakMove::AddCard(0, 0), &mut r);
        g.apply(PazaakMove::AddCard(0, 0), &mut r);
        let PazaakView::Setup { available, chosen } = g.view() else {
            panic!()
        };
        assert_eq!((chosen.len(), available[0][0]), (2, 0), "two owned");
        g.apply(PazaakMove::Play, &mut r);
        assert!(matches!(g.view(), PazaakView::Setup { .. }), "not ten yet");
        g.apply(PazaakMove::RemoveCard(0), &mut r);
        let PazaakView::Setup { available, chosen } = g.view() else {
            panic!()
        };
        assert_eq!((chosen.len(), available[0][0]), (1, 1));
    }

    #[test]
    fn the_table_deals_four_draws_a_card_and_takes_one_hand_card_a_turn() {
        let mut r = roller();
        let mut g = game();
        at_table(&mut g, &mut r);
        assert_eq!(g.player.hand.len(), 4);
        assert_eq!(g.player.played.len(), 1, "a card drawn");
        assert!(g.player.to_play);
        let before = g.player.total;
        let first = g.player.hand[0];
        g.apply(PazaakMove::PlayHand(0), &mut r);
        assert_eq!(g.player.total, before + first.value());
        assert_eq!(g.player.hand.len(), 3);
        g.apply(PazaakMove::PlayHand(0), &mut r);
        assert_eq!(g.player.hand.len(), 3, "one a turn");
    }

    #[test]
    fn a_card_either_way_flips_and_plays_as_shown() {
        let mut r = roller();
        let mut g = game();
        at_table(&mut g, &mut r);
        g.player.hand[0] = PazaakCard::Either(3);
        g.player.total = 10;
        g.apply(PazaakMove::FlipHand(0), &mut r);
        assert_eq!(g.player.hand[0], PazaakCard::Either(-3));
        g.apply(PazaakMove::PlayHand(0), &mut r);
        assert_eq!(g.player.total, 7);
    }

    #[test]
    fn over_twenty_at_the_end_of_a_turn_loses_the_set_and_a_hand_card_can_save_it() {
        let mut r = roller();
        let mut g = game();
        at_table(&mut g, &mut r);
        g.player.total = 23;
        g.player.hand[0] = PazaakCard::Minus(3);
        g.apply(PazaakMove::PlayHand(0), &mut r);
        assert_eq!(g.player.total, 20);
        g.apply(PazaakMove::EndTurn, &mut r);
        assert_eq!(g.message, None);

        let mut g = game();
        at_table(&mut g, &mut r);
        g.player.total = 23;
        g.apply(PazaakMove::EndTurn, &mut r);
        assert_eq!(g.message, Some(STR_THEY_WIN_SET));
        assert_eq!(g.opponent.wins, 1);
    }

    #[test]
    fn both_standing_the_higher_total_wins_and_equal_totals_tie() {
        let mut r = roller();
        for (mine, theirs, wanted) in [
            (20, 18, STR_YOU_WIN_SET),
            (18, 20, STR_THEY_WIN_SET),
            (19, 19, STR_TIED_SET),
        ] {
            let mut g = game();
            at_table(&mut g, &mut r);
            g.player.total = mine;
            g.opponent.total = theirs;
            g.opponent_seat.stood = true;
            g.apply(PazaakMove::Stand, &mut r);
            assert_eq!(g.message, Some(wanted), "{mine} {theirs}");
        }
    }

    #[test]
    fn nine_cards_without_going_over_win_the_set() {
        let mut r = roller();
        let mut g = game();
        at_table(&mut g, &mut r);
        g.player.played = vec![PazaakCard::Main(1); 8];
        g.player.total = 8;
        g.player.hand[0] = PazaakCard::Plus(1);
        g.apply(PazaakMove::PlayHand(0), &mut r);
        assert_eq!(g.message, Some(STR_YOU_WIN_SET));
    }

    #[test]
    fn nothing_moves_under_a_message_and_three_sets_end_the_game() {
        let mut r = roller();
        let mut g = game();
        at_table(&mut g, &mut r);
        g.player.wins = 2;
        g.player.total = 20;
        g.opponent.total = 10;
        g.opponent_seat.stood = true;
        g.apply(PazaakMove::Stand, &mut r);
        assert_eq!(g.message, Some(STR_YOU_WIN_SET));
        let before = g.player.clone();
        g.apply(PazaakMove::EndTurn, &mut r);
        assert_eq!(g.player, before);
        g.apply(PazaakMove::Understood, &mut r);
        assert_eq!(g.message, Some(STR_YOU_WIN_GAME));
        assert_eq!(g.result, None);
        g.apply(PazaakMove::Understood, &mut r);
        assert_eq!(g.result, Some(true));
    }

    #[test]
    fn a_new_set_clears_the_table_and_keeps_the_hands() {
        let mut r = roller();
        let mut g = game();
        at_table(&mut g, &mut r);
        g.apply(PazaakMove::PlayHand(0), &mut r);
        g.player.total = 25;
        g.apply(PazaakMove::EndTurn, &mut r);
        g.apply(PazaakMove::Understood, &mut r);
        assert_eq!(g.player.hand.len(), 3, "used cards stay gone");
        assert_eq!(g.player.played.len(), 1, "a fresh table and a card drawn");
        assert!(g.player.to_play, "the loser starts");
    }

    #[test]
    fn the_opponent_saves_itself_with_a_card_and_stands_when_high() {
        let mut r = roller();
        let mut g = game();
        at_table(&mut g, &mut r);
        g.opponent.hand = vec![PazaakCard::Either(4)];
        g.opponent.total = 24;
        g.opponent_choose();
        assert_eq!(g.opponent.total, 20);
        assert!(g.opponent_seat.stood);
        assert!(g.opponent.hand.is_empty());
    }

    #[test]
    fn a_whole_game_always_ends() {
        for seed in 0..40usize {
            let mut x = seed;
            let mut r = move |n: usize| {
                x = x.wrapping_mul(2_654_435_761).wrapping_add(97);
                (x >> 8) % n.max(1)
            };
            let mut g = game();
            at_table(&mut g, &mut r);
            let mut moves = 0;
            while g.result.is_none() {
                moves += 1;
                assert!(moves < 2000, "seed {seed}");
                let m = if g.message.is_some() {
                    PazaakMove::Understood
                } else if g.player.total >= 18 {
                    PazaakMove::Stand
                } else {
                    PazaakMove::EndTurn
                };
                g.apply(m, &mut r);
            }
            assert!(g.player.wins == 3 || g.opponent.wins == 3);
        }
    }
}
