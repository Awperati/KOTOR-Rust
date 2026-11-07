//! `optfeedback.gui` and `optautopause.gui`, from the pause menu or the main menu's options, and the words beside them.
//! What each box says comes from the talk table. **Assumed:** which boxes start ticked: the video pauses for an enemy, and
//! the others I could not see. Mine Sighted is greyed: there are no mines to sight.

use super::{Screen, Settings, Stage};
use kotor_ui::{Hit, MenuView, Row};

/// A row of the Feedback list: its name and words. Controller Vibration has no use with a mouse.
const FEEDBACK: [(u32, u32); 5] = [
    (42279, 42286),
    (42280, 42287),
    (42281, 42288),
    (42283, 42289),
    (42285, 42291),
];
/// The owner's rows after those: their names when on and off, and their words. Not in the talk table.
const STYLE_ROWS: [(&str, &str, &str); 14] = [
    (
        "Game font: Noto Sans",
        "Game font: Original",
        "All the game's text in Noto Sans Regular, or in the game's own fonts.",
    ),
    (
        "Text box style: Glow",
        "Text box style: Original",
        "Text boxes black with a glowing blue and yellow edge, or as the game has them.",
    ),
    (
        "Underwear style: Conventional",
        "Underwear style: Original",
        "The player's female bodies in conventional underwear, or as the game has them.",
    ),
    (
        "Early Force sensitivity: On",
        "Early Force sensitivity: Off (original)",
        "The main character feels the Force from the first room, or from Dantooine as the game has it. For new games only: a game keeps what it started with.",
    ),
    (
        "Click to walk: On",
        "Click to walk: Off",
        "A click on the floor walks there. Off: only the keys move the player, and clicks still use what is under them.",
    ),
    (
        "Auto-advance dialogue: On",
        "Auto-advance dialogue: Off",
        "Cutscene lines move on when their voice ends, or after a few seconds with none. Off, they wait for a click or Space.",
    ),
    (
        "Seamless areas: On",
        "Seamless areas: Off",
        "A door opens onto the next area and you walk through, with no loading screen. Off: every door loads the next area on a loading screen, as the game does. Takes hold at the next door.",
    ),
    (
        "English: Australian",
        "English: American (original)",
        "What the game shows is spelt as in Australia: armour, colour, centre. Or as the game's files have it.",
    ),
    (
        "Nameplates: On",
        "Nameplates: Off",
        "A name and a health bar over every enemy in sight. Click one to attack it. F6 turns all of this off and on.",
    ),
    (
        "Neutral creatures' plates: Always",
        "Neutral creatures' plates: Under the cursor",
        "Plates on neutral creatures all the time, or only under the cursor or as the target.",
    ),
    (
        "Friends' plates: Always",
        "Friends' plates: Under the cursor",
        "Plates on the party and allies all the time, or only under the cursor or as the target.",
    ),
    (
        "Object labels: On",
        "Object labels: Off",
        "Containers and remains in gold while they hold loot, and what can be used in cyan.",
    ),
    (
        "Door labels: On",
        "Door labels: Off",
        "Labels on closed doors too.",
    ),
    (
        "Looted containers: Hidden",
        "Looted containers: Say [empty]",
        "What has nothing left has no label, or says so in grey.",
    ),
];
/// The rows after those that go round a list of values.
const VALUE_ROWS: [(&str, &str); 3] = [
    (
        "Nameplate range",
        "How far away an enemy gets a plate, in metres.",
    ),
    ("Nameplate size", "How large plates are, in per cent."),
    (
        "Cel shading",
        "Light in steps and a thin dark outline round characters and props, in the manner of a cartoon. Off, Subtle or Strong. F7 turns it off and on.",
    ),
];
/// What the Cel shading row says.
pub(super) const CEL_NAMES: [&str; 3] = ["Off", "Subtle", "Strong"];
/// A box on the Auto-Pause page, and its words.
const AUTOPAUSE: [(&str, u32); 6] = [
    ("CB_ENDROUND", 42445),
    ("CB_ENEMYSIGHTED", 42446),
    ("CB_MINESIGHTED", 49117),
    ("CB_PARTYKILLED", 42447),
    ("CB_ACTIONMENU", 48416),
    ("CB_TRIGGERS", 48214),
];
/// The box on the Gameplay page that picks the movement, and what it says. It takes the place of the game's Reverse
/// Minigame Y Axis, which was greyed: the page has no room of its own for another.
pub(super) const MOVEMENT_BOX: &str = "CB_REVERSE";
pub(super) const MOVEMENT_MODERN: &str = "Movement: Modern";
pub(super) const MOVEMENT_CLASSIC: &str = "Movement: Classic";
const MOVEMENT_WORDS: &str = "Ticked, Modern: the keys go where the camera looks, with momentum, gravity, a sprint a quarter faster and a roll on its key. Not ticked, Classic: the walk as the game has it.";
/// Words for the boxes on the Gameplay page that do something new.
const GAMEPLAY_WORDS: [(&str, u32); 2] = [("CB_LEVELUP", 42266), ("CB_DISABLEMOVE", 42484)];

/// Graphics boxes: the talk table's words, and why a greyed one is greyed. These need work in the renderer: this one has
/// one sample a pixel, no mipmaps, no shadow pass, no grass and no frame effects.
const GRAPHICS_WORDS: [(&str, u32, Option<&str>); 8] = [
    (
        "CB_SHADOWS",
        47951,
        Some("Not built: no shadows are drawn yet."),
    ),
    ("CB_GRASS", 47955, Some("Not built: no grass is drawn yet.")),
    (
        "BTN_TEXQUAL",
        47965,
        Some("Not built: every texture is used whole."),
    ),
    (
        "BTN_ANTIALIAS",
        47971,
        Some("Not built: one sample a pixel."),
    ),
    (
        "BTN_ANISOTROPY",
        47961,
        Some("Not built: textures have no mipmaps yet."),
    ),
    (
        "CB_FRAMEBUFF",
        47957,
        Some("Not built: no frame effects are drawn yet."),
    ),
    (
        "CB_SOFTSHADOWS",
        47953,
        Some("Not built: no shadows are drawn yet."),
    ),
    ("CB_VSYNC", 47969, None),
];

fn feedback_flag(s: &mut Settings, row: usize) -> Option<&mut bool> {
    Some(match row {
        0 => &mut s.hide_unequippable,
        1 => &mut s.tutorials,
        2 => &mut s.subtitles,
        3 => &mut s.minimap,
        4 => &mut s.floating_numbers,
        5 => &mut s.noto_font,
        6 => &mut s.glow_boxes,
        7 => &mut s.conventional_underwear,
        8 => &mut s.early_force,
        9 => &mut s.click_to_walk,
        10 => &mut s.auto_advance,
        11 => &mut s.seamless_areas,
        12 => &mut s.australian_english,
        13 => &mut s.nameplates,
        14 => &mut s.nameplates_neutral,
        15 => &mut s.nameplates_friendly,
        16 => &mut s.object_labels,
        17 => &mut s.door_labels,
        18 => &mut s.hide_empty,
        _ => return None,
    })
}

/// The value after this one in a list that goes round.
fn next_of(list: &[i32], now: i32) -> i32 {
    let at = list.iter().position(|v| *v == now).map_or(0, |n| n + 1);
    list[at % list.len()]
}

fn autopause_flag<'a>(s: &'a mut Settings, tag: &str) -> Option<&'a mut bool> {
    Some(match tag {
        "CB_ENDROUND" => &mut s.pause_round,
        "CB_ENEMYSIGHTED" => &mut s.pause_enemy,
        "CB_PARTYKILLED" => &mut s.pause_party,
        "CB_ACTIONMENU" => &mut s.pause_action,
        "CB_TRIGGERS" => &mut s.pause_target,
        _ => return None,
    })
}

impl Stage {
    /// The words of what the cursor last rested on, in a page's list for them.
    fn option_words(&mut self, v: &mut MenuView, list: &str) {
        match (self.menus.option_words, self.menus.option_reason) {
            (Some(words), reason) => {
                let text = self.ui.strref(words);
                let text = match reason {
                    Some(why) => format!("{why}\n\n{text}"),
                    None => text,
                };
                v.text(list, text);
            }
            (None, Some(why)) => v.text(list, why),
            (None, None) => v.hide(&[list]),
        }
    }

    pub(super) fn gameplay_words(&mut self, v: &mut MenuView) {
        self.option_words(v, "LB_DESC");
    }

    pub(super) fn graphics_words(&mut self, v: &mut MenuView) {
        self.option_words(v, "LB_DESC");
    }

    /// `optgraphicsadv.gui`. V-Sync is the one box the renderer can honour now.
    pub(super) fn graphics_advanced_view(&mut self) -> MenuView {
        let mut v = MenuView::new("optgraphicsadv");
        v.inert(&[
            "BTN_TEXQUAL",
            "BTN_ANTIALIAS",
            "BTN_ANISOTROPY",
            "CB_FRAMEBUFF",
            "CB_SOFTSHADOWS",
        ]);
        v.hide(&[
            "BTN_TEXQUALLEFT",
            "BTN_TEXQUALRIGHT",
            "BTN_ANTIALIASLEFT",
            "BTN_ANTIALIASRIGHT",
            "BTN_ANISOTROPYLEFT",
            "BTN_ANISOTROPYRIGHT",
        ]);
        v.value("CB_VSYNC", self.menus.settings.vsync as i32, 1);
        self.option_words(&mut v, "LB_DESC");
        v
    }

    pub(super) fn graphics_advanced_press(&mut self, tag: &str) {
        let s = &mut self.menus.settings;
        match tag {
            "CB_VSYNC" => s.vsync = !s.vsync,
            "BTN_DEFAULT" => s.vsync = Settings::default().vsync,
            "BTN_BACK" => {
                self.menus.option_words = None;
                self.menus.stack.pop();
                return;
            }
            "BTN_CANCEL" => {
                s.vsync = self.menus.vsync_before;
                self.menus.option_words = None;
                self.menus.stack.pop();
            }
            _ => return,
        }
        self.menus.wanted_vsync = Some(self.menus.settings.vsync);
        self.settings_changed();
    }

    /// What V-Sync should become, once.
    pub fn take_wanted_vsync(&mut self) -> Option<bool> {
        self.menus.wanted_vsync.take()
    }

    pub(super) fn open_graphics_advanced(&mut self) {
        self.menus.vsync_before = self.menus.settings.vsync;
        self.menus.option_words = None;
        self.menus.stack.push(Screen::GraphicsAdvanced);
    }

    pub(super) fn feedback_view(&mut self) -> MenuView {
        let mut v = MenuView::new("optfeedback");
        let mut settings = self.menus.settings;
        let mut rows: Vec<Row> = FEEDBACK
            .iter()
            .enumerate()
            .map(|(n, (name, _))| Row {
                text: self.ui.strref(*name),
                selected: feedback_flag(&mut settings, n).is_some_and(|f| *f),
                ..Row::default()
            })
            .collect();
        for (n, (on, off, _)) in STYLE_ROWS.iter().enumerate() {
            let ticked = feedback_flag(&mut settings, FEEDBACK.len() + n).is_some_and(|f| *f);
            rows.push(Row {
                text: if ticked { on } else { off }.to_string(),
                selected: ticked,
                ..Row::default()
            });
        }
        for (n, value) in [settings.nameplate_range, settings.nameplate_size]
            .into_iter()
            .enumerate()
        {
            let unit = if n == 0 { "m" } else { "%" };
            rows.push(Row {
                text: format!("{}: {value} {unit}", VALUE_ROWS[n].0),
                selected: true,
                ..Row::default()
            });
        }
        rows.push(Row {
            text: format!(
                "{}: {}",
                VALUE_ROWS[2].0,
                CEL_NAMES[settings.cel_shading.clamp(0, 2) as usize]
            ),
            selected: settings.cel_shading > 0,
            ..Row::default()
        });
        // More rows than fit: the wheel brings up the rest.
        v.lists
            .insert("LB_OPTIONS".to_string(), (rows, self.menus.first));
        self.option_words(&mut v, "LB_DESC");
        v
    }

    pub(super) fn autopause_view(&mut self) -> MenuView {
        let mut v = MenuView::new("optautopause");
        let mut settings = self.menus.settings;
        for (tag, _) in AUTOPAUSE {
            let on = autopause_flag(&mut settings, tag).is_some_and(|f| *f);
            v.value(tag, on as i32, 1);
        }
        v.inert(&["CB_MINESIGHTED"]);
        self.option_words(&mut v, "LB_DETAILS");
        v
    }

    /// The cursor moved over an option page: what is under it says its words.
    pub(super) fn option_hover(&mut self, screen: Screen) {
        if let (Screen::Graphics | Screen::GraphicsAdvanced, Some(Hit::Button(tag))) =
            (screen, &self.menus.hover)
        {
            if let Some((_, words, why)) = GRAPHICS_WORDS.iter().find(|g| g.0 == tag) {
                self.menus.option_words = Some(*words);
                self.menus.option_reason = *why;
            }
            return;
        }
        let words = match (screen, &self.menus.hover) {
            (Screen::Feedback, Some(Hit::Row(_, row))) => FEEDBACK.get(*row).map(|f| f.1),
            (Screen::AutoPause, Some(Hit::Button(tag))) => {
                AUTOPAUSE.iter().find(|a| a.0 == tag).map(|a| a.1)
            }
            (Screen::Gameplay, Some(Hit::Button(tag))) => {
                GAMEPLAY_WORDS.iter().find(|a| a.0 == tag).map(|a| a.1)
            }
            _ => None,
        };
        if words.is_some() {
            self.menus.option_words = words;
            self.menus.option_reason = None;
        }
        if matches!((screen, &self.menus.hover), (Screen::Gameplay, Some(Hit::Button(tag))) if tag == MOVEMENT_BOX)
        {
            self.menus.option_words = None;
            self.menus.option_reason = Some(MOVEMENT_WORDS);
        }
        if let (Screen::Feedback, Some(Hit::Row(_, row))) = (screen, &self.menus.hover) {
            let past = row.checked_sub(FEEDBACK.len());
            let words = past.and_then(|n| STYLE_ROWS.get(n).map(|r| r.2)).or(past
                .and_then(|n| n.checked_sub(STYLE_ROWS.len()))
                .and_then(|n| VALUE_ROWS.get(n).map(|r| r.1)));
            if let Some(words) = words {
                self.menus.option_words = None;
                self.menus.option_reason = Some(words);
            }
        }
    }

    /// The cel shading level the renderer is to draw with.
    pub fn cel_shading(&self) -> u8 {
        self.menus.settings.cel_shading.clamp(0, 2) as u8
    }

    /// The cel shading key: off, or back to what it was (subtle when it was never on).
    pub fn toggle_cel_shading(&mut self) {
        let s = &mut self.menus.settings;
        if s.cel_shading > 0 {
            s.cel_last = s.cel_shading;
            s.cel_shading = 0;
        } else {
            s.cel_shading = s.cel_last.clamp(1, 2);
        }
        self.settings_changed();
    }

    /// The nameplates key: all of them off, or on.
    pub fn toggle_nameplates(&mut self) {
        self.menus.settings.nameplates = !self.menus.settings.nameplates;
        self.settings_changed();
    }

    /// A click on a row of the Feedback list.
    pub(super) fn feedback_row(&mut self, row: usize) {
        if let Some(flag) = feedback_flag(&mut self.menus.settings, row) {
            *flag = !*flag;
        }
        let s = &mut self.menus.settings;
        match row.checked_sub(FEEDBACK.len() + STYLE_ROWS.len()) {
            Some(0) => {
                s.nameplate_range = next_of(&super::super::nameplates::RANGES, s.nameplate_range)
            }
            Some(1) => {
                s.nameplate_size = next_of(&super::super::nameplates::SIZES, s.nameplate_size)
            }
            Some(2) => {
                s.cel_shading = (s.cel_shading + 1) % 3;
                s.cel_last = s.cel_shading.max(1);
            }
            _ => {}
        }
        self.settings_changed();
    }

    pub(super) fn option_page_press(&mut self, screen: Screen, tag: &str) {
        match tag {
            "BTN_BACK" => {
                self.menus.option_words = None;
                self.menus.first = 0;
                self.menus.stack.pop();
            }
            "BTN_DEFAULT" => {
                let d = Settings::default();
                let s = &mut self.menus.settings;
                if screen == Screen::Feedback {
                    s.hide_unequippable = d.hide_unequippable;
                    s.tutorials = d.tutorials;
                    s.subtitles = d.subtitles;
                    s.minimap = d.minimap;
                    s.floating_numbers = d.floating_numbers;
                } else {
                    s.pause_round = d.pause_round;
                    s.pause_enemy = d.pause_enemy;
                    s.pause_party = d.pause_party;
                    s.pause_action = d.pause_action;
                    s.pause_target = d.pause_target;
                }
                self.settings_changed();
            }
            _ => {
                if let Some(flag) = autopause_flag(&mut self.menus.settings, tag) {
                    *flag = !*flag;
                    self.settings_changed();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::Settings;

    /// The level is kept in the settings file and read back, and a level out of range is brought in.
    #[test]
    fn the_cel_shading_level_goes_through_the_settings_file() {
        for level in 0..=2 {
            let s = Settings {
                cel_shading: level,
                ..Settings::default()
            };
            let back = Settings::from_text(&s.to_text());
            assert_eq!(back.cel_shading, level);
        }
        let wild = Settings::from_text("cel_shading=9\n");
        assert_eq!(wild.cel_shading, 2, "brought into range");
        let missing = Settings::from_text("music=0.5\n");
        assert_eq!(
            missing.cel_shading,
            Settings::default().cel_shading,
            "an old file keeps the default"
        );
    }
}
