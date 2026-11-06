//! The real-time loop: a fixed-timestep driver that joins world updates, the player's movement and animation
//! clips. A renderer asks it for [`Pose`] requests each frame; nothing here draws.

use crate::world::{DoorState, Kind, ObjId, World};
use std::collections::HashMap;

/// The animation an object is playing: a clip name from its supermodel and seconds into it.
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub name: &'static str,
    pub time: f32,
}

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
    /// A clip playing once over everything else but death: name and seconds left.
    once: HashMap<ObjId, (&'static str, f32)>,
    log_seen: usize,
    names: HashMap<String, &'static str>,
    headings: HashMap<ObjId, f32>,
    /// Steps run so far.
    pub steps: u64,
}

/// Attack and flinch clips last 1.5 s and dodges about 0.3 s in the humanoid supermodels (measured).
const COMBAT_CLIP_SECONDS: f32 = 1.5;
const DODGE_CLIP_SECONDS: f32 = 0.4;

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

impl Game {
    pub fn new(world: World) -> Game {
        Game {
            world,
            dt: 1.0 / 60.0,
            accumulator: 0.0,
            clips: HashMap::new(),
            last_pos: HashMap::new(),
            once: HashMap::new(),
            log_seen: 0,
            names: HashMap::new(),
            headings: HashMap::new(),
            steps: 0,
        }
    }

    /// Feed `real_dt` seconds of wall-clock time; runs as many whole fixed steps as fit. Returns how many ran.
    pub fn advance(&mut self, real_dt: f32) -> usize {
        self.accumulator = (self.accumulator + real_dt.max(0.0)).min(MAX_CATCH_UP);
        let mut n = 0;
        while self.accumulator >= self.dt {
            self.accumulator -= self.dt;
            self.fixed_step();
            n += 1;
        }
        n
    }

    fn fixed_step(&mut self) {
        let walking = self.world.pc_walking();
        self.world.step(self.dt);
        self.steps += 1;
        self.start_combat_clips();
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
            let (pos, down) = {
                let o = &self.world.objects[&id];
                (o.position, o.dead || o.down)
            };
            let mut moved = false;
            let mut ran = false;
            if let Some(l) = self.last_pos.insert(id, pos) {
                let (dx, dy) = (pos[0] - l[0], pos[1] - l[1]);
                let d = (dx * dx + dy * dy).sqrt();
                if d > 0.002 && d < 1.0 {
                    moved = true;
                    // A walk covers about 0.03 m in a step of a sixtieth of a second, a run about 0.07 m.
                    ran = d / self.dt > 2.5;
                    self.headings.insert(id, dy.atan2(dx));
                }
            }
            let once = self.once.get_mut(&id).and_then(|o| {
                o.1 -= self.dt;
                (o.1 > 0.0).then_some(o.0)
            });
            if once.is_none() {
                self.once.remove(&id);
            }
            let ready = self
                .world
                .combat
                .target
                .contains_key(&id)
                .then(|| self.style_clip(id, Move::Ready));
            let want = if down {
                "die"
            } else if let Some(o) = once {
                o
            } else if ran {
                "run"
            } else if moved || (id == pc && (walking || self.world.pc_walking())) {
                "walk"
            } else {
                ready.unwrap_or("pause1")
            };
            let c = self.clips.entry(id).or_insert(Clip {
                name: want,
                time: (id % 7) as f32 * 0.37,
            });
            if c.name == want {
                c.time += self.dt;
            } else {
                *c = Clip {
                    name: want,
                    time: 0.0,
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
        for l in &new {
            if l.outcome == crate::combat::Outcome::Miss {
                let dodge = self.style_clip(l.target, Move::Dodge);
                self.play_once(l.target, dodge, DODGE_CLIP_SECONDS);
            } else {
                let hit = self.style_clip(l.target, Move::Hit);
                self.play_once(l.target, hit, COMBAT_CLIP_SECONDS);
            }
        }
        for l in &new {
            let swing = self.style_clip(l.attacker, Move::Attack);
            self.play_once(l.attacker, swing, COMBAT_CLIP_SECONDS);
        }
    }

    fn play_once(&mut self, id: ObjId, name: &'static str, seconds: f32) {
        self.clips.remove(&id);
        self.once.insert(id, (name, seconds));
    }

    /// The clip for a move with the weapon a creature holds.
    fn style_clip(&mut self, id: ObjId, how: Move) -> &'static str {
        let (stance, ranged) = self.weapon_stance(id);
        let name = match how {
            Move::Attack if ranged => format!("b{stance}a1"),
            Move::Attack if stance == 2 || stance == 3 => format!("c{stance}a1"),
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
        let stance = match wield {
            1..=3 => wield,
            4 => 5,
            5 => 7,
            6 => 9,
            _ => 8,
        };
        (stance as i32, ranged)
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
    pub fn poses(&self) -> Vec<Pose> {
        self.world
            .objects
            .values()
            .filter(|o| matches!(o.kind, Kind::Creature | Kind::Door | Kind::Placeable))
            .map(|o| Pose {
                id: o.id,
                kind: o.kind,
                position: o.position,
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
                        },
                        DoorState::Opening(t) => Clip {
                            name: "opening1",
                            time: t,
                        },
                        DoorState::Open => Clip {
                            name: "opened1",
                            time: 0.0,
                        },
                        DoorState::Closing(t) => Clip {
                            name: "closing1",
                            time: t,
                        },
                    }),
                    _ => self.clips.get(&o.id).cloned(),
                },
            })
            .collect()
    }
}
