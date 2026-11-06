//! The game's own keys, from `keymap.2da`.

use crate::world::World;

impl World {
    /// The key `keymap.2da` gives an action, by its `name` column. A name on two rows is the last row's.
    pub fn original_key(&mut self, name: &str) -> Option<String> {
        let t = self.table("keymap")?;
        (0..t.row_count())
            .rev()
            .find(|r| t.get(*r, "name") == Some(name))
            .and_then(|r| t.get(r, "character"))
            .map(str::to_string)
    }
}
