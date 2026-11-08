//! Classic or Modern: how a game is played, chosen when it starts and kept with its saves. The owner, 8 November 2026: "Upon starting
//! singeplayer you're faced with 2 immediate gamemodes Classic or Modern". See the game modes spec beside the session notes.
//!
//! Classic plays as the original: tank movement, a game that pauses, fights in rounds. Modern has the new controls and no pause
//! during play but Escape's. What each mode lets the player do is the movement's and the combat's to say: this is the one flag.

/// The mode of a game. A save from before modes has none: what Options had then is what it gets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GameMode {
    #[default]
    Classic,
    Modern,
}

impl GameMode {
    /// What a save keeps of it.
    pub fn number(self) -> u8 {
        match self {
            GameMode::Classic => 0,
            GameMode::Modern => 1,
        }
    }

    /// From a save's number: anything but 1 is Classic.
    pub fn from_number(n: i64) -> GameMode {
        if n == 1 {
            GameMode::Modern
        } else {
            GameMode::Classic
        }
    }

    /// Its name, for the screens.
    pub fn name(self) -> &'static str {
        match self {
            GameMode::Classic => "Classic",
            GameMode::Modern => "Modern",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mode_goes_through_its_number_and_back() {
        for m in [GameMode::Classic, GameMode::Modern] {
            assert_eq!(GameMode::from_number(m.number() as i64), m);
        }
        assert_eq!(GameMode::from_number(7), GameMode::Classic);
        assert_eq!(GameMode::default(), GameMode::Classic);
        assert_eq!(GameMode::Modern.name(), "Modern");
    }
}
