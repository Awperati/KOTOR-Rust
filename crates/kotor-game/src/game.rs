//! The real-time loop: a fixed-timestep driver that joins world updates, the player's movement and animation
//! clips. A renderer asks it for [`Pose`] requests each frame; nothing here draws.

use crate::world::{DoorState, Kind, ObjId, World};
use std::collections::HashMap;

/// Metres a second the walk clip shows, `walkrate` of `creaturespeed.2da`.
const WALK_CLIP_SPEED: f32 = 1.7;

/// The animation an object is playing: a clip name from its supermodel and seconds into it.
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub name: &'static str,
    pub time: f32,
    /// The clip this one took over from, while it still shows through.
    pub fade: Option<Fade>,
}

/// The clip a new one is blending in from: it goes on playing, and its weight drops from 1 to 0 over [`BLEND_SECONDS`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fade {
    pub name: &'static str,
    pub time: f32,
    /// 1 as the new clip starts, 0 when the old one is gone.
    pub weight: f32,
}

/// The clip an `ANIMATION_` constant of `nwscript.nss` asks for, and whether it loops. Those without a clip here are left out.
///
/// **Assumed:** the clip each constant stands for, from the constants' names and the clips the supermodels carry
/// (`kotor-tool anims S_Male02`). `ANIMATION_LOOPING_WELD` (25) is a droid's own clip.
fn script_clip(animation: i32) -> Option<(&'static str, bool)> {
    Some(match animation {
        0 => ("pause1", true),
        1 => ("pause2", true),
        2 => ("listen", true),
        3 => ("meditate", true),
        4 => ("kneel", true),
        5 => ("tlknorm", true),
        6 => ("tlkplead", true),
        7 => ("tlkforce", true),
        8 => ("tlklaugh", true),
        9 => ("tlksad", true),
        10 => ("getfromgnd", true),
        11 => ("getfromtbl", true),
        12 => ("pausetrd", true),
        13 => ("pausepsn", true),
        14 => ("flirt", true),
        15 => ("usecomplp", true),
        16 => ("dance", true),
        17 => ("dance1", true),
        18 => ("horror", true),
        21 => ("spasm", true),
        22 => ("sleep", true),
        23 => ("prone", true),
        24 => ("pause3", true),
        25 => ("weld", true),
        26 => ("dead", true),
        27 => ("talkinj", true),
        28 => ("listeninj", true),
        29 => ("treatinjlp", true),
        30 => ("dead1", true),
        100 => ("hturnl", false),
        101 => ("hturnr", false),
        102 => ("pause2", false),
        103 => ("pausetrd", false),
        104 => ("salute", false),
        105 => ("bow", false),
        106 => ("greeting", false),
        107 => ("taunt", false),
        108..=110 => ("victory", false),
        112 => ("inject", false),
        113 => ("usecomp", false),
        114 => ("persuade", false),
        115 => ("activate", false),
        116 => ("choke", true),
        117 | 118 => ("throwsab", false),
        120 => ("treatinj", false),
        _ => return None,
    })
}

/// How long one clip takes to give way to the next, for walking, running and standing.
///
/// **Assumed:** the game's value. The owner saw no smoothing at all; a fifth of a second is a short, common blend.
pub const BLEND_SECONDS: f32 = 0.2;

/// What a renderer needs to draw one object this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Pose {
    pub id: ObjId,
    pub kind: Kind,
    pub position: [f32; 3],
    /// Facing in radians (0 = +x): the player's walk heading, the direction anyone else last moved, else its facing.
    pub heading: f32,
    pub clip: Option<Clip>,
}

/// Fixed-step game loop around a [`World`].
pub struct Game {
    pub world: World,
    /// Length of one simulation step in seconds (1/60).
    pub dt: f32,
    accumulator: f32,
    clips: HashMap<ObjId, Clip>,
    last_pos: HashMap<ObjId, [f32; 3]>,
    /// Where each creature was before the last step, to draw it between steps.
    before_step: HashMap<ObjId, [f32; 3]>,
    /// Clips playing once over everything else but death, the one playing first: name and seconds left.
    once: HashMap<ObjId, Vec<(&'static str, f32)>>,
    /// Who lay knocked down on the last step: when it ends, they get up.
    knocked: std::collections::HashSet<ObjId>,
    /// The looping clip a script put a creature in, until it moves.
    scripted: HashMap<ObjId, &'static str>,
    /// Until what step a creature that fought keeps its combat stance.
    ready_until: HashMap<ObjId, u64>,
    log_seen: usize,
    names: HashMap<String, &'static str>,
    headings: HashMap<ObjId, f32>,
    /// The line of a conversation last turned to.
    line_seen: Option<usize>,
    /// Steps run so far.
    pub steps: u64,
}

/// Attack and flinch clips last 1.47 to 1.5 s and dodges 0.3 to 0.43 s in the humanoid supermodels (measured).
const COMBAT_CLIP_SECONDS: f32 = 1.5;
/// How long the `getupdead` clip plays when a push or knockdown ends. **Assumed**, not read from the model.
const GET_UP_SECONDS: f32 = 3.0;
const DODGE_CLIP_SECONDS: f32 = 0.43;
/// How long a creature that took part in a fight keeps its combat stance after its last blow. A round is 3 s.
///
/// **Assumed:** the game keeps the stance for as long as the fight lasts. This covers the gap between two rounds.
/// `throwgren` and the first stretch of a cast clip (`castout1` to `3`), measured in the humanoid supermodel.
const THROW_SECONDS: f32 = 1.5;
/// `inject` and `activate` of the humanoid supermodel (`kotor-tool anims S_Male02`).
const INJECT_SECONDS: f32 = 1.5;
const ACTIVATE_SECONDS: f32 = 1.0;

/// The clip of using an item whose ability (`spells.2da` label, lower case) is `ITEM_ABILITY_*` or `PLOT_*`: medicine and
/// stimulants are injected (`inject`, which `animations.2da` marks to hide what is held), shields, kits, devices and the plot
/// items are `activate`. **Assumed:** which of the two goes with which item; the game's files give the clips and no table of
/// them. `None` for a Force power, or an item whose throw is handled elsewhere.
pub fn item_use_clip(label: &str) -> Option<(&'static str, f32)> {
    if !(label.starts_with("item_") || label.starts_with("plot_")) {
        return None;
    }
    let injected = [
        "med_pack",
        "life_support",
        "remove_poison",
        "stim",
        "adrenal",
        "combat_shot",
        "serum",
    ];
    if injected.iter().any(|w| label.contains(w)) {
        Some(("inject", INJECT_SECONDS))
    } else {
        Some(("activate", ACTIVATE_SECONDS))
    }
}
const CAST_START_SECONDS: f32 = 0.333;
const STANCE_HOLD_SECONDS: f32 = 3.5;

#[derive(Clone, Copy)]
enum Move {
    Attack,
    Hit,
    Dodge,
    Ready,
}

/// Longest stretch of real time one `advance` call will simulate; the rest is dropped, so a slow frame does not
/// make the next ones run ever more steps.
const MAX_CATCH_UP: f32 = 0.25;
/// Most fixed steps one call may run: about a fifth of a second of simulation at its slowest.
pub const MAX_STEPS_PER_CALL: usize = 4096;

impl Game {
    pub fn new(world: World) -> Game {
        Game {
            world,
            dt: 1.0 / 60.0,
            accumulator: 0.0,
            clips: HashMap::new(),
            last_pos: HashMap::new(),
            before_step: HashMap::new(),
            once: HashMap::new(),
            knocked: Default::default(),
            ready_until: HashMap::new(),
            scripted: HashMap::new(),
            log_seen: 0,
            names: HashMap::new(),
            headings: HashMap::new(),
            line_seen: None,
            steps: 0,
        }
    }

    /// Feed `real_dt` seconds of wall-clock time; runs as many whole fixed steps as fit. Returns how many ran.
    pub fn advance(&mut self, real_dt: f32) -> usize {
        self.advance_at(real_dt, 1.0)
    }

    /// Like [`Game::advance`], with `speed` game seconds to each second of wall-clock time.
    pub fn advance_at(&mut self, real_dt: f32, speed: f32) -> usize {
        let n = self.steps_due(real_dt, speed);
        for _ in 0..n {
            self.step_once();
        }
        n
    }

    /// How many fixed steps `real_dt` seconds at `speed` come to, at most [`MAX_STEPS_PER_CALL`]. Time beyond what a call
    /// can catch up on is dropped, so a slow frame does not make the next ones run ever more steps.
    pub fn steps_due(&mut self, real_dt: f32, speed: f32) -> usize {
        let limit = MAX_CATCH_UP * speed.max(1.0);
        self.accumulator = (self.accumulator + real_dt.max(0.0) * speed).min(limit);
        let n = ((self.accumulator / self.dt) as usize).min(MAX_STEPS_PER_CALL);
        self.accumulator -= n as f32 * self.dt;
        n
    }

    /// One fixed step. All game time passes here, however fast the caller goes.
    pub fn step_once(&mut self) {
        self.before_step.clear();
        self.before_step.extend(
            self.world
                .objects
                .values()
                .filter(|o| o.kind == Kind::Creature)
                .map(|o| (o.id, o.position)),
        );
        self.face_speaker();
        let walking = self.world.pc_walking();
        self.world.step(self.dt);
        self.steps += 1;
        self.start_combat_clips();
        // A power has its own gesture. A body without the clip keeps its idle.
        for (id, spell) in std::mem::take(&mut self.world.casts) {
            self.play_cast(id, spell);
        }
        for (id, animation, seconds) in std::mem::take(&mut self.world.animations) {
            match script_clip(animation) {
                Some((name, true)) => {
                    self.scripted.insert(id, name);
                }
                Some((name, false)) => {
                    self.scripted.remove(&id);
                    self.play_once(id, name, if seconds > 0.0 { seconds } else { 1.5 });
                }
                None => {}
            }
        }
        let pc = self.world.pc;
        let ids: Vec<ObjId> = self
            .world
            .objects
            .values()
            .filter(|o| matches!(o.kind, Kind::Creature))
            .map(|o| o.id)
            .collect();
        self.clips.retain(|id, _| ids.contains(id));
        for id in ids {
            // Whoever moved this step walks; a walk cycle lasts one second, which is also what `walkdist` (metres per
            // cycle) at 1.6 m/s assumes. A jump of more than a metre is a teleport, not a step.
            // A creature a push or knockdown has thrown lies as a body does, and gets up when it ends (bug 36).
            let knocked = self.world.is_knocked_down(id);
            let (pos, down) = {
                let o = &self.world.objects[&id];
                (o.position, o.dead || o.down || knocked)
            };
            if knocked {
                self.knocked.insert(id);
            } else if self.knocked.remove(&id) && !down {
                self.play_once(id, "getupdead", GET_UP_SECONDS);
            }
            let mut moved = false;
            let mut ran = false;
            if let Some(l) = self.last_pos.insert(id, pos) {
                let (dx, dy) = (pos[0] - l[0], pos[1] - l[1]);
                let d = (dx * dx + dy * dy).sqrt();
                if d > 0.002 && d < 1.0 {
                    moved = true;
                    // A walk covers about 0.03 m in a step of a sixtieth of a second, a run 0.09 m.
                    ran = d / self.dt > 2.5;
                    self.headings.insert(id, dy.atan2(dx));
                }
            }
            if moved {
                self.scripted.remove(&id);
            }
            let dt = self.dt;
            let once = self.once.get_mut(&id).and_then(|q| {
                while let Some(first) = q.first_mut() {
                    first.1 -= dt;
                    if first.1 > 0.0 {
                        return Some(first.0);
                    }
                    // What the clip overran by is lost: a step is short.
                    q.remove(0);
                }
                None
            });
            if once.is_none() {
                self.once.remove(&id);
            }
            if self.world.combat.target.contains_key(&id) {
                self.ready_until
                    .insert(id, self.steps + (STANCE_HOLD_SECONDS / self.dt) as u64);
            }
            let in_stance = self.ready_until.get(&id).is_some_and(|u| *u > self.steps);
            let ready = in_stance.then(|| self.style_clip(id, Move::Ready));
            let want = if down {
                "die"
            } else if let Some(o) = once {
                o
            } else if ran {
                "run"
            } else if moved || (id == pc && (walking || self.world.pc_walking())) {
                "walk"
            } else {
                self.scripted
                    .get(&id)
                    .copied()
                    .or(ready)
                    .unwrap_or("pause1")
            };
            let dt = self.dt;
            let c = self.clips.entry(id).or_insert(Clip {
                name: want,
                time: (id % 7) as f32 * 0.37,
                fade: None,
            });
            c.fade = c.fade.and_then(|f| {
                let weight = f.weight - dt / BLEND_SECONDS;
                (weight > 0.0).then_some(Fade {
                    time: f.time + dt,
                    weight,
                    ..f
                })
            });
            if c.name == want {
                // A sprint plays the run faster, so the feet keep to the floor: `rundist` is 3.96 m a cycle.
                let modern = self.world.movement == crate::movement::MovementMode::Modern;
                let rate = if modern && id == pc && want == "run" {
                    (self.world.pc_body.speed() / crate::RUN_SPEED).clamp(0.5, 1.3)
                } else if modern && id == pc && want == "walk" {
                    // And a walk, going from a stand to a run through it. The clip covers 1.7 m a second.
                    (self.world.pc_body.speed() / WALK_CLIP_SPEED).clamp(0.6, 1.5)
                } else {
                    1.0
                };
                c.time += dt * rate;
            } else {
                // A dead body does not blend: it falls as its own clip says.
                let fade = (want != "die").then_some(Fade {
                    name: c.name,
                    time: c.time + dt,
                    weight: 1.0,
                });
                *c = Clip {
                    name: want,
                    time: 0.0,
                    fade,
                };
            }
        }
    }

    /// Every attack resolved since the last step starts its clips: the attacker swings or fires, the target flinches
    /// or dodges.
    fn start_combat_clips(&mut self) {
        let n = self.world.combat.log.len();
        let new: Vec<_> = self.world.combat.log[self.log_seen.min(n)..].to_vec();
        self.log_seen = n;
        // Two who fight each other strike in the same step: a swing is not to be replaced by the flinch at the other's.
        let hold = self.steps + (STANCE_HOLD_SECONDS / self.dt) as u64;
        for l in &new {
            self.ready_until.insert(l.attacker, hold);
            self.ready_until.insert(l.target, hold);
            if l.outcome == crate::combat::Outcome::Miss {
                let dodge = self.style_clip(l.target, Move::Dodge);
                self.play_once(l.target, dodge, DODGE_CLIP_SECONDS);
            } else {
                let hit = self.style_clip(l.target, Move::Hit);
                self.play_once(l.target, hit, COMBAT_CLIP_SECONDS);
            }
        }
        // A swing waits for a flinch or dodge in progress, so that being hit is always seen to the end.
        for l in &new {
            let swing = self.style_clip(l.attacker, Move::Attack);
            self.queue_once(l.attacker, swing, COMBAT_CLIP_SECONDS);
        }
    }

    /// The attacker of a ground finisher plays the owner's `finisher` clip for `seconds`, over the swing it began.
    pub fn play_finisher(&mut self, id: ObjId, seconds: f32) {
        self.play_once(id, "finisher", seconds);
    }

    /// Keep a creature in its fighting stance a little longer, as a fight does. Modern combat's aim calls it.
    pub fn hold_stance(&mut self, id: ObjId) {
        self.ready_until
            .insert(id, self.steps + (STANCE_HOLD_SECONDS / self.dt) as u64);
    }

    /// Play a clip at once, over what is playing.
    fn play_once(&mut self, id: ObjId, name: &'static str, seconds: f32) {
        // The same clip again starts over.
        if let Some(c) = self.clips.get_mut(&id).filter(|c| c.name == name) {
            c.time = 0.0;
        }
        self.once.insert(id, vec![(name, seconds)]);
    }

    /// Play a clip after the one playing, which may be a flinch. One waits at most: a newer one replaces it.
    fn queue_once(&mut self, id: ObjId, name: &'static str, seconds: f32) {
        match self.once.get_mut(&id) {
            Some(q) if !q.is_empty() => {
                q.truncate(1);
                q.push((name, seconds));
            }
            _ => self.play_once(id, name, seconds),
        }
    }

    /// The gesture of a power or a thrown grenade, from the spell's `conjanim` and `castanim` in `spells.2da`: a throw is
    /// `throwgren`, and a power is the first stretch of its cast clip and then its loop, each once.
    ///
    /// **Assumed:** which of the three cast clips (`castout1` to `3`) goes with which `conjanim` (`hand` 1, `head` 2),
    /// and that the loop clip plays once.
    fn play_cast(&mut self, id: ObjId, spell: i32) {
        let (conj, cast, label) = {
            let t = self.world.table("spells");
            let get = |c: &str| {
                t.as_ref()
                    .and_then(|t| t.get(spell.max(0) as usize, c))
                    .unwrap_or("")
                    .to_ascii_lowercase()
            };
            (get("conjanim"), get("castanim"), get("label"))
        };
        if conj == "throw" || cast == "throw" {
            self.play_once(id, "throwgren", THROW_SECONDS);
            return;
        }
        // An item's ability, not a Force power: it has its own gestures, never the cast of a power (a medpac once played
        // Force Push's).
        if let Some((clip, seconds)) = item_use_clip(&label) {
            self.play_once(id, clip, seconds);
            return;
        }
        let (start, hold, a, b) = match conj.as_str() {
            "head" => ("castout2", "castoutlp2", CAST_START_SECONDS, 1.167),
            _ => ("castout1", "castoutlp1", CAST_START_SECONDS, 0.933),
        };
        self.play_once(id, start, a);
        self.queue_once(id, hold, b);
    }

    /// The clip for a move with the weapon a creature holds.
    fn style_clip(&mut self, id: ObjId, how: Move) -> &'static str {
        let (stance, ranged) = self.weapon_stance(id);
        let name = match how {
            Move::Attack if ranged => format!("b{stance}a1"),
            Move::Attack if (2..=4).contains(&stance) => format!("c{stance}a1"),
            Move::Attack => format!("g{stance}a1"),
            Move::Hit => format!("g{stance}g1"),
            Move::Dodge => format!("g{stance}d1"),
            Move::Ready => format!("g{stance}r1"),
        };
        self.names
            .entry(name)
            .or_insert_with_key(|n| Box::leak(n.clone().into_boxed_str()))
    }

    /// The digit in a combat clip's name for the weapon in hand, and whether it fires. `weaponwield` comes from
    /// `baseitems.2da`.
    ///
    /// **Assumption, read from the clip names and not checked against the game:** wield 1 baton, 2 one-handed and
    /// 3 double-bladed use the same digit; 4 pistol uses 5, 5 rifle uses 7, 6 heavy uses 9; no weapon uses 8.
    fn weapon_stance(&mut self, id: ObjId) -> (i32, bool) {
        let w = &self.world;
        let base = w
            .object(id)
            .and_then(|o| o.equipped.get(&4))
            .and_then(|i| w.object(*i))
            .map(|i| i.base_item);
        let Some(base) = base else {
            return (8, false);
        };
        let row = base.max(0) as usize;
        let Some(t) = self.world.table("baseitems") else {
            return (8, false);
        };
        let wield = t.int(row, "weaponwield").unwrap_or(0);
        let ranged = t.int(row, "rangedweapon").unwrap_or(0) > 0;
        // A weapon in the left hand too: two blades are the fourth set (`g4`, `c4`), two pistols the sixth (`g6`, `b6`).
        let off_wield = self
            .world
            .object(id)
            .and_then(|o| o.equipped.get(&5))
            .and_then(|i| self.world.object(*i))
            .map(|i| i.base_item.max(0) as usize)
            .and_then(|b| self.world.table("baseitems")?.int(b, "weaponwield"))
            .unwrap_or(0);
        let stance = match (wield, off_wield) {
            (1..=2, 1..=2) => 4,
            (4, 4) => 6,
            (1..=3, _) => wield,
            (4, _) => 5,
            (5, _) => 7,
            (6, _) => 9,
            _ => 8,
        };
        (stance as i32, ranged)
    }

    /// Whoever says a line and whoever it is said to turn to each other, as the line's Speaker and Listener have it, and not
    /// always to the player. A script can hold someone still (`SetLockOrientationInDialog`), and what is not a creature does not
    /// turn. This is here and not in the window's loop because the player's heading steers its next walk: it has to fall on the
    /// same step at any speed.
    fn face_speaker(&mut self) {
        let serial = self.world.dialogue_serial();
        if serial == self.line_seen {
            return;
        }
        self.line_seen = serial;
        let Some((speaker, listener)) = self
            .world
            .dialogue_pair()
            .filter(|(speaker, listener)| speaker != listener)
        else {
            return;
        };
        let (Some(from), Some(to)) = (self.world.object(speaker), self.world.object(listener))
        else {
            return;
        };
        let turns = |o: &crate::world::Object| {
            o.kind == Kind::Creature && !self.world.orientation_locked.contains(&o.id)
        };
        let (speaker_turns, listener_turns) = (turns(from), turns(to));
        let to_listener =
            (to.position[1] - from.position[1]).atan2(to.position[0] - from.position[0]);
        if speaker_turns {
            self.face(speaker, to_listener);
        }
        if listener_turns {
            self.face(listener, to_listener + std::f32::consts::PI);
        }
    }

    /// Which way a creature faces now, as last turned: in radians.
    pub fn facing_of(&self, id: ObjId) -> Option<f32> {
        self.headings.get(&id).copied()
    }

    /// Turn a creature to face an angle in radians; it keeps it until it moves.
    pub fn face(&mut self, id: ObjId, angle: f32) {
        self.headings.insert(id, angle);
        if id == self.world.pc {
            self.world.pc_mover.heading = angle;
        }
    }

    /// The clip an object is playing right now.
    pub fn clip(&self, id: ObjId) -> Option<&Clip> {
        self.clips.get(&id)
    }

    /// Draw requests for every placed object that has a body: position, facing and clip.
    /// Where to draw a creature: between where the last step found it and where it left it, by how much of the next step
    /// has passed. The game moves 60 times a second and a screen does not, and with nothing between them everything that
    /// moves shakes. A jump of over a metre is a teleport, and is not drawn on its way.
    pub fn drawn_position(&self, id: ObjId) -> Option<[f32; 3]> {
        let now = self.world.objects.get(&id)?.position;
        let Some(was) = self.before_step.get(&id) else {
            return Some(now);
        };
        if (now[0] - was[0]).hypot(now[1] - was[1]) > 1.0 {
            return Some(now);
        }
        let share = (self.accumulator / self.dt).clamp(0.0, 1.0);
        Some([0, 1, 2].map(|i| was[i] + (now[i] - was[i]) * share))
    }

    pub fn poses(&self) -> Vec<Pose> {
        self.world
            .objects
            .values()
            .filter(|o| matches!(o.kind, Kind::Creature | Kind::Door | Kind::Placeable))
            .map(|o| Pose {
                id: o.id,
                kind: o.kind,
                position: self.drawn_position(o.id).unwrap_or(o.position),
                heading: if o.id == self.world.pc {
                    self.world.pc_mover.heading
                } else {
                    self.headings
                        .get(&o.id)
                        .copied()
                        .unwrap_or(o.facing.to_radians())
                },
                clip: match o.kind {
                    Kind::Door => Some(match o.door {
                        DoorState::Closed => Clip {
                            name: "closed",
                            time: 0.0,
                            fade: None,
                        },
                        DoorState::Opening(t) => Clip {
                            name: "opening1",
                            time: t,
                            fade: None,
                        },
                        DoorState::Open => Clip {
                            name: "opened1",
                            time: 0.0,
                            fade: None,
                        },
                        DoorState::Closing(t) => Clip {
                            name: "closing1",
                            time: t,
                            fade: None,
                        },
                    }),
                    _ => self.clips.get(&o.id).cloned().map(|c| Clip {
                        time: c.time + self.accumulator.max(0.0),
                        ..c
                    }),
                },
            })
            .collect()
    }
}
