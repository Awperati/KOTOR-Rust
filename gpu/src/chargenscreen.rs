//! Making a character, in the order the reference video has it from 0:22: `classsel.gui`, then `maincg.gui` with
//! `qorcpnl.gui` and `quickpnl.gui` or `custpnl.gui` over it, and a page for each step: `portcust`, `abchrgen`, `skchrgen`,
//! `ftchrgen` and `name`. New Game starts what comes out. The rules are `World::create_character`: see `docs/CHARGEN.md`.
//!
//! The player object of the world under the main menu is what the screens show and the pages spend on. Play makes a fresh
//! world from the spec, as any new game does. **Assumed:** Back undoes the step before it, and Cancel on the steps' panel
//! goes back to Quick or Custom: the video only goes forward. Quick Character takes the class's recommended scores, skills
//! and feats. The mist round the figures is the emitters of `cgmain_light.mdl` and `cghead_light.mdl`. Their lights are not
//! used, and the video's mist is a little thicker.

use super::{Screen, Stage};
use crate::effects::{Effects, ParticleBatch};
use kotor_game::{
    attribute_cost, BodySize, CharacterLook, CharacterSpec, Clip, ATTRIBUTE_BASE, POINT_POOL,
};
use kotor_render::face::FaceShape;
use kotor_render::face::LEKKU_KEY;
use kotor_render::marks::{HAIR_COLOURS, HAIR_KEY, MARKS, MARK_MOST, MARK_STEP};
use kotor_render::races;
use kotor_render::shape::{BodyShape, SLIDER_MOST, SLIDER_STEP};
use kotor_render::underwear::{list_underwear_variants, UnderwearVariant};
use kotor_render::{Camera, Rig, Scene};
use kotor_ui::{Hit, MenuView, Row};
use std::rc::Rc;

const SOLDIER: i32 = 0;
const SCOUT: i32 = 1;
const SCOUNDREL: i32 = 2;
const MALE: i32 = 0;
const FEMALE: i32 = 1;
const STR_MALE: u32 = 646;
const STR_FEMALE: u32 = 647;
const STR_QUICK_HELP: u32 = 241;
const STR_CUSTOM_HELP: u32 = 242;
/// Each button of `classsel.gui`, as the video has them: the class and the figure's place. From the words at 0:24.
const FIGURES: [(&str, &str, i32, i32); 6] = [
    ("BTN_SEL1", "3D_MODEL1", SCOUNDREL, MALE),
    ("BTN_SEL2", "3D_MODEL2", SCOUT, MALE),
    ("BTN_SEL3", "3D_MODEL3", SOLDIER, MALE),
    ("BTN_SEL4", "3D_MODEL4", SOLDIER, FEMALE),
    ("BTN_SEL5", "3D_MODEL5", SCOUT, FEMALE),
    ("BTN_SEL6", "3D_MODEL6", SCOUNDREL, FEMALE),
];
/// Where the figures' holes are in `classsel.gui`: left, top, width, height.
const FIGURE_HOLES: [[f32; 4]; 6] = [
    [58.0, 121.0, 89.0, 207.0],
    [155.0, 131.0, 69.0, 187.0],
    [242.0, 131.0, 69.0, 187.0],
    [328.0, 131.0, 69.0, 187.0],
    [416.0, 131.0, 69.0, 187.0],
    [503.0, 131.0, 69.0, 187.0],
];
/// How much more each side the chosen one has, as the file has the first slot and the video the one the mouse is on.
const CHOSEN_GROWS: f32 = 10.0;
const BUTTON_OVER_HOLE: f32 = 3.0;
const SHEET_HOLE: [f32; 4] = [155.0, 89.0, 162.0, 186.0];
/// Where the head is on the portrait page, for the whole body.
const BODY_HOLE: [f32; 4] = [330.0, 150.0, 162.0, 195.0];
/// The row the file has not got on that page, in the free space under its picture.
const UNDERWEAR_LEFT: &str = "BTN_UNDERWEAR_LEFT";
const UNDERWEAR_RIGHT: &str = "BTN_UNDERWEAR_RIGHT";
const UNDERWEAR_WORDS: &str = "LBL_UNDERWEAR";
const UNDERWEAR_LEFT_AT: [i32; 4] = [150, 345, 27, 47];
const UNDERWEAR_RIGHT_AT: [i32; 4] = [463, 345, 27, 47];
const UNDERWEAR_WORDS_AT: [i32; 4] = [180, 345, 280, 47];
/// A Custom character's page for how they look, on the Options pages' frame, with the figure on the right.
const LOOK_TITLE: &str = "Appearance";
const LOOK_LIST: &str = "LB_OPTIONS";
const LOOK_FIGURE: &str = "LOOK_FIGURE";
const LOOK_HOLE: [f32; 4] = [330.0, 80.0, 256.0, 320.0];
const STR_OK: u32 = 1580;
const STR_CANCEL: u32 = 1581;
/// How far a drag across the figure turns it: radians for each layout unit, so that the hole's width is most of a turn.
const TURN_PER_UNIT: f32 = 0.02;
/// What each view shows, down from the top of the head in metres. The whole body has no number.
const VIEWS: [(&str, Option<f32>); 3] = [
    ("Full body", None),
    ("Upper body", Some(0.95)),
    ("Head", Some(0.38)),
];
/// Air above and below what a view shows, as a share of it.
const VIEW_MARGIN: f32 = 0.06;
/// The idle poses: their words, and the clip, which every player body has.
const POSES: [(&str, &str); 4] = [
    ("Idle 1", "pause1"),
    ("Idle 2", "pause2"),
    ("Idle 3", "pause3"),
    ("Listening", "listen"),
];
/// The lights: their words, how far the figure and camera turn together under the one lamp, and how bright, where 0 is as
/// everywhere else. The lamp lights both faces of a surface alike, so a light from the left is a light from the right.
const LIGHTS: [(&str, f32, f32); 4] = [
    ("Standard", 0.0, 0.0),
    ("Front", 0.54, 1.25),
    ("Side", -1.03, 1.0),
    ("Low", 0.0, 0.6),
];
/// The sliders of the page: the name a look keeps each by, its words, and whether it is of the face. In the order of
/// `kotor_render::shape::BODY_SLIDERS` and then `kotor_render::face::FACE_SLIDERS`.
const SLIDERS: [(&str, &str, bool); 13] = [
    ("height", "Height", false),
    ("shoulders", "Shoulders", false),
    ("chest", "Chest", false),
    ("waist", "Waist", false),
    ("hips", "Hips", false),
    ("arms", "Arms", false),
    ("legs", "Legs", false),
    ("brow", "Brow", true),
    ("nose", "Nose", true),
    ("jaw", "Jaw", true),
    ("cheeks", "Cheeks", true),
    ("eyes", "Eyes", true),
    ("mouth", "Mouth", true),
];
/// The view a face slider brings the figure to, from the whole body, and the one the lekku slider does.
const FACE_VIEW: usize = 2;
const LEKKU_VIEW: usize = 1;
use super::STOCK_UNDERWEAR;
const STOCK_UNDERWEAR_NAME: &str = "None (the game's own)";
/// Saved looks: the file beside the settings that keeps them, and how far a random look's sliders go each way, in per cent.
const LOOKS_FILE: &str = "looks.txt";
const RANDOM_MOST: i32 = 60;
/// One row of that page.
#[derive(Clone, Copy, PartialEq)]
enum LookRow {
    Head,
    Body,
    Underwear,
    /// A slider, by its place in `SLIDERS`.
    Slider(usize),
    /// A scar, tattoo or make-up, by its place in `kotor_render::marks::MARKS`.
    Mark(usize),
    /// The hair's colour, from `kotor_render::marks::HAIR_COLOURS`.
    Hair,
    /// How the player looks: Human, or another race's head on their body. See `kotor_render::races`.
    Race,
    /// A Twi'lek's head-tails, longer or shorter.
    Lekku,
    /// Every slider at random, and the head and the body.
    Random,
    /// Every slider back to 0.
    Reset,
    /// The looks saved before: the arrows go through them, and each goes on the figure.
    Saved,
    /// Keep this look with the others.
    Save,
    View,
    Pose,
    Light,
}
/// The tag each attribute starts with, its place in the scores, and the talk table's words about it.
const ATTRIBUTES: [(&str, usize, u32); 6] = [
    ("STR", 0, 222),
    ("DEX", 1, 223),
    ("CON", 2, 224),
    ("INT", 3, 226),
    ("WIS", 4, 225),
    ("CHA", 5, 227),
];
/// A standing figure's height, with a little air, and the camera's view.
const FIGURE_HEIGHT: f32 = 1.9;
const FIGURE_MARGIN: f32 = 1.1;
const FIGURE_FOV: f32 = 30.0;
/// The figure faces the camera, which looks along +y.
const FACING_CAMERA: f32 = -std::f32::consts::FRAC_PI_2;
/// The head on the portrait page: where its middle goes on the screen, how much of the world the screen's height shows,
/// and how far below the top of the body its middle is. By eye from the video at 0:31.
const HEAD_AT: (f32, f32) = (451.0, 232.0);
const HEAD_VIEW: f32 = 0.95;
const HEAD_BELOW_TOP: f32 = 0.14;
/// The head turns a little away from the portrait, as in the video.
const HEAD_FACING: f32 = -1.25;

const BODY_MIST: &str = "cgmain_light";
const HEAD_MIST: &str = "cghead_light";
/// Seconds of mist made at once when a screen opens, so that it is there already.
const MIST_START: f32 = 6.0;
const MIST_STEP: f32 = 1.0 / 30.0;

#[derive(Clone, Copy, PartialEq)]
enum Step {
    Portrait,
    Attributes,
    Skills,
    Feats,
    Name,
    Play,
}
const QUICK_STEPS: [Step; 3] = [Step::Portrait, Step::Name, Step::Play];
const CUSTOM_STEPS: [Step; 6] = [
    Step::Portrait,
    Step::Attributes,
    Step::Skills,
    Step::Feats,
    Step::Name,
    Step::Play,
];

/// A character being made.
pub(super) struct Draft {
    spec: CharacterSpec,
    /// Quick or Custom, once chosen.
    custom: Option<bool>,
    /// The first step not done.
    next: usize,
    /// The portraits on offer, and the one on show.
    portraits: Vec<i32>,
    pick: usize,
    /// What a page found, for its Cancel.
    page_pick: usize,
    page_body: BodySize,
    page_scores: [i32; 6],
    /// The underwear on offer, the one on show, and what its page found for its Cancel.
    underwear: Vec<UnderwearVariant>,
    underwear_pick: usize,
    page_underwear: usize,
    /// The portrait page shows the whole body, not the head: the underwear has just been changed.
    body_on_show: bool,
    /// The attribute whose words are on show.
    about: usize,
    /// The numbers have been settled, and the sheet shows them.
    settled: bool,
    /// How the Appearance page shows the figure: turned this far, and which view, pose and light. Only for looking.
    turn: f32,
    view: usize,
    pose: usize,
    light: usize,
    /// The cursor is over the figure, for the wheel.
    over_figure: bool,
    /// How a body stands at rest, so that the view does not breathe with the idle: for its appearance and sliders.
    measured: Option<(i32, CharacterLook, Stance)>,
    /// What the Appearance page found of the sliders, for its Cancel.
    page_look: CharacterLook,
    /// The looks kept in the file beside the settings, and the one on the figure.
    saved: Vec<SavedLook>,
    saved_pick: Option<usize>,
    /// Where the random looks are in their sequence.
    seed: u64,
}

/// A look kept to use again: its name, the head, the body and the sliders.
#[derive(Clone, PartialEq)]
pub(super) struct SavedLook {
    name: String,
    portrait: i32,
    body: BodySize,
    sliders: std::collections::BTreeMap<String, i32>,
}

impl SavedLook {
    /// One line of the file: the name, the portrait's row, the body and the sliders, between tabs.
    fn to_line(&self) -> String {
        let body = match self.body {
            BodySize::Small => "small",
            BodySize::Medium => "medium",
            BodySize::Large => "large",
        };
        let sliders: Vec<String> = self
            .sliders
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        format!(
            "{}\t{}\t{body}\t{}",
            self.name.replace('\t', " "),
            self.portrait,
            sliders.join(",")
        )
    }

    fn from_line(line: &str) -> Option<SavedLook> {
        let mut parts = line.split('\t');
        let name = parts.next()?.trim().to_string();
        let portrait = parts.next()?.trim().parse().ok()?;
        let body = match parts.next()?.trim() {
            "small" => BodySize::Small,
            "large" => BodySize::Large,
            _ => BodySize::Medium,
        };
        let sliders = parts
            .next()
            .unwrap_or("")
            .split(',')
            .filter_map(|kv| {
                let (k, v) = kv.split_once('=')?;
                let v: i32 = v.trim().parse().ok()?;
                (v != 0).then(|| (k.trim().to_string(), v.clamp(-SLIDER_MOST, SLIDER_MOST)))
            })
            .collect();
        (!name.is_empty()).then_some(SavedLook {
            name,
            portrait,
            body,
            sliders,
        })
    }
}

/// The looks in a file's text, one a line.
pub(super) fn read_looks(text: &str) -> Vec<SavedLook> {
    text.lines().filter_map(SavedLook::from_line).collect()
}

/// A step of a small random sequence: the next number, and the seed after it.
fn next_random(seed: &mut u64) -> u64 {
    // xorshift64*.
    let mut x = (*seed).max(1);
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    *seed = x;
    x.wrapping_mul(0x2545_F491_4F6C_DD1D)
}

/// How low and high a body stands at rest, facing the camera, and the middle across of what each view shows.
#[derive(Clone, Copy)]
struct Stance {
    low: f32,
    top: f32,
    middles: [[f32; 2]; 3],
}

impl Draft {
    fn steps(&self) -> &'static [Step] {
        match self.custom {
            Some(true) => &CUSTOM_STEPS,
            Some(false) => &QUICK_STEPS,
            None => &[],
        }
    }

    fn spent(&self) -> i32 {
        self.spec
            .abilities
            .iter()
            .map(|s| attribute_cost(*s).unwrap_or(0))
            .sum()
    }
}

/// The three saves in their box: the file has them 14 apart, too close for the words. Each row's controls, and their places
/// across and how wide.
pub(super) fn spread_saves(v: &mut MenuView) {
    const ROWS: [(i32, &str, &str, &str, &str); 3] = [
        (
            357,
            "LBL_FORTITUDE",
            "OLD_FORT_LBL",
            "FORT_ARROW_LBL",
            "NEW_FORT_LBL",
        ),
        (
            373,
            "LBL_REFLEX",
            "OLD_REFL_LBL",
            "REFL_ARROW_LBL",
            "NEW_REFL_LBL",
        ),
        (
            389,
            "LBL_WILL",
            "OLD_WILL_LBL",
            "WILL_ARROW_LBL",
            "NEW_WILL_LBL",
        ),
    ];
    const HIGH: i32 = 21;
    for (top, name, old, arrow, new) in ROWS {
        for (tag, left, wide) in [
            (name, 325, 158),
            (old, 488, 42),
            (arrow, 515, 34),
            (new, 532, 42),
        ] {
            v.patches.entry(tag.to_string()).or_default().place = Some([left, top, wide, HIGH]);
        }
    }
}

impl Stage {
    /// New Game goes through these screens. Off unless asked for, as a test wants a game at once.
    /// Whether Settings' early Force choice counts: off for tests and the pilot, which play the game's own rules.
    pub fn enable_early_force(&mut self, on: bool) {
        self.menus.early_force_on = on;
    }

    /// For a driver whose game began before anything could ask for the rule: the same new game, with it.
    pub fn restart_with_early_force(&mut self) -> Result<(), String> {
        self.menus.early_force_on = true;
        let module = self.first_module.clone();
        self.start_module(&module)
    }

    pub fn enable_chargen(&mut self, on: bool) {
        self.menus.chargen_on = on;
    }

    /// New Game: the first of the screens, or a game at once when they are off.
    pub(super) fn begin_new_game(&mut self) -> bool {
        if !self.menus.chargen_on {
            return false;
        }
        self.menus.chargen = None;
        self.menus.hover = None;
        self.set_preview_underwear("");
        self.menus.stack.push(Screen::ChargenClass);
        true
    }

    /// The character the player has made, for the world of a new game.
    pub(in crate::stage) fn take_new_character(&mut self) -> Option<CharacterSpec> {
        self.menus.new_character.take()
    }

    fn figure_portrait(&mut self, gender: i32, slot: usize) -> Option<i32> {
        let all = self.game.world.player_portraits(gender);
        all.get(slot % all.len().max(1)).copied()
    }

    fn sync_character(&mut self) {
        if let Some(d) = &self.menus.chargen {
            let spec = d.spec.clone();
            if let Err(e) = self.game.world.create_character(&spec) {
                self.menus.message = e;
            }
        }
    }

    /// `classsel.gui`, with a hole for each figure.
    pub(super) fn chargen_class_view(&mut self) -> MenuView {
        let mut v = MenuView::new("classsel");
        v.holes = FIGURES.iter().map(|f| f.1.to_string()).collect();
        if let Some(Hit::Button(tag)) = &self.menus.hover {
            if let Some(n) = FIGURES.iter().position(|f| f.0 == tag) {
                self.menus.chargen_big = n;
            }
        }
        for (n, (button, hole, _, _)) in FIGURES.iter().enumerate() {
            let [x, y, w, h] = self.class_figure_hole(n);
            let at = |grow: f32| {
                [
                    (x - grow) as i32,
                    (y - grow) as i32,
                    (w + 2.0 * grow) as i32,
                    (h + 2.0 * grow) as i32,
                ]
            };
            v.patches.entry(hole.to_string()).or_default().place = Some(at(0.0));
            v.patches.entry(button.to_string()).or_default().place = Some(at(BUTTON_OVER_HOLE));
        }
        let under = match &self.menus.hover {
            Some(Hit::Button(tag)) => FIGURES.iter().find(|f| f.0 == tag),
            _ => None,
        };
        match under {
            Some(&(_, _, class, gender)) => {
                let (class_word, words) = match class {
                    SOLDIER => (134, 32111),
                    SCOUT => (133, 32110),
                    _ => (135, 32109),
                };
                let gender_word = if gender == MALE { STR_MALE } else { STR_FEMALE };
                let title = format!(
                    "{} {}",
                    self.ui.strref(gender_word),
                    self.ui.strref(class_word)
                );
                v.text("LBL_CLASS", title);
                v.text("LBL_DESC", self.ui.strref(words));
            }
            None => {
                v.text("LBL_CLASS", "");
                v.text("LBL_DESC", "");
            }
        }
        v
    }

    /// A figure's hole: the file's first is big and the rest small, and the big one goes where the mouse has been.
    fn class_figure_hole(&self, slot: usize) -> [f32; 4] {
        let [x, _, w, _] = FIGURE_HOLES[slot];
        let [_, small_y, small_w, small_h] = FIGURE_HOLES[1];
        let (cx, grow) = (
            x + w / 2.0,
            if slot == self.menus.chargen_big {
                CHOSEN_GROWS
            } else {
                0.0
            },
        );
        [
            cx - small_w / 2.0 - grow,
            small_y - grow,
            small_w + 2.0 * grow,
            small_h + 2.0 * grow,
        ]
    }

    fn figure_holes(&self) -> [[f32; 4]; 6] {
        [0, 1, 2, 3, 4, 5].map(|n| self.class_figure_hole(n))
    }

    fn choose_figure(&mut self, slot: usize) {
        let (_, _, class, gender) = FIGURES[slot];
        let w = &mut self.game.world;
        let portraits = w.player_portraits(gender);
        let pick = slot % portraits.len().max(1);
        let Some(portrait) = portraits.get(pick).copied() else {
            return;
        };
        self.set_preview_underwear("");
        // The mod packs may have been switched on since the game began.
        self.find_mod_heads();
        self.menus.chargen = Some(Draft {
            // One that does not fit yet is not offered.
            underwear: list_underwear_variants()
                .into_iter()
                .filter(|v| v.fits)
                .chain(std::iter::once(UnderwearVariant {
                    id: STOCK_UNDERWEAR.to_string(),
                    name: STOCK_UNDERWEAR_NAME.to_string(),
                    fits: true,
                    note: "The game's own body, as it is painted".to_string(),
                }))
                .collect(),
            underwear_pick: 0,
            page_underwear: 0,
            body_on_show: false,
            spec: CharacterSpec {
                name: String::new(),
                class,
                portrait,
                body: BodySize::of_class(class),
                abilities: [ATTRIBUTE_BASE; 6],
                skills: Vec::new(),
                feats: Vec::new(),
                look: CharacterLook::default(),
            },
            custom: None,
            next: 0,
            portraits,
            pick,
            page_pick: pick,
            page_body: BodySize::of_class(class),
            page_scores: [ATTRIBUTE_BASE; 6],
            about: 0,
            settled: false,
            turn: 0.0,
            view: 0,
            pose: 0,
            light: 0,
            over_figure: false,
            measured: None,
            page_look: CharacterLook::default(),
            saved: Vec::new(),
            saved_pick: None,
            seed: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos() as u64 | 1),
        });
        self.sync_character();
        self.menus.stack.push(Screen::ChargenMain);
        self.menus.hover = None;
    }

    /// `maincg.gui`, as the level-up panel has it, with the panel for where the character has got to.
    pub(super) fn chargen_main_view(&mut self) -> MenuView {
        let mut v = MenuView::new("maincg");
        spread_saves(&mut v);
        v.holes.push("MODEL_LBL".to_string());
        v.fill("PORTRAIT_LBL", self.portrait());
        v.hide(&[
            "OLD_VIT_LBL",
            "NEW_VIT_LBL",
            "VIT_ARROW_LBL",
            "OLD_DEF_LBL",
            "NEW_DEF_LBL",
            "DEF_ARROW_LBL",
            "OLD_LBL",
            "NEW_LBL",
            "OLD_FORT_LBL",
            "OLD_REFL_LBL",
            "OLD_WILL_LBL",
            "FORT_ARROW_LBL",
            "REFL_ARROW_LBL",
            "WILL_ARROW_LBL",
            "LBL_LEVEL",
            "LBL_LEVEL_VAL",
        ]);
        let Some((custom, next, settled, steps, name)) = self
            .menus
            .chargen
            .as_ref()
            .map(|d| (d.custom, d.next, d.settled, d.steps(), d.spec.name.clone()))
        else {
            return v;
        };
        v.text("LBL_NAME", name);
        let pc = self.game.world.pc;
        if let Some(s) = self.game.world.sheet(pc) {
            v.text("LBL_CLASS", s.class);
            if settled {
                v.text("LBL_VIT", s.max_hp.to_string());
                v.text("LBL_DEF", s.defense.to_string());
                for (n, (tag, _, _)) in ATTRIBUTES.iter().enumerate() {
                    v.text(&format!("{tag}_AB_LBL"), s.abilities[n].0.to_string());
                }
                let saves = [s.fortitude, s.reflex, s.will];
                for (n, tag) in ["FORT", "REFL", "WILL"].iter().enumerate() {
                    v.text(&format!("NEW_{tag}_LBL"), saves[n].to_string());
                }
            } else {
                v.hide(&[
                    "LBL_VIT",
                    "LBL_DEF",
                    "NEW_FORT_LBL",
                    "NEW_REFL_LBL",
                    "NEW_WILL_LBL",
                ]);
            }
        }
        if !settled {
            for (tag, _, _) in ATTRIBUTES {
                v.hide(&[&format!("{tag}_AB_LBL")]);
            }
        }
        let panel = match custom {
            None => {
                let mut p = MenuView::new("qorcpnl");
                let words = match &self.menus.hover {
                    Some(Hit::Button(tag)) if tag == "CUST_CHAR_BTN" => STR_CUSTOM_HELP,
                    _ => STR_QUICK_HELP,
                };
                p.text("LB_DESC", self.ui.strref(words));
                p
            }
            Some(custom) => {
                let mut p = MenuView::new(if custom { "custpnl" } else { "quickpnl" });
                if next == 0 {
                    p.inert(&["BTN_BACK"]);
                }
                if custom {
                    p.text("BTN_STEPNAME1", LOOK_TITLE);
                }
                for n in 0..steps.len() {
                    let tag = format!("BTN_STEPNAME{}", n + 1);
                    if n == next {
                        p.lit(&tag);
                        p.fill(&format!("LBL_{}", n + 1), "lbl_cg_circ2");
                    } else if n > next {
                        p.inert(&[&tag]);
                    }
                }
                p
            }
        };
        let mut panel = panel;
        panel.hover = self.menus.hover.clone();
        v.top = Some(Box::new(panel));
        v
    }

    /// Quick Character: everything as the class's tables recommend it.
    fn quick_build(&mut self) {
        let Some(class) = self.menus.chargen.as_ref().map(|d| d.spec.class) else {
            return;
        };
        let scores = self.game.world.recommended_abilities(class);
        if let (Some(d), Some(scores)) = (self.menus.chargen.as_mut(), scores) {
            d.spec.abilities = scores;
            d.spec.skills.clear();
            d.spec.feats.clear();
            d.settled = true;
        }
        self.sync_character();
        let w = &mut self.game.world;
        let pc = w.pc;
        for skill in w.recommended_skills(class) {
            while w.buy_skill_rank(pc, skill).is_ok() {}
        }
        let mut feats = Vec::new();
        while let Some(feat) = w.recommended_feat(pc) {
            if w.choose_feat(pc, feat).is_err() {
                break;
            }
            feats.push(feat);
        }
        let skills = owned_skills(self);
        if let Some(d) = self.menus.chargen.as_mut() {
            d.spec.skills = skills;
            d.spec.feats = feats;
        }
    }

    fn finish_step(&mut self, step: Step) {
        if let Some(d) = self.menus.chargen.as_mut() {
            if let Some(n) = d.steps().iter().position(|s| *s == step) {
                d.next = d.next.max(n + 1);
            }
        }
    }

    /// Back on the panel: the step before it is undone.
    fn step_back(&mut self) {
        let Some(d) = self.menus.chargen.as_mut() else {
            return;
        };
        if d.next == 0 {
            d.custom = None;
            d.settled = false;
            d.spec.abilities = [ATTRIBUTE_BASE; 6];
            d.spec.skills.clear();
            d.spec.feats.clear();
        } else {
            d.next -= 1;
            match d.steps()[d.next] {
                Step::Attributes => {
                    d.spec.abilities = [ATTRIBUTE_BASE; 6];
                    d.settled = false;
                }
                Step::Skills => d.spec.skills.clear(),
                Step::Feats => d.spec.feats.clear(),
                _ => {}
            }
        }
        self.sync_character();
    }

    fn do_step(&mut self, step: Step) {
        match step {
            Step::Portrait => {
                if let Some(d) = self.menus.chargen.as_mut() {
                    d.page_pick = d.pick;
                }
                if let Some(d) = self.menus.chargen.as_mut() {
                    d.page_underwear = d.underwear_pick;
                    d.page_body = d.spec.body;
                    d.page_look = d.spec.look.clone();
                    d.body_on_show = false;
                }
                let saved = self
                    .looks_file()
                    .and_then(|p| std::fs::read_to_string(p).ok());
                if let Some(d) = self.menus.chargen.as_mut() {
                    d.saved = saved.map_or(Vec::new(), |t| read_looks(&t));
                    d.saved_pick = None;
                }
                // Quick Character keeps the game's short page.
                let custom = self.menus.chargen.as_ref().and_then(|d| d.custom);
                self.menus.stack.push(if custom == Some(true) {
                    Screen::ChargenLook
                } else {
                    Screen::ChargenPortrait
                });
                self.menus.hover = None;
                self.menus.first = 0;
            }
            Step::Attributes => {
                if let Some(d) = self.menus.chargen.as_mut() {
                    d.page_scores = d.spec.abilities;
                    d.about = 0;
                }
                self.menus.stack.push(Screen::ChargenAttributes);
                self.menus.hover = None;
                self.tutorial(super::notices::TUTORIAL_LEVEL_ABILITIES);
            }
            Step::Skills => self.enter_level_page(Screen::LevelSkills),
            Step::Feats => self.enter_level_page(Screen::LevelFeats),
            Step::Name => {
                self.menus.name = self
                    .menus
                    .chargen
                    .as_ref()
                    .map_or(String::new(), |d| d.spec.name.clone());
                self.menus.stack.push(Screen::ChargenName);
                self.menus.hover = None;
            }
            Step::Play => self.play_new_character(),
        }
    }

    /// The character goes into a new game.
    fn play_new_character(&mut self) {
        let Some(d) = self.menus.chargen.take() else {
            return;
        };
        self.menus.new_character = Some(d.spec.clone());
        self.new_game_now();
        // It says why on the screen, and the character is still there.
        if !self.menus.stack.is_empty() {
            self.menus.chargen = Some(d);
        }
    }

    /// A page of the level-up screens has been left with OK: what it did is part of the character.
    pub(super) fn chargen_page_accepted(&mut self, step: usize, taken: Vec<i32>) {
        const SKILLS_PAGE: usize = 1;
        let skills = owned_skills(self);
        let Some(d) = self.menus.chargen.as_mut() else {
            return;
        };
        if step == SKILLS_PAGE {
            d.spec.skills = skills;
            self.finish_step(Step::Skills);
        } else {
            d.spec.feats = taken;
            self.finish_step(Step::Feats);
        }
        self.sync_character();
    }

    /// `portcust.gui`: the portrait and, in a hole, the head. The file has no row for underwear, so it gets one under them.
    pub(super) fn chargen_portrait_view(&mut self) -> MenuView {
        let mut v = MenuView::new("portcust");
        v.holes.push("LBL_HEAD".to_string());
        v.fill("LBL_PORTRAIT", self.portrait());
        let pc = self.game.world.pc;
        let gender = self.game.world.object(pc).map_or(MALE, |o| o.gender);
        let Some(d) = self.menus.chargen.as_ref() else {
            return v;
        };
        // The variants are for the player's female bodies: a male one has nothing to change.
        let choice = gender == FEMALE && d.underwear.len() > 1;
        if let (true, Some(wear)) = (choice, d.underwear.get(d.underwear_pick)) {
            let words = format!("Underwear: {}", wear.name);
            v.copy_control("BTN_ARRL", UNDERWEAR_LEFT, UNDERWEAR_LEFT_AT);
            v.copy_control("BTN_ARRR", UNDERWEAR_RIGHT, UNDERWEAR_RIGHT_AT);
            v.copy_control("SUB_TITLE_LBL", UNDERWEAR_WORDS, UNDERWEAR_WORDS_AT);
            v.text(UNDERWEAR_WORDS, words);
        }
        v
    }

    /// The rows of the Appearance page. The underwear variants are for the player's female bodies.
    fn look_rows(&self) -> Vec<LookRow> {
        let w = &self.game.world;
        let gender = w.object(w.pc).map_or(MALE, |o| o.gender);
        let choice = self
            .menus
            .chargen
            .as_ref()
            .is_some_and(|d| gender == FEMALE && d.underwear.len() > 1);
        let race = self
            .menus
            .chargen
            .as_ref()
            .map_or(0, |d| races::race_of(|k| slider(&d.spec.look, k)));
        let mut rows = vec![LookRow::Race, LookRow::Head];
        // A race with a body of its own, the Jawa, is all robe and hood: no body size, underwear, face or hair to change.
        if !races::own_body(race) {
            rows.push(LookRow::Body);
            if choice {
                rows.push(LookRow::Underwear);
            }
            // A race of its own has no hair to colour. A Twi'lek has head-tails instead.
            if race == 0 {
                rows.push(LookRow::Hair);
            } else {
                rows.push(LookRow::Lekku);
            }
            rows.extend((0..SLIDERS.len()).map(LookRow::Slider));
            rows.extend((0..MARKS.len()).map(LookRow::Mark));
        }
        rows.extend([
            LookRow::Random,
            LookRow::Reset,
            LookRow::Saved,
            LookRow::Save,
        ]);
        rows.extend([LookRow::View, LookRow::Pose, LookRow::Light]);
        rows
    }

    /// The Appearance page: `optfeedback.gui`, with a list of what can be changed, and the figure where its words go.
    pub(super) fn chargen_look_view(&mut self) -> MenuView {
        let mut v = MenuView::new("optfeedback");
        v.text("LBL_TITLE", LOOK_TITLE);
        v.hide(&["LB_DESC", "BTN_DEFAULT"]);
        v.copy_control("LBL_TITLE", LOOK_FIGURE, LOOK_HOLE.map(|n| n as i32));
        v.text(LOOK_FIGURE, "");
        v.holes.push(LOOK_FIGURE.to_string());
        v.copy_control("BTN_BACK", "BTN_ACCEPT", [113, 410, 146, 28]);
        v.text("BTN_ACCEPT", self.ui.strref(STR_OK));
        v.text("BTN_BACK", self.ui.strref(STR_CANCEL));
        let rows = self.look_rows();
        let d_woman = {
            let w = &self.game.world;
            w.object(w.pc).is_some_and(|o| o.gender == FEMALE)
        };
        let Some(d) = self.menus.chargen.as_ref() else {
            return v;
        };
        let rows = rows
            .into_iter()
            .map(|row| {
                let (name, choice) = match row {
                    LookRow::Lekku => {
                        let v = slider(&d.spec.look, LEKKU_KEY);
                        let words = if v == 0 {
                            "0".to_string()
                        } else {
                            format!("{v:+}")
                        };
                        ("Lekku".to_string(), words)
                    }
                    LookRow::Race => (
                        "Race".to_string(),
                        races::RACES[races::race_of(|k| slider(&d.spec.look, k))].to_string(),
                    ),
                    LookRow::Head => {
                        let race = races::race_of(|k| slider(&d.spec.look, k));
                        let heads = races::heads(race, d_woman).len();
                        let choice = if race == 0 || heads == 0 {
                            format!("Portrait {} of {}", d.pick + 1, d.portraits.len())
                        } else {
                            let n = slider(&d.spec.look, races::RACE_HEAD_KEY)
                                .clamp(0, heads as i32 - 1);
                            format!("{} {} of {heads}", races::RACES[race], n + 1)
                        };
                        ("Preset".to_string(), choice)
                    }
                    LookRow::Body => (
                        "Body".to_string(),
                        match d.spec.body {
                            BodySize::Small => "Small",
                            BodySize::Medium => "Medium",
                            BodySize::Large => "Large",
                        }
                        .to_string(),
                    ),
                    LookRow::Underwear => (
                        "Underwear".to_string(),
                        d.underwear
                            .get(d.underwear_pick)
                            .map_or(String::new(), |u| u.name.clone()),
                    ),
                    LookRow::Slider(n) => {
                        let v = slider(&d.spec.look, SLIDERS[n].0);
                        let words = if v == 0 {
                            "0".to_string()
                        } else {
                            format!("{v:+}")
                        };
                        (SLIDERS[n].1.to_string(), words)
                    }
                    LookRow::Hair => {
                        let n =
                            slider(&d.spec.look, HAIR_KEY).clamp(0, HAIR_COLOURS.len() as i32 - 1);
                        (
                            "Hair colour".to_string(),
                            HAIR_COLOURS[n as usize].0.to_string(),
                        )
                    }
                    LookRow::Mark(n) => {
                        let v = slider(&d.spec.look, MARKS[n].0);
                        let words = if v <= 0 {
                            "None".to_string()
                        } else {
                            format!("{v}%")
                        };
                        (MARKS[n].1.to_string(), words)
                    }
                    LookRow::Random => ("Random look".to_string(), "Roll".to_string()),
                    LookRow::Reset => ("Sliders".to_string(), "All to 0".to_string()),
                    LookRow::Saved => (
                        "Saved look".to_string(),
                        match (d.saved_pick, d.saved.len()) {
                            (_, 0) => "None yet".to_string(),
                            (Some(n), _) => {
                                d.saved.get(n).map_or(String::new(), |l| l.name.clone())
                            }
                            (None, all) => format!("{all} saved"),
                        },
                    ),
                    LookRow::Save => ("Save this look".to_string(), "Save".to_string()),
                    LookRow::View => ("View".to_string(), VIEWS[d.view].0.to_string()),
                    LookRow::Pose => ("Pose".to_string(), POSES[d.pose].0.to_string()),
                    LookRow::Light => ("Lighting".to_string(), LIGHTS[d.light].0.to_string()),
                };
                Row {
                    text: name,
                    choice: Some(choice),
                    ..Row::default()
                }
            })
            .collect();
        // The list is longer than the page: the wheel and a drag go down it.
        let first = self.menus.first;
        v.lists.insert(LOOK_LIST.to_string(), (rows, first));
        v
    }

    /// An arrow on a row of the Appearance page: it goes on the figure at once.
    pub(super) fn look_step(&mut self, row: usize, by: i32) {
        match self.look_rows().get(row) {
            Some(LookRow::Race) => self.turn_race(by),
            Some(LookRow::Lekku) => {
                if let Some(d) = self.menus.chargen.as_mut() {
                    // The tails hang to the shoulders: from the whole body or the head, the view comes to the upper body.
                    d.view = LEKKU_VIEW;
                    let v = (slider(&d.spec.look, LEKKU_KEY) + by * SLIDER_STEP)
                        .clamp(-SLIDER_MOST, SLIDER_MOST);
                    if v == 0 {
                        d.spec.look.sliders.remove(LEKKU_KEY);
                    } else {
                        d.spec.look.sliders.insert(LEKKU_KEY.to_string(), v);
                    }
                }
                self.sync_character();
            }
            Some(LookRow::Head) => self.load_preset(by),
            Some(LookRow::Underwear) => self.turn_underwear(by),
            Some(LookRow::Body) => {
                const SIZES: [BodySize; 3] = [BodySize::Small, BodySize::Medium, BodySize::Large];
                if let Some(d) = self.menus.chargen.as_mut() {
                    let now = SIZES.iter().position(|b| *b == d.spec.body).unwrap_or(1);
                    d.spec.body = SIZES[(now as i32 + by).rem_euclid(3) as usize];
                }
                self.sync_character();
            }
            Some(LookRow::Slider(n)) => {
                if let Some(d) = self.menus.chargen.as_mut() {
                    let (name, _, face) = SLIDERS[*n];
                    // A face is shaped where it can be seen: from the whole body, the view comes to the head.
                    if face && d.view == 0 {
                        d.view = FACE_VIEW;
                    }
                    let v = (slider(&d.spec.look, name) + by * SLIDER_STEP)
                        .clamp(-SLIDER_MOST, SLIDER_MOST);
                    if v == 0 {
                        d.spec.look.sliders.remove(name);
                    } else {
                        d.spec.look.sliders.insert(name.to_string(), v);
                    }
                }
                self.sync_character();
            }
            Some(LookRow::Hair) => {
                if let Some(d) = self.menus.chargen.as_mut() {
                    // Hair is on the head: from the whole body, the view comes to it.
                    if d.view == 0 {
                        d.view = FACE_VIEW;
                    }
                    let now = slider(&d.spec.look, HAIR_KEY).max(0) as usize;
                    let next = cycle(now.min(HAIR_COLOURS.len() - 1), by, HAIR_COLOURS.len());
                    if next == 0 {
                        d.spec.look.sliders.remove(HAIR_KEY);
                    } else {
                        d.spec
                            .look
                            .sliders
                            .insert(HAIR_KEY.to_string(), next as i32);
                    }
                }
                self.sync_character();
            }
            Some(LookRow::Mark(n)) => {
                if let Some(d) = self.menus.chargen.as_mut() {
                    let name = MARKS[*n].0;
                    // A mark is on the face: from the whole body, the view comes to the head.
                    if d.view == 0 {
                        d.view = FACE_VIEW;
                    }
                    let v = (slider(&d.spec.look, name) + by * MARK_STEP).clamp(0, MARK_MOST);
                    if v == 0 {
                        d.spec.look.sliders.remove(name);
                    } else {
                        d.spec.look.sliders.insert(name.to_string(), v);
                    }
                }
                self.sync_character();
            }
            Some(LookRow::Random) => self.random_look(),
            Some(LookRow::Reset) => {
                if let Some(d) = self.menus.chargen.as_mut() {
                    d.spec.look.sliders.clear();
                    d.saved_pick = None;
                }
                self.sync_character();
            }
            Some(LookRow::Saved) => self.saved_look_step(by),
            Some(LookRow::Save) => self.save_look(),
            Some(LookRow::View) => self.look_view_step(by, true),
            Some(LookRow::Pose) => {
                if let Some(d) = self.menus.chargen.as_mut() {
                    d.pose = cycle(d.pose, by, POSES.len());
                }
            }
            Some(LookRow::Light) => {
                if let Some(d) = self.menus.chargen.as_mut() {
                    d.light = cycle(d.light, by, LIGHTS.len());
                }
            }
            None => {}
        }
    }

    /// The file beside the settings that keeps the saved looks, if the settings have a file.
    fn looks_file(&self) -> Option<std::path::PathBuf> {
        self.menus
            .settings_file
            .as_ref()
            .map(|p| p.with_file_name(LOOKS_FILE))
    }

    /// This look kept with the others, as the next "Look" by number.
    fn save_look(&mut self) {
        let Some(file) = self.looks_file() else {
            self.menus.message = "There is no settings folder to keep looks in.".to_string();
            return;
        };
        let Some(d) = self.menus.chargen.as_mut() else {
            return;
        };
        let number = (1..)
            .find(|n| !d.saved.iter().any(|l| l.name == format!("Look {n}")))
            .unwrap_or(1);
        d.saved.push(SavedLook {
            name: format!("Look {number}"),
            portrait: d.spec.portrait,
            body: d.spec.body,
            sliders: d.spec.look.sliders.clone(),
        });
        d.saved_pick = Some(d.saved.len() - 1);
        let text: String = d.saved.iter().map(|l| l.to_line() + "\n").collect();
        let name = format!("Look {number}");
        let written = file
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&file, text));
        self.menus.message = match written {
            Ok(()) => format!("Saved as {name}."),
            Err(e) => format!("Cannot keep the look: {e}"),
        };
    }

    /// The next or last saved look, on the figure. A look of a head of the other sex keeps this head.
    fn saved_look_step(&mut self, by: i32) {
        let Some(d) = self.menus.chargen.as_mut() else {
            return;
        };
        if d.saved.is_empty() {
            return;
        }
        let pick = match d.saved_pick {
            Some(n) => cycle(n, by, d.saved.len()),
            None if by < 0 => d.saved.len() - 1,
            None => 0,
        };
        d.saved_pick = Some(pick);
        let look = d.saved[pick].clone();
        if let Some(head) = d.portraits.iter().position(|p| *p == look.portrait) {
            d.pick = head;
            d.spec.portrait = look.portrait;
        }
        d.spec.body = look.body;
        d.spec.look.sliders = look.sliders;
        self.sync_character();
    }

    /// Every slider at random, mostly near the middle and some at 0, and a head and a body of the sex chosen.
    fn random_look(&mut self) {
        let Some(d) = self.menus.chargen.as_mut() else {
            return;
        };
        let steps = (RANDOM_MOST / SLIDER_STEP) as u64;
        d.spec.look.sliders.clear();
        for (name, _, _) in SLIDERS {
            let r = next_random(&mut d.seed);
            let v = (r % (2 * steps + 1)) as i32 - steps as i32;
            if v != 0 && !r.is_multiple_of(3) {
                d.spec
                    .look
                    .sliders
                    .insert(name.to_string(), v * SLIDER_STEP);
            }
        }
        if !d.portraits.is_empty() {
            d.pick = (next_random(&mut d.seed) % d.portraits.len() as u64) as usize;
            d.spec.portrait = d.portraits[d.pick];
        }
        d.spec.body = [BodySize::Small, BodySize::Medium, BodySize::Large]
            [(next_random(&mut d.seed) % 3) as usize];
        d.saved_pick = None;
        self.sync_character();
    }

    /// The saved looks, for a test: their names.
    pub fn saved_look_names(&self) -> Vec<String> {
        self.menus.chargen.as_ref().map_or(Vec::new(), |d| {
            d.saved.iter().map(|l| l.name.clone()).collect()
        })
    }

    /// The next view, closer for 1. The arrows go round, and the wheel stops at the ends.
    fn look_view_step(&mut self, by: i32, round: bool) {
        if let Some(d) = self.menus.chargen.as_mut() {
            d.view = if round {
                cycle(d.view, by, VIEWS.len())
            } else {
                (d.view as i32 + by).clamp(0, VIEWS.len() as i32 - 1) as usize
            };
        }
    }

    /// Whether a pixel of a window of `size` is on the Appearance page's figure, where a drag turns it.
    pub(super) fn look_figure_under(&self, at: (f32, f32), size: (u32, u32)) -> bool {
        if self.top() != Some(Screen::ChargenLook) {
            return false;
        }
        let x = at.0 * 640.0 / size.0.max(1) as f32;
        let y = at.1 * 480.0 / size.1.max(1) as f32;
        let [left, top, wide, high] = LOOK_HOLE;
        x >= left && x < left + wide && y >= top && y < top + high
    }

    /// The cursor moved on the Appearance page: whether the wheel is for the figure.
    pub(super) fn look_hover(&mut self, at: (f32, f32), size: (u32, u32)) {
        let over = self.look_figure_under(at, size);
        if let Some(d) = self.menus.chargen.as_mut() {
            d.over_figure = over;
        }
    }

    /// A drag across the figure, in pixels of a window of `size`: it turns, the way the drag goes.
    pub(super) fn turn_look_figure(&mut self, across: f32, size: (u32, u32)) {
        let units = across * 640.0 / size.0.max(1) as f32;
        if let Some(d) = self.menus.chargen.as_mut() {
            d.turn = (d.turn + units * TURN_PER_UNIT).rem_euclid(std::f32::consts::TAU);
        }
    }

    /// The wheel over the figure goes closer or further. Elsewhere it is the list's.
    pub(super) fn look_wheel(&mut self, lines: f32) -> bool {
        let over = self.top() == Some(Screen::ChargenLook)
            && self.menus.chargen.as_ref().is_some_and(|d| d.over_figure);
        if over && lines != 0.0 {
            self.look_view_step(lines.signum() as i32, false);
        }
        over
    }

    /// How the Appearance page shows the figure, for a test: turned how far, and the words of its view, pose and light.
    pub fn look_preview(&self) -> Option<(f32, &'static str, &'static str, &'static str)> {
        let d = self.menus.chargen.as_ref()?;
        Some((d.turn, VIEWS[d.view].0, POSES[d.pose].0, LIGHTS[d.light].0))
    }

    /// The next or last underwear: it goes on the figure at once, and is on the character when the game starts.
    fn turn_underwear(&mut self, by: i32) {
        let Some(d) = self.menus.chargen.as_mut() else {
            return;
        };
        let count = d.underwear.len().max(1) as i32;
        d.underwear_pick = (d.underwear_pick as i32 + by).rem_euclid(count) as usize;
        d.body_on_show = true;
        self.wear_underwear_picked();
    }

    fn wear_underwear_picked(&mut self) {
        let id = self
            .menus
            .chargen
            .as_ref()
            .and_then(|d| d.underwear.get(d.underwear_pick))
            .map_or(String::new(), |v| v.id.clone());
        self.set_preview_underwear(&id);
    }

    fn turn_portrait(&mut self, by: i32) {
        let Some(d) = self.menus.chargen.as_mut() else {
            return;
        };
        d.body_on_show = false;
        let count = d.portraits.len().max(1) as i32;
        d.pick = (d.pick as i32 + by).rem_euclid(count) as usize;
        if let Some(p) = d.portraits.get(d.pick) {
            d.spec.portrait = *p;
        }
        self.sync_character();
    }

    /// The owner: "make the portraits themselves just presets and then allow for full facial customisation as well". The
    /// Preset row's arrows load the next portrait's head as it is: the face's sliders, marks and hair colour go back to
    /// the head's own, and the body's sliders stay. Every row can then change it. **Assumed:** the game's portraits have
    /// no shape of their own, so their preset is the head with all of those at 0.
    fn load_preset(&mut self, by: i32) {
        let woman = {
            let w = &self.game.world;
            w.object(w.pc).is_some_and(|o| o.gender == FEMALE)
        };
        let Some(d) = self.menus.chargen.as_mut() else {
            return;
        };
        let face = kotor_render::face::FACE_SLIDERS
            .iter()
            .copied()
            .chain(MARKS.iter().map(|m| m.0))
            .chain([HAIR_KEY, LEKKU_KEY]);
        for key in face {
            d.spec.look.sliders.remove(key);
        }
        d.saved_pick = None;
        // Another race's presets are its heads; a Human's are the game's portraits.
        let race = races::race_of(|k| slider(&d.spec.look, k));
        let heads = races::heads(race, woman).len() as i32;
        if heads > 0 {
            let n = (slider(&d.spec.look, races::RACE_HEAD_KEY) + by).rem_euclid(heads);
            d.spec
                .look
                .sliders
                .insert(races::RACE_HEAD_KEY.to_string(), n);
            self.sync_character();
        } else {
            self.turn_portrait(by);
        }
    }

    /// The next race or the last: its first head, as it is. A race of its own has no hair colour.
    fn turn_race(&mut self, by: i32) {
        if let Some(d) = self.menus.chargen.as_mut() {
            let now = races::race_of(|k| slider(&d.spec.look, k));
            // A mod pack's race is passed by when its pack is off.
            let mut next = cycle(now, by, races::RACES.len());
            while !races::offered(next) {
                next = cycle(next, by, races::RACES.len());
            }
            let looks = &mut d.spec.look.sliders;
            for key in kotor_render::face::FACE_SLIDERS
                .iter()
                .copied()
                .chain(MARKS.iter().map(|m| m.0))
                .chain([HAIR_KEY, LEKKU_KEY, races::RACE_KEY, races::RACE_HEAD_KEY])
            {
                looks.remove(key);
            }
            if next != 0 {
                looks.insert(races::RACE_KEY.to_string(), next as i32);
            }
            d.saved_pick = None;
            // The head is what changes: the view comes to it.
            if d.view == 0 {
                d.view = FACE_VIEW;
            }
        }
        self.sync_character();
    }

    fn set_portrait(&mut self, pick: usize) {
        if let Some(d) = self.menus.chargen.as_mut() {
            d.pick = pick;
            if let Some(p) = d.portraits.get(pick) {
                d.spec.portrait = *p;
            }
        }
        self.sync_character();
    }

    /// `abchrgen.gui`: scores bought from a pool.
    pub(super) fn chargen_attributes_view(&mut self) -> MenuView {
        let mut v = MenuView::new("abchrgen");
        let under = match &self.menus.hover {
            Some(Hit::Button(tag)) => ATTRIBUTES.iter().position(|a| tag.starts_with(a.0)),
            _ => None,
        };
        let Some(d) = self.menus.chargen.as_mut() else {
            return v;
        };
        if let Some(n) = under {
            d.about = n;
        }
        let (scores, about, left) = (d.spec.abilities, d.about, POINT_POOL - d.spent());
        v.text("REMAINING_SELECTIONS_LBL", left.to_string());
        for (tag, n, _) in ATTRIBUTES {
            v.text(&format!("{tag}_POINTS_BTN"), scores[n].to_string());
            v.live(&format!("{tag}_LBL"));
            if scores[n] <= ATTRIBUTE_BASE {
                v.hide(&[&format!("{tag}_MINUS_BTN")]);
            }
            if next_cost(scores[n]).is_none_or(|c| c > left) {
                v.hide(&[&format!("{tag}_PLUS_BTN")]);
            }
        }
        let (_, n, words) = ATTRIBUTES[about];
        v.text("LB_DESC", self.ui.strref(words));
        let cost = next_cost(scores[n]).unwrap_or(0);
        v.text("COST_POINTS_LBL", cost.to_string());
        let modifier = (scores[n] - 10).div_euclid(2);
        v.text("LBL_ABILITY_MOD", format!("{modifier:+}"));
        v
    }

    fn change_score(&mut self, tag: &str) {
        let Some(&(name, n, _)) = ATTRIBUTES.iter().find(|a| tag.starts_with(a.0)) else {
            return;
        };
        let Some(d) = self.menus.chargen.as_mut() else {
            return;
        };
        let score = d.spec.abilities[n];
        let left = POINT_POOL - d.spent();
        if tag == format!("{name}_PLUS_BTN") {
            if next_cost(score).is_some_and(|c| c <= left) {
                d.spec.abilities[n] += 1;
            }
        } else if score > ATTRIBUTE_BASE {
            d.spec.abilities[n] -= 1;
        }
        d.about = n;
    }

    /// `name.gui`.
    pub(super) fn chargen_name_view(&mut self) -> MenuView {
        let mut v = MenuView::new("name");
        v.text("NAME_BOX_EDIT", format!("{}_", self.menus.name));
        v
    }

    pub(super) fn chargen_name_accept(&mut self) {
        let name = self.menus.name.trim().to_string();
        if name.is_empty() {
            self.menus.message = "Give the character a name.".to_string();
            return;
        }
        if let Some(d) = self.menus.chargen.as_mut() {
            d.spec.name = name;
        }
        self.sync_character();
        self.finish_step(Step::Name);
        self.menus.stack.pop();
    }

    /// Esc, or the screen's way back.
    pub(super) fn chargen_escape(&mut self, screen: Screen) {
        match screen {
            Screen::ChargenClass => {
                self.menus.stack.pop();
            }
            Screen::ChargenMain => {
                if self
                    .menus
                    .chargen
                    .as_ref()
                    .is_some_and(|d| d.custom.is_some())
                {
                    self.step_back();
                } else {
                    self.menus.chargen = None;
                    self.menus.stack.pop();
                }
            }
            Screen::ChargenPortrait => {
                let pick = self.menus.chargen.as_ref().map_or(0, |d| d.page_pick);
                self.set_portrait(pick);
                if let Some(d) = self.menus.chargen.as_mut() {
                    d.underwear_pick = d.page_underwear;
                }
                self.wear_underwear_picked();
                self.menus.stack.pop();
            }
            Screen::ChargenLook => {
                if let Some(d) = self.menus.chargen.as_mut() {
                    d.spec.body = d.page_body;
                    d.underwear_pick = d.page_underwear;
                    d.spec.look = d.page_look.clone();
                }
                let pick = self.menus.chargen.as_ref().map_or(0, |d| d.page_pick);
                self.set_portrait(pick);
                self.wear_underwear_picked();
                self.menus.stack.pop();
            }
            Screen::ChargenAttributes => {
                if let Some(d) = self.menus.chargen.as_mut() {
                    d.spec.abilities = d.page_scores;
                }
                self.menus.stack.pop();
            }
            _ => {
                self.menus.stack.pop();
            }
        }
        self.menus.hover = None;
    }

    /// What a button on these screens does.
    pub(super) fn chargen_press(&mut self, screen: Screen, tag: &str) {
        match (screen, tag) {
            (Screen::ChargenClass, "BTN_BACK") => self.chargen_escape(screen),
            (Screen::ChargenClass, _) => {
                if let Some(slot) = FIGURES.iter().position(|f| f.0 == tag) {
                    self.choose_figure(slot);
                }
            }
            (Screen::ChargenMain, "QUICK_CHAR_BTN" | "CUST_CHAR_BTN") => {
                let custom = tag == "CUST_CHAR_BTN";
                if let Some(d) = self.menus.chargen.as_mut() {
                    d.custom = Some(custom);
                    d.next = 0;
                }
                if !custom {
                    self.quick_build();
                }
                self.menus.hover = None;
            }
            (Screen::ChargenMain, "BTN_BACK") => {
                // On the steps' panel it undoes one, and there is none to undo at first.
                let first = self
                    .menus
                    .chargen
                    .as_ref()
                    .is_some_and(|d| d.custom.is_some() && d.next == 0);
                if !first {
                    self.chargen_escape(screen);
                }
            }
            (Screen::ChargenMain, "BTN_CANCEL") => {
                if let Some(d) = self.menus.chargen.as_mut() {
                    d.next = 0;
                }
                self.step_back();
                self.menus.hover = None;
            }
            (Screen::ChargenMain, _) => {
                let Some(n) = tag
                    .strip_prefix("BTN_STEPNAME")
                    .and_then(|n| n.parse::<usize>().ok())
                else {
                    return;
                };
                let Some(step) = self.menus.chargen.as_ref().and_then(|d| {
                    (n - 1 <= d.next)
                        .then(|| d.steps().get(n - 1).copied())
                        .flatten()
                }) else {
                    return;
                };
                self.do_step(step);
            }
            (Screen::ChargenPortrait, UNDERWEAR_LEFT) => self.turn_underwear(-1),
            (Screen::ChargenPortrait, UNDERWEAR_RIGHT) => self.turn_underwear(1),
            (Screen::ChargenPortrait, "BTN_ARRL") => self.turn_portrait(-1),
            (Screen::ChargenPortrait, "BTN_ARRR") => self.turn_portrait(1),
            (Screen::ChargenPortrait | Screen::ChargenLook, "BTN_ACCEPT") => {
                self.finish_step(Step::Portrait);
                self.menus.stack.pop();
                self.menus.hover = None;
            }
            (
                Screen::ChargenPortrait
                | Screen::ChargenLook
                | Screen::ChargenAttributes
                | Screen::ChargenName,
                "BTN_BACK",
            ) => self.chargen_escape(screen),
            (Screen::ChargenAttributes, "BTN_RECOMMENDED") => {
                let class = self.menus.chargen.as_ref().map_or(0, |d| d.spec.class);
                let scores = self.game.world.recommended_abilities(class);
                if let (Some(d), Some(scores)) = (self.menus.chargen.as_mut(), scores) {
                    d.spec.abilities = scores;
                }
            }
            (Screen::ChargenAttributes, "BTN_ACCEPT") => {
                let left = self
                    .menus
                    .chargen
                    .as_ref()
                    .map_or(0, |d| POINT_POOL - d.spent());
                if left > 0 {
                    self.menus.message = "Spend the points first.".to_string();
                    return;
                }
                if let Some(d) = self.menus.chargen.as_mut() {
                    d.settled = true;
                }
                self.sync_character();
                self.finish_step(Step::Attributes);
                self.menus.stack.pop();
                self.menus.hover = None;
            }
            (Screen::ChargenAttributes, _)
                if tag.ends_with("_PLUS_BTN") || tag.ends_with("_MINUS_BTN") =>
            {
                self.change_score(tag)
            }
            (Screen::ChargenName, "END_BTN") => self.chargen_name_accept(),
            (Screen::ChargenName, "BTN_RANDOM") => {
                let gender = self
                    .game
                    .world
                    .object(self.game.world.pc)
                    .map_or(0, |o| o.gender);
                self.menus.name = self.game.world.random_name(gender);
            }
            _ => {}
        }
    }

    /// Where figures stand for their holes, and the camera, which is the same for all so that they keep their shape.
    fn places_in_holes(&self, holes: &[[f32; 4]]) -> (Vec<[f32; 3]>, Camera) {
        let height = holes.iter().map(|h| h[3]).fold(f32::MAX, f32::min);
        let tan = (FIGURE_FOV.to_radians() / 2.0).tan();
        let aspect = self.ui_width as f32 / 480.0;
        let depth = FIGURE_MARGIN * FIGURE_HEIGHT / (2.0 * tan * (height / 480.0));
        let places = holes
            .iter()
            .map(|hole| {
                let across = ((hole[0] + hole[2] / 2.0) / 320.0 - 1.0) * depth * tan * aspect;
                let up = (1.0 - (hole[1] + hole[3] / 2.0) / 240.0) * depth * tan;
                [across, 0.0, up - FIGURE_HEIGHT / 2.0]
            })
            .collect();
        let camera = Camera {
            eye: [0.0, -depth, 0.0],
            target: [0.0, 0.0, 0.0],
            fov: FIGURE_FOV,
        };
        (places, camera)
    }

    fn figures_in_holes(&mut self, figures: &[(i32, [f32; 4])]) -> (Scene, Camera) {
        let holes: Vec<[f32; 4]> = figures.iter().map(|f| f.1).collect();
        let (places, camera) = self.places_in_holes(&holes);
        let mut scene = Scene::default();
        for (nth, ((appearance, _), at)) in figures.iter().zip(places).enumerate() {
            if let Some(rig) = self.clothed_rig(*appearance) {
                let clip = self.idling(nth);
                let wear = self.preview_underwear.clone();
                self.add_creature(&mut scene, &rig, &[], &clip, at, FACING_CAMERA, &wear);
            }
        }
        (scene, camera)
    }

    /// The mist's model and where it goes, for the screen that is up.
    fn mist_places(&mut self) -> Option<(&'static str, Vec<[f32; 3]>)> {
        match self.top()? {
            Screen::ChargenClass => {
                let holes = self.figure_holes();
                Some((BODY_MIST, self.places_in_holes(&holes).0))
            }
            Screen::ChargenMain => Some((BODY_MIST, self.places_in_holes(&[SHEET_HOLE]).0)),
            Screen::ChargenPortrait => {
                let (_, camera) = self.chargen_scene()?;
                Some((HEAD_MIST, vec![[0.0, 0.0, camera.target[2]]]))
            }
            _ => None,
        }
    }

    /// The mist goes on while one of these screens is up, and starts over when another comes.
    pub(in crate::stage) fn chargen_mist_tick(&mut self, dt: f32) {
        let top = self.top();
        let Some((model_name, places)) = self.mist_places() else {
            self.menus.chargen_mist = None;
            return;
        };
        let eye = [0.0, -FIGURE_HEIGHT, 0.0];
        if self.menus.chargen_mist.as_ref().map(|m| m.0) != top {
            let mut mist = Effects::default();
            if let Some(model) = self.model(model_name) {
                for at in places {
                    // The model's camera looks along -x, and ours along +y.
                    mist.load_model(
                        &model,
                        &[
                            [0.0, 1.0, 0.0, at[0]],
                            [-1.0, 0.0, 0.0, at[1]],
                            [0.0, 0.0, 1.0, at[2]],
                            [0.0, 0.0, 0.0, 1.0],
                        ],
                    );
                }
            }
            for _ in 0..(MIST_START / MIST_STEP) as usize {
                mist.update(MIST_STEP, eye);
            }
            self.menus.chargen_mist = top.map(|t| (t, mist));
        }
        if let Some((_, mist)) = self.menus.chargen_mist.as_mut() {
            mist.update(dt, eye);
        }
    }

    /// The mist's particles, when one of these screens is up.
    pub(super) fn chargen_particles(&mut self) -> Option<Vec<ParticleBatch>> {
        let (_, camera) = self.chargen_scene()?;
        let (_, mist) = self.menus.chargen_mist.as_ref()?;
        Some(mist.batches(&camera))
    }

    /// The whole body with nothing over its underwear, in a hole.
    fn body_in_hole(&mut self, appearance: i32, hole: [f32; 4]) -> (Scene, Camera) {
        let (places, camera) = self.places_in_holes(&[hole]);
        let mut scene = Scene::default();
        let head = self.draft_race_head().map(|h| h.0);
        if let Some(rig) = self.bare_rig_head(appearance, head) {
            let clip = self.idling(0);
            let wear = self.draft_wear(appearance);
            self.add_creature(
                &mut scene,
                &rig,
                &[],
                &clip,
                places[0],
                FACING_CAMERA,
                &wear,
            );
        }
        (scene, camera)
    }

    /// The Appearance page's figure, turned, posed, seen whole or close and lit as its rows say.
    fn look_scene(&mut self, appearance: i32) -> (Scene, Camera) {
        let (turn, view, pose, light) = self
            .menus
            .chargen
            .as_ref()
            .map_or((0.0, 0, 0, 0), |d| (d.turn, d.view, d.pose, d.light));
        let mut scene = Scene::default();
        let head = self.draft_race_head().map(|h| h.0);
        let Some(rig) = self.bare_rig_head(appearance, head) else {
            return (scene, default_camera());
        };
        let look = self
            .menus
            .chargen
            .as_ref()
            .map_or(CharacterLook::default(), |d| d.spec.look.clone());
        let rig = self.shaped_rig(rig, &look);
        let wear = self.draft_wear(appearance);
        let stance = self.look_stance(&rig, appearance, &look, &wear);
        let clip = Some(Clip {
            name: POSES[pose].1,
            time: self.idle_clock,
            fade: None,
        });
        let facing = FACING_CAMERA + turn;
        self.add_creature(&mut scene, &rig, &[], &clip, [0.0; 3], facing, &wear);
        grow(&mut scene, 0, [0.0; 3], body_shape(&look).height_scale());
        // The whole body is framed for the tallest it can be, so that a taller one is seen to be. A close view follows the head.
        let scale = body_shape(&look).height_scale();
        let tallest = BodyShape {
            height: 1.0,
            ..BodyShape::default()
        }
        .height_scale();
        let (from, top) = match VIEWS[view].1 {
            None => (stance.low, stance.top * tallest),
            Some(down) => (stance.top * scale - down, stance.top * scale),
        };
        let aspect = self.ui_width as f32 / 480.0;
        // The middle of what is shown turns with the body.
        let [x, y] = stance.middles[view].map(|v| v * scale);
        let (sin, cos) = turn.sin_cos();
        let middle = [cos * x - sin * y, sin * x + cos * y];
        let camera = camera_on_hole(LOOK_HOLE, (from, top), middle, aspect);
        let (_, spin, bright) = LIGHTS[light];
        let mut scene = if spin == 0.0 {
            scene
        } else {
            scene.moved(spin, [0.0; 3])
        };
        if bright > 0.0 {
            for m in &mut scene.meshes {
                m.shade = bright;
            }
        }
        (scene, spun(camera, spin))
    }

    /// How a body stands at rest, measured once for each body and look.
    fn look_stance(
        &mut self,
        rig: &Rig,
        appearance: i32,
        look: &CharacterLook,
        wear: &str,
    ) -> Stance {
        let known = self.menus.chargen.as_ref().and_then(|d| d.measured.clone());
        if let Some((_, _, stance)) = known.filter(|m| m.0 == appearance && &m.1 == look) {
            return stance;
        }
        let mut scene = Scene::default();
        self.add_creature(&mut scene, rig, &[], &None, [0.0; 3], FACING_CAMERA, wear);
        let points: Vec<[f32; 3]> = scene
            .meshes
            .iter()
            .flat_map(|m| m.positions.iter().copied())
            .collect();
        let (low, top) = points
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[2]), b.max(p[2])));
        if low >= top {
            return Stance {
                low: 0.0,
                top: FIGURE_HEIGHT,
                middles: [[0.0; 2]; 3],
            };
        }
        // The middle of the box round what each view shows.
        let middles = VIEWS.map(|(_, down)| {
            let from = down.map_or(low, |d| top - d);
            let mut b = [f32::MAX, f32::MIN, f32::MAX, f32::MIN];
            for p in points.iter().filter(|p| p[2] >= from) {
                b = [
                    b[0].min(p[0]),
                    b[1].max(p[0]),
                    b[2].min(p[1]),
                    b[3].max(p[1]),
                ];
            }
            [(b[0] + b[1]) / 2.0, (b[2] + b[3]) / 2.0]
        });
        let stance = Stance { low, top, middles };
        if let Some(d) = self.menus.chargen.as_mut() {
            d.measured = Some((appearance, look.clone(), stance));
        }
        stance
    }

    /// A body reshaped by a look's sliders, kept for the last body and look asked for: a slider's arrow reshapes it once.
    fn shaped_rig(&mut self, rig: Rc<Rig>, look: &CharacterLook) -> Rc<Rig> {
        let shape = body_shape(look);
        let face = FaceShape::from_sliders(|name| slider(look, name));
        if shape.is_plain() && face.is_plain() {
            return rig;
        }
        let key = Rc::as_ptr(&rig) as usize;
        if let Some((k, sliders, shaped)) = &self.menus.shaped {
            if *k == key && *sliders == look.sliders {
                return shaped.clone();
            }
        }
        let shaped = Rc::new(rig.shaped(&shape, &face));
        self.menus.shaped = Some((key, look.sliders.clone(), shaped.clone()));
        shaped
    }

    /// The player's body as their look has it: anyone else's as it is.
    pub(in crate::stage) fn shaped_for(&mut self, id: kotor_game::ObjId, rig: Rc<Rig>) -> Rc<Rig> {
        let w = &self.game.world;
        if id != w.pc {
            return rig;
        }
        let Some(look) = w.object(id).map(|o| o.look.clone()) else {
            return rig;
        };
        self.shaped_rig(rig, &look)
    }

    /// The player drawn taller or shorter by their look, about their feet: the meshes from `from` on. Only for how it looks.
    pub(in crate::stage) fn grow_to_height(
        &self,
        scene: &mut Scene,
        from: usize,
        id: kotor_game::ObjId,
        feet: [f32; 3],
    ) {
        let w = &self.game.world;
        if id != w.pc {
            return;
        }
        if let Some(o) = w.object(id) {
            grow(scene, from, feet, body_shape(&o.look).height_scale());
        }
    }

    /// What the figures of character creation wear, on the body of `appearance`: the underwear on show, and the marks,
    /// hair colour and race of the look being made.
    fn draft_wear(&mut self, appearance: i32) -> String {
        let Some(look) = self.menus.chargen.as_ref().map(|d| d.spec.look.clone()) else {
            return self.preview_underwear.clone();
        };
        let head = self.draft_race_head();
        let underwear = self.preview_underwear.clone();
        self.dress_with(&underwear, &look, appearance, head)
    }

    /// The head of another race the look being made puts on the player: model and texture.
    fn draft_race_head(&self) -> Option<(&'static str, &'static str)> {
        let d = self.menus.chargen.as_ref()?;
        let w = &self.game.world;
        let woman = w.object(w.pc).is_some_and(|o| o.gender == FEMALE);
        kotor_render::races::head_of(|k| slider(&d.spec.look, k), woman)
    }

    /// The head on the portrait page, large, in its hole.
    fn head_in_hole(&mut self, appearance: i32) -> (Scene, Camera) {
        let mut scene = Scene::default();
        let head = self.draft_race_head().map(|h| h.0);
        let Some(rig) = self.clothed_rig_head(appearance, head) else {
            return (scene, default_camera());
        };
        let wear = self.draft_wear(appearance);
        self.add_creature(&mut scene, &rig, &[], &None, [0.0; 3], HEAD_FACING, &wear);
        let top = scene
            .meshes
            .iter()
            .flat_map(|m| m.positions.iter())
            .fold(0.0, |top: f32, p| top.max(p[2]));
        let tan = (FIGURE_FOV.to_radians() / 2.0).tan();
        let depth = HEAD_VIEW / (2.0 * tan);
        let aspect = self.ui_width as f32 / 480.0;
        let across = (HEAD_AT.0 / 320.0 - 1.0) * depth * tan * aspect;
        let up = (1.0 - HEAD_AT.1 / 240.0) * depth * tan;
        let middle = top - HEAD_BELOW_TOP;
        let camera = Camera {
            eye: [-across, -depth, middle - up],
            target: [-across, 0.0, middle - up],
            fov: FIGURE_FOV,
        };
        (scene, camera)
    }

    /// What these screens show in their holes, and with what camera.
    pub(super) fn chargen_scene(&mut self) -> Option<(Scene, Camera)> {
        match self.top()? {
            Screen::ChargenClass => {
                let mut figures = Vec::new();
                for (slot, (_, _, class, gender)) in FIGURES.iter().enumerate() {
                    let portrait = self.figure_portrait(*gender, slot)?;
                    let body = BodySize::of_class(*class);
                    let look = self.game.world.portrait_appearance(portrait, body)?;
                    figures.push((look, self.class_figure_hole(slot)));
                }
                Some(self.figures_in_holes(&figures))
            }
            Screen::ChargenMain => {
                let w = &self.game.world;
                let look = w.object(w.pc)?.appearance;
                Some(self.figures_in_holes(&[(look, SHEET_HOLE)]))
            }
            Screen::ChargenPortrait => {
                let w = &self.game.world;
                let look = w.object(w.pc)?.appearance;
                if self.menus.chargen.as_ref().is_some_and(|d| d.body_on_show) {
                    return Some(self.body_in_hole(look, BODY_HOLE));
                }
                Some(self.head_in_hole(look))
            }
            Screen::ChargenLook => {
                let w = &self.game.world;
                let look = w.object(w.pc)?.appearance;
                Some(self.look_scene(look))
            }
            _ => None,
        }
    }

    /// Whether a text box is up on these screens.
    pub(super) fn chargen_typing(&self) -> bool {
        matches!(self.menus.stack.last(), Some(Screen::ChargenName))
    }
}

/// What the next point of a score costs, if it can go higher.
fn next_cost(score: i32) -> Option<i32> {
    Some(attribute_cost(score + 1)? - attribute_cost(score)?)
}

/// The player's ranks, as a spec has them.
fn owned_skills(s: &Stage) -> Vec<(usize, i32)> {
    let w = &s.game.world;
    let ranks = w.object(w.pc).map_or(Vec::new(), |o| o.skills.clone());
    ranks
        .into_iter()
        .enumerate()
        .filter(|(_, r)| *r > 0)
        .collect()
}

/// A slider of a look, in per cent: 0 when it was never moved.
fn slider(look: &CharacterLook, name: &str) -> i32 {
    look.sliders.get(name).copied().unwrap_or(0)
}

/// The body's sliders of a look.
fn body_shape(look: &CharacterLook) -> BodyShape {
    BodyShape::from_sliders(|name| slider(look, name))
}

/// Meshes from `from` on, bigger or smaller about `feet`.
fn grow(scene: &mut Scene, from: usize, feet: [f32; 3], scale: f32) {
    if scale == 1.0 {
        return;
    }
    for m in &mut scene.meshes[from..] {
        for p in &mut m.positions {
            for k in 0..3 {
                p[k] = feet[k] + (p[k] - feet[k]) * scale;
            }
        }
    }
}

/// One step round a list of `len`, either way.
fn cycle(now: usize, by: i32, len: usize) -> usize {
    (now as i32 + by).rem_euclid(len.max(1) as i32) as usize
}

/// A camera that shows a figure from `low` to `high` in a hole, with air: `middle` is where it is across and deep.
fn camera_on_hole(
    hole: [f32; 4],
    (low, high): (f32, f32),
    middle: [f32; 2],
    aspect: f32,
) -> Camera {
    let tan = (FIGURE_FOV.to_radians() / 2.0).tan();
    let span = (high - low) * (1.0 + 2.0 * VIEW_MARGIN);
    // The hole's height shows `span` at this depth.
    let depth = span * 240.0 / (hole[3] * tan);
    let across = ((hole[0] + hole[2] / 2.0) / 320.0 - 1.0) * depth * tan * aspect;
    let up = (1.0 - (hole[1] + hole[3] / 2.0) / 240.0) * depth * tan;
    let height = (low + high) / 2.0 - up;
    let [x, y] = middle;
    Camera {
        eye: [x - across, y - depth, height],
        target: [x - across, y, height],
        fov: FIGURE_FOV,
    }
}

/// A camera turned about the upright line through the middle of the world, as `Scene::moved` turns a scene.
fn spun(camera: Camera, yaw: f32) -> Camera {
    let (sin, cos) = yaw.sin_cos();
    let turn = |p: [f32; 3]| [cos * p[0] - sin * p[1], sin * p[0] + cos * p[1], p[2]];
    Camera {
        eye: turn(camera.eye),
        target: turn(camera.target),
        fov: camera.fov,
    }
}

fn default_camera() -> Camera {
    Camera {
        eye: [0.0, -3.0, 1.0],
        target: [0.0, 0.0, 1.0],
        fov: FIGURE_FOV,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_look_goes_through_its_line_and_back() {
        let look = SavedLook {
            name: "Look 2".to_string(),
            portrait: 12,
            body: BodySize::Large,
            sliders: [("nose".to_string(), 30), ("height".to_string(), -100)].into(),
        };
        let text = format!("{}\nnot a look\n", look.to_line());
        let back = read_looks(&text);
        assert_eq!(back.len(), 1);
        assert!(back[0] == look);
        let mut seed = 7;
        let (a, b) = (next_random(&mut seed), next_random(&mut seed));
        assert_ne!(a, b);
    }
}
