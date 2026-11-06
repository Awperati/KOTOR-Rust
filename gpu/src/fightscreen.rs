//! The second and third slots under an enemy's bar, what they have queued, and the words that rise over a fight, as the
//! reference video has them at 6:05. **Assumed:** words last `FLOAT_SECONDS` and rise `FLOAT_RISE`: not measured.

use super::Stage;
use kotor_game::{CombatChoice, Kind, ObjId};
use kotor_ui::Float;

const STR_MISS: u32 = 42134;
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

    /// An arrow on a slot.
    pub(super) fn turn_slot(&mut self, tag: &str) {
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
