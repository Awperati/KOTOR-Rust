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
const STYLE_ROWS: [(&str, &str, &str); 9] = [
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
        "Movement: Modern",
        "Movement: Classic",
        "Modern: momentum, gravity, a sprint a quarter faster and a roll on its key. Classic: the walk as the game has it.",
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
];
/// A box on the Auto-Pause page, and its words.
const AUTOPAUSE: [(&str, u32); 6] = [
    ("CB_ENDROUND", 42445),
    ("CB_ENEMYSIGHTED", 42446),
    ("CB_MINESIGHTED", 49117),
    ("CB_PARTYKILLED", 42447),
    ("CB_ACTIONMENU", 48416),
    ("CB_TRIGGERS", 48214),
];
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
        9 => &mut s.movement_modern,
        10 => &mut s.click_to_walk,
        11 => &mut s.auto_advance,
        12 => &mut s.seamless_areas,
        13 => &mut s.australian_english,
        _ => return None,
    })
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
        // Thirteen rows, and six fit: the wheel brings up the rest.
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
        if let (Screen::Feedback, Some(Hit::Row(_, row))) = (screen, &self.menus.hover) {
            if let Some((_, _, words)) = row
                .checked_sub(FEEDBACK.len())
                .and_then(|n| STYLE_ROWS.get(n))
            {
                self.menus.option_words = None;
                self.menus.option_reason = Some(words);
            }
        }
    }

    /// A click on a row of the Feedback list.
    pub(super) fn feedback_row(&mut self, row: usize) {
        if let Some(flag) = feedback_flag(&mut self.menus.settings, row) {
            *flag = !*flag;
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
