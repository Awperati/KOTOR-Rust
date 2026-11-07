//! `optkeymapping.gui` and `optmouse.gui`, from the Gameplay page. The page lists what this engine can do with a key, in
//! the words of `keymap.2da`'s rows.
//!
//! **Differences:** the table has more rows, for things not built, and those are left out. Its keys differ from these in
//! places (P for the character, L for quests, Z and C to strafe). The first keys are the ones `docs/PLAY.md` lists, which
//! came first here, and the Default button goes between them and the table's, called Original. A key taken from another
//! action swaps with it. Mini Games is greyed: none are built. **Assumed:** reversed buttons swap everywhere, on the
//! screens too.

use super::{Screen, Stage};
use kotor_ui::{MenuView, Row};
use std::collections::BTreeMap;

/// The roll and the jump are not in the game, so they have no words in the talk table.
const NOT_IN_TALK_TABLE: u32 = 0;
/// An action: what it is called, the talk table's name for it, its page, and its key by `winit`'s name for it.
const KEY_ACTIONS: [(&str, u32, usize, &str); 28] = [
    ("MoveForward", 48025, 0, "KeyW"),
    ("MoveBack", 48027, 0, "KeyS"),
    ("MoveLeft", 48029, 0, "KeyA"),
    ("MoveRight", 48031, 0, "KeyD"),
    ("TurnLeft", 48319, 0, "ArrowLeft"),
    ("TurnRight", 48321, 0, "ArrowRight"),
    ("Roll", NOT_IN_TALK_TABLE, 0, "ControlLeft"),
    ("Jump", NOT_IN_TALK_TABLE, 0, "Space"),
    ("Use", 48313, 1, "KeyE"),
    ("ChangeLeader", 48033, 1, "Tab"),
    ("Equip", 48037, 1, "KeyU"),
    ("Inventory", 48039, 1, "KeyI"),
    ("Character", 48041, 1, "KeyC"),
    ("Abilities", 48043, 1, "KeyK"),
    ("Messages", 48045, 1, "KeyL"),
    ("Quests", 48050, 1, "KeyJ"),
    ("Map", 48048, 1, "KeyM"),
    ("Options", 48052, 1, "KeyO"),
    ("Pause", 48019, 1, "KeyX"),
    ("CancelCombat", 48315, 1, "KeyF"),
    ("Pickup", NOT_IN_TALK_TABLE, 1, "KeyF"),
    ("AttackMode", NOT_IN_TALK_TABLE, 1, "KeyB"),
    ("SoloMode", 48035, 1, "KeyV"),
    ("Nameplates", NOT_IN_TALK_TABLE, 1, "F6"),
    ("CelShading", NOT_IN_TALK_TABLE, 1, "F7"),
    ("ReloadCustom", NOT_IN_TALK_TABLE, 1, "F8"),
    ("CustomClip", NOT_IN_TALK_TABLE, 1, "F9"),
    ("CustomGarment", NOT_IN_TALK_TABLE, 1, "F10"),
];
/// Each action's name in `keymap.2da`. Not Pause: the table's key for it is Space, which jumps here, as the owner asked.
const ORIGINAL_NAMES: [(&str, &str); 18] = [
    ("MoveForward", "ActionUp"),
    ("MoveBack", "ActionDown"),
    ("MoveLeft", "ActionLeft"),
    ("MoveRight", "ActionRight"),
    ("TurnLeft", "CameraRotateLeft"),
    ("TurnRight", "CameraRotateRight"),
    ("Use", "DefaultAction"),
    ("ChangeLeader", "ChangeChar"),
    ("Equip", "Equip"),
    ("Inventory", "Inventory"),
    ("Character", "Character"),
    ("Abilities", "SkillsAndFeats"),
    ("Messages", "Messages"),
    ("Quests", "Quests"),
    ("Map", "Map"),
    ("Options", "Options"),
    ("CancelCombat", "CancleCombat"),
    ("SoloMode", "PartyActive"),
];
const ORIGINAL_LABEL: &str = "Original";
const MINI_GAMES_PAGE: usize = 2;
const FILTERS: [&str; 3] = ["BTN_Filter_Move", "BTN_Filter_Game", "BTN_Filter_Mini"];
const STR_REVERSE_WORDS: u32 = 48450;
/// The page is about more than the mouse now, and the box for the on-screen controls goes under the file's.
const CONTROLS_TITLE: &str = "Controls";
const TOUCH_BOX: &str = "CB_TOUCH";
const TOUCH_BOX_AT: [i32; 4] = [60, 232, 240, 40];
const TOUCH_BOX_WORDS: &str = "On-screen Controls";
const TOUCH_BOX_ABOUT: &str = "A stick, round buttons and bigger menu keys on the HUD, for a touch screen or the mouse. The blue button at the top right turns them on and off too.";
const LIST: &str = "LST_EventList";
/// Keys that always mean the same, whatever the page says.
const FIXED_KEYS: [&str; 15] = [
    "Escape",
    "Enter",
    "NumpadEnter",
    "ShiftLeft",
    "ShiftRight",
    "Digit0",
    "Digit1",
    "Digit2",
    "Digit3",
    "Digit4",
    "Digit5",
    "Digit6",
    "Digit7",
    "Digit8",
    "Digit9",
];

/// The keys in use, by action.
#[derive(Clone, PartialEq)]
pub(super) struct KeyMap(BTreeMap<&'static str, String>);

impl Default for KeyMap {
    fn default() -> KeyMap {
        KeyMap(
            KEY_ACTIONS
                .iter()
                .map(|(name, _, _, key)| (*name, key.to_string()))
                .collect(),
        )
    }
}

impl KeyMap {
    /// As lines of the settings file.
    pub(super) fn to_text(&self) -> String {
        self.0
            .iter()
            .map(|(action, key)| format!("key.{action}={key}\n"))
            .collect()
    }

    /// Lines of the settings file. Anything it does not know, or a key used twice, stays as it was.
    pub(super) fn read(&mut self, text: &str) {
        // A file from before the jump has Space for the pause, by default or not: it goes to the new key.
        let before_jump = !text.contains("key.Jump=");
        for line in text.lines() {
            let Some((name, key)) = line.strip_prefix("key.").and_then(|l| l.split_once('='))
            else {
                continue;
            };
            if before_jump && name == "Pause" && key == "Space" {
                continue;
            }
            let known = KEY_ACTIONS.iter().find(|a| a.0 == name);
            if let Some((action, ..)) = known {
                self.0.insert(action, key.to_string());
            }
        }
    }

    /// Give an action a key. Whoever had it gets the old one.
    fn bind(&mut self, action: &'static str, key: &str) {
        let old = self.0.get(action).cloned().unwrap_or_default();
        for k in self.0.values_mut() {
            if k == key {
                *k = old.clone();
            }
        }
        self.0.insert(action, key.to_string());
    }
}

/// An action with no words in the talk table, as the page says it: `AttackMode` is Attack Mode.
fn spaced(action: &str) -> String {
    let mut out = String::new();
    for c in action.chars() {
        if c.is_uppercase() && !out.is_empty() {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

/// A key as `keymap.2da` writes it, by `winit`'s name: `W` is `KeyW`, `1` is `Digit1`.
fn key_from_table(key: &str) -> String {
    match key.chars().next() {
        Some(c) if key.len() == 1 && c.is_ascii_alphabetic() => {
            format!("Key{}", c.to_ascii_uppercase())
        }
        Some(c) if key.len() == 1 && c.is_ascii_digit() => format!("Digit{c}"),
        _ => key.to_string(),
    }
}

/// A key as the page shows it: `KeyW` is W, `Digit1` is 1, and `ArrowUp` is Up.
fn pretty(key: &str) -> &str {
    key.strip_prefix("Key")
        .or(key.strip_prefix("Digit"))
        .or(key.strip_prefix("Arrow"))
        .unwrap_or(key)
}

impl Stage {
    /// The key of an action, as the page shows it.
    pub(super) fn key_words(&self, action: &str) -> String {
        let key = self.menus.keys.0.get(action).cloned().unwrap_or_default();
        pretty(&key).to_string()
    }

    /// What a key does in the game, by `winit`'s name for it.
    pub fn action_for_key(&self, key: &str) -> Option<&'static str> {
        let mut on_key = self.menus.keys.0.iter().filter(|(_, k)| *k == key);
        let first = on_key.next().map(|(a, _)| *a)?;
        // F stops a fight, as it does in the game, and picks up when there is no fight to stop.
        let shared = first == "CancelCombat" && on_key.any(|(a, _)| *a == "Pickup");
        Some(if shared && !self.party_blocked() {
            "Pickup"
        } else {
            first
        })
    }

    /// Words with `{Action}` in them, with the key that action has now.
    pub(super) fn with_keys(&self, words: &str) -> String {
        let mut out = words.to_string();
        for (action, key) in self.menus.keys.0.iter() {
            out = out.replace(&format!("{{{action}}}"), pretty(key));
        }
        out
    }

    /// Each action and its key.
    pub fn key_bindings(&self) -> Vec<(&'static str, String)> {
        let keys = &self.menus.keys.0;
        keys.iter().map(|(a, k)| (*a, k.clone())).collect()
    }

    /// A key was pressed. If the page is waiting for one it takes it, and Esc gives up waiting.
    pub fn capture_key(&mut self, key: &str) -> bool {
        let Some(action) = self.menus.key_capture else {
            return false;
        };
        self.menus.key_capture = None;
        if !FIXED_KEYS.contains(&key) {
            self.menus.keys.bind(action, key);
        }
        true
    }

    /// The key for Cancel Combat: the same as the Disengage button.
    pub fn hud_disengage(&mut self) {
        if !self.menu_open() {
            self.hud_button("BTN_CLEARALL");
        }
    }

    pub(super) fn open_key_map(&mut self) {
        self.menus.keys_before = self.menus.keys.clone();
        self.menus.key_page = 0;
        self.menus.key_capture = None;
        self.menus.first = 0;
        self.menus.stack.push(Screen::KeyMap);
    }

    fn page_actions(&self) -> Vec<(&'static str, u32)> {
        let page = self.menus.key_page;
        KEY_ACTIONS
            .iter()
            .filter(|a| a.2 == page)
            .map(|a| (a.0, a.1))
            .collect()
    }

    pub(super) fn key_map_view(&mut self) -> MenuView {
        let mut v = MenuView::new("optkeymapping");
        v.inert(&[FILTERS[MINI_GAMES_PAGE]]);
        // The button says what a click gives.
        if self.menus.keys == KeyMap::default() {
            v.text("BTN_Default", ORIGINAL_LABEL);
        }
        v.lit(FILTERS[self.menus.key_page]);
        let capture = self.menus.key_capture;
        let mut rows = Vec::new();
        for (action, words) in self.page_actions() {
            let name = if words == NOT_IN_TALK_TABLE {
                spaced(action)
            } else {
                self.ui.strref(words)
            };
            let key = self.menus.keys.0.get(action).cloned().unwrap_or_default();
            let waiting = capture == Some(action);
            let shown = if waiting { "..." } else { pretty(&key) };
            rows.push(Row {
                text: format!("{name}: {shown}"),
                selected: waiting,
                ..Row::default()
            });
        }
        v.lists.insert(LIST.to_string(), (rows, self.menus.first));
        v
    }

    /// A click on a row: the next key pressed is its key.
    pub(super) fn key_row(&mut self, row: usize) {
        self.menus.key_capture = self.page_actions().get(row).map(|a| a.0);
    }

    pub(super) fn key_map_press(&mut self, tag: &str) {
        self.menus.key_capture = None;
        match tag {
            "BTN_Default" => {
                self.menus.keys = if self.menus.keys == KeyMap::default() {
                    self.original_keys()
                } else {
                    KeyMap::default()
                };
            }
            "BTN_Accept" => {
                self.menus.stack.pop();
                self.settings_changed();
            }
            "BTN_Cancel" => self.cancel_key_map(),
            _ => {
                let page = FILTERS.iter().position(|f| *f == tag);
                if let Some(page) = page.filter(|p| *p != MINI_GAMES_PAGE) {
                    self.menus.key_page = page;
                    self.menus.first = 0;
                }
            }
        }
    }

    /// The keys of `keymap.2da`. An action it does not name keeps its first key.
    fn original_keys(&mut self) -> KeyMap {
        let mut keys = KeyMap::default();
        for (action, name) in ORIGINAL_NAMES {
            if let Some(key) = self.game.world.original_key(name) {
                keys.0.insert(action, key_from_table(&key));
            }
        }
        keys
    }

    /// Cancel, or Esc.
    pub(super) fn cancel_key_map(&mut self) {
        self.menus.keys = self.menus.keys_before.clone();
        self.menus.key_capture = None;
        self.menus.stack.pop();
    }

    pub(super) fn mouse_view(&mut self) -> MenuView {
        let mut v = MenuView::new("optmouse");
        let s = self.menus.settings;
        v.text("LBL_TITLE", CONTROLS_TITLE.to_string());
        v.value("SLI_MOUSESEN", (s.mouse_sensitivity * 100.0) as i32, 100);
        v.value("CB_REVBUTTONS", s.reverse_buttons as i32, 1);
        v.copy_control("CB_REVBUTTONS", TOUCH_BOX, TOUCH_BOX_AT);
        v.value(TOUCH_BOX, s.touch_controls as i32, 1);
        v.text(TOUCH_BOX, TOUCH_BOX_WORDS.to_string());
        match &self.menus.hover {
            Some(kotor_ui::Hit::Button(t)) if t == "CB_REVBUTTONS" => {
                let words = self.ui.strref(STR_REVERSE_WORDS);
                v.text("LB_DESC", words);
            }
            Some(kotor_ui::Hit::Button(t)) if t == TOUCH_BOX => {
                v.text("LB_DESC", TOUCH_BOX_ABOUT.to_string());
            }
            _ => v.hide(&["LB_DESC"]),
        }
        v
    }

    pub(super) fn mouse_press(&mut self, tag: &str) {
        let s = &mut self.menus.settings;
        match tag {
            "BTN_BACK" => {
                self.menus.stack.pop();
                return;
            }
            "BTN_DEFAULT" => {
                let d = super::Settings::default();
                (s.mouse_sensitivity, s.reverse_buttons) = (d.mouse_sensitivity, d.reverse_buttons);
                s.touch_controls = d.touch_controls;
            }
            "CB_REVBUTTONS" => s.reverse_buttons = !s.reverse_buttons,
            TOUCH_BOX => s.touch_controls = !s.touch_controls,
            _ => return,
        }
        self.settings_changed();
    }
}

#[cfg(test)]
mod tests {
    use super::KeyMap;

    #[test]
    fn a_settings_file_from_before_the_jump_gives_up_space() {
        let mut k = KeyMap::default();
        k.read("key.Pause=Space\nkey.Inventory=KeyY\n");
        assert_eq!(k.0["Pause"], "KeyX");
        assert_eq!(k.0["Jump"], "Space");
        assert_eq!(k.0["Inventory"], "KeyY");
    }

    #[test]
    fn the_jump_and_the_pause_take_any_key_and_swap() {
        let mut k = KeyMap::default();
        k.bind("Jump", "KeyZ");
        assert_eq!(k.0["Jump"], "KeyZ");
        k.bind("Pause", "KeyZ");
        assert_eq!(
            (k.0["Pause"].as_str(), k.0["Jump"].as_str()),
            ("KeyZ", "KeyX")
        );
    }

    #[test]
    fn a_settings_file_with_the_jump_is_kept_as_it_is() {
        let mut k = KeyMap::default();
        k.read("key.Pause=Space\nkey.Jump=KeyZ\n");
        assert_eq!(k.0["Pause"], "Space");
        assert_eq!(k.0["Jump"], "KeyZ");
    }
}
