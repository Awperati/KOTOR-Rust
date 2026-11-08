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
const STYLE_ROWS: [(&str, &str, &str); 25] = [
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
        "Click to move: On",
        "Click to move: Off",
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
        "A name and a health bar over every enemy in sight. Click one to attack it. {Nameplates} turns all of this off and on.",
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
    (
        "Override folder: On",
        "Override folder: Off",
        "Your install's Override folder comes before the game's own files, as in the original game. Takes hold at the next start. What could not be used is in override_problems.txt beside the settings.",
    ),
    (
        "Override scripts: On",
        "Override scripts: Off",
        "Scripts in the Override folder are run too. A mod's script can change rules this game works out for itself, or need a patched game. Takes hold at the next start.",
    ),
    (
        "Override tables: On",
        "Override tables: Off",
        "Tables in the Override folder are used too. A mod's rows can change rules this game works out for itself, or need a patched game. Takes hold at the next start.",
    ),
    (
        "Override screens: On",
        "Override screens: Off",
        "Menu screens in the Override folder are used too. They are laid out for one window size in a patched game, and buttons can land off the screen here. Takes hold at the next start.",
    ),
    (
        "Override talk table: On",
        "Override talk table: Off",
        "A dialog.tlk in the Override folder takes the place of the game's. It changes wording in about 5,200 lines and the sound names of 16,000. Takes hold at the next start.",
    ),
    (
        "Extended story branches: On",
        "Extended story branches: Off",
        "Scenes added to the game's story, as Trask's question when he meets the Dark Jedi on the Endar Spire. Off: the game's own scenes, as they were.",
    ),
    (
        "Ground finishers: On",
        "Ground finishers: Off",
        "A melee blow on someone knocked down, by a Force push or the like, is an automatic critical, and the attacker swings the owner's finisher move once he has made one. Blasters aim down at them either way. Off: no automatic critical.",
    ),
    (
        "Respawning creatures: On",
        "Respawning creatures: Off",
        "An enemy you killed in a fight comes back to where it began after about five minutes, once you are 60 metres from there and cannot see the place. Not story characters, not bosses, not anyone a cutscene killed; they pay half the experience. docs/respawn.tsv has the numbers and the list. Off: they stay dead.",
    ),
    (
        "Compass: On",
        "Compass: Off",
        "A strip at the top middle of the screen, with what is near marked on it and the quest being tracked.",
    ),
    (
        "Auto attack mode: On",
        "Auto attack mode: Off",
        "Attack mode and combat are one thing: it goes on when a fight starts, for you and for each companion. With this on, it goes off by itself 10 seconds after the fight ends. Off, it stays on until you click the Attack mode button, which when lit ends the fight.",
    ),
    (
        "Detection bars: On",
        "Detection bars: Off",
        "A small arrow round the middle of the screen for each enemy that has noticed you, pointing its way. It fills from white to yellow to red as the enemy notices you, and drains when it loses you.",
    ),
];
/// What the Pickup radius goes round, in metres.
const PICKUP_RADII: [i32; 5] = [2, 3, 4, 5, 8];
/// What the Compass size goes round, in per cent.
const COMPASS_SIZES: [i32; 4] = [75, 100, 125, 150];
/// The rows after those that go round a list of values.
const VALUE_ROWS: [(&str, &str); 7] = [
    (
        "Nameplate range",
        "How far away an enemy gets a plate, in metres.",
    ),
    ("Nameplate size", "How large plates are, in per cent."),
    (
        "Cel shading",
        "Light in steps and a thin dark outline round characters and props, in the manner of a cartoon. Off, Subtle or Strong. {CelShading} turns it off and on.",
    ),
    (
        "Pickup radius",
        "How far the Pickup button and its key reach, in metres.",
    ),
    (
        "Anti-aliasing",
        "Smooths jagged edges. Off: none. 4x and 8x: that many samples a pixel, where the graphics card has them (a lesser count where it does not). 16x: 8x samples and a smoothing pass on the finished picture, which also catches thin cut-out edges. Higher costs more speed.",
    ),
    (
        "Motion blur",
        "Smears the picture along the way the camera moved. Off, Subtle, Moderate or Intense. It follows the camera only, not the menus, and stops on loading screens and while the game is paused.",
    ),
    (
        "Compass size",
        "How big the compass at the top of the screen is, in per cent.",
    ),
];
/// The rows after those, under a heading: the folders of the player's own content, what each row says, and what it is for.
const CUSTOM_HEADING: &str = "Custom content";
const CUSTOM_ROWS: [(&str, &str); 3] = [
    (
        "Textures: Open folder",
        "Your own textures, by the game's texture name, as PNG or TGA. {ReloadCustom} reads them again. A click opens the folder, and makes it if it is not there.",
    ),
    (
        "Animations: Open folder",
        "Your own clips, as .anim.txt files. {ReloadCustom} reads them again, and {CustomClip} plays those of your own. A click opens the folder, and makes it if it is not there.",
    ),
    (
        "Meshes: Open folder",
        "Your own garments, as .glb files with items.txt. {ReloadCustom} reads them again, and {CustomGarment} goes through them. A click opens the folder, and makes it if it is not there.",
    ),
];
/// What the Cel shading row says.
pub(super) const CEL_NAMES: [&str; 3] = ["Off", "Subtle", "Strong"];
/// What the Anti-aliasing row says.
pub(super) const AA_NAMES: [&str; 4] = ["Off", "4x", "8x", "16x"];
/// What the Motion blur row says.
pub(super) const BLUR_NAMES: [&str; 4] = ["Off", "Subtle", "Moderate", "Intense"];
/// The Graphics page's buttons for them. The file has none: they go in the free space over Advanced Options, as the owner
/// looked for them there. They are copies of that button, and a click goes on to the next level, as Screen Resolution does.
/// Anti-aliasing is only on the Advanced page, as the owner asked: "Make it only show in the advanced options".
pub(super) const BLUR_BUTTON: &str = "BTN_MOTIONBLUR";
const BLUR_BUTTON_AT: [i32; 4] = [60, 285, 240, 22];
/// The player's own mod packs, under them: Top Dog for the owner, "Mod packs: Off/On, default Off".
pub(super) const MOD_PACKS_BUTTON: &str = "BTN_MODPACKS";
const MOD_PACKS_BUTTON_AT: [i32; 4] = [60, 320, 240, 22];
const MOD_PACKS_WORDS: &str = "On: skyboxes, saber beams, character textures, weapon models, Twi'lek heads and the playable Jawa are read from your mods folder while the game runs. Nothing is copied into the game. It takes hold at the next start.";
/// The Advanced page's own Anti-Aliasing button and its arrows.
const AA_ADVANCED: [&str; 3] = ["BTN_ANTIALIAS", "BTN_ANTIALIASLEFT", "BTN_ANTIALIASRIGHT"];
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
pub(super) const MOVEMENT_MODERN: &str = "Game mode: Modern";
pub(super) const MOVEMENT_CLASSIC: &str = "Game mode: Classic";
const MOVEMENT_WORDS: &str = "This game's mode, kept with its saves, and the next new game's. Ticked, Modern: you move where the camera looks, roll and jump, aim, lock on, and Force keys; only Escape pauses. Not ticked, Classic: the original game, with tank movement, pausing and fights in rounds.";
/// The box that picks the combat. The file has no such box, nor room in its column: it goes under the words, over Close.
pub(super) const COMBAT_BOX: &str = "CB_COMBAT_MODERN";
pub(super) const COMBAT_BOX_AT: [i32; 4] = [338, 214, 240, 34];
pub(super) const COMBAT_MODERN: &str = "Combat: Modern";
pub(super) const COMBAT_CLASSIC: &str = "Combat: Classic";
const COMBAT_WORDS: &str = "Ticked, Modern: a crosshair, aimed shots, sabre chains, and a key for every Force power. It needs Modern movement. Not ticked, Classic: the game's own rounds.";
/// The box for Click to move: the owner, "Add a click to move toggleable setting in the menu settings".
pub(super) const CLICK_BOX: &str = "CB_CLICK_TO_MOVE";
pub(super) const CLICK_BOX_AT: [i32; 4] = [338, 180, 240, 34];
/// How far an option page's help keeps in from its box, in layout units: as far as Gameplay's is from its frame. The
/// owner, by Top Dog: "give the help text a small inner margin on Feedback, Controls, Graphics and Advanced to match Gameplay".
pub(super) const HELP_MARGIN: i32 = 5;
/// The Gameplay page laid out again: the owner, "The gameplay options menu needs a revamp to fit all of the buttons properly".
/// Two columns under headings: the game's own rules on the left, with the two pages of keys under them, and how you move and
/// fight on the right, in the frame the page's picture has there, with what the cursor is on under them in the same frame. Rows
/// 34 to 38 apart, every one inside the page.
pub(super) const GAMEPLAY_PLACES: [(&str, [i32; 4]); 11] = [
    ("BTN_DIFFICULTY", [60, 102, 240, 36]),
    ("BTN_DIFFLEFT", [60, 108, 24, 25]),
    ("BTN_DIFFRIGHT", [276, 108, 24, 25]),
    ("CB_LEVELUP", [60, 140, 240, 36]),
    ("CB_AUTOSAVE", [60, 178, 240, 36]),
    ("CB_INVERTCAM", [60, 216, 240, 36]),
    (MOVEMENT_BOX, [338, 112, 240, 34]),
    ("CB_DISABLEMOVE", [338, 146, 240, 34]),
    ("BTN_MOUSE", [60, 284, 240, 23]),
    ("BTN_KEYMAP", [60, 312, 240, 23]),
    ("LB_DESC", [334, 256, 248, 120]),
];
/// The headings: their copies of the title, their words and places.
pub(super) const GAMEPLAY_HEADINGS: [(&str, &str, [i32; 4]); 3] = [
    ("LBL_HEAD_GAME", "Game", [60, 82, 240, 18]),
    ("LBL_HEAD_MOVE", "Movement and combat", [330, 92, 256, 18]),
    ("LBL_HEAD_KEYS", "Keys, mouse and touch", [60, 262, 240, 18]),
];
pub(super) const CLICK_ON: &str = "Click to move: On";
pub(super) const CLICK_OFF: &str = "Click to move: Off";
const CLICK_WORDS: &str = "On: a left click on the ground, or a tap, walks you there round what is in the way. A click on a person, a door or a thing still uses it, and the movement keys stop the walk. Off: the ground does nothing when clicked.";
/// How far Pickup reaches for each metre its row says: the owner, "Increase the range of the proximity pickup by about 50%".
pub(super) const PICKUP_REACH: f32 = 1.5;
/// Words for the boxes on the Gameplay page that do something new.
const GAMEPLAY_WORDS: [(&str, u32); 2] = [("CB_LEVELUP", 42266), ("CB_DISABLEMOVE", 42484)];

/// Graphics boxes: the talk table's words, and why a greyed one is greyed. These need work in the renderer: this one has
/// one sample a pixel, no mipmaps, no shadow pass, no grass and no frame effects.
const GRAPHICS_WORDS: [(&str, u32, Option<&str>); 7] = [
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
        19 => &mut s.override_folder,
        20 => &mut s.override_scripts,
        21 => &mut s.override_tables,
        22 => &mut s.override_screens,
        23 => &mut s.override_talk,
        24 => &mut s.extended_story,
        25 => &mut s.ground_finishers,
        26 => &mut s.respawn,
        27 => &mut s.compass,
        28 => &mut s.auto_attack_mode,
        29 => &mut s.detection_bars,
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
        // Words are of the page they were found on: one left by Escape, not Close, does not say them on the next.
        let page = self
            .menus
            .stack
            .iter()
            .rev()
            .find(|s| **s != Screen::Tutorial)
            .copied();
        if self.menus.option_words_page != page {
            self.menus.option_words = None;
            self.menus.option_reason = None;
        }
        // Gameplay's box is placed in from its frame; the others' words would touch theirs.
        if page != Some(Screen::Gameplay) {
            v.patches.entry(list.to_string()).or_default().inset = HELP_MARGIN;
        }
        match (self.menus.option_words, self.menus.option_reason) {
            (Some(words), reason) => {
                let text = self.ui.strref(words);
                let text = match reason {
                    Some(why) => format!("{why}\n\n{text}"),
                    None => text,
                };
                v.text(list, text);
            }
            (None, Some(why)) => {
                let words = self.with_keys(why);
                v.text(list, words);
            }
            (None, None) => v.hide(&[list]),
        }
    }

    pub(super) fn gameplay_words(&mut self, v: &mut MenuView) {
        self.option_words(v, "LB_DESC");
    }

    pub(super) fn graphics_words(&mut self, v: &mut MenuView) {
        self.option_words(v, "LB_DESC");
    }

    /// The Graphics page's Anti-aliasing and Motion blur buttons: copies of Advanced Options, saying their levels.
    pub(super) fn graphics_effect_buttons(&mut self, v: &mut MenuView) {
        let s = self.menus.settings;
        v.copy_control("BTN_ADVANCED", BLUR_BUTTON, BLUR_BUTTON_AT);
        v.copy_control("BTN_ADVANCED", MOD_PACKS_BUTTON, MOD_PACKS_BUTTON_AT);
        let packs = if s.mod_packs { "On" } else { "Off" };
        v.text(MOD_PACKS_BUTTON, format!("Mod packs: {packs}"));
        let blur = BLUR_NAMES[s.motion_blur.clamp(0, 3) as usize];
        v.text(BLUR_BUTTON, format!("Motion blur: {blur}"));
    }

    /// A click on one of them: the next level, round again after the last.
    pub(super) fn graphics_effect_press(&mut self, tag: &str) {
        let s = &mut self.menus.settings;
        match tag {
            BLUR_BUTTON => s.motion_blur = (s.motion_blur + 1) % 4,
            MOD_PACKS_BUTTON => s.mod_packs = !s.mod_packs,
            _ => return,
        }
        self.settings_changed();
    }

    /// `optgraphicsadv.gui`. V-Sync and Anti-aliasing are the boxes the renderer can honour now.
    pub(super) fn graphics_advanced_view(&mut self) -> MenuView {
        let mut v = MenuView::new("optgraphicsadv");
        v.inert(&[
            "BTN_TEXQUAL",
            "BTN_ANISOTROPY",
            "CB_FRAMEBUFF",
            "CB_SOFTSHADOWS",
        ]);
        v.hide(&[
            "BTN_TEXQUALLEFT",
            "BTN_TEXQUALRIGHT",
            "BTN_ANISOTROPYLEFT",
            "BTN_ANISOTROPYRIGHT",
        ]);
        let aa = self.menus.settings.anti_aliasing.clamp(0, 3);
        v.text(
            AA_ADVANCED[0],
            format!("Anti-aliasing: {}", AA_NAMES[aa as usize]),
        );
        if aa == 0 {
            v.hide(&[AA_ADVANCED[1]]);
        }
        if aa == 3 {
            v.hide(&[AA_ADVANCED[2]]);
        }
        v.value("CB_VSYNC", self.menus.settings.vsync as i32, 1);
        self.option_words(&mut v, "LB_DESC");
        v
    }

    pub(super) fn graphics_advanced_press(&mut self, tag: &str) {
        let s = &mut self.menus.settings;
        match tag {
            "CB_VSYNC" => s.vsync = !s.vsync,
            "BTN_ANTIALIAS" => {
                s.anti_aliasing = (s.anti_aliasing + 1) % 4;
                self.settings_changed();
                return;
            }
            "BTN_ANTIALIASLEFT" | "BTN_ANTIALIASRIGHT" => {
                let by = if tag == "BTN_ANTIALIASLEFT" { -1 } else { 1 };
                s.anti_aliasing = (s.anti_aliasing + by).clamp(0, 3);
                self.settings_changed();
                return;
            }
            "BTN_DEFAULT" => {
                s.vsync = Settings::default().vsync;
                s.anti_aliasing = Settings::default().anti_aliasing;
            }
            "BTN_BACK" => {
                self.menus.option_words = None;
                self.menus.stack.pop();
                return;
            }
            "BTN_CANCEL" => {
                s.vsync = self.menus.vsync_before;
                s.anti_aliasing = self.menus.anti_aliasing_before;
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
        self.menus.anti_aliasing_before = self.menus.settings.anti_aliasing;
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
        rows.push(Row {
            text: format!(
                "{}: {} m",
                VALUE_ROWS[3].0,
                settings.pickup_radius as f32 * PICKUP_REACH
            ),
            selected: true,
            ..Row::default()
        });
        rows.push(Row {
            text: format!(
                "{}: {}",
                VALUE_ROWS[4].0,
                AA_NAMES[settings.anti_aliasing.clamp(0, 3) as usize]
            ),
            selected: settings.anti_aliasing > 0,
            ..Row::default()
        });
        rows.push(Row {
            text: format!(
                "{}: {}",
                VALUE_ROWS[5].0,
                BLUR_NAMES[settings.motion_blur.clamp(0, 3) as usize]
            ),
            selected: settings.motion_blur > 0,
            ..Row::default()
        });
        rows.push(Row {
            text: format!("{}: {} %", VALUE_ROWS[6].0, settings.compass_size),
            selected: true,
            ..Row::default()
        });
        rows.push(Row {
            text: CUSTOM_HEADING.to_string(),
            dim: true,
            ..Row::default()
        });
        for (words, _) in CUSTOM_ROWS {
            rows.push(Row {
                text: words.to_string(),
                ..Row::default()
            });
        }
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
        let before = (self.menus.option_words, self.menus.option_reason);
        self.option_hover_words(screen);
        if (self.menus.option_words, self.menus.option_reason) != before {
            self.menus.option_words_page = Some(screen);
        }
    }

    fn option_hover_words(&mut self, screen: Screen) {
        if let (Screen::Graphics | Screen::GraphicsAdvanced, Some(Hit::Button(tag))) =
            (screen, &self.menus.hover)
        {
            let ours = if AA_ADVANCED.contains(&tag.as_str()) {
                Some(VALUE_ROWS[4].1)
            } else if tag == BLUR_BUTTON {
                Some(VALUE_ROWS[5].1)
            } else if tag == MOD_PACKS_BUTTON {
                Some(MOD_PACKS_WORDS)
            } else {
                None
            };
            if ours.is_some() {
                self.menus.option_words = None;
                self.menus.option_reason = ours;
            } else if let Some((_, words, why)) = GRAPHICS_WORDS.iter().find(|g| g.0 == tag) {
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
        if matches!((screen, &self.menus.hover), (Screen::Gameplay, Some(Hit::Button(tag))) if tag == COMBAT_BOX)
        {
            self.menus.option_words = None;
            self.menus.option_reason = Some(COMBAT_WORDS);
        }
        if matches!((screen, &self.menus.hover), (Screen::Gameplay, Some(Hit::Button(tag))) if tag == CLICK_BOX)
        {
            self.menus.option_words = None;
            self.menus.option_reason = Some(CLICK_WORDS);
        }
        if let (Screen::Feedback, Some(Hit::Row(_, row))) = (screen, &self.menus.hover) {
            let past = row.checked_sub(FEEDBACK.len());
            let past_style = past.and_then(|n| n.checked_sub(STYLE_ROWS.len()));
            let past_values = past_style.and_then(|n| n.checked_sub(VALUE_ROWS.len() + 1));
            let words = past
                .and_then(|n| STYLE_ROWS.get(n).map(|r| r.2))
                .or(past_style.and_then(|n| VALUE_ROWS.get(n).map(|r| r.1)))
                .or(past_values.and_then(|n| CUSTOM_ROWS.get(n).map(|r| r.1)));
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

    /// Click to move, as the Gameplay page sets it. For a test or a driver.
    pub fn set_click_to_move(&mut self, on: bool) {
        self.menus.settings.click_to_walk = on;
        self.settings_changed();
    }

    /// How far Pickup reaches now, in metres.
    pub fn pickup_reach(&self) -> f32 {
        self.menus.settings.pickup_radius as f32 * PICKUP_REACH
    }

    /// The anti-aliasing level the renderer is to draw with: 0 off, 1 4x, 2 8x, 3 16x.
    pub fn anti_aliasing(&self) -> u8 {
        self.menus.settings.anti_aliasing.clamp(0, 3) as u8
    }

    /// The motion blur level the renderer is to draw with: 0 off, 1 subtle, 2 moderate, 3 intense.
    pub fn motion_blur(&self) -> u8 {
        self.menus.settings.motion_blur.clamp(0, 3) as u8
    }

    /// The nameplates key: all of them off, or on.
    pub fn toggle_nameplates(&mut self) {
        self.menus.settings.nameplates = !self.menus.settings.nameplates;
        self.settings_changed();
    }

    /// A folder a row has asked to be opened: the window does that, and makes it if it is not there.
    pub fn take_folder_to_open(&mut self) -> Option<std::path::PathBuf> {
        self.menus.folder_to_open.take()
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
            Some(3) => s.pickup_radius = next_of(&PICKUP_RADII, s.pickup_radius),
            Some(4) => s.anti_aliasing = (s.anti_aliasing + 1) % 4,
            Some(5) => s.motion_blur = (s.motion_blur + 1) % 4,
            Some(6) => s.compass_size = next_of(&COMPASS_SIZES, s.compass_size),
            Some(8) => self.menus.folder_to_open = crate::texture_override::folder(),
            Some(9) => self.menus.folder_to_open = Stage::anim_folder(),
            Some(10) => self.menus.folder_to_open = Stage::mesh_folder(),
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
    fn anti_aliasing_and_motion_blur_go_through_the_settings_file() {
        for level in 0..=3 {
            let s = Settings {
                anti_aliasing: level,
                motion_blur: 3 - level,
                ..Settings::default()
            };
            let back = Settings::from_text(&s.to_text());
            assert_eq!(back.anti_aliasing, level);
            assert_eq!(back.motion_blur, 3 - level);
        }
        let wild = Settings::from_text(
            "anti_aliasing=9
motion_blur=-4
",
        );
        assert_eq!(
            (wild.anti_aliasing, wild.motion_blur),
            (3, 0),
            "brought into range"
        );
        let old = Settings::from_text(
            "music=0.5
",
        );
        assert_eq!(
            (old.anti_aliasing, old.motion_blur),
            (1, 1),
            "an old file gets 4x and subtle"
        );
    }

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
