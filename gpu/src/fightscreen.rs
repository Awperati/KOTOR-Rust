//! The three slots under an enemy's bar and the words that rise over a fight. The owner: "The whole left window has up
//! and down arrows and that's where all the physical weapon attacks go, the middle slot is Force powers and the right
//! slot is grenades and other throwables". The reference video agrees at 6:05: arrows on the left, with Power Attack in
//! it. **Assumed:** words last `FLOAT_SECONDS` and rise `FLOAT_RISE`: not measured. **Assumed:** a throwable goes at once
//! and is not queued.

use super::Stage;
use kotor_game::{CombatAction, CombatChoice, Kind, ObjId};
use kotor_ui::{Float, SlotShown};

const STR_MISS: u32 = 42134;
/// The video's box: "ENEMY SIGHTED!", then how to go on.
const STR_ENEMY_SIGHTED: u32 = 48212;
const STR_ROUND: u32 = 42395;
const STR_PARTY_DOWN: u32 = 42397;
const STR_ACTION_MENU: u32 = 48415;
const STR_TARGET_CHANGED: u32 = 42481;
/// How long after a conversation the keys still do not pause or jump. **Assumed:** long enough for one more press.
const AFTER_TALK_SECONDS: f32 = 0.5;
const MOST_QUEUED: usize = 4;
const STR_ATTACK: u32 = 375;
const FLOAT_SECONDS: f32 = 1.5;
const FLOAT_RISE: f32 = 40.0;
const DAMAGE_COLOR: [f32; 3] = [1.0, 0.2, 0.15];
const MISS_COLOR: [f32; 3] = [0.8, 0.8, 0.8];
const XP_COLOR: [f32; 3] = [0.8, 0.3, 1.0];

/// What the slots show, each with whether it has more, and the one under the cursor with its name.
type SlotsView = ([Option<SlotShown>; 3], Option<(usize, String)>);

/// What one thing in a slot does.
#[derive(Clone, Copy, PartialEq)]
enum SlotDoes {
    Attack,
    Queue(CombatAction),
    Throw(ObjId),
    /// Try the target's lock with the skill and this spike.
    Security(Option<ObjId>),
    /// A power the leader uses on themselves: on, or off again.
    OnSelf(i32),
}

/// The Security skill's name in the talk table.
const STR_SECURITY: u32 = 329;
const SECURITY_PICTURE: &str = kotor_ui::LOCK_PICTURE;

/// One thing a slot offers.
struct SlotChoice {
    does: SlotDoes,
    name: String,
    icon: String,
    /// False when it is too dear for now.
    usable: bool,
}

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
    /// What each slot shows.
    picks: [usize; 3],
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
    /// What each quick slot at the bottom right shows.
    quick_picks: [usize; 3],
    /// A driver's player cannot fall.
    unhurt: bool,
}

/// Which of the slots a tag is.
pub fn target_slot(tag: &str) -> Option<usize> {
    match tag {
        "BTN_TARGET0" => Some(0),
        "BTN_TARGET1" => Some(1),
        "BTN_TARGET2" => Some(2),
        _ => None,
    }
}

/// What `--grenadier` hands out.
const GRENADIER_COUNT: i32 = 5;

impl Stage {
    /// Left: the plain attack, then the feats. Middle: Force powers. Right: what can be thrown.
    fn slot_choices(&mut self, slot: usize) -> Vec<SlotChoice> {
        let locked = self.lock_choices();
        if !locked.is_empty() {
            return if slot == 0 { locked } else { Vec::new() };
        }
        let pc = self.game.world.pc;
        let queued = |ui: &mut kotor_ui::Ui, c: CombatChoice| SlotChoice {
            does: SlotDoes::Queue(c.action),
            name: ui.strref(c.name),
            icon: c.icon,
            usable: c.usable,
        };
        match slot {
            0 => {
                let mut out = vec![SlotChoice {
                    does: SlotDoes::Attack,
                    name: self.ui.strref(STR_ATTACK),
                    icon: super::ATTACK_PICTURE.to_string(),
                    usable: true,
                }];
                let feats = self.game.world.combat_feats(pc);
                out.extend(feats.into_iter().map(|c| queued(&mut self.ui, c)));
                out
            }
            1 => {
                let powers = self.game.world.combat_powers(pc);
                powers
                    .into_iter()
                    .map(|c| queued(&mut self.ui, c))
                    .collect()
            }
            _ => self.throwables(),
        }
    }

    /// The ways to try the target's lock: the skill if the leader has points in it, and each spike carried, which is no use
    /// without them. Nothing for what is not locked, or what no skill can open.
    fn lock_choices(&mut self) -> Vec<SlotChoice> {
        let Some(id) = self.target() else {
            return Vec::new();
        };
        let w = &mut self.game.world;
        let shut = w.object(id).is_some_and(|o| {
            o.locked
                && !o.open
                && matches!(o.kind, kotor_game::Kind::Door | kotor_game::Kind::Placeable)
        });
        if !shut || !w.lock_can_be_picked(id) {
            return Vec::new();
        }
        let pc = w.pc;
        let (trained, spikes) = w.lock_ways(pc);
        let views: Vec<_> = spikes.iter().filter_map(|i| w.item_view(*i)).collect();
        let mut out = Vec::new();
        if trained {
            out.push(SlotChoice {
                does: SlotDoes::Security(None),
                name: self.ui.strref(STR_SECURITY),
                icon: SECURITY_PICTURE.to_string(),
                usable: true,
            });
        }
        for v in views {
            out.push(SlotChoice {
                does: SlotDoes::Security(Some(v.id)),
                icon: self.first_picture(&v.icons),
                name: v.name,
                usable: trained,
            });
        }
        out
    }

    pub(super) fn target_has_a_way_in(&mut self) -> bool {
        !self.lock_choices().is_empty()
    }

    pub(super) fn first_picture(&mut self, names: &[String]) -> String {
        names
            .iter()
            .find(|n| self.ui.texture(n).is_some())
            .cloned()
            .unwrap_or_default()
    }

    /// What a quick slot offers. The third has the powers for oneself: the video shows it empty on a Soldier.
    fn quick_choices(&mut self, slot: usize) -> Vec<SlotChoice> {
        let w = &mut self.game.world;
        let pc = w.pc;
        if slot == 2 {
            let powers = w.self_powers(pc);
            return powers
                .into_iter()
                .map(|c| {
                    let CombatAction::Power(spell) = c.action else {
                        unreachable!("self_powers gives powers");
                    };
                    SlotChoice {
                        does: SlotDoes::OnSelf(spell),
                        name: self.ui.strref(c.name),
                        icon: c.icon,
                        usable: c.usable,
                    }
                })
                .collect();
        }
        let mut lists = w.quick_items(pc);
        let items = std::mem::take(&mut lists[slot]);
        let views: Vec<_> = items.iter().filter_map(|i| w.item_view(*i)).collect();
        views
            .into_iter()
            .map(|v| SlotChoice {
                does: SlotDoes::Throw(v.id),
                icon: self.first_picture(&v.icons),
                name: v.name,
                usable: true,
            })
            .collect()
    }

    /// What the quick slots show, and the name of the one under the cursor.
    pub(super) fn quick_view(&mut self) -> ([Option<SlotShown>; 3], Option<String>) {
        let mut shows = [None, None, None];
        let mut hover = None;
        let over = self.menus.fight.hover.clone();
        for (slot, shown) in shows.iter_mut().enumerate() {
            let choices = self.quick_choices(slot);
            if choices.is_empty() {
                continue;
            }
            let pick = &choices[self.menus.fight.quick_picks[slot] % choices.len()];
            let w = &self.game.world;
            *shown = Some(SlotShown {
                picture: pick.icon.clone(),
                more: choices.len() > 1,
                greyed: !pick.usable,
                lit: matches!(pick.does, SlotDoes::OnSelf(p) if w.power_active(w.pc, p)),
            });
            let tag = format!("BTN_ACTION{}", slot + 1);
            if over.as_deref() == Some(tag.as_str()) {
                hover = Some(pick.name.clone());
            }
        }
        (shows, hover)
    }

    fn quick_slot_of(tag: &str) -> usize {
        tag.chars()
            .last()
            .and_then(|c| c.to_digit(10))
            .map_or(1, |n| n as usize)
            .clamp(1, 3)
            - 1
    }

    /// A click on a quick slot: the leader uses what it shows on themselves.
    pub(super) fn use_quick_slot(&mut self, tag: &str) {
        let slot = Self::quick_slot_of(tag);
        let choices = self.quick_choices(slot);
        if choices.is_empty() {
            return;
        }
        let w = &mut self.game.world;
        let pc = w.pc;
        match choices[self.menus.fight.quick_picks[slot] % choices.len()].does {
            SlotDoes::Throw(item) => {
                let _ = w.use_item_on(pc, item, pc);
            }
            SlotDoes::OnSelf(power) if w.power_active(pc, power) => w.end_power(pc, power),
            // At once, in a fight too, and it can fail for want of Force: the slot says nothing.
            SlotDoes::OnSelf(power) => {
                let _ = w.cast_spell(pc, power, pc, false);
            }
            _ => {}
        }
    }

    /// What a quick slot shows, for a test.
    pub fn quick_slot_shown(&mut self, slot: usize) -> Option<SlotShown> {
        self.quick_view().0[slot].clone()
    }

    /// An arrow on a quick slot.
    pub(super) fn turn_quick_slot(&mut self, tag: &str) {
        let slot = Self::quick_slot_of(tag);
        let count = self.quick_choices(slot).len();
        if count == 0 {
            return;
        }
        let pick = self.menus.fight.quick_picks[slot] % count;
        self.menus.fight.quick_picks[slot] = if tag.starts_with("BTN_ACTIONUP") {
            (pick + count - 1) % count
        } else {
            (pick + 1) % count
        };
    }

    /// What the right slot shows, for a test driver.
    pub(in crate::stage) fn right_slot_name(&mut self) -> String {
        let choices = self.throwables();
        if choices.is_empty() {
            return "-".to_string();
        }
        choices[self.menus.fight.picks[2] % choices.len()]
            .name
            .replace(' ', "_")
    }

    fn throwables(&mut self) -> Vec<SlotChoice> {
        let w = &mut self.game.world;
        let pc = w.pc;
        let thrown = w.combat_throwables(pc);
        let views: Vec<_> = thrown.iter().filter_map(|i| w.item_view(*i)).collect();
        views
            .into_iter()
            .map(|v| SlotChoice {
                does: SlotDoes::Throw(v.id),
                icon: v
                    .icons
                    .iter()
                    .find(|n| self.ui.texture(n).is_some())
                    .cloned()
                    .unwrap_or_default(),
                name: v.name,
                usable: true,
            })
            .collect()
    }

    pub(in crate::stage) fn set_hud_hover(&mut self, tag: Option<String>) {
        self.menus.fight.hover = tag;
    }

    /// The cursor is on the first of the four slots at the bottom right.
    pub(super) fn over_first_action_slot(&self) -> bool {
        self.menus.fight.hover.as_deref() == Some("BTN_ACTION0")
    }

    /// The cursor is on the Attack mode button.
    pub(super) fn over_attack_mode(&self) -> bool {
        self.menus.fight.hover.as_deref() == Some(kotor_ui::ATTACK_MODE_BUTTON)
    }

    /// The cursor is on one of the slots under an enemy's bar.
    pub(in crate::stage) fn over_target_slots(&self) -> bool {
        let tag = self.menus.fight.hover.as_deref().unwrap_or("");
        tag.starts_with("BTN_TARGET")
    }

    /// What the slots show and the one under the cursor; what is queued goes after what is under way.
    pub(super) fn slots_view(&mut self, hostile: bool, queue: &mut Vec<String>) -> SlotsView {
        let queued: Vec<_> = self.game.world.combat_queue.iter().copied().collect();
        let mut slots = [None, None, None];
        let mut hover = None;
        let over = self.menus.fight.hover.clone();
        for (slot, shows) in slots.iter_mut().enumerate() {
            let choices = self.slot_choices(slot);
            for q in &queued {
                if let Some(c) = choices.iter().find(|c| c.does == SlotDoes::Queue(*q)) {
                    queue.push(c.icon.clone());
                }
            }
            if !hostile || choices.is_empty() {
                continue;
            }
            let pick = self.menus.fight.picks[slot] % choices.len();
            let shown = &choices[pick];
            *shows = Some(SlotShown {
                picture: shown.icon.clone(),
                more: choices.len() > 1,
                greyed: !shown.usable,
                lit: false,
            });
            if over.as_deref().and_then(target_slot) == Some(slot) {
                hover = Some((slot, shown.name.clone()));
            }
        }
        (slots, hover)
    }

    /// A click on a slot: what it shows is done. Returns whether the player should now go for the target.
    pub(in crate::stage) fn use_slot(&mut self, slot: usize) -> bool {
        let choices = self.slot_choices(slot);
        if choices.is_empty() {
            return false;
        }
        let pick = self.menus.fight.picks[slot] % choices.len();
        if !choices[pick].usable {
            return false;
        }
        match choices[pick].does {
            SlotDoes::Attack => true,
            // Only the quick slots have these.
            SlotDoes::OnSelf(_) => false,
            SlotDoes::Security(spike) => {
                self.game.world.lock_tool = spike;
                true
            }
            SlotDoes::Queue(action) => {
                let w = &mut self.game.world;
                let room = w.combat_queue.len() < MOST_QUEUED;
                if room {
                    w.combat_queue.push_back(action);
                }
                room
            }
            SlotDoes::Throw(item) => {
                if let Some(foe) = self.target() {
                    let w = &mut self.game.world;
                    let pc = w.pc;
                    // It can fail, as with no line to the target: the item stays, and the slot says nothing.
                    let _ = w.use_item_on(pc, item, foe);
                }
                false
            }
        }
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

    /// Whether a pilot playing to reach a fight should stop now: an enemy is in sight.
    pub fn pilot_arrived(&self) -> bool {
        !self.game.world.in_dialogue() && self.seen_enemy().is_some()
    }

    /// The player is one kill of the enemy in sight short of a level. How much the pilot earns on the way changes with
    /// everything it meets, so a driver cannot wait for it to come out just so.
    pub fn one_kill_short_of_a_level(&mut self) {
        let foe = self.seen_enemy();
        let w = &mut self.game.world;
        let (Some(foe), Some((xp, level))) = (foe, w.object(w.pc).map(|o| (o.xp, o.level))) else {
            return;
        };
        let kill = w.kill_xp(foe);
        let mut short = xp;
        while w.level_for_xp(short + kill) <= level {
            short += 1;
        }
        if short > xp {
            w.grant_xp(w.pc, short - xp);
        }
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

    /// A driver's thrower stays where it was armed until its first throw: the AI's decision comes at a round's start, and
    /// by then a soldier that has run at the player is close, where it fights with its weapon and keeps its grenades.
    pub(crate) fn thrower_tick(&mut self) {
        let (Some((who, home)), Some(item)) = (self.armed_home, self.armed) else {
            return;
        };
        let w = &mut self.game.world;
        if w.object(item).map_or(0, |o| o.stack) >= GRENADIER_COUNT {
            w.teleport(who, home);
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

    /// For a test driver: an enemy has grenades, and throws them. The fight is the same every run: a fixed seed, a
    /// player with 500 hit points who starts unhurt, and the nearest enemy that is at least `HOLDS_WITHIN` metres off
    /// (the game's own AI keeps its grenades for a foe farther than that and fights close up with its weapon).
    pub fn arm_nearest_enemy(&mut self) {
        const COUNT: i32 = GRENADIER_COUNT;
        const GRENADE: &str = "g_w_fraggren01";
        const THROWS: i32 = 4;
        const HOLDS_WITHIN: f32 = 5.0;
        const SEED: u64 = 7;
        const HIT_POINTS: i32 = 500;
        let w = &mut self.game.world;
        let pc = w.pc;
        w.set_rng_seed(SEED);
        if let Some(o) = w.object_mut(pc) {
            (o.max_hp, o.hp) = (HIT_POINTS, HIT_POINTS);
        }
        let at = w.object(pc).map_or([0.0; 3], |o| o.position);
        let distance =
            |o: &kotor_game::Object| (o.position[0] - at[0]).hypot(o.position[1] - at[1]);
        let foes: Vec<&kotor_game::Object> = w
            .objects
            .values()
            .filter(|o| !o.dead && o.id != pc && o.kind == kotor_game::Kind::Creature)
            .filter(|o| w.reputation(o.id, pc) <= 10)
            .collect();
        let near = foes
            .iter()
            .filter(|o| distance(o) >= HOLDS_WITHIN)
            .min_by(|a, b| distance(a).total_cmp(&distance(b)))
            .or_else(|| {
                foes.iter()
                    .min_by(|a, b| distance(a).total_cmp(&distance(b)))
            })
            .map(|o| o.id);
        if let Some(near) = near {
            let item = w.spawn_item(GRENADE);
            w.carry(near, item);
            if let Some(o) = w.object_mut(item) {
                o.stack = COUNT;
            }
            if let Some(o) = w.object_mut(near) {
                o.ai_style = THROWS;
                // The party would otherwise kill it before its first throw, which takes up to 18 s.
                o.min_one_hp = true;
            }
            self.armed = Some(item);
            // Stand it on open floor 8 m off, in sight of the player, so it throws the same way every run: farther first, in sight
            // first, and only past 4 m (the AI keeps its grenades for a foe closer than that).
            let spot = w.nav.as_ref().and_then(|nav| {
                [true, false].iter().find_map(|need_sight| {
                    [8.0f32, 7.0, 6.0, 5.5, 5.0, 4.6].iter().find_map(|r| {
                        (0..24).find_map(|k| {
                            let a = k as f32 * std::f32::consts::TAU / 24.0;
                            let (x, y) = (at[0] + r * a.cos(), at[1] + r * a.sin());
                            let z = nav.height_at(x, y, at[2] + 1.0)?;
                            let p = [x, y, z];
                            let (eye, far) = ([at[0], at[1], at[2] + 1.5], [x, y, z + 1.5]);
                            let seen = !nav.sight_blocked(eye, far);
                            (nav.same_island(at, p) && (seen || !need_sight)).then_some(p)
                        })
                    })
                })
            });
            if let Some(p) = spot {
                w.teleport(near, p);
            }
            let home = w.object(near).map_or([0.0; 3], |o| o.position);
            // Held at home in the world's own steps, not once a frame: a slow frame runs several steps, and a soldier that has run up
            // keeps its grenades.
            w.hold_in_place = Some(kotor_game::HoldInPlace {
                who: near,
                home,
                item,
                count: GRENADIER_COUNT,
                player: at,
            });
            self.armed_home = Some((near, home));
        }
    }

    /// Space: shifts in a race, goes on with a line or a container, or else jumps. Not just after a conversation, and not
    /// in a pause.
    pub fn jump_key(&mut self) {
        if !self.menu_open() && self.race_fire() {
            return;
        }
        let w = &self.game.world;
        if self.menu_open() || w.in_dialogue() || w.opened.is_some() {
            self.continue_line();
        } else if !self.menus.paused && w.time - self.menus.fight.talk_ended >= AFTER_TALK_SECONDS {
            self.ask_to_jump();
        }
    }

    /// X: pauses or goes on. It never pauses with a container open, nor just after a conversation: a player going through its
    /// lines presses once more than there are.
    pub fn pause_key(&mut self) {
        let w = &self.game.world;
        if w.opened.is_some() {
            return;
        }
        if self.menus.paused || w.time - self.menus.fight.talk_ended >= AFTER_TALK_SECONDS {
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
            .replace("<Pause>", &self.key_words("Pause"));
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
        if self.menus.auto_pause_on || self.combat_autosave {
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

        if fresh {
            self.autosave_before_fight();
        }
        self.menus.fight.sighted.extend(enemies);
        self.menus.fight.down = down;
        self.menus.fight.round = round;
        self.menus.fight.target = target;
        if !self.menus.auto_pause_on {
            return;
        }
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
        let slot = tag
            .chars()
            .last()
            .and_then(|c| c.to_digit(10))
            .map_or(0, |n| n as usize)
            .min(2);
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
            new.push((
                t.position,
                l.damage,
                l.outcome == kotor_game::Outcome::Glance,
            ));
        }
        self.menus.fight.log_seen = log.len();
        let xp = w.object(w.pc).map_or(0, |o| o.xp);
        let gained = self.menus.fight.xp_seen.map_or(0, |old| xp - old);
        self.menus.fight.xp_seen = Some(xp);
        let over_player = w.object(w.pc).map_or([0.0; 3], |o| o.position);

        let miss = self.ui.strref(STR_MISS);
        for (at, damage, glance) in new {
            let (text, color) = if damage > 0 {
                let text = if glance {
                    format!("Glancing {damage}")
                } else {
                    damage.to_string()
                };
                (text, DAMAGE_COLOR)
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

    /// A picture of the interface by name, for a test.
    pub fn ui_texture_for_test(&mut self, name: &str) -> Option<kotor_image::Image> {
        self.ui.texture(name).cloned()
    }

    /// What the first slot under the target shows, for a test.
    pub fn combat_words_of_slot0(&mut self) -> Option<SlotShown> {
        let action = self.target_view_action();
        self.combat_view(action).slots[0].clone()
    }

    /// What the name bar says of the slot under the cursor, for a test.
    pub fn combat_words(&mut self) -> Option<String> {
        let action = self.target_view_action();
        self.combat_view(action).quick_hover
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

    /// Words rising over the player.
    pub(in crate::stage) fn say_over_player(&mut self, text: String) {
        let p = self.game.world.pc_mover.pos;
        let born = self.game.world.time;
        self.menus.fight.rising.push(Rising {
            at: p,
            text,
            color: XP_COLOR,
            born,
        });
    }

    /// The words rising now. For tests.
    pub fn floats_shown(&self) -> Vec<String> {
        self.floats_view().into_iter().map(|f| f.text).collect()
    }
}
