//! A scripted player: it walks a module the way a person would, using only what a player can do (click to walk, click
//! on things, pick replies, take items, attack), and writes down every place it gets stuck.
//!
//! It knows nothing about the story. In each module it fights what it sees, talks to everyone once, opens every chest
//! and door it can reach, and then leaves by whatever door or trigger leads to another module.

use crate::follow::dist2d;
use crate::game::Game;
use crate::world::{DoorState, Kind, ObjId, World};
use kotor_core::ResType;
use std::collections::{BTreeMap, HashSet};

/// Whether a door can answer back when it is tried.
fn door_has_a_say(o: &crate::world::Object) -> bool {
    !o.conversation.is_empty() || o.scripts.contains_key("OnFailToOpen")
}

/// Things a playthrough can run into.
#[derive(Clone, Debug, PartialEq)]
pub struct Stall {
    /// `walk`, `unreachable`, `locked`, `fight`, `conversation`, `script`, `softlock`, `transition` or `time`.
    pub kind: &'static str,
    pub module: String,
    pub position: [f32; 3],
    pub objective: String,
    pub detail: String,
    /// Seconds into the playthrough.
    pub time: f32,
}

#[derive(Clone, Debug, PartialEq)]
enum Goal {
    Fight(ObjId),
    Use(ObjId, String),
    Walk([f32; 3], String),
}

/// Seconds without moving, while meaning to, before the walk counts as stuck.
const STUCK_SECONDS: f32 = 8.0;
const FIGHT_SECONDS: f32 = 90.0;
const CONVERSATION_SECONDS: f32 = 240.0;
/// How far the pilot notices an enemy.
const FIGHT_RANGE: f32 = 18.0;
/// How long it explores a module before it heads for the way out.
const EXPLORE_SECONDS: f32 = 400.0;
/// How many times a locked door with no script or conversation is tried.
const PLAIN_LOCK_TRIES: u32 = 2;
/// How long a line that wants no answer stays up in a window.
const READ_SECONDS: f32 = 7.0;
/// How many talks in one module may send the pilot back round its doors.
const SECOND_LOOKS: u32 = 30;
/// How long nothing to do has to last before the pilot stops.
const GIVE_UP_SECONDS: f32 = 15.0;
/// How many times a locked door is tried.
const LOCK_TRIES: u32 = 8;

pub struct Autopilot {
    pub stalls: Vec<Stall>,
    pub log: Vec<String>,
    /// Modules reached, in order.
    pub modules: Vec<String>,
    /// Keep the player alive, so that one lost fight does not hide everything behind it.
    pub immortal: bool,
    /// Nothing more to try.
    pub finished: bool,
    /// Leave lines that want no answer to whoever is showing them, as the window does.
    pub read_lines: bool,
    pub time: f32,
    goal: Option<Goal>,
    goal_time: f32,
    tried: HashSet<ObjId>,
    unreachable: HashSet<ObjId>,
    lock_tries: BTreeMap<ObjId, u32>,
    /// How many doors were unlocked plus how many stood fully open when last looked at. A change means more of the
    /// module may be in reach.
    doors_free: usize,
    /// Which line is showing, and for how long.
    line_serial: Option<usize>,
    line_time: f32,
    /// How many more times a finished talk may send it back round the doors of this module.
    second_looks: u32,
    /// Seconds until the next look for foes on the way somewhere.
    look_around: f32,
    /// Seconds with nothing to do.
    idle: f32,
    replies: HashSet<String>,
    reply_turn: usize,
    module: String,
    module_time: f32,
    still: f32,
    last_pos: [f32; 3],
    talk_time: f32,
    next_reply: f32,
    errors_seen: usize,
    thinking: f32,
    kills: usize,
    doors_opened: usize,
    items_taken: usize,
    lines_at_start: usize,
}

impl Default for Autopilot {
    fn default() -> Self {
        Autopilot::new()
    }
}

impl Autopilot {
    pub fn new() -> Autopilot {
        Autopilot {
            stalls: Vec::new(),
            log: Vec::new(),
            modules: Vec::new(),
            immortal: true,
            finished: false,
            read_lines: false,
            time: 0.0,
            goal: None,
            goal_time: 0.0,
            tried: HashSet::new(),
            unreachable: HashSet::new(),
            lock_tries: BTreeMap::new(),
            doors_free: 0,
            line_serial: None,
            line_time: 0.0,
            second_looks: SECOND_LOOKS,
            look_around: 0.0,
            idle: 0.0,
            replies: HashSet::new(),
            reply_turn: 0,
            module: String::new(),
            module_time: 0.0,
            still: 0.0,
            last_pos: [0.0; 3],
            talk_time: 0.0,
            next_reply: 0.0,
            errors_seen: 0,
            thinking: 0.0,
            kills: 0,
            doors_opened: 0,
            items_taken: 0,
            lines_at_start: 0,
        }
    }

    fn stall(&mut self, w: &World, kind: &'static str, objective: &str, detail: String) {
        let position = w.object(w.pc).map_or([0.0; 3], |o| o.position);
        let s = Stall {
            kind,
            module: w.module.clone(),
            position,
            objective: objective.to_string(),
            detail,
            time: self.time,
        };
        self.log.push(format!("STALL {s:?}"));
        self.stalls.push(s);
    }

    /// Decide and act for `dt` seconds. The caller advances the game.
    pub fn step(&mut self, g: &mut Game, dt: f32) {
        self.time += dt;
        self.module_time += dt;
        self.goal_time += dt;
        self.notice_changes(g);
        if self.finished {
            return;
        }
        let w = &mut g.world;
        if self.immortal {
            let pc = w.pc;
            if let Some(o) = w.object_mut(pc) {
                if o.hp < o.max_hp / 2 || o.down || o.dead {
                    o.hp = o.max_hp.max(1);
                    o.down = false;
                    o.dead = false;
                }
            }
        }
        if w.in_dialogue() {
            self.converse(w, dt);
            return;
        }
        if self.talk_time > 0.0 && self.second_looks > 0 {
            // A talk can change what a door does: this is how Trask comes to open the first one.
            self.second_looks -= 1;
            self.forget_doors(w);
        }
        self.talk_time = 0.0;
        if w.opened.is_some() {
            self.loot(w);
            return;
        }
        self.watch_goal(g, dt);
        if self.goal.is_none() {
            self.thinking -= dt;
            if self.thinking <= 0.0 {
                self.thinking = 0.25;
                self.choose_goal(g);
            }
        }
    }

    fn notice_changes(&mut self, g: &mut Game) {
        let w = &mut g.world;
        if w.module != self.module {
            if !self.module.is_empty() {
                self.log.push(format!(
                    "left {} after {:.0} s: {} kills so far, {} doors, {} items, {} lines",
                    self.module,
                    self.module_time,
                    self.kills,
                    self.doors_opened,
                    self.items_taken,
                    w.transcript.len() - self.lines_at_start
                ));
            }
            self.module = w.module.clone();
            self.modules.push(self.module.clone());
            self.log
                .push(format!("entered {} at {:.0} s", self.module, self.time));
            self.goal = None;
            self.tried.clear();
            self.unreachable.clear();
            self.lock_tries.clear();
            self.second_looks = SECOND_LOOKS;
            self.module_time = 0.0;
            self.still = 0.0;
        }
        let free = w
            .objects
            .values()
            .filter(|o| o.kind == Kind::Door)
            .map(|o| usize::from(!o.locked) + usize::from(o.door == DoorState::Open))
            .sum();
        if free != self.doors_free {
            self.doors_free = free;
            self.forget_doors(w);
            // Someone who has spoken may have more to say now: Carth does once `end_door04` opens.
            let pc = w.pc;
            let friends: Vec<ObjId> = self
                .tried
                .iter()
                .copied()
                .filter(|id| {
                    w.object(*id)
                        .is_some_and(|o| o.kind == Kind::Creature && !o.conversation.is_empty())
                        && w.reputation(pc, *id) > 10
                })
                .collect();
            for f in friends {
                self.tried.remove(&f);
            }
        }
        while self.errors_seen < w.errors.len() {
            let e = w.errors[self.errors_seen].clone();
            self.errors_seen += 1;
            let w2: &World = w;
            let position = w2.object(w2.pc).map_or([0.0; 3], |o| o.position);
            self.log.push(format!("STALL script error: {e}"));
            self.stalls.push(Stall {
                kind: "script",
                module: w2.module.clone(),
                position,
                objective: "-".to_string(),
                detail: e,
                time: self.time,
            });
        }
    }

    /// Try every door again, and whatever seemed out of reach.
    fn forget_doors(&mut self, w: &World) {
        self.unreachable.clear();
        self.lock_tries.clear();
        // A plain locked door stays as it is; one with a script or a conversation may give way to someone later.
        self.tried.retain(|id| {
            w.object(*id)
                .is_none_or(|o| o.kind != Kind::Door || (o.locked && !door_has_a_say(o)))
        });
    }

    /// Take a reply: one not said before if there is one, or else any.
    fn converse(&mut self, w: &mut World, dt: f32) {
        self.talk_time += dt;
        self.next_reply -= dt;
        if self.talk_time > CONVERSATION_SECONDS {
            let view = w.dialogue_view().map(|v| v.1).unwrap_or_default();
            let detail = format!(
                "{} says {view:?}{}",
                w.dialogue_conversation().map_or(String::new(), |c| c.name),
                if w.conversation_paused() {
                    ", paused"
                } else {
                    ""
                }
            );
            self.stall(w, "conversation", "end the conversation", detail);
            self.talk_time = 0.0;
        }
        if self.next_reply > 0.0 {
            return;
        }
        self.next_reply = 0.2;
        let Some((_, _, _, replies)) = w.dialogue_view() else {
            return;
        };
        if w.conversation_paused() {
            return;
        }
        // In a window a spoken line ends by itself. A silent one waits for a key, so the pilot presses it after a while.
        let serial = w.dialogue_serial();
        if serial != self.line_serial {
            self.line_serial = serial;
            self.line_time = 0.0;
        }
        self.line_time += 0.2;
        if self.read_lines && replies.is_empty() && self.line_time < READ_SECONDS {
            return;
        }
        // Once everything has been said, take them in a scattered order: one leads out of the loop, and plain turns
        // can keep missing it when menus of different sizes take turns.
        self.reply_turn = self.reply_turn.wrapping_mul(2_654_435_761).wrapping_add(11);
        let pick = replies
            .iter()
            .position(|r| !self.replies.contains(r))
            .unwrap_or((self.reply_turn >> 8) % replies.len().max(1));
        if let Some(r) = replies.get(pick) {
            self.replies.insert(r.clone());
        }
        w.dialogue_choose(pick);
    }

    fn loot(&mut self, w: &mut World) {
        for (item, _) in w.container_items() {
            w.take_item(item);
            self.items_taken += 1;
        }
        w.opened = None;
        self.wear_what_fits(w);
    }

    /// Put carried items on the first free slot they fit, using the slots in `baseitems.2da`.
    fn wear_what_fits(&mut self, w: &mut World) {
        let pc = w.pc;
        let Some(table) = w.table("baseitems") else {
            return;
        };
        let carried: Vec<ObjId> = w
            .object(pc)
            .map(|o| o.inventory.clone())
            .unwrap_or_default();
        for item in carried {
            let Some(base) = w.object(item).map(|i| i.base_item) else {
                continue;
            };
            let mask = table
                .get(base.max(0) as usize, "equipableslots")
                .and_then(|m| i64::from_str_radix(m.trim_start_matches("0x"), 16).ok())
                .unwrap_or(0);
            // Weapons, armour and the like: the first set bit that is free.
            let slot = (0..12).find(|b| {
                mask & (1 << b) != 0 && w.object(pc).is_some_and(|o| !o.equipped.contains_key(b))
            });
            if let Some(slot) = slot {
                if let Some(o) = w.object_mut(pc) {
                    o.equipped.insert(slot, item);
                    o.inventory.retain(|i| *i != item);
                }
                self.log.push(format!(
                    "wearing {} in slot {slot}",
                    w.object(item).map_or("?", |i| i.template.as_str())
                ));
            }
        }
    }

    /// Hold a carried or left-hand melee weapon in the right hand, and put a ranged one away.
    fn take_up_melee(&mut self, w: &mut World) {
        const RIGHT: i32 = 4;
        const LEFT: i32 = 5;
        let pc = w.pc;
        let ranged = |w: &mut World, item: ObjId| {
            let base = w.object(item).map_or(0, |i| i.base_item);
            w.table("baseitems")
                .and_then(|t| t.int(base.max(0) as usize, "rangedweapon"))
                .is_some_and(|r| r != 0)
        };
        let Some(right) = w.object(pc).and_then(|o| o.equipped.get(&RIGHT).copied()) else {
            return;
        };
        if !ranged(w, right) {
            return;
        }
        let left = w.object(pc).and_then(|o| o.equipped.get(&LEFT).copied());
        let carried = w
            .object(pc)
            .map(|o| o.inventory.clone())
            .unwrap_or_default();
        let mask = |w: &mut World, item: ObjId| {
            let base = w.object(item).map_or(0, |i| i.base_item);
            w.table("baseitems")
                .and_then(|t| {
                    t.get(base.max(0) as usize, "equipableslots")
                        .map(str::to_string)
                })
                .and_then(|m| i64::from_str_radix(m.trim_start_matches("0x"), 16).ok())
                .unwrap_or(0)
        };
        let melee = left
            .into_iter()
            .chain(carried)
            .find(|i| mask(w, *i) & (1 << RIGHT) != 0 && !ranged(w, *i));
        let Some(melee) = melee else {
            return;
        };
        if let Some(o) = w.object_mut(pc) {
            o.equipped.insert(RIGHT, melee);
            if left == Some(melee) {
                o.equipped.remove(&LEFT);
            } else {
                o.inventory.retain(|i| *i != melee);
            }
            o.inventory.push(right);
        }
        let name = w
            .object(melee)
            .map_or(String::new(), |i| i.template.clone());
        self.log
            .push(format!("took up {name} instead of a ranged weapon"));
    }

    /// Is the current goal done, or stuck?
    fn watch_goal(&mut self, g: &mut Game, dt: f32) {
        let w = &mut g.world;
        let pos = w.object(w.pc).map_or([0.0; 3], |o| o.position);
        let Some(goal) = self.goal.clone() else {
            return;
        };
        match goal {
            Goal::Fight(t) => {
                let over = w.object(t).is_none_or(|o| o.dead || o.down);
                if over {
                    self.kills += 1;
                    self.goal = None;
                } else if self.goal_time > FIGHT_SECONDS {
                    let name = w.object(t).map_or(String::new(), |o| o.tag.clone());
                    self.stall(
                        w,
                        "fight",
                        &format!("kill {name}"),
                        format!("still alive after {FIGHT_SECONDS} s"),
                    );
                    let pc = w.pc;
                    w.stop_attacking(pc);
                    self.tried.insert(t);
                    self.goal = None;
                }
            }
            Goal::Use(_, label) | Goal::Walk(_, label) => {
                if !w.pc_walking() {
                    self.goal = None;
                    return;
                }
                // A foe comes first: it is looked for again as soon as this goal is gone. Sight is dear, so not every tick.
                self.look_around -= dt;
                if self.look_around <= 0.0 {
                    self.look_around = 0.5;
                    if self.nearest_foe(w).is_some() {
                        w.pc_mover.stop();
                        w.pending = None;
                        self.goal = None;
                        return;
                    }
                }
                // Sliding along someone in the way looks like moving, so progress is measured from where it last was.
                if dist2d(pos, self.last_pos) > 1.0 {
                    self.last_pos = pos;
                    self.still = 0.0;
                } else {
                    self.still += dt;
                }
                if self.still > STUCK_SECONDS {
                    self.stall(
                        w,
                        "walk",
                        &label,
                        format!("no movement for {STUCK_SECONDS} s"),
                    );
                    w.pc_mover.stop();
                    w.pending = None;
                    self.still = 0.0;
                    self.goal = None;
                }
            }
        }
    }

    fn set_goal(&mut self, goal: Goal) {
        self.idle = 0.0;
        self.last_pos = [f32::MAX; 3];
        self.goal = Some(goal);
        self.goal_time = 0.0;
        self.still = 0.0;
    }

    /// Whoever hostile is closest, in sight and not given up on.
    fn nearest_foe(&self, w: &World) -> Option<ObjId> {
        let pc = w.pc;
        let here = w.object(pc).map_or([0.0; 3], |o| o.position);
        w.objects
            .values()
            .filter(|o| {
                o.kind == Kind::Creature
                    && o.id != pc
                    && !o.dead
                    && !o.party_member
                    && !self.tried.contains(&o.id)
                    && dist2d(o.position, here) <= FIGHT_RANGE
                    && w.reputation(pc, o.id) <= 10
                    && w.clear_line(here, o.position)
            })
            .min_by(|a, b| dist2d(a.position, here).total_cmp(&dist2d(b.position, here)))
            .map(|o| o.id)
    }

    fn choose_goal(&mut self, g: &mut Game) {
        let w = &mut g.world;
        let pc = w.pc;
        let here = w.object(pc).map_or([0.0; 3], |o| o.position);
        // 1. Whoever is close and hostile.
        if let Some(id) = self.nearest_foe(w) {
            w.attack(pc, id);
            self.set_goal(Goal::Fight(id));
            return;
        }
        let exploring = self.module_time < EXPLORE_SECONDS;
        // 2. Everyone to talk to, then everything to use, then every door.
        if exploring {
            let target = self.nearest_new(w, here);
            if let Some((id, label)) = target {
                if w.click(id) {
                    self.note_use(w, id);
                    self.set_goal(Goal::Use(id, label));
                } else {
                    self.unreachable.insert(id);
                }
                return;
            }
        }
        // 3. A way out: a door or a trigger that names another module.
        let way_out = w
            .objects
            .values()
            .filter(|o| {
                !o.link_module.is_empty()
                    && matches!(o.kind, Kind::Door | Kind::Trigger)
                    && !self.tried.contains(&o.id)
            })
            .map(|o| {
                let at = if o.polygon.len() >= 3 {
                    let n = o.polygon.len() as f32;
                    [
                        o.polygon.iter().map(|p| p[0]).sum::<f32>() / n,
                        o.polygon.iter().map(|p| p[1]).sum::<f32>() / n,
                        o.position[2],
                    ]
                } else {
                    o.position
                };
                (o.id, o.kind, at, o.link_module.clone())
            })
            .min_by(|a, b| dist2d(a.2, here).total_cmp(&dist2d(b.2, here)));
        let mut no_way_out = None;
        if let Some((id, kind, at, module)) = way_out {
            let label = format!("leave for {module}");
            let moved = if kind == Kind::Door {
                w.click(id)
            } else {
                w.walk_to(at)
            };
            if moved {
                self.tried.insert(id);
                self.set_goal(Goal::Walk(at, label));
                return;
            }
            // Doors may yet open: this only counts once nothing else is left.
            no_way_out = Some(label);
        }
        // 4. Nothing new: what is left unreachable or locked may have opened up, once.
        if !self.unreachable.is_empty() && self.doors_opened > 0 {
            let left: Vec<ObjId> = self.unreachable.drain().collect();
            let mut any = false;
            for id in left {
                if w.click(id) {
                    self.note_use(w, id);
                    let label = w.describe(id);
                    self.set_goal(Goal::Use(id, label));
                    any = true;
                    break;
                }
                self.tried.insert(id);
            }
            if any {
                return;
            }
        }
        // A door may be swinging open and a script may be about to act: nothing to do has to last a while.
        self.idle += 0.25;
        if self.idle < GIVE_UP_SECONDS {
            return;
        }
        if let Some(label) = no_way_out {
            self.stall(
                w,
                "transition",
                &label,
                "no route to the way out".to_string(),
            );
        }
        self.give_up(w);
    }

    fn give_up(&mut self, w: &mut World) {
        let locked: Vec<String> = w
            .objects
            .values()
            .filter(|o| o.kind == Kind::Door && !o.open && o.locked)
            .map(|o| o.tag.clone())
            .collect();
        let ways = w
            .objects
            .values()
            .filter(|o| !o.link_module.is_empty())
            .count();
        self.stall(
            w,
            "softlock",
            "find the way out",
            format!("nothing left to do; {ways} ways out tried; locked doors: {locked:?}"),
        );
        self.finished = true;
    }

    fn note_use(&mut self, w: &World, id: ObjId) {
        if let Some(o) = w.object(id) {
            if o.kind == Kind::Door {
                self.doors_opened += 1;
            }
        }
    }

    /// The nearest thing not yet tried: a friendly creature with something to say, a useable placeable, a door.
    fn nearest_new(&mut self, w: &mut World, here: [f32; 3]) -> Option<(ObjId, String)> {
        let pc = w.pc;
        let candidates: Vec<(ObjId, f32)> = w
            .objects
            .values()
            .filter(|o| {
                o.id != pc
                    && !o.dead
                    && !o.party_member
                    && !self.tried.contains(&o.id)
                    && !self.unreachable.contains(&o.id)
                    && match o.kind {
                        Kind::Creature => !o.conversation.is_empty(),
                        Kind::Placeable => true,
                        Kind::Door => !o.open && o.link_module.is_empty(),
                        _ => false,
                    }
            })
            .map(|o| (o.id, dist2d(o.position, here)))
            .collect();
        let mut best: Option<(f32, ObjId)> = None;
        for (id, d) in candidates {
            let usable = match w.object(id).map(|o| (o.kind, o.template.clone())) {
                Some((Kind::Placeable, template)) => w
                    .inst
                    .read(&template, ResType(2044))
                    .ok()
                    .and_then(|b| kotor_gff::Gff::parse(&b).ok())
                    .and_then(|g| g.root.int("Useable"))
                    .is_some_and(|u| u != 0),
                Some((Kind::Creature, _)) => w.reputation(pc, id) > 10,
                _ => true,
            };
            if !usable {
                self.tried.insert(id);
                continue;
            }
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, id));
            }
        }
        let (_, id) = best?;
        let (door, locked, label) = w
            .object(id)
            .map(|o| (o.kind == Kind::Door, o.locked, o.tag.clone()))?;
        if door && !locked {
            // Open or not, the next look at the doors decides: `tried` would keep one that a script closes again.
            let n = self.lock_tries.entry(id).or_insert(0);
            *n += 1;
            if *n >= LOCK_TRIES {
                self.tried.insert(id);
            }
        } else if locked {
            let n = self.lock_tries.entry(id).or_insert(0);
            *n += 1;
            // Someone nearby may have more to say now: this is how the first door gets opened.
            let round = *n;
            let at = w.object(id).map_or(here, |o| o.position);
            let friends: Vec<ObjId> = w
                .objects
                .values()
                .filter(|o| {
                    o.kind == Kind::Creature
                        && o.id != pc
                        && !o.dead
                        && !o.conversation.is_empty()
                        && dist2d(o.position, at) < 25.0
                })
                .map(|o| o.id)
                .collect();
            for f in friends {
                if self.tried.remove(&f) {
                    self.log
                        .push(format!("asking {f} again about {label}, try {round}"));
                }
            }
            let n = self.lock_tries.entry(id).or_insert(0);
            let swap = *n > 1;
            // Trask keeps the bridge door shut until the player holds a melee weapon (`k_pend_door20`).
            if swap {
                self.take_up_melee(w);
            }
            let n = self.lock_tries.entry(id).or_insert(0);
            // A door may only give way to someone else in the party, as the first one does.
            if *n > 2 && w.next_leader() {
                self.log.push(format!(
                    "tried {label} with {}",
                    w.object(w.pc).map_or("?", |o| o.tag.as_str())
                ));
            }
            let n = *n;
            let limit = if w.object(id).is_some_and(door_has_a_say) {
                LOCK_TRIES
            } else {
                PLAIN_LOCK_TRIES
            };
            if n >= limit {
                self.tried.insert(id);
                self.stall(
                    w,
                    "locked",
                    &format!("open {label}"),
                    format!("still locked after {limit} tries"),
                );
            }
        } else {
            self.tried.insert(id);
        }
        Some((id, label))
    }

    /// A short summary of how far it got.
    pub fn summary(&self, w: &World) -> String {
        let mut by_kind: BTreeMap<&str, usize> = BTreeMap::new();
        for s in &self.stalls {
            *by_kind.entry(s.kind).or_default() += 1;
        }
        format!(
            "{:.0} s played; modules {:?}; {} kills, {} doors, {} items, {} lines; stalls {:?}",
            self.time,
            self.modules,
            self.kills,
            self.doors_opened,
            self.items_taken,
            w.transcript.len(),
            by_kind
        )
    }
}
