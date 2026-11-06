//! The level-up screens, as the reference video has them at 22:20 and 1:23:25: `leveluppnl.gui` over `maincg.gui` lists
//! the steps, and each step is a page: `abchrgen`, `skchrgen`, `ftchrgen` and `pwrlvlup`.
//!
//! The level is taken when the panel opens, so a page spends through the rules as they are. Cancel on a page puts the
//! creature back as the page found it, and Back on the panel as the panel did.
//! **Assumed:** points left unspent stay for the next level, except the attribute point, which OK asks for. Nothing
//! after the video's level 8 was compared, and Force powers not at all: the video was not searched for them.

use super::{Screen, Stage};
use kotor_game::{Choice, ObjId, Object};
use kotor_ui::{Cell, Hit, MenuView, Row};

const STR_LEVEL_UP: u32 = 1071;
const STR_ADD: u32 = 38455;
const STR_REMOVE: u32 = 38456;
const STR_CLASS_SKILL: u32 = 38152;
const STR_CROSS_CLASS: u32 = 38153;

/// The tag each ability starts with, and the talk table's words about it.
const ABILITIES: [(&str, usize, u32); 6] = [
    ("STR", 0, 222),
    ("DEX", 1, 223),
    ("CON", 2, 224),
    ("INT", 3, 226),
    ("WIS", 4, 225),
    ("CHA", 5, 227),
];
/// The tags of a skill's row and its arrows, by its row of `skills.2da`.
const SKILLS: [(&str, &str); 8] = [
    ("COMPUTER_USE", "COM"),
    ("DEMOLITIONS", "DEM"),
    ("STEALTH", "STE"),
    ("AWARENESS", "AWA"),
    ("PERSUADE", "PER"),
    ("REPAIR", "REP"),
    ("SECURITY", "SEC"),
    ("TREAT_INJURY", "TRE"),
];
const ACCEPT_STEP: usize = 4;

/// A level being taken.
pub(super) struct Draft {
    who: ObjId,
    /// The creature as the panel and as this page found it.
    before: Object,
    page: Object,
    /// Fortitude, reflex and will as they were.
    saves: [i32; 3],
    /// Attributes, skills, feats, powers: the page was left with OK.
    done: [bool; ACCEPT_STEP],
    has_abilities: bool,
    has_powers: bool,
    ability_points: i32,
    power_points: i32,
    /// Both as this page found them.
    page_points: (i32, i32),
    /// Feats or powers this page has added, in order.
    taken: Vec<i32>,
    /// The ability or skill whose words are on show.
    about: Option<usize>,
    /// The feat or power under the cursor, and the one clicked.
    hover_choice: Option<i32>,
    chosen: Option<i32>,
}

impl Stage {
    fn restore(&mut self, object: Object) {
        if let Some(o) = self.game.world.object_mut(object.id) {
            *o = object;
        }
    }

    fn level_draft(&mut self) -> Option<&mut Draft> {
        self.menus.level.as_mut()
    }

    /// Take the next level: the panel comes up, with hit points, skill points and feat slots already given.
    pub(super) fn start_level_up(&mut self) {
        let w = &mut self.game.world;
        let who = w.pc;
        let (Some(before), Some(sheet)) = (w.object(who).cloned(), w.sheet(who)) else {
            return;
        };
        let Some(up) = w.level_up(who) else { return };
        let ability_points = w.ability_points_at(up.level);
        let power_points = w.power_slots_at(before.class, up.level);
        self.menus.level = Some(Draft {
            who,
            page: before.clone(),
            before,
            saves: [sheet.fortitude, sheet.reflex, sheet.will],
            done: [false; ACCEPT_STEP],
            has_abilities: ability_points > 0,
            has_powers: power_points > 0,
            ability_points,
            power_points,
            page_points: (0, 0),
            taken: Vec::new(),
            about: None,
            hover_choice: None,
            chosen: None,
        });
        self.menus.stack.push(Screen::LevelUp);
    }

    /// Whether a step has anything to do at this level.
    fn step_needed(&self, step: usize) -> bool {
        let Some(d) = &self.menus.level else {
            return false;
        };
        let w = &self.game.world;
        let slots = w.object(d.who).map_or((0, 0), |o| o.feat_slots);
        match step {
            0 => d.has_abilities,
            2 => slots.0 + slots.1 > 0 || d.done[2],
            3 => d.has_powers,
            _ => true,
        }
    }

    /// The first step not done yet.
    fn next_step(&self) -> usize {
        let done = |s: usize| self.menus.level.as_ref().is_some_and(|d| d.done[s]);
        (0..ACCEPT_STEP)
            .find(|s| self.step_needed(*s) && !done(*s))
            .unwrap_or(ACCEPT_STEP)
    }

    /// `maincg.gui` with `leveluppnl.gui` over it. There is no 3D figure yet.
    pub(super) fn level_panel_view(&mut self) -> MenuView {
        let mut v = MenuView::new("maincg");
        let title = self.ui.strref(STR_LEVEL_UP);
        v.text("MAIN_TITLE_LBL", title);
        v.fill("PORTRAIT_LBL", self.portrait());
        v.hide(&[
            "MODEL_LBL",
            "LBL_BEVEL_L",
            "LBL_BEVEL_R",
            "LBL_BEVEL_M",
            "OLD_VIT_LBL",
            "NEW_VIT_LBL",
            "VIT_ARROW_LBL",
            "OLD_DEF_LBL",
            "NEW_DEF_LBL",
            "DEF_ARROW_LBL",
        ]);
        let (who, saves) = self
            .menus
            .level
            .as_ref()
            .map_or((self.game.world.pc, [0; 3]), |d| (d.who, d.saves));
        if let Some(s) = self.game.world.sheet(who) {
            v.text("LBL_NAME", s.name);
            v.text("LBL_CLASS", s.class);
            v.text("LBL_LEVEL_VAL", s.level.to_string());
            v.text("LBL_VIT", s.max_hp.to_string());
            v.text("LBL_DEF", s.defense.to_string());
            for (tag, _, _) in ABILITIES {
                let n = ABILITIES.iter().position(|a| a.0 == tag).unwrap_or(0);
                v.text(&format!("{tag}_AB_LBL"), s.abilities[n].0.to_string());
            }
            let new = [s.fortitude, s.reflex, s.will];
            for (n, tag) in ["FORT", "REFL", "WILL"].iter().enumerate() {
                v.text(&format!("OLD_{tag}_LBL"), saves[n].to_string());
                v.text(&format!("NEW_{tag}_LBL"), new[n].to_string());
            }
        }
        let mut panel = MenuView::new("leveluppnl");
        let next = self.next_step();
        for step in 0..=ACCEPT_STEP {
            let n = step + 1;
            let (name, circle, number) = (
                format!("BTN_STEPNAME{n}"),
                format!("LBL_{n}"),
                format!("LBL_NUM{n}"),
            );
            if !self.step_needed(step) {
                panel.hide(&[&name, &circle, &number]);
            } else if step == next {
                panel.lit(&name);
                panel.fill(&circle, "lbl_cg_circ2");
            } else if step > next {
                panel.inert(&[&name]);
            }
        }
        panel.hover = self.menus.hover.clone();
        v.top = Some(Box::new(panel));
        v
    }

    /// A page's title, in place of the file's "Character Generation".
    fn page_title(&mut self, v: &mut MenuView) {
        let title = self.ui.strref(STR_LEVEL_UP);
        v.text("MAIN_TITLE_LBL", title);
    }

    /// `abchrgen.gui`.
    pub(super) fn level_abilities_view(&mut self) -> MenuView {
        let mut v = MenuView::new("abchrgen");
        self.page_title(&mut v);
        let Some((who, points, page, about)) = self
            .menus
            .level
            .as_ref()
            .map(|d| (d.who, d.ability_points, d.page.abilities, d.about))
        else {
            return v;
        };
        v.text("REMAINING_SELECTIONS_LBL", points.to_string());
        let scores = self.game.world.object(who).map_or([10; 6], |o| o.abilities);
        for (tag, n, _) in ABILITIES {
            v.text(&format!("{tag}_POINTS_BTN"), scores[n].to_string());
            v.live(&format!("{tag}_LBL"));
            if scores[n] <= page[n] {
                v.hide(&[&format!("{tag}_MINUS_BTN")]);
            }
            if points < 1 {
                v.hide(&[&format!("{tag}_PLUS_BTN")]);
            }
        }
        match about.and_then(|a| ABILITIES.get(a)) {
            Some((_, n, words)) => {
                let text = self.ui.strref(*words);
                v.text("LB_DESC", text);
                v.text("COST_POINTS_LBL", "1");
                let modifier = (scores[*n] - 10).div_euclid(2);
                v.text("LBL_ABILITY_MOD", format!("{modifier:+}"));
            }
            None => v.hide(&[
                "COST_LBL",
                "COST_POINTS_LBL",
                "LBL_MODIFIER",
                "LBL_ABILITY_MOD",
            ]),
        }
        v
    }

    /// `skchrgen.gui`.
    pub(super) fn level_skills_view(&mut self) -> MenuView {
        let mut v = MenuView::new("skchrgen");
        self.page_title(&mut v);
        let Some((who, page_ranks, about)) = self
            .menus
            .level
            .as_ref()
            .map(|d| (d.who, d.page.skills.clone(), d.about))
        else {
            return v;
        };
        let (points, class) = self
            .game
            .world
            .object(who)
            .map_or((0, 0), |o| (o.skill_points, o.class));
        v.text("REMAINING_SELECTIONS_LBL", points.to_string());
        for (n, (row, arrows)) in SKILLS.iter().enumerate() {
            let w = &mut self.game.world;
            let ranks = w.skill_ranks(who, n);
            let (cost, cap) = w.skill_price(who, n).unwrap_or((1, 0));
            v.text(&format!("{row}_POINTS_BTN"), ranks.to_string());
            v.live(&format!("{row}_LBL"));
            if ranks <= page_ranks.get(n).copied().unwrap_or(0) {
                v.hide(&[&format!("{arrows}_MINUS_BTN")]);
            }
            if points < cost || ranks >= cap {
                v.hide(&[&format!("{arrows}_PLUS_BTN")]);
            }
        }
        match about.filter(|a| *a < SKILLS.len()) {
            Some(n) => {
                let w = &mut self.game.world;
                let words = w.skill_description(n);
                let class_skill = w.is_class_skill(class, n);
                let cost = w.skill_price(who, n).map_or(1, |p| p.0);
                let text = self.ui.strref(words);
                v.text("LB_DESC", text);
                v.text("COST_POINTS_LBL", cost.to_string());
                let kind = if class_skill {
                    STR_CLASS_SKILL
                } else {
                    STR_CROSS_CLASS
                };
                let kind = self.ui.strref(kind);
                v.text("CLASSSKL_LBL", kind);
            }
            None => v.hide(&["COST_LBL", "COST_POINTS_LBL", "CLASSSKL_LBL"]),
        }
        v
    }

    fn level_choices(&mut self, screen: Screen) -> Vec<Vec<Choice>> {
        let Some(who) = self.menus.level.as_ref().map(|d| d.who) else {
            return Vec::new();
        };
        match screen {
            Screen::LevelPowers => self.game.world.power_choices(who),
            _ => self.game.world.feat_choices(who),
        }
    }

    fn choice_rows(&mut self, choices: &[Vec<Choice>]) -> Vec<Row> {
        let chosen = self.menus.level.as_ref().and_then(|d| d.chosen);
        choices
            .iter()
            .map(|chain| Row {
                cells: chain
                    .iter()
                    .map(|c| Cell {
                        icon: c.icon.clone(),
                        dim: !c.owned && !c.can_take,
                        owned: c.owned,
                        selected: chosen == Some(c.id),
                    })
                    .collect(),
                ..Row::default()
            })
            .collect()
    }

    /// What a feat or power page shows of the choice under the cursor, else the one clicked, and its button.
    fn choice_words(
        &mut self,
        v: &mut MenuView,
        choices: &[Vec<Choice>],
        name_tag: &str,
        button: &str,
    ) {
        let (hover, chosen, taken) = self
            .menus
            .level
            .as_ref()
            .map_or((None, None, Vec::new()), |d| {
                (d.hover_choice, d.chosen, d.taken.clone())
            });
        let find = |id: Option<i32>| choices.iter().flatten().find(|c| Some(c.id) == id);
        let shown = find(hover).or(find(chosen));
        let name = shown.map_or(String::new(), |c| self.ui.strref(c.name));
        let words = shown.map_or(String::new(), |c| self.ui.strref(c.description));
        v.text(name_tag, name);
        v.text("LB_DESC", words);
        let removing = chosen.is_some_and(|c| taken.contains(&c));
        let label = if removing { STR_REMOVE } else { STR_ADD };
        let label = self.ui.strref(label);
        v.text(button, label);
        if !removing && !find(chosen).is_some_and(|c| c.can_take) {
            v.inert(&[button]);
        }
    }

    /// `ftchrgen.gui`.
    pub(super) fn level_feats_view(&mut self) -> MenuView {
        let mut v = MenuView::new("ftchrgen");
        self.page_title(&mut v);
        let choices = self.level_choices(Screen::LevelFeats);
        let rows = self.choice_rows(&choices);
        v.lists
            .insert("LB_FEATS".to_string(), (rows, self.menus.first));
        let slots = self
            .menus
            .level
            .as_ref()
            .and_then(|d| self.game.world.object(d.who))
            .map_or((0, 0), |o| o.feat_slots);
        v.text(
            "STD_REMAINING_SELECTIONS_LBL",
            (slots.0 + slots.1).to_string(),
        );
        self.choice_words(&mut v, &choices, "LBL_NAME", "BTN_SELECT");
        v
    }

    /// `pwrlvlup.gui`.
    pub(super) fn level_powers_view(&mut self) -> MenuView {
        let mut v = MenuView::new("pwrlvlup");
        self.page_title(&mut v);
        let choices = self.level_choices(Screen::LevelPowers);
        let rows = self.choice_rows(&choices);
        v.lists
            .insert("LB_POWERS".to_string(), (rows, self.menus.first));
        let points = self.menus.level.as_ref().map_or(0, |d| d.power_points);
        v.text("REMAINING_SELECTIONS_LBL", points.to_string());
        self.choice_words(&mut v, &choices, "LBL_POWER", "SELECT_BTN");
        v
    }

    /// The cursor moved on a level-up screen: what is under it says its words.
    pub(super) fn level_hover(&mut self, screen: Screen) {
        let hover = self.menus.hover.clone();
        let cell = match &hover {
            Some(Hit::Cell(_, row, cell)) => self
                .level_choices(screen)
                .get(*row)
                .and_then(|chain| chain.get(*cell))
                .map(|c| c.id),
            _ => None,
        };
        let row = match (&hover, screen) {
            (Some(Hit::Button(tag)), Screen::LevelAbilities) => {
                ABILITIES.iter().position(|a| tag.starts_with(a.0))
            }
            (Some(Hit::Button(tag)), Screen::LevelSkills) => SKILLS
                .iter()
                .position(|s| tag.starts_with(s.0) || tag.starts_with(&format!("{}_", s.1))),
            _ => None,
        };
        if let Some(d) = self.level_draft() {
            d.hover_choice = cell;
            if row.is_some() {
                d.about = row;
            }
        }
    }

    /// A click on a picture in a list of feats or powers.
    pub(super) fn level_cell(&mut self, screen: Screen, row: usize, cell: usize) {
        let id = self
            .level_choices(screen)
            .get(row)
            .and_then(|chain| chain.get(cell))
            .map(|c| c.id);
        if let Some(d) = self.level_draft() {
            d.chosen = id;
        }
    }

    fn enter_page(&mut self, screen: Screen) {
        let Some(d) = self.menus.level.as_ref() else {
            return;
        };
        let Some(now) = self.game.world.object(d.who).cloned() else {
            return;
        };
        if let Some(d) = self.level_draft() {
            d.page = now;
            d.page_points = (d.ability_points, d.power_points);
            d.taken.clear();
            d.about = None;
            d.hover_choice = None;
            d.chosen = None;
        }
        self.menus.first = 0;
        self.menus.stack.push(screen);
    }

    /// Back to how the page was found.
    fn reset_page(&mut self) {
        let Some(page) = self.menus.level.as_ref().map(|d| d.page.clone()) else {
            return;
        };
        self.restore(page);
        if let Some(d) = self.level_draft() {
            (d.ability_points, d.power_points) = d.page_points;
            d.taken.clear();
        }
    }

    /// Cancel on a page, or Esc.
    pub(super) fn cancel_page(&mut self) {
        self.reset_page();
        self.menus.stack.pop();
    }

    /// Back on the panel, or Esc: the level has not been taken.
    pub(super) fn abandon_level(&mut self) {
        if let Some(d) = self.menus.level.take() {
            self.restore(d.before);
        }
        self.menus.stack.pop();
    }

    fn accept_page(&mut self, step: usize) {
        let Some(d) = self.menus.level.as_ref() else {
            return;
        };
        if step == 0 {
            if d.ability_points > 0 {
                self.menus.message = "Spend the point first.".to_string();
                return;
            }
            let (who, before) = (d.who, d.page.abilities);
            self.game.world.settle_abilities(who, before);
        }
        if let Some(d) = self.level_draft() {
            d.done[step] = true;
        }
        self.menus.stack.pop();
    }

    fn accept_level(&mut self) {
        self.menus.level = None;
        self.menus.stack.pop();
    }

    fn change_ability(&mut self, tag: &str) {
        let Some(&(name, n, _)) = ABILITIES.iter().find(|a| tag.starts_with(a.0)) else {
            return;
        };
        let raise = tag == format!("{name}_PLUS_BTN");
        let Some((who, points, page)) = self
            .menus
            .level
            .as_ref()
            .map(|d| (d.who, d.ability_points, d.page.abilities[n]))
        else {
            return;
        };
        let Some(o) = self.game.world.object_mut(who) else {
            return;
        };
        let spent = if raise && points > 0 {
            o.abilities[n] += 1;
            1
        } else if !raise && o.abilities[n] > page {
            o.abilities[n] -= 1;
            -1
        } else {
            0
        };
        if let Some(d) = self.level_draft() {
            d.ability_points -= spent;
            d.about = Some(n);
        }
    }

    fn change_skill(&mut self, tag: &str) {
        let Some(n) = SKILLS
            .iter()
            .position(|s| tag.starts_with(&format!("{}_", s.1)))
        else {
            return;
        };
        let Some((who, page)) = self
            .menus
            .level
            .as_ref()
            .map(|d| (d.who, d.page.skills.get(n).copied().unwrap_or(0)))
        else {
            return;
        };
        let w = &mut self.game.world;
        if tag.ends_with("_PLUS_BTN") {
            let _ = w.buy_skill_rank(who, n);
        } else if w.skill_ranks(who, n) > page {
            w.refund_skill_rank(who, n);
        }
        if let Some(d) = self.level_draft() {
            d.about = Some(n);
        }
    }

    /// Add a feat or power with what the page has left.
    fn add_choice(&mut self, screen: Screen, id: i32) -> bool {
        let Some((who, points)) = self.menus.level.as_ref().map(|d| (d.who, d.power_points)) else {
            return false;
        };
        let w = &mut self.game.world;
        let added = if screen == Screen::LevelFeats {
            w.choose_feat(who, id).is_ok()
        } else if points > 0 && w.can_take_power(who, id) {
            if let Some(o) = w.object_mut(who) {
                o.spells.insert(id);
            }
            true
        } else {
            false
        };
        if let (true, Some(d)) = (added, self.level_draft()) {
            d.taken.push(id);
            if screen == Screen::LevelPowers {
                d.power_points -= 1;
            }
        }
        added
    }

    /// Add or Remove. Taking one back takes back what needed it, as the rest are added again.
    fn toggle_choice(&mut self, screen: Screen) {
        let Some((chosen, taken)) = self
            .menus
            .level
            .as_ref()
            .and_then(|d| Some((d.chosen?, d.taken.clone())))
        else {
            return;
        };
        if taken.contains(&chosen) {
            self.reset_page();
            for id in taken.into_iter().filter(|id| *id != chosen) {
                self.add_choice(screen, id);
            }
        } else {
            self.add_choice(screen, chosen);
        }
    }

    /// The page's Recommended button: it starts again and spends everything as the class's tables say.
    fn recommend_page(&mut self, screen: Screen) {
        let Some(who) = self.menus.level.as_ref().map(|d| d.who) else {
            return;
        };
        self.reset_page();
        let class = self.game.world.object(who).map_or(0, |o| o.class);
        match screen {
            Screen::LevelAbilities => {
                let n = self.game.world.primary_ability(class);
                let points = self.menus.level.as_ref().map_or(0, |d| d.ability_points);
                if let Some(o) = self.game.world.object_mut(who) {
                    o.abilities[n] += points;
                }
                if let Some(d) = self.level_draft() {
                    d.ability_points = 0;
                }
            }
            Screen::LevelSkills => {
                for skill in self.game.world.recommended_skills(class) {
                    while self.game.world.buy_skill_rank(who, skill).is_ok() {}
                }
            }
            Screen::LevelFeats => {
                while let Some(feat) = self.game.world.recommended_feat(who) {
                    if !self.add_choice(screen, feat) {
                        break;
                    }
                }
            }
            Screen::LevelPowers => {
                while let Some(power) = self.game.world.recommended_power(who) {
                    if !self.add_choice(screen, power) {
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    /// The Auto Level Up button: every step as Recommended has it.
    pub(super) fn auto_level_up(&mut self) {
        self.start_level_up();
        for (step, screen) in [
            (0, Screen::LevelAbilities),
            (1, Screen::LevelSkills),
            (2, Screen::LevelFeats),
            (3, Screen::LevelPowers),
        ] {
            if self.step_needed(step) {
                self.enter_page(screen);
                self.recommend_page(screen);
                self.accept_page(step);
            }
        }
        self.accept_level();
    }

    /// What a button on a level-up screen does.
    pub(super) fn level_press(&mut self, screen: Screen, tag: &str) {
        match (screen, tag) {
            (Screen::LevelUp, "BTN_BACK") => self.abandon_level(),
            (Screen::LevelUp, _) => {
                let number = tag
                    .strip_prefix("BTN_STEPNAME")
                    .or(tag.strip_prefix("LBL_"))
                    .and_then(|n| n.parse::<usize>().ok());
                let Some(step) = number.map(|n| n - 1) else {
                    return;
                };
                if step > self.next_step() || !self.step_needed(step) {
                    return;
                }
                match step {
                    0 => self.enter_page(Screen::LevelAbilities),
                    1 => self.enter_page(Screen::LevelSkills),
                    2 => self.enter_page(Screen::LevelFeats),
                    3 => self.enter_page(Screen::LevelPowers),
                    _ => self.accept_level(),
                }
            }
            (_, "BTN_BACK" | "BACK_BTN") => self.cancel_page(),
            (_, "BTN_RECOMMENDED" | "RECOMMENDED_BTN") => self.recommend_page(screen),
            (Screen::LevelAbilities, "BTN_ACCEPT") => self.accept_page(0),
            (Screen::LevelSkills, "BTN_ACCEPT") => self.accept_page(1),
            (Screen::LevelFeats, "BTN_ACCEPT") => self.accept_page(2),
            (Screen::LevelPowers, "ACCEPT_BTN") => self.accept_page(3),
            (Screen::LevelFeats | Screen::LevelPowers, "BTN_SELECT" | "SELECT_BTN") => {
                self.toggle_choice(screen)
            }
            (Screen::LevelAbilities, _)
                if tag.ends_with("_PLUS_BTN") || tag.ends_with("_MINUS_BTN") =>
            {
                self.change_ability(tag)
            }
            (Screen::LevelSkills, _)
                if tag.ends_with("_PLUS_BTN") || tag.ends_with("_MINUS_BTN") =>
            {
                self.change_skill(tag)
            }
            _ => {}
        }
    }
}
