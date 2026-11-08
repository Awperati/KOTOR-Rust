//! `optkeymapping.gui` and `optmouse.gui`, from the Gameplay page. The page lists what this engine can do with a key, in
//! the words of `keymap.2da`'s rows.
//!
//! **Differences:** the table has more rows, for things not built, and those are left out. Its keys differ from these in
//! places (P for the character, L for quests, Z and C to strafe). The first keys are the ones `docs/PLAY.md` lists, which
//! came first here, and the Default button goes between them and the table's, called Original. A key taken from another
//! action swaps with it. Mini Games, of which none are built, became the Force page: each power can have a key, Modern combat's
//! only. **Assumed:** reversed buttons swap everywhere, on the screens too.

use super::{Screen, Stage};
use kotor_ui::{MenuView, Row};
use std::collections::BTreeMap;

/// The roll and the jump are not in the game, so they have no words in the talk table.
const NOT_IN_TALK_TABLE: u32 = 0;
/// An action: what it is called, the talk table's name for it, its page, and its key by `winit`'s name for it.
const KEY_ACTIONS: [(&str, u32, usize, &str); 29] = [
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
    ("Pause", 48019, 1, "KeyB"),
    ("CancelCombat", 48315, 1, "KeyF"),
    // The owner: "Pressing T opens the Proximity Container Pickup". F was Pickup's out of a fight and Cancel Combat's in one.
    ("Pickup", NOT_IN_TALK_TABLE, 1, "KeyT"),
    ("AttackMode", NOT_IN_TALK_TABLE, 1, "KeyX"),
    ("LockOn", NOT_IN_TALK_TABLE, 1, "KeyQ"),
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
/// The Force page, on what was the Mini Games button.
const FORCE_PAGE: usize = 2;
const FORCE_WORDS: &str = "Force";
/// Each power, by its row of `spells.2da`, which gives its name. Their keys start empty: few letters are free.
const FORCE_ROWS: [(&str, i32); 42] = [
    ("Force4", 4),
    ("Force7", 7),
    ("Force8", 8),
    ("Force9", 9),
    ("Force10", 10),
    ("Force11", 11),
    ("Force12", 12),
    ("Force13", 13),
    ("Force15", 15),
    ("Force16", 16),
    ("Force17", 17),
    ("Force18", 18),
    ("Force19", 19),
    ("Force20", 20),
    ("Force22", 22),
    ("Force23", 23),
    ("Force24", 24),
    ("Force25", 25),
    ("Force26", 26),
    ("Force27", 27),
    ("Force28", 28),
    ("Force29", 29),
    ("Force30", 30),
    ("Force31", 31),
    ("Force32", 32),
    ("Force33", 33),
    ("Force34", 34),
    ("Force35", 35),
    ("Force36", 36),
    ("Force37", 37),
    ("Force38", 38),
    ("Force40", 40),
    ("Force41", 41),
    ("Force42", 42),
    ("Force43", 43),
    ("Force44", 44),
    ("Force45", 45),
    ("Force46", 46),
    ("Force47", 47),
    ("Force48", 48),
    ("Force49", 49),
    ("Force50", 50),
];
/// What a key has in front of its name with Alt held, and what a row with no key shows.
const ALT: &str = "Alt+";
const NO_KEY: &str = "-";

/// The power an action on the Force page is.
pub(in crate::stage) fn force_row(action: &str) -> Option<i32> {
    FORCE_ROWS.iter().find(|r| r.0 == action).map(|r| r.1)
}

/// The Force page's actions and their powers, in its order.
pub(in crate::stage) fn force_rows() -> &'static [(&'static str, i32)] {
    &FORCE_ROWS
}

const FILTERS: [&str; 3] = ["BTN_Filter_Move", "BTN_Filter_Game", "BTN_Filter_Mini"];
const STR_REVERSE_WORDS: u32 = 48450;
/// The page is about more than the mouse now, and the box for the on-screen controls goes under the file's.
const CONTROLS_TITLE: &str = "Controls";
const TOUCH_BOX: &str = "CB_TOUCH";
const TOUCH_BOX_AT: [i32; 4] = [60, 232, 240, 40];
const TOUCH_BOX_WORDS: &str = "On-screen Controls";
/// The look stick's speed, under that: a copy of the mouse's slider and its name.
pub(super) const LOOK_SLIDER: &str = "SLI_LOOKSEN";
const LOOK_SLIDER_AT: [i32; 4] = [60, 297, 240, 24];
const LOOK_LABEL: &str = "LBL_LOOKSEN";
const LOOK_LABEL_AT: [i32; 4] = [60, 272, 240, 24];
const LOOK_WORDS: &str = "Look Stick Speed";
const LOOK_ABOUT: &str =
    "How fast the on-screen look stick turns the camera. The Invert Camera box goes for it too.";
const TOUCH_BOX_ABOUT: &str = "A stick, round buttons and bigger menu keys on the HUD, for a touch screen or the mouse. The blue button at the top right turns them on and off too.";
const LIST: &str = "LST_EventList";
/// A settings file written since the owner's "Make X attack mode and B the Pause button rather", and his "Pressing T opens the
/// Proximity Container Pickup", says so.
const KEY_DEFAULTS_LINE: &str = "key_defaults=3";
/// Each moved default: what the key was, and what it is.
const MOVED_DEFAULTS: [(&str, &str, &str); 3] = [
    ("Pause", "KeyX", "KeyB"),
    ("AttackMode", "KeyB", "KeyX"),
    ("Pickup", "KeyF", "KeyT"),
];
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
        let keys: String = self
            .0
            .iter()
            .map(|(action, key)| format!("key.{action}={key}\n"))
            .collect();
        format!("{KEY_DEFAULTS_LINE}\n{keys}")
    }

    /// A file from before the two keys changed places: what still has its old key goes to its new one, if nothing else has
    /// that. What the player moved stays where they put it.
    fn move_to_new_defaults(&mut self) {
        let unmoved = |k: &KeyMap, (action, old, _): (&str, &str, &str)| {
            k.0.get(action).is_some_and(|key| key == old)
        };
        // One a file never named has its new key already, as the defaults give it.
        let moved_already = |k: &KeyMap, (action, _, new): (&str, &str, &str)| {
            k.0.get(action).is_some_and(|key| key == new)
        };
        if MOVED_DEFAULTS
            .iter()
            .all(|m| unmoved(self, *m) || moved_already(self, *m))
        {
            for (action, _, new) in MOVED_DEFAULTS {
                // A key the player gave to something else stays theirs: no two on one.
                let taken = self
                    .0
                    .iter()
                    .any(|(a, k)| k == new && !MOVED_DEFAULTS.iter().any(|m| m.0 == *a));
                if !taken {
                    self.0.insert(action, new.to_string());
                }
            }
            return;
        }
        for moved in MOVED_DEFAULTS {
            let (action, _, new) = moved;
            if unmoved(self, moved) && !self.0.values().any(|key| key == new) {
                self.0.insert(action, new.to_string());
            }
        }
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
            let actions = KEY_ACTIONS.iter().map(|a| a.0);
            let known = actions.chain(FORCE_ROWS.iter().map(|r| r.0));
            if let Some(action) = known.into_iter().find(|a| *a == name) {
                self.0.insert(action, key.to_string());
            }
        }
        if !text.lines().any(|l| l == KEY_DEFAULTS_LINE) {
            self.move_to_new_defaults();
        }
        // A file from before the lock-on may have its key on something the player chose: that stays, and the lock-on has none.
        if !text.contains("key.LockOn=") {
            let lock = KeyMap::default().0["LockOn"].clone();
            if self.0.iter().any(|(a, k)| *a != "LockOn" && *k == lock) {
                self.0.insert("LockOn", String::new());
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
        // A power that gave up its key to one with none has none.
        self.0
            .retain(|a, k| !k.is_empty() || force_row(a).is_none());
    }

    /// These keys without the Force page's.
    fn without_powers(&self) -> KeyMap {
        let mut keys = self.clone();
        keys.0.retain(|a, _| force_row(a).is_none());
        keys
    }

    /// The Default button leaves the Force page's keys alone, except any that a first key now needs.
    fn keep_powers_of(&mut self, other: &KeyMap) {
        for (action, key) in other.0.iter().filter(|(a, _)| force_row(a).is_some()) {
            if !self.0.values().any(|k| k == key) {
                self.0.insert(action, key.clone());
            }
        }
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

/// A key as the page shows it: `KeyW` is W, `Digit1` is 1, `ArrowUp` is Up, and `Alt+KeyG` is Alt+G.
fn pretty(key: &str) -> String {
    let (alt, plain) = key.strip_prefix(ALT).map_or(("", key), |k| (ALT, k));
    let plain = plain
        .strip_prefix("Key")
        .or(plain.strip_prefix("Digit"))
        .or(plain.strip_prefix("Arrow"))
        .unwrap_or(plain);
    format!("{alt}{plain}")
}

impl Stage {
    /// The key of an action, as the page shows it.
    pub(super) fn key_words(&self, action: &str) -> String {
        let key = self.menus.keys.0.get(action).cloned().unwrap_or_default();
        pretty(&key)
    }

    /// What a key does in the game, by `winit`'s name for it. With Alt held, a key with nothing of its own does what it always does.
    pub fn action_for_key(&self, key: &str) -> Option<&'static str> {
        let mut on_key = self.menus.keys.0.iter().filter(|(_, k)| *k == key);
        let Some(first) = on_key.next().map(|(a, _)| *a) else {
            return key
                .strip_prefix(ALT)
                .and_then(|plain| self.action_for_key(plain));
        };
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
            out = out.replace(&format!("{{{action}}}"), &pretty(key));
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
        if self.medication_warning_key(key) {
            return true;
        }
        let Some(action) = self.menus.key_capture else {
            return false;
        };
        // Alt alone is half of a key: the page waits for the rest.
        if matches!(key, "AltLeft" | "AltRight") {
            return true;
        }
        self.menus.key_capture = None;
        self.menus.key_refused = None;
        let plain = key.strip_prefix(ALT).unwrap_or(key);
        if FIXED_KEYS.contains(&plain) || key == "Alt+F4" {
            return true;
        }
        // A power with no key to give back would leave the other action with none.
        let keys = &self.menus.keys.0;
        let taken = keys
            .iter()
            .any(|(a, k)| k == key && *a != action && force_row(a).is_none());
        if force_row(action).is_some() && !keys.contains_key(action) && taken {
            self.menus.key_refused = Some((action, key.to_string()));
            return true;
        }
        self.menus.keys.bind(action, key);
        true
    }

    /// The key for Cancel Combat: attack mode off, as a click on the lit Attack mode button does. The Disengage button is gone.
    pub fn hud_disengage(&mut self) {
        if !self.menu_open() {
            self.disengage();
        }
    }

    pub(super) fn open_key_map(&mut self) {
        self.menus.keys_before = self.menus.keys.clone();
        self.menus.key_page = 0;
        self.menus.key_capture = None;
        self.menus.key_refused = None;
        self.menus.first = 0;
        self.menus.stack.push(Screen::KeyMap);
    }

    fn page_actions(&mut self) -> Vec<(&'static str, u32)> {
        let page = self.menus.key_page;
        if page == FORCE_PAGE {
            let w = &mut self.game.world;
            return FORCE_ROWS
                .iter()
                .map(|(action, row)| {
                    let words = w.power_row(*row).map(|r| r.0);
                    (*action, words.unwrap_or(NOT_IN_TALK_TABLE))
                })
                .collect();
        }
        KEY_ACTIONS
            .iter()
            .filter(|a| a.2 == page)
            .map(|a| (a.0, a.1))
            .collect()
    }

    pub(super) fn key_map_view(&mut self) -> MenuView {
        let mut v = MenuView::new("optkeymapping");
        v.text(FILTERS[FORCE_PAGE], FORCE_WORDS);
        // The button says what a click gives.
        if self.menus.keys.without_powers() == KeyMap::default() {
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
            let refused = self.menus.key_refused.as_ref();
            let shown = match refused.filter(|(a, _)| *a == action) {
                _ if waiting => "...".to_string(),
                Some((_, taken)) => format!("{} is taken", pretty(taken)),
                None if key.is_empty() => NO_KEY.to_string(),
                None => pretty(&key),
            };
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
        self.menus.key_refused = None;
        self.menus.key_capture = self.page_actions().get(row).map(|a| a.0);
    }

    pub(super) fn key_map_press(&mut self, tag: &str) {
        self.menus.key_capture = None;
        self.menus.key_refused = None;
        match tag {
            "BTN_Default" => {
                let mut keys = if self.menus.keys.without_powers() == KeyMap::default() {
                    self.original_keys()
                } else {
                    KeyMap::default()
                };
                keys.keep_powers_of(&self.menus.keys);
                self.menus.keys = keys;
            }
            "BTN_Accept" => {
                self.menus.stack.pop();
                self.settings_changed();
            }
            "BTN_Cancel" => self.cancel_key_map(),
            _ => {
                if let Some(page) = FILTERS.iter().position(|f| *f == tag) {
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
        v.copy_control("LBL_MOUSESEN", LOOK_LABEL, LOOK_LABEL_AT);
        v.text(LOOK_LABEL, LOOK_WORDS.to_string());
        v.copy_control("SLI_MOUSESEN", LOOK_SLIDER, LOOK_SLIDER_AT);
        v.value(LOOK_SLIDER, (s.look_stick_speed * 100.0) as i32, 100);
        v.patches.entry("LB_DESC".to_string()).or_default().inset = super::optionpages::HELP_MARGIN;
        match &self.menus.hover {
            Some(kotor_ui::Hit::Slider(t, _)) if t == LOOK_SLIDER => {
                v.text("LB_DESC", LOOK_ABOUT.to_string());
            }
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
                s.look_stick_speed = d.look_stick_speed;
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
    use super::{force_row, pretty, KeyMap, KEY_DEFAULTS_LINE};

    #[test]
    fn a_power_starts_with_no_key_and_keeps_one_through_the_file() {
        let mut k = KeyMap::default();
        assert!(k.0.keys().all(|a| force_row(a).is_none()));
        k.bind("Force23", "KeyG");
        k.bind("Force50", "Alt+KeyW");
        let text = k.to_text();
        assert!(text.contains("key.Force23=KeyG"));
        let mut again = KeyMap::default();
        again.read(&text);
        assert_eq!(again.0["Force23"], "KeyG");
        assert_eq!(again.0["Force50"], "Alt+KeyW");
        assert_eq!(again.0["MoveForward"], "KeyW", "Alt+W is another key");
        // A file from before the page has none, and one from after is no trouble for what it does not name.
        let mut old = KeyMap::default();
        old.read("key.Inventory=KeyY\nkey.Jump=Space\nkey.NoSuchThing=KeyZ\n");
        assert!(old.0.keys().all(|a| force_row(a).is_none()));
        assert_eq!(old.0["Inventory"], "KeyY");
    }

    #[test]
    fn a_file_from_before_the_lock_on_keeps_what_it_put_on_its_key() {
        let mut k = KeyMap::default();
        k.read("key.Inventory=KeyY\nkey.Jump=Space\n");
        assert_eq!(k.0["LockOn"], "KeyQ");
        let mut k = KeyMap::default();
        k.read("key.Force23=KeyQ\nkey.Jump=Space\n");
        assert_eq!(k.0["Force23"], "KeyQ");
        assert_eq!(k.0["LockOn"], "");
    }

    #[test]
    fn two_powers_swap_and_one_that_gives_up_its_key_has_none() {
        let mut k = KeyMap::default();
        k.bind("Force23", "KeyG");
        k.bind("Force50", "KeyH");
        k.bind("Force23", "KeyH");
        assert_eq!(k.0["Force23"], "KeyH");
        assert_eq!(k.0["Force50"], "KeyG");
        k.bind("Force46", "KeyG");
        assert!(!k.0.contains_key("Force50"));
        assert_eq!(pretty("Alt+KeyG"), "Alt+G");
        assert_eq!(pretty("KeyG"), "G");
    }

    #[test]
    fn a_settings_file_from_before_the_jump_gives_up_space() {
        let mut k = KeyMap::default();
        k.read("key.Pause=Space\nkey.Inventory=KeyY\n");
        assert_eq!(k.0["Pause"], "KeyB");
        assert_eq!(k.0["Jump"], "Space");
        assert_eq!(k.0["Inventory"], "KeyY");
    }

    #[test]
    fn pickup_goes_from_f_to_t_unless_t_is_taken() {
        // A file from after the X and B swap, with Pickup on F as it was.
        let mut k = KeyMap::default();
        k.read("key_defaults=2\nkey.Pickup=KeyF\nkey.CancelCombat=KeyF\n");
        assert_eq!(k.0["Pickup"], "KeyT");
        assert_eq!(k.0["CancelCombat"], "KeyF");
        // The player put something on T: Pickup stays where it was.
        let mut k = KeyMap::default();
        k.read("key_defaults=2\nkey.Pickup=KeyF\nkey.Inventory=KeyT\n");
        assert_eq!(k.0["Pickup"], "KeyF");
        assert_eq!(k.0["Inventory"], "KeyT");
    }

    #[test]
    fn the_jump_and_the_pause_take_any_key_and_swap() {
        let mut k = KeyMap::default();
        k.bind("Jump", "KeyZ");
        assert_eq!(k.0["Jump"], "KeyZ");
        k.bind("Pause", "KeyZ");
        assert_eq!(
            (k.0["Pause"].as_str(), k.0["Jump"].as_str()),
            ("KeyZ", "KeyB")
        );
    }

    #[test]
    fn a_settings_file_from_before_the_swap_goes_to_the_new_keys_if_it_never_moved_them() {
        let mut k = KeyMap::default();
        k.read("key.Pause=KeyX\nkey.AttackMode=KeyB\nkey.Jump=Space\n");
        assert_eq!(k.0["Pause"], "KeyB");
        assert_eq!(k.0["AttackMode"], "KeyX");
    }

    #[test]
    fn what_the_player_moved_before_the_swap_stays_and_the_other_goes_if_it_can() {
        // The pause moved, and attack mode did not.
        let mut k = KeyMap::default();
        k.read("key.Pause=KeyG\nkey.AttackMode=KeyB\n");
        assert_eq!(k.0["Pause"], "KeyG");
        assert_eq!(k.0["AttackMode"], "KeyX");
        // Attack mode moved, and the pause did not.
        let mut k = KeyMap::default();
        k.read("key.Pause=KeyX\nkey.AttackMode=KeyG\n");
        assert_eq!(k.0["Pause"], "KeyB");
        assert_eq!(k.0["AttackMode"], "KeyG");
        // Something else has the new key: no two on one, so it stays.
        let mut k = KeyMap::default();
        k.read("key.Pause=KeyX\nkey.AttackMode=KeyG\nkey.Inventory=KeyB\n");
        assert_eq!(k.0["Pause"], "KeyX");
        assert_eq!(k.0["Inventory"], "KeyB");
    }

    #[test]
    fn a_settings_file_from_after_the_swap_is_kept_as_it_is_even_at_the_old_keys() {
        // The player has put them back: it is not done over at each start.
        let mut k = KeyMap::default();
        k.bind("Pause", "KeyX");
        assert_eq!(
            (k.0["Pause"].as_str(), k.0["AttackMode"].as_str()),
            ("KeyX", "KeyB")
        );
        let text = k.to_text();
        assert!(text.contains(KEY_DEFAULTS_LINE));
        let mut again = KeyMap::default();
        again.read(&text);
        assert_eq!(again.0["Pause"], "KeyX");
        assert_eq!(again.0["AttackMode"], "KeyB");
    }

    #[test]
    fn a_settings_file_with_the_jump_is_kept_as_it_is() {
        let mut k = KeyMap::default();
        k.read("key.Pause=Space\nkey.Jump=KeyZ\n");
        assert_eq!(k.0["Pause"], "Space");
        assert_eq!(k.0["Jump"], "KeyZ");
    }
}
