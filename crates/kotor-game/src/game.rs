//! The real-time loop: a fixed-timestep driver that joins world updates, the player's movement and animation
//! clips. A renderer asks it for [`Pose`] requests each frame; nothing here draws.

use crate::world::{Kind, ObjId, World};
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
    /// Facing in radians (0 = +x); only the player's is tracked.
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
    /// Steps run so far.
    pub steps: u64,
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
            // The player walks while it has a route; everything else idles. A walk cycle lasts one second,
            // which is also what `walkdist` (metres per cycle) at 1.6 m/s assumes.
            let want = if id == pc && (walking || self.world.pc_walking()) {
                "walk"
            } else {
                "pause1"
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
                    0.0
                },
                clip: self.clips.get(&o.id).cloned(),
            })
            .collect()
    }
}
