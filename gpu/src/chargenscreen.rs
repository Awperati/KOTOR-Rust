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
use kotor_game::{attribute_cost, BodySize, CharacterSpec, ATTRIBUTE_BASE, POINT_POOL};
use kotor_render::{Camera, Scene};
use kotor_ui::{Hit, MenuView};

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
const SHEET_HOLE: [f32; 4] = [155.0, 89.0, 162.0, 186.0];
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
    page_scores: [i32; 6],
    /// The attribute whose words are on show.
    about: usize,
    /// The numbers have been settled, and the sheet shows them.
    settled: bool,
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

impl Stage {
    /// New Game goes through these screens. Off unless asked for, as a test wants a game at once.
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

    fn choose_figure(&mut self, slot: usize) {
        let (_, _, class, gender) = FIGURES[slot];
        let w = &mut self.game.world;
        let portraits = w.player_portraits(gender);
        let pick = slot % portraits.len().max(1);
        let Some(portrait) = portraits.get(pick).copied() else {
            return;
        };
        self.menus.chargen = Some(Draft {
            spec: CharacterSpec {
                name: String::new(),
                class,
                portrait,
                body: BodySize::of_class(class),
                abilities: [ATTRIBUTE_BASE; 6],
                skills: Vec::new(),
                feats: Vec::new(),
            },
            custom: None,
            next: 0,
            portraits,
            pick,
            page_pick: pick,
            page_scores: [ATTRIBUTE_BASE; 6],
            about: 0,
            settled: false,
        });
        self.sync_character();
        self.menus.stack.push(Screen::ChargenMain);
        self.menus.hover = None;
    }

    /// `maincg.gui`, as the level-up panel has it, with the panel for where the character has got to.
    pub(super) fn chargen_main_view(&mut self) -> MenuView {
        let mut v = MenuView::new("maincg");
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
                self.menus.stack.push(Screen::ChargenPortrait);
                self.menus.hover = None;
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

    /// `portcust.gui`: the portrait and, in a hole, the head.
    pub(super) fn chargen_portrait_view(&mut self) -> MenuView {
        let mut v = MenuView::new("portcust");
        v.holes.push("LBL_HEAD".to_string());
        v.fill("LBL_PORTRAIT", self.portrait());
        v
    }

    fn turn_portrait(&mut self, by: i32) {
        let Some(d) = self.menus.chargen.as_mut() else {
            return;
        };
        let count = d.portraits.len().max(1) as i32;
        d.pick = (d.pick as i32 + by).rem_euclid(count) as usize;
        if let Some(p) = d.portraits.get(d.pick) {
            d.spec.portrait = *p;
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
            (Screen::ChargenPortrait, "BTN_ARRL") => self.turn_portrait(-1),
            (Screen::ChargenPortrait, "BTN_ARRR") => self.turn_portrait(1),
            (Screen::ChargenPortrait, "BTN_ACCEPT") => {
                self.finish_step(Step::Portrait);
                self.menus.stack.pop();
                self.menus.hover = None;
            }
            (
                Screen::ChargenPortrait | Screen::ChargenAttributes | Screen::ChargenName,
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
        for ((appearance, _), at) in figures.iter().zip(places) {
            if let Some(rig) = self.clothed_rig(*appearance) {
                self.add_creature(&mut scene, &rig, &[], &None, at, FACING_CAMERA);
            }
        }
        (scene, camera)
    }

    /// The mist's model and where it goes, for the screen that is up.
    fn mist_places(&mut self) -> Option<(&'static str, Vec<[f32; 3]>)> {
        match self.top()? {
            Screen::ChargenClass => Some((BODY_MIST, self.places_in_holes(&FIGURE_HOLES).0)),
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

    /// The head on the portrait page, large, in its hole.
    fn head_in_hole(&mut self, appearance: i32) -> (Scene, Camera) {
        let mut scene = Scene::default();
        let Some(rig) = self.clothed_rig(appearance) else {
            return (scene, default_camera());
        };
        self.add_creature(&mut scene, &rig, &[], &None, [0.0; 3], HEAD_FACING);
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
                    figures.push((look, FIGURE_HOLES[slot]));
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
                Some(self.head_in_hole(look))
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

fn default_camera() -> Camera {
    Camera {
        eye: [0.0, -3.0, 1.0],
        target: [0.0, 0.0, 1.0],
        fov: FIGURE_FOV,
    }
}
