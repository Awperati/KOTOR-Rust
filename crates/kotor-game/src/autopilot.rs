//! A scripted player: it walks a module the way a person would, using only what a player can do (click to walk, click
//! on things, pick replies, take items, attack), and writes down every place it gets stuck.
//!
//! It knows nothing about the story. In each module it fights what it sees, talks to everyone once, opens every chest
//! and door it can reach, and then leaves by whatever door or trigger leads to another module.

use crate::follow::dist2d;
use crate::game::Game;
use crate::world::{DoorState, Kind, ObjId, World};
use std::collections::{BTreeMap, BTreeSet, HashSet};

/// Fixed steps between two entries of [`Autopilot::digests`].
const DIGEST_STEPS: u64 = 600;

/// A fingerprint of what the game remembers: the clock, the module, every object's place and condition, everything said,
/// the fight log and the global numbers. Two runs with the same digests have played the same game.
pub fn state_digest(w: &World) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8423_2325;
    let mut feed = |bytes: &[u8]| {
        for b in bytes {
            h = (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3);
        }
    };
    feed(&w.time.to_le_bytes());
    feed(w.module.as_bytes());
    feed(&w.pc.to_le_bytes());
    for o in w.objects.values() {
        feed(&o.id.to_le_bytes());
        for v in o.position {
            feed(&v.to_le_bytes());
        }
        feed(&o.hp.to_le_bytes());
        feed(&[
            u8::from(o.dead),
            u8::from(o.down),
            u8::from(o.open),
            u8::from(o.locked),
        ]);
        for i in &o.inventory {
            feed(&i.to_le_bytes());
        }
        let mut worn: Vec<_> = o.equipped.iter().map(|(s, i)| (*s, *i)).collect();
        worn.sort_unstable();
        for (slot, item) in worn {
            feed(&slot.to_le_bytes());
            feed(&item.to_le_bytes());
        }
    }
    for l in &w.transcript {
        feed(l.dialogue.as_bytes());
        feed(&l.node.to_le_bytes());
        feed(l.text.as_bytes());
    }
    for a in &w.combat.log {
        feed(&a.attacker.to_le_bytes());
        feed(&a.target.to_le_bytes());
        feed(&a.roll.to_le_bytes());
        feed(&a.damage.to_le_bytes());
        feed(&a.target_hp.to_le_bytes());
    }
    let mut globals: Vec<_> = w.global_numbers.iter().collect();
    globals.sort_unstable();
    for (k, v) in globals {
        feed(k.as_bytes());
        feed(&v.to_le_bytes());
    }
    h
}

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
/// How long it goes on with no module it has not seen. Its longest wait so far is 4,100 s, in the Sith base and back.
const NEW_MODULE_SECONDS: f32 = 8000.0;
/// A mortal pilot takes a medpack below half its hit points, at most one a round.
const MEDPACK_SECONDS: f32 = 3.0;
const GEAR_SECONDS: f32 = 1.0;
/// Taken off how far a placeable is when a mortal pilot picks what to do next: more than across any module.
const LOOK_FIRST: f32 = 10_000.0;
const GRENADE_SECONDS: f32 = 6.0;
/// A foe with more hit points left than this is worth a grenade.
const GRENADE_HP: i32 = 20;
/// How many medpacks it buys up to.
const MEDPACKS_WANTED: i32 = 10;
const SKILL_COMPUTER: usize = 0;
const SKILL_SECURITY: usize = 6;
const SLOT_ARMOUR: i32 = 1;
const SLOT_RIGHT_HAND: i32 = 4;
const SLOT_LEFT_HAND: i32 = 5;

/// The party member best at a skill leads, if better than the leader.
pub fn lead_with_the_best_at(w: &mut World, skill: usize) {
    let rank = |w: &World, id: ObjId| {
        w.object(id)
            .filter(|o| !o.dead && !o.down)
            .and_then(|o| o.skills.get(skill).copied())
            .unwrap_or(-1)
    };
    let best = w.party.iter().copied().max_by_key(|id| rank(w, *id));
    if let Some(id) = best.filter(|id| rank(w, *id) > rank(w, w.pc)) {
        w.switch_leader(id);
    }
}

/// How one fight of the pilot's went.
#[derive(Clone, Debug, PartialEq)]
pub struct FightResult {
    pub module: String,
    pub foe: String,
    pub won: bool,
    pub hp_before: i32,
    pub hp_after: i32,
    pub max_hp: i32,
    pub seconds: f32,
    pub medpacks: u32,
    pub time: f32,
}

pub struct Autopilot {
    pub stalls: Vec<Stall>,
    pub log: Vec<String>,
    /// Modules reached, in order.
    pub modules: Vec<String>,
    /// Keep the player alive, so that one lost fight does not hide everything behind it. When off it plays as a person
    /// would: it spends what levels give, wears the best it has found, takes medpacks, and a party that is down ends it.
    pub immortal: bool,
    /// Every fight the pilot chose, in order.
    pub fights: Vec<FightResult>,
    fight: Option<FightResult>,
    medpack_wait: f32,
    gear_wait: f32,
    ways_retried: bool,
    new_module_at: f32,
    party_filled: bool,
    grenade_wait: f32,
    /// Where each module's ways out go, as far as it has seen.
    links: BTreeMap<String, BTreeSet<String>>,
    /// The things that took it to another module, by module and tag, and where to.
    exits: BTreeMap<(String, String), String>,
    /// Nothing more to try.
    pub finished: bool,
    /// Hold lines that want no answer for as long as a person would read them, in game time.
    pub read_lines: bool,
    /// The state of the world every ten game seconds: (fixed step, [`state_digest`]).
    pub digests: Vec<(u64, u64)>,
    pub time: f32,
    goal: Option<Goal>,
    goal_time: f32,
    tried: BTreeSet<ObjId>,
    unreachable: BTreeSet<ObjId>,
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
            fights: Vec::new(),
            fight: None,
            medpack_wait: 0.0,
            gear_wait: 0.0,
            ways_retried: false,
            new_module_at: 0.0,
            party_filled: false,
            grenade_wait: 0.0,
            links: BTreeMap::new(),
            exits: BTreeMap::new(),
            finished: false,
            read_lines: false,
            digests: Vec::new(),
            time: 0.0,
            goal: None,
            goal_time: 0.0,
            tried: BTreeSet::new(),
            unreachable: BTreeSet::new(),
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

    /// Play `n` fixed steps. The pilot decides once per step, so the run depends only on how many steps have passed, not
    /// on how they are grouped into frames or how fast they go.
    pub fn run_steps(&mut self, g: &mut Game, n: usize) {
        let dt = g.dt;
        for _ in 0..n {
            self.step(g, dt);
            g.step_once();
            if g.steps.is_multiple_of(DIGEST_STEPS) {
                self.digests.push((g.steps, state_digest(&g.world)));
            }
        }
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
        self.spend_what_levels_give(w);
        // Two modules with only each other to go to would hold it for good: `tar_m04aa` and `tar_m05aa` did.
        if self.time - self.new_module_at > NEW_MODULE_SECONDS {
            self.stall(
                w,
                "softlock",
                "find a new module",
                format!("none in {NEW_MODULE_SECONDS} s"),
            );
            self.finished = true;
            return;
        }
        if self.immortal {
            let pc = w.pc;
            if let Some(o) = w.object_mut(pc) {
                if o.hp < o.max_hp / 2 || o.down || o.dead {
                    o.hp = o.max_hp.max(1);
                    o.down = false;
                    o.dead = false;
                }
            }
        } else {
            if w.game_over {
                self.die(w);
                return;
            }
            self.lead_with_someone_standing(w);
        }
        if w.store.is_some() {
            self.shop(w);
        }
        if w.in_dialogue() {
            self.converse(w, dt);
            return;
        }
        self.fill_the_party(w);
        if !self.immortal {
            self.look_after_itself(w, dt);
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

    /// Sell what it does not wear, and stock up on medpacks: a mortal pilot ran out with nothing else to heal it.
    fn shop(&mut self, w: &mut World) {
        let spare: Vec<ObjId> = w
            .sell_lines()
            .into_iter()
            .filter(|l| {
                w.slots_of(l.item) != 0
                    && w.object(l.item)
                        .is_some_and(|o| !o.template.starts_with("g_i_medeqpmnt"))
            })
            .map(|l| l.item)
            .collect();
        for item in spare {
            let _ = w.store_sell(item);
        }
        let packs = |w: &World| -> i32 {
            w.carried(w.pc)
                .iter()
                .filter_map(|i| w.object(*i))
                .filter(|o| o.template.starts_with("g_i_medeqpmnt"))
                .map(|o| o.stack)
                .sum()
        };
        let line = w
            .store_lines()
            .into_iter()
            .filter(|l| {
                w.object(l.item)
                    .is_some_and(|o| o.template.starts_with("g_i_medeqpmnt"))
            })
            .min_by_key(|l| l.price);
        if let Some(line) = line {
            while packs(w) < MEDPACKS_WANTED && w.store_buy(line.item).is_ok() {
                self.log
                    .push(format!("bought a medpack for {}", line.price));
                if !line.infinite {
                    break;
                }
            }
        }
        w.close_store();
    }

    /// Take along whoever is on offer, as a person would at the party screen. Lowest slot first: the first to join.
    /// Once a visit: `k_ptar_duelring` takes people out of the party for a while and puts them back itself.
    fn fill_the_party(&mut self, w: &mut World) {
        if self.party_filled || w.party.len() >= crate::party::PARTY_SIZE {
            return;
        }
        self.party_filled = true;
        // Whoever leads has a slot too, and is not in `party`.
        let mut slots = w.party_slots();
        let leading: Vec<i32> = w
            .npc_slots
            .iter()
            .filter(|(_, id)| **id == w.pc)
            .map(|(s, _)| *s)
            .collect();
        let free: Vec<i32> = w
            .available
            .keys()
            .copied()
            .filter(|s| !slots.contains(s) && !leading.contains(s) && !w.set_aside.contains(s))
            .take(crate::party::PARTY_SIZE - w.party.len())
            .collect();
        if free.is_empty() {
            return;
        }
        slots.extend(leading);
        slots.extend(free);
        self.log.push(format!("party is now slots {slots:?}"));
        w.choose_party(&slots, "");
    }

    /// Take each level as the Auto Level Up button does, and spend what is unspent.
    fn spend_what_levels_give(&mut self, w: &mut World) {
        w.hold_levels = true;
        let pc = w.pc;
        let everyone: Vec<ObjId> = std::iter::once(pc).chain(w.party.iter().copied()).collect();
        for id in everyone {
            while w.level_ready(id) {
                let Some(up) = w.level_up(id) else {
                    break;
                };
                let class = w.object(id).map_or(0, |o| o.class);
                let n = w.primary_ability(class);
                let points = w.ability_points_at(up.level);
                if let Some(before) = w.object(id).map(|o| o.abilities) {
                    if let Some(o) = w.object_mut(id) {
                        o.abilities[n] += points;
                    }
                    w.settle_abilities(id, before);
                }
                if id == pc {
                    self.log.push(format!("took level {}", up.level));
                }
                if w.is_early(id) {
                    for _ in 0..w.power_slots_for(id, up.level) {
                        if let Some(power) = w.recommended_power(id) {
                            if let Some(o) = w.object_mut(id) {
                                o.spells.insert(power);
                            }
                        }
                    }
                }
            }
            let Some(class) = w.object(id).map(|o| o.class) else {
                continue;
            };
            if w.object(id).is_some_and(|o| o.skill_points > 0) {
                for skill in w.recommended_skills(class) {
                    while w.buy_skill_rank(id, skill).is_ok() {}
                }
            }
            if w.object(id).is_some_and(|o| o.feat_slots != (0, 0)) {
                while let Some(feat) = w.recommended_feat(id) {
                    if w.choose_feat(id, feat).is_err() {
                        break;
                    }
                }
            }
        }
    }

    /// The whole party is down: this is where the game ends.
    fn die(&mut self, w: &mut World) {
        self.end_fight(w, false);
        self.stall(
            w,
            "death",
            "stay alive",
            "the whole party is down".to_string(),
        );
        self.finished = true;
    }

    /// Control goes to someone still standing when the leader is knocked out.
    fn lead_with_someone_standing(&mut self, w: &mut World) {
        let pc = w.pc;
        if w.object(pc).is_some_and(|o| o.down) {
            let next = w
                .party
                .iter()
                .copied()
                .find(|id| w.object(*id).is_some_and(|o| !o.dead && !o.down));
            self.end_fight(w, false);
            if let Some(id) = next {
                w.switch_leader(id);
                self.goal = None;
            }
        }
    }

    /// Medpacks, and the best of what has been found.
    fn look_after_itself(&mut self, w: &mut World, dt: f32) {
        self.medpack_wait -= dt;
        self.gear_wait -= dt;
        if self.gear_wait <= 0.0 {
            self.gear_wait = GEAR_SECONDS;
            self.gear_up(w);
            // A grenade at whatever has plenty left, every few seconds.
            self.grenade_wait -= GEAR_SECONDS;
            if let Some(Goal::Fight(foe)) = self.goal {
                let tough = w.object(foe).is_some_and(|o| o.hp > GRENADE_HP);
                if tough && self.grenade_wait <= 0.0 {
                    let pc = w.pc;
                    if let Some(g) = w.combat_throwables(pc).first().copied() {
                        if w.use_item_on(pc, g, foe).is_ok() {
                            self.grenade_wait = GRENADE_SECONDS;
                            self.log.push(format!("threw {g} at {foe}"));
                        }
                    }
                }
            }
            // The first feat on offer, with each attack: a person with a good one would.
            if matches!(self.goal, Some(Goal::Fight(_))) && w.combat_queue.is_empty() {
                let pc = w.pc;
                if let Some(c) = w.combat_feats(pc).first() {
                    w.combat_queue.push_back(c.action);
                }
            }
        }
        let pc = w.pc;
        // Whoever is worst off, party included. The party has one bag.
        let hurt = std::iter::once(pc)
            .chain(w.party.iter().copied())
            .filter(|id| {
                w.object(*id)
                    .is_some_and(|o| !o.dead && !o.down && o.hp * 2 < o.max_hp)
            })
            .min_by_key(|id| w.object(*id).map_or(0, |o| o.hp));
        let Some(who) = hurt.filter(|_| self.medpack_wait <= 0.0) else {
            return;
        };
        let pack = w.carried(pc).into_iter().find(|i| {
            w.object(*i)
                .is_some_and(|o| o.template.starts_with("g_i_medeqpmnt"))
        });
        if let Some(pack) = pack {
            self.medpack_wait = MEDPACK_SECONDS;
            if w.use_item(who, pack).is_ok() {
                if let Some(f) = self.fight.as_mut() {
                    f.medpacks += 1;
                }
            }
        }
    }

    /// Wear the armour with the most `baseac` and hold the melee weapon with the most damage, as this pilot only fights
    /// up close. Anything else goes on a free slot. The left hand stays empty: two weapons are not built.
    fn gear_up(&mut self, w: &mut World) {
        let pc = w.pc;
        let Some(t) = w.table("baseitems") else {
            return;
        };
        let cell = |w: &World, item: Option<ObjId>, c: &str| {
            item.and_then(|i| w.object(i))
                .and_then(|o| t.int(o.base_item.max(0) as usize, c))
                .unwrap_or(0)
        };
        let melee = |w: &World, item: Option<ObjId>| {
            if cell(w, item, "rangedweapon") != 0 {
                return -1;
            }
            cell(w, item, "numdice") * (cell(w, item, "dietoroll") + 1)
        };
        for item in w.carried(pc) {
            let mask = w.slots_of(item);
            if mask == 0 || w.can_wear(pc, item).is_err() {
                continue;
            }
            let worn =
                |w: &World, slot: i32| w.object(pc).and_then(|o| o.equipped.get(&slot).copied());
            let fits = |slot: i32| mask & (1 << slot) != 0;
            let slot = if fits(SLOT_ARMOUR) {
                (worn(w, SLOT_ARMOUR).is_none()
                    || cell(w, Some(item), "baseac") > cell(w, worn(w, SLOT_ARMOUR), "baseac"))
                .then_some(SLOT_ARMOUR)
            } else if fits(SLOT_RIGHT_HAND) {
                let held = worn(w, SLOT_RIGHT_HAND);
                let score = melee(w, Some(item));
                (score > 0 && (held.is_none() || score > melee(w, held))).then_some(SLOT_RIGHT_HAND)
            } else {
                (0..12).find(|s| *s != SLOT_LEFT_HAND && fits(*s) && worn(w, *s).is_none())
            };
            if let Some(slot) = slot {
                if w.equip(pc, item, Some(slot)).is_ok() {
                    let name = w.object(pc).and_then(|o| o.equipped.get(&slot).copied());
                    let name = name
                        .and_then(|i| w.object(i))
                        .map_or("?", |o| o.template.as_str());
                    self.log.push(format!("wearing {name} in slot {slot}"));
                }
            }
        }
    }

    fn begin_fight(&mut self, w: &World, foe: ObjId) {
        let me = w.object(w.pc);
        self.fight = Some(FightResult {
            module: w.module.clone(),
            foe: w.object(foe).map_or(String::new(), |o| o.tag.clone()),
            won: false,
            hp_before: me.map_or(0, |o| o.hp),
            hp_after: 0,
            max_hp: me.map_or(0, |o| o.max_hp),
            seconds: self.time,
            medpacks: 0,
            time: self.time,
        });
    }

    fn end_fight(&mut self, w: &World, won: bool) {
        let Some(mut f) = self.fight.take() else {
            return;
        };
        let me = w.object(w.pc);
        f.won = won;
        f.hp_after = me.map_or(0, |o| o.hp);
        f.max_hp = me.map_or(f.max_hp, |o| o.max_hp);
        f.seconds = self.time - f.time;
        if !self.immortal {
            self.log.push(format!(
                "FIGHT {} {} {} at {:.0} s: {} to {} of {} hit points in {:.0} s, {} medpacks",
                if won { "won" } else { "lost" },
                f.foe,
                f.module,
                f.time,
                f.hp_before,
                f.hp_after,
                f.max_hp,
                f.seconds,
                f.medpacks
            ));
        }
        self.fights.push(f);
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
            // What it was using took it away: a ladder is a way out, for when the module is done.
            if let Some(Goal::Use(_, tag)) = &self.goal {
                self.exits
                    .insert((self.module.clone(), tag.clone()), w.module.clone());
            }
            self.module = w.module.clone();
            if !self.modules.contains(&self.module) {
                self.new_module_at = self.time;
            }
            self.modules.push(self.module.clone());
            self.log
                .push(format!("entered {} at {:.0} s", self.module, self.time));
            self.goal = None;
            self.tried.clear();
            self.unreachable.clear();
            self.lock_tries.clear();
            self.second_looks = SECOND_LOOKS;
            self.ways_retried = false;
            self.party_filled = false;
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
            // Conversations name scripts the game's files do not have, such as `k_pend_carth11`. Nothing waits on them.
            if e.starts_with("script ") && e.ends_with(" not found") {
                self.log.push(format!("note: {e}"));
                continue;
            }
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
        // The pilot alone moves a conversation on, so that a window and a headless run go through the same lines.
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
        // What its skills and tools offer comes first: `end_securitycomp`'s overload clears a room. Each owner's replies
        // count apart, since the Sith base's terminals say the same things and open different doors.
        let owner = w.dialogue_owner().unwrap_or(0);
        let seen = |p: &Self, r: &String| p.replies.contains(&format!("{owner} {r}"));
        // Nobody who has read it says what gets them shot by a guard who cannot be hurt, unless nothing else is on offer.
        let fatal: Vec<bool> = (0..replies.len()).map(|k| w.reply_is_fatal(k)).collect();
        let safe: Vec<usize> = (0..replies.len()).filter(|k| !fatal[*k]).collect();
        let choices = if safe.is_empty() {
            (0..replies.len()).collect()
        } else {
            safe
        };
        let skilled = choices
            .iter()
            .copied()
            .find(|k| replies[*k].starts_with('[') && !seen(self, &replies[*k]));
        let pick = skilled
            .or_else(|| choices.iter().copied().find(|k| !seen(self, &replies[*k])))
            .or_else(|| {
                (!choices.is_empty()).then(|| choices[(self.reply_turn >> 8) % choices.len()])
            })
            .unwrap_or(0);
        if let Some(r) = replies.get(pick) {
            self.replies.insert(format!("{owner} {r}"));
        }
        w.dialogue_choose(pick);
    }

    fn loot(&mut self, w: &mut World) {
        for (item, _) in w.container_items() {
            w.take_item(item);
            self.items_taken += 1;
        }
        w.opened = None;
        if self.immortal {
            self.wear_what_fits(w);
        } else {
            self.gear_up(w);
        }
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
                // A foe can stop being one: the duel ring's fighters surrender.
                let over =
                    w.object(t).is_none_or(|o| o.dead || o.down) || w.reputation(w.pc, t) > 10;
                if over {
                    self.end_fight(w, true);
                    self.kills += 1;
                    self.goal = None;
                } else if self.goal_time <= FIGHT_SECONDS {
                    // A script can end the attack with `CancelCombat`, and a person would click on the foe again.
                    let pc = w.pc;
                    if w.combat.target.get(&pc) != Some(&t) && !w.in_dialogue() {
                        w.attack(pc, t);
                    }
                } else {
                    self.end_fight(w, false);
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
                        // It never got there: the escape pod in `end_m01ab` stays to be used after the fight.
                        if let Some(Goal::Use(id, _)) = self.goal {
                            self.tried.remove(&id);
                        }
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
                    && !o.plot
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
        // Someone else had a try at a door. The story is the main character's: `k_ptar_duelring` empties the party by
        // counting those beside the leader, and never ends if the main character is one of them.
        let hero = w.hero;
        if w.pc != hero && w.object(hero).is_some_and(|o| !o.dead && !o.down) {
            w.switch_leader(hero);
        }
        let pc = w.pc;
        let here = w.object(pc).map_or([0.0; 3], |o| o.position);
        // 1. Whoever is close and hostile.
        if let Some(id) = self.nearest_foe(w) {
            w.attack(pc, id);
            self.begin_fight(w, id);
            self.set_goal(Goal::Fight(id));
            return;
        }
        // Going anywhere else ends any fight, or the chase would hold the player where it can still reach its target.
        w.stop_attacking(pc);
        let exploring = self.module_time < EXPLORE_SECONDS;
        // 2. Everyone to talk to, then everything to use, then every door.
        if exploring {
            let target = self.nearest_new(w, here);
            if let Some((id, label)) = target {
                // Whoever is best at it goes up to it. Scripts read the leader's skill: a person changes character for this.
                let skill = w.object(id).and_then(|o| match o.kind {
                    // A door with a say of its own answers to who it answers to, as the Endar Spire's first does.
                    Kind::Door if o.locked && !door_has_a_say(o) => Some(SKILL_SECURITY),
                    Kind::Placeable if !o.conversation.is_empty() => Some(SKILL_COMPUTER),
                    _ => None,
                });
                if let Some(skill) = skill {
                    lead_with_the_best_at(w, skill);
                }
                if w.click(id) {
                    self.note_use(w, id);
                    self.set_goal(Goal::Use(id, label));
                } else {
                    // Not `tried`: it has to be there when a door opens. The footlocker in `end_m01ab` lies behind one.
                    self.tried.remove(&id);
                    self.unreachable.insert(id);
                }
                return;
            }
        }
        // 3. A way out: a door or a trigger that names another module.
        let mut ways: Vec<(ObjId, Kind, [f32; 3], String)> = w
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
            .collect();
        let module = &self.module;
        ways.extend(
            w.objects
                .values()
                .filter(|o| o.kind == Kind::Placeable && !self.tried.contains(&o.id))
                .filter_map(|o| {
                    let to = self.exits.get(&(module.clone(), o.tag.clone()))?;
                    Some((o.id, o.kind, o.position, to.clone()))
                }),
        );
        self.links
            .entry(self.module.clone())
            .or_default()
            .extend(ways.iter().map(|w| w.3.clone()));
        let hops = self.hops_to_somewhere_new();
        // Nearest to somewhere it has not been, then where it has been least: the Sith commander's building is a way out it
        // would otherwise never take, and the lower city has two doors it would go back and forth through for good.
        ways.sort_by(|a, b| {
            let visits = |m: &String| self.modules.iter().filter(|v| *v == m).count();
            let hop = |m: &String| hops.get(m).copied().unwrap_or(u32::MAX);
            hop(&a.3)
                .cmp(&hop(&b.3))
                .then(visits(&a.3).cmp(&visits(&b.3)))
                .then(dist2d(a.2, here).total_cmp(&dist2d(b.2, here)))
        });
        let mut no_way_out = None;
        for (id, kind, at, module) in ways {
            let label = format!("leave for {module}");
            let moved = if kind != Kind::Trigger {
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
            no_way_out.get_or_insert(label);
        }
        // 4. Nothing new: what is left unreachable or locked may have opened up, once.
        if !self.unreachable.is_empty() && self.doors_opened > 0 {
            let left: Vec<ObjId> = std::mem::take(&mut self.unreachable).into_iter().collect();
            let mut any = false;
            for id in left {
                if w.click(id) {
                    self.note_use(w, id);
                    let label = w.describe(id);
                    self.set_goal(Goal::Use(id, label));
                    any = true;
                    break;
                }
                // Still out of reach. A door opening later brings it back: the footlocker in `end_m01ab` lies behind one.
                self.unreachable.insert(id);
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
        // A way out it set off for and never reached, because a fight came first, is still a way out. Once a visit.
        if !self.ways_retried {
            self.ways_retried = true;
            self.idle = 0.0;
            self.tried
                .retain(|id| w.object(*id).is_none_or(|o| o.link_module.is_empty()));
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

    /// How many ways out each module it knows of is from one it has not been to.
    fn hops_to_somewhere_new(&self) -> BTreeMap<String, u32> {
        let mut hops: BTreeMap<String, u32> = self
            .links
            .values()
            .flatten()
            .filter(|m| !self.modules.contains(m))
            .map(|m| (m.clone(), 0))
            .collect();
        for n in 1..=self.links.len() as u32 {
            for (from, to) in &self.links {
                if !hops.contains_key(from) && to.iter().any(|m| hops.get(m) == Some(&(n - 1))) {
                    hops.insert(from.clone(), n);
                }
            }
        }
        hops
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
                        Kind::Placeable => !self
                            .exits
                            .contains_key(&(self.module.clone(), o.tag.clone())),
                        Kind::Door => !o.open && o.link_module.is_empty(),
                        _ => false,
                    }
            })
            .map(|o| (o.id, dist2d(o.position, here)))
            .collect();
        let mut best: Option<(f32, ObjId)> = None;
        for (id, d) in candidates {
            let usable = match w.object(id).map(|o| (o.kind, o.useable)) {
                Some((Kind::Placeable, useable)) => useable,
                Some((Kind::Creature, _)) => w.reputation(pc, id) > 10,
                _ => true,
            };
            if !usable {
                // A foe with something to say stays a foe, and it can turn friendly: only a thing is done with for good.
                if w.object(id).is_none_or(|o| o.kind != Kind::Creature) {
                    self.tried.insert(id);
                }
                continue;
            }
            // A mortal pilot looks round first: a talk can open a door on what it is not ready for, as the damaged droid
            // in `end_m01ab` does, and the footlocker and panel near it are the way to be ready.
            let look_first =
                !self.immortal && w.object(id).is_some_and(|o| o.kind == Kind::Placeable);
            let d = if look_first { d - LOOK_FIRST } else { d };
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
