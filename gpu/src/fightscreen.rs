//! The second and third slots under an enemy's bar, what they have queued, and the words that rise over a fight, as the
//! reference video has them at 6:05. **Assumed:** words last `FLOAT_SECONDS` and rise `FLOAT_RISE`: not measured.

use super::Stage;
use kotor_game::{CombatChoice, Kind, ObjId};
use kotor_ui::Float;

const STR_MISS: u32 = 42134;
/// The video's box: "ENEMY SIGHTED!", then how to go on.
const STR_ENEMY_SIGHTED: u32 = 48212;
const STR_ROUND: u32 = 42395;
const STR_PARTY_DOWN: u32 = 42397;
const STR_ACTION_MENU: u32 = 48415;
const STR_TARGET_CHANGED: u32 = 42481;
/// Words the talk table fills in for the key, as the viewer has it.
const PAUSE_KEY: &str = "Space";
/// How long after a conversation Space still does not pause. **Assumed:** long enough for one more press.
const AFTER_TALK_SECONDS: f32 = 0.5;
const MOST_QUEUED: usize = 4;
const FLOAT_SECONDS: f32 = 1.5;
const FLOAT_RISE: f32 = 40.0;
const DAMAGE_COLOR: [f32; 3] = [1.0, 0.2, 0.15];
const MISS_COLOR: [f32; 3] = [0.8, 0.8, 0.8];
const XP_COLOR: [f32; 3] = [0.8, 0.3, 1.0];

/// What the two slots show, each with whether it has more, and the one under the cursor with its name.
type SlotsView = ([Option<(String, bool)>; 2], Option<(usize, String)>);

/// A word rising over someone.
#[derive(Clone)]
struct Rising {
    at: [f32; 3],
    text: String,
    color: [f32; 3],
    born: f32,
}

#[derive(Default)]
pub(super) struct Fight {
    /// What the feats' and the powers' slots show.
    picks: [usize; 2],
    /// The HUD's button under the cursor.
    hover: Option<String>,
    rising: Vec<Rising>,
    /// How much of the fight's log and the player's experience has been shown.
    log_seen: usize,
    xp_seen: Option<i32>,
    /// What has already paused the game: enemies seen, the last round, who was down and the last target.
    sighted: std::collections::BTreeSet<ObjId>,
    /// When a conversation was last open.
    talk_ended: f32,
    round: u32,
    down: std::collections::BTreeSet<ObjId>,
    target: Option<ObjId>,
    /// A driver's player cannot fall.
    unhurt: bool,
}

/// Which of the two choosing slots a tag is.
pub fn target_slot(tag: &str) -> Option<usize> {
    match tag {
        "BTN_TARGET1" => Some(0),
        "BTN_TARGET2" => Some(1),
        _ => None,
    }
}

impl Stage {
    fn slot_choices(&mut self, slot: usize) -> Vec<CombatChoice> {
        let w = &mut self.game.world;
        let pc = w.pc;
        if slot == 0 {
            w.combat_feats(pc)
        } else {
            w.combat_powers(pc)
        }
    }

    pub(in crate::stage) fn set_hud_hover(&mut self, tag: Option<String>) {
        self.menus.fight.hover = tag;
    }

    /// The cursor is on one of the slots under an enemy's bar.
    pub(in crate::stage) fn over_target_slots(&self) -> bool {
        let tag = self.menus.fight.hover.as_deref().unwrap_or("");
        tag.starts_with("BTN_TARGET")
    }

    /// What the two slots show and the one under the cursor; what is queued goes after what is under way.
    pub(super) fn slots_view(&mut self, hostile: bool, queue: &mut Vec<String>) -> SlotsView {
        let queued: Vec<_> = self.game.world.combat_queue.iter().copied().collect();
        let mut slots = [None, None];
        let mut hover = None;
        let over = self.menus.fight.hover.clone();
        for (slot, shows) in slots.iter_mut().enumerate() {
            let choices = self.slot_choices(slot);
            for q in &queued {
                if let Some(c) = choices.iter().find(|c| c.action == *q) {
                    queue.push(c.icon.clone());
                }
            }
            if !hostile || choices.is_empty() {
                continue;
            }
            let pick = self.menus.fight.picks[slot] % choices.len();
            let shown = &choices[pick];
            *shows = Some((shown.icon.clone(), choices.len() > 1));
            if over.as_deref().and_then(target_slot) == Some(slot) {
                hover = Some((slot + 1, self.ui.strref(shown.name)));
            }
        }
        (slots, hover)
    }

    /// A click on a slot: what it shows is done next. Returns whether anything was queued.
    pub(in crate::stage) fn queue_slot(&mut self, slot: usize) -> bool {
        let choices = self.slot_choices(slot);
        if choices.is_empty() || self.game.world.combat_queue.len() >= MOST_QUEUED {
            return false;
        }
        let pick = self.menus.fight.picks[slot] % choices.len();
        self.game.world.combat_queue.push_back(choices[pick].action);
        true
    }

    /// The nearest living enemy the leader has seen.
    pub fn seen_enemy(&self) -> Option<ObjId> {
        let w = &self.game.world;
        let pc = w.pc;
        let here = w.object(pc)?.position;
        w.combat
            .perceived
            .iter()
            .filter(|(watcher, _)| *watcher == pc)
            .map(|(_, seen)| *seen)
            .filter(|id| w.object(*id).is_some_and(|o| !o.dead) && w.reputation(pc, *id) <= 10)
            .min_by_key(|id| {
                let o = w.object(*id).map_or(here, |o| o.position);
                (((o[0] - here[0]).hypot(o[1] - here[1])) * 100.0) as i32
            })
    }

    /// Whether a pilot playing to reach a fight should stop now: an enemy is in sight, or with `for_level` one whose
    /// death gives the player a level.
    pub fn pilot_arrived(&self, for_level: bool) -> bool {
        let w = &self.game.world;
        if w.in_dialogue() {
            return false;
        }
        let (Some(foe), Some(pc)) = (self.seen_enemy(), w.object(w.pc)) else {
            return false;
        };
        !for_level || w.level_for_xp(pc.xp + w.kill_xp(foe)) > pc.level
    }

    /// The pilot stops and the player takes over at the normal speed, with levels and screens as in a played game. It
    /// could not die, so its wounds count for nothing.
    pub fn take_over_from_pilot(&mut self, auto_pause: bool) {
        self.pilot = None;
        self.speed = 1.0;
        self.enable_auto_pause(auto_pause);
        self.disengage();
        let w = &mut self.game.world;
        w.hold_levels = true;
        w.party_screen = true;
        let pc = w.pc;
        if let Some(o) = w.object_mut(pc) {
            o.hp = o.max_hp;
        }
    }

    /// The player's wounds heal at once, for a driver handed a fight a player of that level could not win.
    pub fn keep_player_standing(&mut self, on: bool) {
        self.menus.fight.unhurt = on;
    }

    /// Things pause the game by themselves, as the Auto-Pause page says. Off unless asked for, like the pop-ups.
    pub fn enable_auto_pause(&mut self, on: bool) {
        self.menus.auto_pause_on = on;
    }

    /// The game is paused. For tests and the driver.
    pub fn paused(&self) -> bool {
        self.menus.paused
    }

    /// Space: goes on with a line or a container, or else pauses. It never pauses in a conversation, nor just after
    /// one: a player going through its lines presses once more than there are.
    pub fn space_key(&mut self) {
        let w = &self.game.world;
        if self.menu_open() || w.in_dialogue() || w.opened.is_some() {
            self.continue_line();
        } else if self.menus.paused || w.time - self.menus.fight.talk_ended >= AFTER_TALK_SECONDS {
            self.toggle_pause();
        }
    }

    /// The pause key, or the hourglass on the HUD.
    pub fn toggle_pause(&mut self) {
        if self.started && !self.menu_open() && !self.game.world.in_dialogue() {
            self.menus.paused = !self.menus.paused;
            self.menus.pause_note.clear();
        }
    }

    /// Pause with a title over how to go on.
    fn pause_for(&mut self, title: u32) {
        let line = self.ui.strref(STR_ENEMY_SIGHTED);
        let go_on = line
            .lines()
            .nth(1)
            .unwrap_or("")
            .replace("<Pause>", PAUSE_KEY);
        let title = if title == STR_ENEMY_SIGHTED {
            line.lines().next().unwrap_or("").to_string()
        } else {
            self.ui.strref(title)
        };
        self.menus.paused = true;
        self.menus.pause_note = format!("{title}\n{go_on}");
    }

    /// See what should pause the game. Returns whether it is paused now.
    pub(in crate::stage) fn pause_tick(&mut self) -> bool {
        if self.menus.fight.unhurt {
            let w = &mut self.game.world;
            let pc = w.pc;
            if let Some(o) = w.object_mut(pc) {
                o.hp = o.max_hp;
            }
        }
        if self.game.world.in_dialogue() {
            self.menus.paused = false;
            self.menus.fight.talk_ended = self.game.world.time;
            return false;
        }
        if self.menus.auto_pause_on {
            self.auto_pause();
        }
        self.menus.paused
    }

    fn auto_pause(&mut self) {
        let s = self.menus.settings;
        let w = &self.game.world;
        let pc = w.pc;
        let enemies: Vec<ObjId> = w
            .combat
            .perceived
            .iter()
            .filter(|(watcher, _)| *watcher == pc || w.party.contains(watcher))
            .map(|(_, seen)| *seen)
            .filter(|id| w.object(*id).is_some_and(|o| !o.dead) && w.reputation(pc, *id) <= 10)
            .collect();
        let fresh = enemies
            .iter()
            .any(|e| !self.menus.fight.sighted.contains(e));
        let down: std::collections::BTreeSet<ObjId> = std::iter::once(pc)
            .chain(w.party.iter().copied())
            .filter(|id| w.object(*id).is_some_and(|o| o.down || o.dead))
            .collect();
        let new_down = down.iter().any(|d| !self.menus.fight.down.contains(d));
        let round = w.combat.round;
        let round_over = round != self.menus.fight.round && self.party_blocked();
        let target = w.combat.target.get(&pc).copied();
        let old_target = self.menus.fight.target;
        let retarget = target.is_some() && old_target.is_some() && target != old_target;

        self.menus.fight.sighted.extend(enemies);
        self.menus.fight.down = down;
        self.menus.fight.round = round;
        self.menus.fight.target = target;
        if fresh && s.pause_enemy {
            self.pause_for(STR_ENEMY_SIGHTED);
        } else if new_down && s.pause_party {
            self.pause_for(STR_PARTY_DOWN);
        } else if retarget && s.pause_target {
            self.pause_for(STR_TARGET_CHANGED);
        } else if round_over && s.pause_round {
            self.pause_for(STR_ROUND);
        }
    }

    /// Whether the game is paused, and the words to show.
    pub(in crate::stage) fn pause_view(&self) -> (bool, String) {
        (self.menus.paused, self.menus.pause_note.clone())
    }

    /// An arrow on a slot.
    pub(super) fn turn_slot(&mut self, tag: &str) {
        if self.menus.auto_pause_on && self.menus.settings.pause_action && self.party_blocked() {
            self.pause_for(STR_ACTION_MENU);
        }
        let slot = usize::from(tag.ends_with('2'));
        let count = self.slot_choices(slot).len();
        if count == 0 {
            return;
        }
        let pick = self.menus.fight.picks[slot];
        self.menus.fight.picks[slot] = if tag.contains("UP") {
            (pick + count - 1) % count
        } else {
            (pick + 1) % count
        };
    }

    /// Once a step: what the fight's log and the player's experience have added.
    pub(super) fn floats_tick(&mut self) {
        let w = &self.game.world;
        let time = w.time;
        let is_party = |id: ObjId| id == w.pc || w.party.contains(&id);
        let log = &w.combat.log;
        let seen = self.menus.fight.log_seen.min(log.len());
        let mut new = Vec::new();
        let mut hit = None;
        for l in &log[seen..] {
            if !is_party(l.attacker) && !is_party(l.target) {
                continue;
            }
            let Some(t) = w.object(l.target).filter(|o| o.kind == Kind::Creature) else {
                continue;
            };
            hit = Some(t.position);
            new.push((t.position, l.damage));
        }
        self.menus.fight.log_seen = log.len();
        let xp = w.object(w.pc).map_or(0, |o| o.xp);
        let gained = self.menus.fight.xp_seen.map_or(0, |old| xp - old);
        self.menus.fight.xp_seen = Some(xp);
        let over_player = w.object(w.pc).map_or([0.0; 3], |o| o.position);

        let miss = self.ui.strref(STR_MISS);
        for (at, damage) in new {
            let (text, color) = if damage > 0 {
                (damage.to_string(), DAMAGE_COLOR)
            } else {
                (miss.clone(), MISS_COLOR)
            };
            self.menus.fight.rising.push(Rising {
                at,
                text,
                color,
                born: time,
            });
        }
        if gained > 0 {
            self.menus.fight.rising.push(Rising {
                at: hit.unwrap_or(over_player),
                text: format!("XP {gained}"),
                color: XP_COLOR,
                born: time - 0.2,
            });
        }
        self.menus
            .fight
            .rising
            .retain(|r| time - r.born < FLOAT_SECONDS);
    }

    /// The rising words where they are now.
    pub(in crate::stage) fn floats_view(&self) -> Vec<Float> {
        if !self.menus.settings.floating_numbers {
            return Vec::new();
        }
        let time = self.game.world.time;
        let mut out = Vec::new();
        for r in &self.menus.fight.rising {
            if let Some((x, y)) = self.on_screen(r.at, 2.0) {
                let age = (time - r.born) / FLOAT_SECONDS;
                out.push(Float {
                    x,
                    y: y - (age * FLOAT_RISE) as i32,
                    text: r.text.clone(),
                    color: r.color,
                });
            }
        }
        out
    }

    /// The words rising now. For tests.
    pub fn floats_shown(&self) -> Vec<String> {
        self.floats_view().into_iter().map(|f| f.text).collect()
    }
}
