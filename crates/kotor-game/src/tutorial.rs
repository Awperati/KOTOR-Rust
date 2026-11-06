//! Tutorial pop-ups: `tutorial.2da` says what each says, in up to three pages. Each comes once in a game.

use crate::world::World;

/// How many pages a row of the table can have.
const PAGES: [&str; 3] = ["message0", "message1", "message2"];

impl World {
    /// The words of a row of `tutorial.2da`, page by page, if it has not come up yet in this game.
    pub fn tutorial_once(&mut self, row: i32) -> Vec<u32> {
        if !self.tutorial_seen.insert(row) {
            return Vec::new();
        }
        let Some(t) = self.table("tutorial") else {
            return Vec::new();
        };
        PAGES
            .iter()
            .filter_map(|c| t.int(row.max(0) as usize, c))
            .map(|n| n as u32)
            .collect()
    }
}
