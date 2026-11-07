//! Modern movement: a body with a velocity, a facing that follows it, gravity, and a roll.
//!
//! The Classic walk puts the player at full speed in one frame, turns it at a fixed rate and sets its height to the
//! floor's every step. This body accelerates and slows down, carves its turns, snaps to slopes and stairs smoothly,
//! falls off ledges along an arc, jumps and rolls on a key. It is pure: the floor is asked through [`Ground`], so the tests
//! need no game files.
//!
//! **Numbers measured:** today's run speed is `RUN_SPEED` 5.4 m/s (`runrate` of `creaturespeed.2da`, which is also
//! `rundist` 3.96 m over the 0.733 s run cycle; the reference video gives 5.8 m/s, give or take 0.6). Sprint is 1.25
//! times that: 6.75 m/s. **Assumed (all marked "assumed"):** every other constant below is a feel choice, not data.
//! The supermodels have no roll, dive, jump or landing clip (checked in `S_Male02`, `S_Female03` and their chains), so
//! the roll and the landing are drawn as a procedural pose from [`Body::roll_progress`] and [`Body::land_dip`].

/// Today's run speed, metres per second.
pub const RUN: f32 = 5.4;
/// Sprint over today's run speed.
pub const SPRINT_RATIO: f32 = 1.25;
/// Sprint speed: 6.75 m/s.
pub const SPRINT: f32 = RUN * SPRINT_RATIO;

/// A gap in the floor is looked across in steps of this many metres, this many times. Assumed.
const GAP_PROBE: f32 = 0.3;
const GAP_PROBES: usize = 5;
/// Heights over the feet that a wall is looked for at in the walkmesh, in metres. Assumed: a wall reaches both, and a cliff
/// below neither.
const WALL_HEIGHTS: [f32; 2] = [0.6, 1.0];
/// Between these heights over the feet, anything a model has stops a fall: a low wall, a rail. Assumed: higher than the
/// ground's own bumps, and as high as a body's chest.
const BARRIER_LOW: f32 = 0.25;
const BARRIER_HIGH: f32 = 1.2;
/// A step of a frame is short: a barrier is looked for this far on from it. Assumed.
const BARRIER_LOOK: f32 = 0.6;
/// A jump that comes down on its own level is this many metres per second faster than it went up, at most, and lands light.
const LIGHT_LANDING: f32 = 0.5;
/// A body going along a wall tries these turns from the way it is pushed, in degrees: the nearest that gets through. Assumed.
const SLIDE_TURNS: [f32; 2] = [35.0, 70.0];
/// How much faster a roll goes round when something else is waiting for it: a click to go or attack. Assumed.
const ROLL_HURRY: f32 = 2.5;
/// The least speed going out over a gap, so that nothing hangs over it. Assumed.
const GAP_SPEED: f32 = 2.5;
/// The least a body moves in a step over a gap, on its way to the floor it set out for: that speed at 60 steps a second.
const GAP_STEP: f32 = GAP_SPEED / 60.0;

/// Which movement the player has.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MovementMode {
    #[default]
    Modern,
    Classic,
}

/// Feel constants. Assumed, except `sprint`, `run` and `walk`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tuning {
    /// `walkrate` of `creaturespeed.2da`.
    pub walk: f32,
    pub run: f32,
    pub sprint: f32,
    /// Metres per second squared while pushing. Assumed: full sprint in about a quarter of a second.
    pub accel: f32,
    /// Letting go. Assumed: from a sprint to a stop in about 0.18 s, over 0.6 m.
    pub decel: f32,
    /// Share of `accel` and `decel` in the air. Assumed.
    pub air_control: f32,
    /// How fast the facing closes on the way it moves, per second (exponential). Assumed.
    pub turn_smooth: f32,
    /// Never faster than this, radians per second. The reference video's hero turns 180 degrees in 5 frames.
    pub turn_max: f32,
    /// Metres per second squared. Assumed: 1.8 g, a heavy, quick fall rather than the moon's.
    pub gravity: f32,
    /// Highest step the body walks up without leaving the ground. Assumed.
    pub step_up: f32,
    /// Greatest drop the body walks down as a slope or stair. Past it, it falls. Assumed.
    pub step_down: f32,
    /// A deeper drop than this is a wall: nobody leaves the level for a floor far below. Assumed.
    pub max_drop: f32,
    /// How fast height closes on the floor, per second (exponential). Assumed.
    pub snap_rate: f32,
    /// The body may lag the floor by this much at most, so it never sinks into it. Assumed.
    pub snap_lag: f32,
    /// A fall of more than this many metres per second at the end is a hard landing with recovery. Assumed.
    pub land_threshold: f32,
    /// Seconds of slowed movement after a landing of 1 m/s past the threshold, and the most. Assumed.
    pub land_per_speed: f32,
    pub land_max: f32,
    /// Share of speed kept during a landing recovery. Assumed.
    pub land_slow: f32,
    /// A fall deeper than this below where it began is a void: the body goes back. Assumed.
    pub void_depth: f32,
    /// Roll: metres, seconds, and seconds before the next. Assumed.
    pub roll_distance: f32,
    pub roll_time: f32,
    pub roll_cooldown: f32,
    /// Speed handed on after a roll, as a share of sprint. Assumed.
    pub roll_exit: f32,
    /// How high a jump lifts the feet, metres. Assumed: over a crate, under most ceilings.
    pub jump_height: f32,
}

impl Default for Tuning {
    fn default() -> Tuning {
        Tuning {
            walk: 1.7,
            run: RUN,
            sprint: SPRINT,
            accel: 28.0,
            decel: 38.0,
            air_control: 0.3,
            turn_smooth: 14.0,
            turn_max: 18.8,
            gravity: 17.7,
            // The same reach as the Classic steering.
            step_up: 0.6,
            step_down: 0.35,
            max_drop: 6.0,
            snap_rate: 22.0,
            snap_lag: 0.15,
            land_threshold: 3.5,
            land_per_speed: 0.06,
            land_max: 0.45,
            land_slow: 0.5,
            void_depth: 40.0,
            roll_distance: 3.2,
            roll_time: 0.55,
            roll_cooldown: 0.3,
            roll_exit: 0.7,
            jump_height: 1.1,
        }
    }
}

/// The floor, as the body asks it.
pub trait Ground {
    /// Height of the highest walkable floor under `(x, y)` at or below `max_z`; `None` where there is none.
    fn floor(&self, x: f32, y: f32, max_z: f32) -> Option<f32>;
    /// Whether a straight move from `a` to `b` is stopped (a closed door's line).
    fn blocked(&self, a: [f32; 3], b: [f32; 3]) -> bool;
    /// Whether a wall of the walkmesh stands between two points.
    fn wall(&self, _a: [f32; 3], _b: [f32; 3]) -> bool {
        false
    }
    /// Whether anything that the models have and the walkmesh does not, such as a railing, stands on the line from `a` to
    /// `b` between two heights.
    fn barrier(&self, _a: [f32; 2], _b: [f32; 2], _low: f32, _high: f32) -> bool {
        false
    }
    /// Whether a walk joins the floors under two points. Nobody falls where they could never walk back from.
    fn way_back(&self, _a: [f32; 3], _b: [f32; 3]) -> bool {
        true
    }
}

/// What the player asks for this step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    /// A unit vector on the floor, or none.
    pub dir: Option<[f32; 2]>,
    /// Walk (slowly), as Shift does today. Without it, the body sprints.
    pub walk: bool,
    /// Roll now.
    pub roll: bool,
}

/// What happened during a step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Events {
    pub left_ground: bool,
    /// Fell onto the floor at this speed, metres per second.
    pub landed: Option<f32>,
    pub roll_started: bool,
    pub roll_ended: bool,
    pub jumped: bool,
    /// Fell out of the world and was put back where it last stood.
    pub reset: bool,
}

impl Events {
    /// Take in what a later step did. The last landing wins.
    pub fn merge(&mut self, o: Events) {
        self.left_ground |= o.left_ground;
        self.landed = o.landed.or(self.landed);
        self.roll_started |= o.roll_started;
        self.roll_ended |= o.roll_ended;
        self.jumped |= o.jumped;
        self.reset |= o.reset;
    }
}

/// Which pace the body is at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gait {
    Idle,
    Walk,
    Run,
    Sprint,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Roll {
    dir: [f32; 2],
    t: f32,
    hurry: bool,
}

/// Where the body is, how it moves, and what it is doing.
#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    pub pos: [f32; 3],
    /// Horizontal velocity, metres per second.
    pub vel: [f32; 2],
    /// Vertical velocity.
    pub vz: f32,
    /// Radians, 0 = +x, counter-clockwise.
    pub heading: f32,
    pub grounded: bool,
    roll: Option<Roll>,
    cooldown: f32,
    land_left: f32,
    land_total: f32,
    /// A jump's own landing: the crouch, with no loss of speed.
    land_light: bool,
    /// In the air by its own jump.
    jumping: bool,
    fall_from: f32,
    last_stood: [f32; 3],
    /// Out over a gap: the spot on the lower floor it set out for, with the floor's height. It always gets there.
    gap_to: Option<[f32; 3]>,
}

fn wrap(a: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    (a + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI
}

impl Body {
    pub fn new(pos: [f32; 3], heading: f32) -> Body {
        Body {
            pos,
            vel: [0.0; 2],
            vz: 0.0,
            heading,
            grounded: true,
            roll: None,
            cooldown: 0.0,
            land_left: 0.0,
            land_total: 0.0,
            land_light: false,
            jumping: false,
            fall_from: pos[2],
            last_stood: pos,
            gap_to: None,
        }
    }

    pub fn speed(&self) -> f32 {
        self.vel[0].hypot(self.vel[1])
    }

    /// Carry the body over a change of space, a turn of `yaw` about the vertical and then a shift, as a seamless
    /// crossing makes: where it is, which way it faces and moves, and where it last stood, so a roll or a fall goes on.
    pub fn carry(&mut self, yaw: f32, shift: [f32; 3]) {
        let (s, c) = yaw.sin_cos();
        let spin = |v: [f32; 2]| [c * v[0] - s * v[1], s * v[0] + c * v[1]];
        let put = |p: [f32; 3]| {
            let q = spin([p[0], p[1]]);
            [q[0] + shift[0], q[1] + shift[1], p[2] + shift[2]]
        };
        self.pos = put(self.pos);
        self.last_stood = put(self.last_stood);
        self.vel = spin(self.vel);
        self.gap_to = self.gap_to.map(put);
        self.heading = wrap(self.heading + yaw);
        self.fall_from += shift[2];
        if let Some(r) = self.roll.as_mut() {
            r.dir = spin(r.dir);
        }
    }

    /// How fast a jump leaves the floor, for it to reach `jump_height`.
    fn jump_speed(t: &Tuning) -> f32 {
        (2.0 * t.gravity * t.jump_height).sqrt()
    }

    /// Leave the floor, if the body is on it, on its feet and not rolling. Says whether it did.
    pub fn jump(&mut self, t: &Tuning) -> bool {
        if !self.grounded || self.roll.is_some() || (self.landing() && !self.land_light) {
            return false;
        }
        self.vz = Self::jump_speed(t);
        self.grounded = false;
        self.jumping = true;
        self.fall_from = self.pos[2];
        self.land_left = 0.0;
        true
    }

    /// 0 to 1 through a jump, by how fast the body rises and falls: 0.5 at the top. None when not jumping.
    pub fn jump_progress(&self, t: &Tuning) -> Option<f32> {
        let v = Self::jump_speed(t);
        self.jumping
            .then(|| ((v - self.vz) / (2.0 * v)).clamp(0.0, 1.0))
    }

    pub fn rolling(&self) -> bool {
        self.roll.is_some()
    }

    /// Something is waiting for the roll, so it comes round faster. It does not stop dead: the body would be upside down.
    pub fn hurry_roll(&mut self) {
        if let Some(r) = self.roll.as_mut() {
            r.hurry = true;
        }
    }

    /// 0 to 1 through the roll; 0 when not rolling.
    pub fn roll_progress(&self, t: &Tuning) -> f32 {
        self.roll
            .map_or(0.0, |r| (r.t / t.roll_time).clamp(0.0, 1.0))
    }

    /// 0 to 1: how deep into the landing crouch the body is (1 at the moment of impact).
    pub fn land_dip(&self) -> f32 {
        if self.land_total > 0.0 {
            (self.land_left / self.land_total).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    pub fn landing(&self) -> bool {
        self.land_left > 0.0
    }

    pub fn gait(&self, t: &Tuning) -> Gait {
        let s = self.speed();
        if s < 0.25 {
            Gait::Idle
        } else if s < (t.walk + t.run) * 0.5 {
            Gait::Walk
        } else if s < (t.run + t.sprint) * 0.5 {
            Gait::Run
        } else {
            Gait::Sprint
        }
    }

    /// Distance a roll covers by time `t`: speed is `v * (1 - (t/T)^2)`, so it starts fast and eases out. Exact, so the
    /// total does not depend on the step.
    fn roll_distance_at(t: &Tuning, at: f32) -> f32 {
        let tt = at.clamp(0.0, t.roll_time);
        let vmax = t.roll_distance * 1.5 / t.roll_time;
        vmax * (tt - tt * tt * tt / (3.0 * t.roll_time * t.roll_time))
    }

    /// Advance by `dt` seconds.
    pub fn step(&mut self, input: Input, ground: &dyn Ground, t: &Tuning, dt: f32) -> Events {
        let mut ev = Events::default();
        let mut left = dt;
        while left > 1e-6 {
            let h = left.min(1.0 / 60.0);
            left -= h;
            self.substep(input, ground, t, h, &mut ev);
        }
        ev
    }

    fn substep(&mut self, input: Input, ground: &dyn Ground, t: &Tuning, dt: f32, ev: &mut Events) {
        let stood = self.grounded;
        self.cooldown = (self.cooldown - dt).max(0.0);
        self.land_left = (self.land_left - dt).max(0.0);
        if input.roll && self.grounded && self.roll.is_none() && self.cooldown <= 0.0 {
            let dir = input
                .dir
                .unwrap_or([self.heading.cos(), self.heading.sin()]);
            self.roll = Some(Roll {
                dir,
                t: 0.0,
                hurry: false,
            });
            self.heading = dir[1].atan2(dir[0]);
            ev.roll_started = true;
        }
        if let Some(r) = self.roll {
            let t0 = r.t;
            let rate = if r.hurry { ROLL_HURRY } else { 1.0 };
            let t1 = (t0 + dt * rate).min(t.roll_time);
            let d = Self::roll_distance_at(t, t1) - Self::roll_distance_at(t, t0);
            let moved = self.move_by([r.dir[0] * d, r.dir[1] * d], ground, t, dt);
            self.vel = [moved[0] / dt, moved[1] / dt];
            self.roll = Some(Roll { t: t1, ..r });
            if t1 >= t.roll_time {
                self.roll = None;
                self.cooldown = t.roll_cooldown;
                let s = t.sprint * t.roll_exit;
                self.vel = [r.dir[0] * s, r.dir[1] * s];
                ev.roll_ended = true;
            }
        } else {
            self.accelerate(input, t, dt);
            let d = [self.vel[0] * dt, self.vel[1] * dt];
            let moved = self.move_by(d, ground, t, dt);
            // Stopped or turned aside by something: what it moves at now is what it did.
            if moved != d {
                self.vel = [moved[0] / dt, moved[1] / dt];
            }
            self.face(input, t, dt);
        }
        ev.left_ground |= stood && !self.grounded;
        self.vertical(ground, t, dt, ev);
    }

    fn accelerate(&mut self, input: Input, t: &Tuning, dt: f32) {
        let top = if input.walk { t.walk } else { t.sprint };
        let top = if self.land_left > 0.0 && !self.land_light {
            top * t.land_slow
        } else {
            top
        };
        let want = input.dir.map_or([0.0; 2], |d| [d[0] * top, d[1] * top]);
        let air = if self.grounded { 1.0 } else { t.air_control };
        let rate = if input.dir.is_some() {
            t.accel
        } else {
            t.decel
        } * air;
        let (dx, dy) = (want[0] - self.vel[0], want[1] - self.vel[1]);
        let dl = dx.hypot(dy);
        let most = rate * dt;
        if dl <= most {
            self.vel = want;
        } else {
            self.vel[0] += dx / dl * most;
            self.vel[1] += dy / dl * most;
        }
    }

    /// Face the way pushed, smoothly; with nothing pushed, the way still moving.
    fn face(&mut self, input: Input, t: &Tuning, dt: f32) {
        let aim = match input.dir {
            Some(d) => Some(d[1].atan2(d[0])),
            None if self.speed() > 1.0 => Some(self.vel[1].atan2(self.vel[0])),
            None => None,
        };
        if let Some(a) = aim {
            let err = wrap(a - self.heading);
            let eased = err * (1.0 - (-t.turn_smooth * dt).exp());
            let most = t.turn_max * dt;
            self.heading = wrap(self.heading + eased.clamp(-most, most));
        }
    }

    /// Whether a jump may not go on to a spot. What holds a body on the floor holds it in the air, measured from where it
    /// jumped: it does not go over a rail, or to a floor with no walk back. A jump higher than a rail still stops at it.
    fn jump_stopped(&self, ground: &dyn Ground, t: &Tuning, to: [f32; 2], u: [f32; 2]) -> bool {
        let p = self.pos;
        let from = self.fall_from;
        let there = [to[0] + u[0] * BARRIER_LOOK, to[1] + u[1] * BARRIER_LOOK];
        let walled = ground.barrier([p[0], p[1]], there, from + BARRIER_LOW, from + BARRIER_HIGH)
            || WALL_HEIGHTS
                .iter()
                .any(|h| ground.wall([p[0], p[1], from + h], [there[0], there[1], from + h]));
        let reach = (p[2] + 0.1).max(from + 0.1);
        match ground.floor(to[0], to[1], reach) {
            // On its own level.
            Some(f) if (f - from).abs() <= t.step_down => false,
            Some(f) => {
                f < from - t.max_drop
                    || walled
                    || !ground.way_back(self.last_stood, [to[0], to[1], f])
            }
            None => walled,
        }
    }

    /// Whether a wall or railing stands between the body and a spot, at either height it could stop a body.
    fn barrier(&self, ground: &dyn Ground, to: [f32; 2]) -> bool {
        let p = self.pos;
        ground.barrier([p[0], p[1]], to, p[2] + BARRIER_LOW, p[2] + BARRIER_HIGH)
            || WALL_HEIGHTS.iter().any(|h| {
                let z = p[2] + h;
                ground.wall([p[0], p[1], z], [to[0], to[1], z])
            })
    }

    /// The spot on a lower floor that lies this way from `(x, y)`, over a gap, with no wall or railing in between. A cliff
    /// leaves such a gap in the walkable faces.
    fn drop_ahead(
        &self,
        ground: &dyn Ground,
        t: &Tuning,
        x: f32,
        y: f32,
        u: [f32; 2],
    ) -> Option<[f32; 3]> {
        let z = self.pos[2];
        let top = if self.grounded {
            z - t.step_down
        } else {
            z + 0.1
        };
        for k in 1..=GAP_PROBES {
            let s = k as f32 * GAP_PROBE;
            let (qx, qy) = (x + u[0] * s, y + u[1] * s);
            if let Some(f) = ground.floor(qx, qy, top) {
                let free = f >= z - t.max_drop
                    && !self.barrier(ground, [qx, qy])
                    && (!self.grounded || ground.way_back(self.pos, [qx, qy, f]));
                return free.then_some([qx, qy, f]);
            }
        }
        None
    }

    /// On its way over a gap to the floor it set out for: nothing stops it, or it would hang over nothing and fall out of the
    /// world, and it goes fast enough to be over that floor before it has fallen past it. Returns how far it moved, or none
    /// once there is a floor under it or it is not over a gap.
    fn cross_gap(
        &mut self,
        d: [f32; 2],
        ground: &dyn Ground,
        t: &Tuning,
        dt: f32,
    ) -> Option<[f32; 2]> {
        let to = self.gap_to?;
        let p = self.pos;
        if self.grounded || ground.floor(p[0], p[1], p[2] + 0.1).is_some() {
            self.gap_to = None;
            return None;
        }
        let (dx, dy) = (to[0] - p[0], to[1] - p[1]);
        let left = dx.hypot(dy).max(1e-6);
        let high = (p[2] - to[2]).max(0.0);
        let falling = (self.vz + (self.vz * self.vz + 2.0 * t.gravity * high).sqrt()) / t.gravity;
        let in_time = left * dt / falling.max(dt);
        let step = d[0].hypot(d[1]).max(GAP_STEP).max(in_time).min(left);
        let moved = [dx / left * step, dy / left * step];
        self.pos[0] += moved[0];
        self.pos[1] += moved[1];
        Some(moved)
    }

    /// Move on the floor by `d`, sliding along what stops it. Returns how far it did move.
    fn move_by(&mut self, d: [f32; 2], ground: &dyn Ground, t: &Tuning, dt: f32) -> [f32; 2] {
        if let Some(moved) = self.cross_gap(d, ground, t, dt) {
            return moved;
        }
        if d[0] == 0.0 && d[1] == 0.0 {
            return d;
        }
        let p = self.pos;
        let reach = if self.grounded {
            p[2] + t.step_up
        } else {
            p[2] + 0.1
        };
        let ok = |dx: f32, dy: f32| -> Option<[f32; 2]> {
            let (x, y) = (p[0] + dx, p[1] + dy);
            if ground.blocked(p, [x, y, p[2]]) {
                return None;
            }
            let l = dx.hypot(dy).max(1e-6);
            let u = [dx / l, dy / l];
            let here = ground.floor(x, y, reach);
            if !self.grounded {
                if self.jumping && self.jump_stopped(ground, t, [x, y], u) {
                    return None;
                }
                return (here.is_some() || self.drop_ahead(ground, t, x, y, u).is_some())
                    .then_some([x, y]);
            }
            let clear = match here {
                Some(f) if f < p[2] - t.max_drop => return None,
                Some(f) if f < p[2] - t.step_down && !ground.way_back(p, [x, y, f]) => return None,
                Some(f)
                    if f < p[2] - t.step_down
                        && self.barrier(
                            ground,
                            [x + u[0] * BARRIER_LOOK, y + u[1] * BARRIER_LOOK],
                        ) =>
                {
                    return None
                }
                // Keep off the very edge of the floor, as the Classic steering does.
                Some(_) => ground
                    .floor(x + u[0] * 0.3, y + u[1] * 0.3, reach)
                    .is_some(),
                None => false,
            };
            (clear || self.drop_ahead(ground, t, x, y, u).is_some()).then_some([x, y])
        };
        // Along a wall at any angle: first straight on, then turning aside, and it goes more slowly the further it turns.
        let turned = |degrees: f32| {
            let (s, c) = degrees.to_radians().sin_cos();
            [(d[0] * c - d[1] * s) * c, (d[0] * s + d[1] * c) * c]
        };
        let Some(q) = ok(d[0], d[1]).or_else(|| {
            SLIDE_TURNS.iter().find_map(|a| {
                let (left, right) = (turned(*a), turned(-*a));
                ok(left[0], left[1]).or_else(|| ok(right[0], right[1]))
            })
        }) else {
            return [0.0; 2];
        };
        // Asked from where it stands: a spot over a gap is on no floor, so it has no way back.
        let (sx, sy) = (q[0] - p[0], q[1] - p[1]);
        let l = sx.hypot(sy).max(1e-6);
        let over_gap = self.grounded && ground.floor(q[0], q[1], reach).is_none();
        if over_gap {
            self.gap_to = self.drop_ahead(ground, t, q[0], q[1], [sx / l, sy / l]);
        }
        self.pos[0] = q[0];
        self.pos[1] = q[1];
        // Out over a gap: nothing under it now, so it falls, and it keeps going until it is over the lower floor.
        if over_gap {
            self.grounded = false;
            self.vz = 0.0;
            self.fall_from = p[2];
            if self.speed() < GAP_SPEED {
                self.vel = [sx / l * GAP_SPEED, sy / l * GAP_SPEED];
            }
        }
        [q[0] - p[0], q[1] - p[1]]
    }

    /// The body was moved along a route by someone else: take its place and facing, and keep gravity and the snap.
    pub fn carried_to(
        &mut self,
        xy: [f32; 2],
        heading: f32,
        ground: &dyn Ground,
        t: &Tuning,
        dt: f32,
    ) -> Events {
        let mut ev = Events::default();
        if dt > 0.0 {
            self.vel = [(xy[0] - self.pos[0]) / dt, (xy[1] - self.pos[1]) / dt];
        }
        self.pos[0] = xy[0];
        self.pos[1] = xy[1];
        self.heading = heading;
        self.cooldown = (self.cooldown - dt).max(0.0);
        self.land_left = (self.land_left - dt).max(0.0);
        self.vertical(ground, t, dt, &mut ev);
        ev
    }

    /// Gravity, and the snap to the floor.
    fn vertical(&mut self, ground: &dyn Ground, t: &Tuning, dt: f32, ev: &mut Events) {
        let (x, y) = (self.pos[0], self.pos[1]);
        if self.grounded {
            let floor = ground.floor(x, y, self.pos[2] + t.step_up);
            match floor {
                Some(f) if f >= self.pos[2] - t.step_down => {
                    let z = self.pos[2] + (f - self.pos[2]) * (1.0 - (-t.snap_rate * dt).exp());
                    // Never sink into the floor or hang above it by more than the lag.
                    self.pos[2] = z.clamp(f - t.snap_lag, f + t.snap_lag);
                    if (self.pos[2] - f).abs() < 0.005 {
                        self.pos[2] = f;
                    }
                    self.last_stood = self.pos;
                }
                Some(_) => {
                    self.grounded = false;
                    self.vz = 0.0;
                    self.fall_from = self.pos[2];
                    ev.left_ground = true;
                }
                // No floor here at all: a spot off the walkmesh stays as it is, as in the Classic walk.
                None => {}
            }
            return;
        }
        self.vz -= t.gravity * dt;
        let z1 = self.pos[2] + self.vz * dt;
        // A roll carries on through the air; the floor to land on is the one under the new spot. Crossing a gap that is the
        // floor it set out for, however far down the body has got.
        let top = self.gap_to.map_or(self.pos[2], |g| self.pos[2].max(g[2]));
        let floor = ground.floor(x, y, top + 0.1);
        // A jump comes down on no floor higher up that has no walk back: it would be stranded there.
        let floor = match floor {
            Some(f)
                if self.jumping
                    && f > self.fall_from + t.step_up
                    && !ground.way_back(self.last_stood, [x, y, f]) =>
            {
                ground.floor(x, y, self.fall_from + t.step_up)
            }
            f => f,
        };
        match floor {
            Some(f) if z1 <= f => {
                let impact = -self.vz;
                self.pos[2] = f;
                self.vz = 0.0;
                self.grounded = true;
                self.gap_to = None;
                self.last_stood = self.pos;
                ev.landed = Some(impact);
                let light = self.jumping && impact <= Self::jump_speed(t) + LIGHT_LANDING;
                self.jumping = false;
                self.land_light = light;
                if impact > t.land_threshold {
                    self.land_total =
                        ((impact - t.land_threshold) * t.land_per_speed + 0.12).min(t.land_max);
                    self.land_left = self.land_total;
                    if !light {
                        self.vel[0] *= t.land_slow;
                        self.vel[1] *= t.land_slow;
                    }
                }
            }
            _ => {
                self.pos[2] = z1;
                if self.fall_from - z1 > t.void_depth {
                    self.pos = self.last_stood;
                    self.gap_to = None;
                    self.jumping = false;
                    self.vel = [0.0; 2];
                    self.vz = 0.0;
                    self.grounded = true;
                    ev.reset = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A floor made from a function of x and y, with an optional rectangle of it.
    struct Hills<F: Fn(f32, f32) -> Option<f32>>(F);

    impl<F: Fn(f32, f32) -> Option<f32>> Ground for Hills<F> {
        fn floor(&self, x: f32, y: f32, max_z: f32) -> Option<f32> {
            self.0(x, y).filter(|f| *f <= max_z + 1e-4)
        }
        fn blocked(&self, _: [f32; 3], _: [f32; 3]) -> bool {
            false
        }
    }

    fn flat() -> Hills<impl Fn(f32, f32) -> Option<f32>> {
        Hills(|_, _| Some(0.0))
    }

    const DT: f32 = 1.0 / 60.0;

    fn run_for(b: &mut Body, g: &dyn Ground, input: Input, secs: f32) {
        let t = Tuning::default();
        let n = (secs / DT).round() as usize;
        for _ in 0..n {
            b.step(input, g, &t, DT);
        }
    }

    #[test]
    fn sprint_is_a_quarter_faster_than_todays_run() {
        let t = Tuning::default();
        let ratio = t.sprint / crate::RUN_SPEED;
        assert!((ratio - 1.25).abs() < 0.02 * 1.25, "{ratio}");
        assert_eq!(RUN, crate::RUN_SPEED);
        let mut b = Body::new([0.0; 3], 0.0);
        run_for(
            &mut b,
            &flat(),
            Input {
                dir: Some([1.0, 0.0]),
                ..Input::default()
            },
            2.0,
        );
        assert!(
            (b.speed() / crate::RUN_SPEED - 1.25).abs() < 0.025,
            "{}",
            b.speed()
        );
        assert_eq!(b.gait(&t), Gait::Sprint);
    }

    #[test]
    fn it_accelerates_and_does_not_start_at_full_speed() {
        let mut b = Body::new([0.0; 3], 0.0);
        let push = Input {
            dir: Some([1.0, 0.0]),
            ..Input::default()
        };
        run_for(&mut b, &flat(), push, DT);
        assert!(b.speed() > 0.0 && b.speed() < 1.0, "{}", b.speed());
        // Full sprint in a quarter of a second or a little more, not before 0.2 s.
        let mut c = Body::new([0.0; 3], 0.0);
        run_for(&mut c, &flat(), push, 0.2);
        assert!(c.speed() < SPRINT - 0.5, "{}", c.speed());
        run_for(&mut c, &flat(), push, 0.1);
        assert!((c.speed() - SPRINT).abs() < 1e-3);
    }

    #[test]
    fn it_slows_down_over_a_short_distance_and_stops() {
        let mut b = Body::new([0.0; 3], 0.0);
        run_for(
            &mut b,
            &flat(),
            Input {
                dir: Some([1.0, 0.0]),
                ..Input::default()
            },
            1.5,
        );
        let x0 = b.pos[0];
        run_for(&mut b, &flat(), Input::default(), 0.1);
        assert!(b.speed() > 0.5, "not an instant stop: {}", b.speed());
        run_for(&mut b, &flat(), Input::default(), 0.4);
        assert!(b.speed() < 1e-4);
        let slide = b.pos[0] - x0;
        assert!(slide > 0.2 && slide < 0.9, "slid {slide} m");
    }

    #[test]
    fn it_turns_with_momentum() {
        let mut b = Body::new([0.0; 3], 0.0);
        run_for(
            &mut b,
            &flat(),
            Input {
                dir: Some([1.0, 0.0]),
                ..Input::default()
            },
            1.0,
        );
        // Reverse: it neither flips at once nor keeps going.
        let back = Input {
            dir: Some([-1.0, 0.0]),
            ..Input::default()
        };
        run_for(&mut b, &flat(), back, 3.0 * DT);
        assert!(b.vel[0] > 0.0, "still going forward: {}", b.vel[0]);
        assert!(b.heading.cos() > 0.0, "facing has not flipped yet");
        run_for(&mut b, &flat(), back, 1.0);
        assert!(b.vel[0] < -SPRINT + 0.01);
        assert!(b.heading.cos() < -0.99);
    }

    #[test]
    fn facing_never_turns_faster_than_the_limit() {
        let t = Tuning::default();
        let mut b = Body::new([0.0; 3], 0.0);
        let mut last = b.heading;
        for _ in 0..120 {
            b.step(
                Input {
                    dir: Some([0.0, 1.0]),
                    ..Input::default()
                },
                &flat(),
                &t,
                DT,
            );
            assert!(wrap(b.heading - last).abs() <= t.turn_max * DT + 1e-5);
            last = b.heading;
        }
    }

    #[test]
    fn walking_is_slower_than_sprinting() {
        let mut b = Body::new([0.0; 3], 0.0);
        run_for(
            &mut b,
            &flat(),
            Input {
                dir: Some([1.0, 0.0]),
                walk: true,
                ..Input::default()
            },
            1.0,
        );
        assert!((b.speed() - 1.7).abs() < 1e-3);
        assert_eq!(b.gait(&Tuning::default()), Gait::Walk);
    }

    #[test]
    fn it_follows_a_slope_without_sinking_or_floating() {
        // A ramp of 20 degrees from x = 2 up to x = 8.
        let ramp = Hills(|x, _| Some(((x - 2.0).clamp(0.0, 6.0)) * 20f32.to_radians().tan()));
        let t = Tuning::default();
        let mut b = Body::new([0.0; 3], 0.0);
        let push = Input {
            dir: Some([1.0, 0.0]),
            ..Input::default()
        };
        for _ in 0..180 {
            let ev = b.step(push, &ramp, &t, DT);
            assert!(!ev.left_ground);
            let floor = ramp.0(b.pos[0], b.pos[1]).unwrap();
            assert!(
                b.pos[2] >= floor - t.snap_lag - 1e-4,
                "sank {}",
                floor - b.pos[2]
            );
            assert!(b.pos[2] <= floor + t.snap_lag + 1e-4);
            assert!(b.grounded);
        }
        run_for(&mut b, &ramp, Input::default(), 1.0);
        let floor = ramp.0(b.pos[0], b.pos[1]).unwrap();
        assert!(
            (b.pos[2] - floor).abs() < 0.01,
            "settled {} vs {floor}",
            b.pos[2]
        );
    }

    #[test]
    fn a_stair_is_climbed_smoothly() {
        // Steps of 0.2 m every metre.
        let stairs = Hills(|x, _| Some((x.max(0.0).floor()).min(5.0) * 0.2));
        let t = Tuning::default();
        let mut b = Body::new([0.0, 0.0, 0.0], 0.0);
        let push = Input {
            dir: Some([1.0, 0.0]),
            ..Input::default()
        };
        let mut last = 0.0f32;
        let mut biggest = 0.0f32;
        for _ in 0..150 {
            b.step(push, &stairs, &t, DT);
            biggest = biggest.max(b.pos[2] - last);
            last = b.pos[2];
            assert!(b.grounded);
        }
        assert!(biggest < 0.2, "a single step moved up {biggest}");
        assert!((b.pos[2] - 1.0).abs() < 0.16);
    }

    fn ledge() -> Hills<impl Fn(f32, f32) -> Option<f32>> {
        // Floor at 0 for x < 3, then a drop of 2 m for x >= 3.
        Hills(|x, _| Some(if x < 3.0 { 0.0 } else { -2.0 }))
    }

    #[test]
    fn it_drops_off_a_ledge_on_an_arc_and_lands() {
        let t = Tuning::default();
        let g = ledge();
        let mut b = Body::new([0.0; 3], 0.0);
        let push = Input {
            dir: Some([1.0, 0.0]),
            ..Input::default()
        };
        let (mut left, mut landed, mut lowest) = (false, None, 0.0f32);
        let mut heights = Vec::new();
        for _ in 0..240 {
            let ev = b.step(push, &g, &t, DT);
            left |= ev.left_ground;
            if ev.landed.is_some() {
                landed = ev.landed;
            }
            lowest = lowest.min(b.pos[2]);
            heights.push(b.pos[2]);
            assert!(
                b.pos[2] >= -2.0 - 1e-4,
                "went through the floor: {}",
                b.pos[2]
            );
        }
        assert!(left, "never left the ground");
        let v = landed.expect("never landed");
        // v = sqrt(2 g h) for a drop of about 2 m (the step already taken is small).
        let want = (2.0 * t.gravity * 2.0f32).sqrt();
        assert!(
            (v - want).abs() < want * 0.1,
            "impact {v}, want about {want}"
        );
        assert!(b.grounded && (b.pos[2] + 2.0).abs() < 1e-3);
        // It fell: heights are not a straight line down. Early drop is slow, late drop is fast.
        let i = heights.iter().position(|h| *h < -0.05).unwrap();
        let (d1, d2) = (
            heights[i] - heights[i + 3],
            heights[i + 20] - heights[i + 23],
        );
        assert!(d2 > d1 * 2.0, "no arc: {d1} then {d2}");
    }

    #[test]
    fn a_hard_landing_slows_the_body_for_a_moment() {
        let t = Tuning::default();
        let g = ledge();
        let mut b = Body::new([2.0, 0.0, 0.0], 0.0);
        let push = Input {
            dir: Some([1.0, 0.0]),
            ..Input::default()
        };
        let mut saw = false;
        for _ in 0..240 {
            let ev = b.step(push, &g, &t, DT);
            if ev.landed.is_some() {
                saw = true;
                assert!(b.landing());
                assert!(b.land_dip() > 0.9);
            }
        }
        assert!(saw);
        assert!(!b.landing());
    }

    #[test]
    fn a_small_step_down_is_not_a_fall() {
        let g = Hills(|x, _| Some(if x < 3.0 { 0.0 } else { -0.2 }));
        let t = Tuning::default();
        let mut b = Body::new([2.0, 0.0, 0.0], 0.0);
        let push = Input {
            dir: Some([1.0, 0.0]),
            ..Input::default()
        };
        for _ in 0..90 {
            let ev = b.step(push, &g, &t, DT);
            assert!(!ev.left_ground && ev.landed.is_none());
        }
        assert!((b.pos[2] + 0.2).abs() < 0.01);
    }

    #[test]
    fn the_edge_of_the_floor_stops_it_when_there_is_nothing_below() {
        let g = Hills(|x, _| if x < 3.0 { Some(0.0) } else { None });
        let mut b = Body::new([0.0; 3], 0.0);
        run_for(
            &mut b,
            &g,
            Input {
                dir: Some([1.0, 0.0]),
                ..Input::default()
            },
            3.0,
        );
        assert!(b.grounded && b.pos[0] < 3.0 && b.pos[2] == 0.0);
    }

    #[test]
    fn a_gap_before_a_lower_floor_is_crossed_by_falling() {
        // As on Dantooine: the cliff face leaves a gap between the floor above and the one below.
        let g = Hills(|x, _| {
            if x < 3.0 {
                Some(0.0)
            } else if x < 3.4 {
                None
            } else {
                Some(-2.0)
            }
        });
        let t = Tuning::default();
        let mut b = Body::new([0.0; 3], 0.0);
        let push = Input {
            dir: Some([1.0, 0.0]),
            ..Input::default()
        };
        let (mut left, mut landed) = (false, false);
        for _ in 0..180 {
            let ev = b.step(push, &g, &t, DT);
            left |= ev.left_ground;
            landed |= ev.landed.is_some();
            assert!(b.pos[2] >= -2.0 - 1e-4);
        }
        assert!(left && landed);
        assert!(b.grounded && b.pos[0] > 3.4 && (b.pos[2] + 2.0).abs() < 1e-3);
    }

    #[test]
    fn a_wall_at_chest_height_stops_it_going_over_the_gap() {
        struct Walled;
        impl Ground for Walled {
            fn floor(&self, x: f32, _: f32, max_z: f32) -> Option<f32> {
                let f = if x < 3.0 {
                    Some(0.0)
                } else if x < 3.4 {
                    None
                } else {
                    Some(-2.0)
                };
                f.filter(|f| *f <= max_z + 1e-4)
            }
            fn blocked(&self, _: [f32; 3], _: [f32; 3]) -> bool {
                false
            }
            fn wall(&self, a: [f32; 3], b: [f32; 3]) -> bool {
                a[0] < 3.2 && b[0] >= 3.2
            }
        }
        let mut b = Body::new([0.0; 3], 0.0);
        run_for(
            &mut b,
            &Walled,
            Input {
                dir: Some([1.0, 0.0]),
                ..Input::default()
            },
            3.0,
        );
        assert!(
            b.grounded && b.pos[0] < 3.0 && b.pos[2] == 0.0,
            "{:?}",
            b.pos
        );
    }

    #[test]
    fn a_floor_with_no_way_back_is_a_wall() {
        struct Pit;
        impl Ground for Pit {
            fn floor(&self, x: f32, _: f32, max_z: f32) -> Option<f32> {
                Some(if x < 3.0 { 0.0 } else { -2.0 }).filter(|f| *f <= max_z + 1e-4)
            }
            fn blocked(&self, _: [f32; 3], _: [f32; 3]) -> bool {
                false
            }
            fn way_back(&self, _: [f32; 3], _: [f32; 3]) -> bool {
                false
            }
        }
        let mut b = Body::new([0.0; 3], 0.0);
        run_for(
            &mut b,
            &Pit,
            Input {
                dir: Some([1.0, 0.0]),
                ..Input::default()
            },
            3.0,
        );
        assert!(
            b.grounded && b.pos[0] < 3.0 && b.pos[2] == 0.0,
            "{:?}",
            b.pos
        );
    }

    #[test]
    fn a_drop_deeper_than_the_limit_is_a_wall() {
        let g = Hills(|x, _| Some(if x < 3.0 { 0.0 } else { -20.0 }));
        let mut b = Body::new([0.0; 3], 0.0);
        run_for(
            &mut b,
            &g,
            Input {
                dir: Some([1.0, 0.0]),
                ..Input::default()
            },
            3.0,
        );
        assert!(b.grounded && b.pos[0] < 3.0 && b.pos[2] == 0.0);
    }

    #[test]
    fn a_spot_off_the_floor_does_not_fall_for_ever() {
        let g = Hills(|_, _| None);
        let mut b = Body::new([0.0, 0.0, 5.0], 0.0);
        run_for(&mut b, &g, Input::default(), 2.0);
        assert!(b.grounded && b.pos[2] == 5.0);
    }

    #[test]
    fn carried_along_a_route_it_still_snaps_and_falls() {
        let t = Tuning::default();
        let g = ledge();
        let mut b = Body::new([0.0; 3], 0.0);
        let mut x = 0.0;
        for _ in 0..120 {
            x += 0.09;
            b.carried_to([x, 0.0], 0.0, &g, &t, DT);
        }
        assert!((b.speed() - 5.4).abs() < 0.01, "{}", b.speed());
        assert!((b.pos[2] + 2.0).abs() < 1e-3, "{}", b.pos[2]);
    }

    #[test]
    fn a_roll_covers_its_distance_in_its_time() {
        let t = Tuning::default();
        for dt in [1.0 / 60.0, 1.0 / 120.0, 1.0 / 144.0] {
            let mut b = Body::new([0.0; 3], 0.0);
            let go = Input {
                dir: Some([1.0, 0.0]),
                roll: true,
                ..Input::default()
            };
            let (mut secs, mut started, mut ended) = (0.0, false, false);
            let mut x_end = 0.0;
            for i in 0..400 {
                let ev = b.step(
                    if i == 0 {
                        go
                    } else {
                        Input { roll: false, ..go }
                    },
                    &flat(),
                    &t,
                    dt,
                );
                started |= ev.roll_started;
                if b.rolling() {
                    secs += dt;
                }
                if ev.roll_ended && !ended {
                    ended = true;
                    x_end = b.pos[0];
                    break;
                }
            }
            assert!(started && ended);
            assert!(
                (x_end - t.roll_distance).abs() < t.roll_distance * 0.02,
                "{x_end}"
            );
            assert!(
                (secs - t.roll_time).abs() < t.roll_time * 0.1 + dt,
                "{secs}"
            );
        }
    }

    #[test]
    fn a_roll_is_faster_than_a_sprint_and_has_a_cooldown() {
        let t = Tuning::default();
        let peak = Tuning::default().roll_distance * 1.5 / t.roll_time;
        assert!(peak > t.sprint);
        let mut b = Body::new([0.0; 3], 0.0);
        let go = Input {
            dir: Some([1.0, 0.0]),
            roll: true,
            ..Input::default()
        };
        b.step(go, &flat(), &t, DT);
        assert!(b.rolling());
        // Asking again during the roll or just after does nothing new.
        let mut starts = 0;
        for _ in 0..90 {
            if b.step(go, &flat(), &t, DT).roll_started {
                starts += 1;
            }
        }
        // The key is held: one more roll after the cooldown, no more than that in 1.5 s.
        assert_eq!(starts, 1, "rolls started while held: {starts}");
    }

    #[test]
    fn a_roll_cannot_start_in_the_air() {
        let t = Tuning::default();
        let g = ledge();
        let mut b = Body::new([2.9, 0.0, 0.0], 0.0);
        let push = Input {
            dir: Some([1.0, 0.0]),
            ..Input::default()
        };
        for _ in 0..10 {
            b.step(push, &g, &t, DT);
        }
        assert!(!b.grounded);
        let ev = b.step(Input { roll: true, ..push }, &g, &t, DT);
        assert!(!ev.roll_started && !b.rolling());
    }

    #[test]
    fn a_roll_goes_where_it_faces_when_nothing_is_pushed() {
        let t = Tuning::default();
        let mut b = Body::new([0.0; 3], std::f32::consts::FRAC_PI_2);
        b.step(
            Input {
                roll: true,
                ..Input::default()
            },
            &flat(),
            &t,
            DT,
        );
        run_for(&mut b, &flat(), Input::default(), 0.7);
        assert!(b.pos[1] > 2.5 && b.pos[0].abs() < 0.05, "{:?}", b.pos);
    }

    #[test]
    fn a_wall_stops_a_roll_and_the_slide_keeps_along_it() {
        struct Wall;
        impl Ground for Wall {
            fn floor(&self, _: f32, _: f32, _: f32) -> Option<f32> {
                Some(0.0)
            }
            fn blocked(&self, a: [f32; 3], b: [f32; 3]) -> bool {
                a[0] < 1.0 && b[0] >= 1.0
            }
        }
        let mut b = Body::new([0.0; 3], 0.0);
        run_for(
            &mut b,
            &Wall,
            Input {
                dir: Some([1.0, 0.0]),
                ..Input::default()
            },
            2.0,
        );
        assert!(b.pos[0] < 1.0);
        assert!(b.speed() < 1e-3);
    }

    /// A floor from x = -50 to a wall along y = x, on the side where y is greater.
    fn slanted_wall() -> Hills<impl Fn(f32, f32) -> Option<f32>> {
        Hills(|x, y| (y < x + 2.0 && x.abs() < 50.0 && y.abs() < 50.0).then_some(0.0))
    }

    #[test]
    fn a_body_slides_smoothly_along_a_wall_at_an_angle() {
        let t = Tuning::default();
        let g = slanted_wall();
        let mut b = Body::new([0.0, 0.0, 0.0], 0.0);
        let input = Input {
            dir: Some([0.0, 1.0]),
            ..Input::default()
        };
        let dt = 1.0 / 60.0;
        for _ in 0..60 {
            b.step(input, &g, &t, dt);
        }
        let mut last = b.pos;
        for _ in 0..60 {
            b.step(input, &g, &t, dt);
            let (dx, dy) = (b.pos[0] - last[0], b.pos[1] - last[1]);
            // Along the wall, up and to the right, frame after frame: no steps of x alone and y alone.
            let along = (dx + dy) / std::f32::consts::SQRT_2;
            let moved = dx.hypot(dy);
            assert!(moved > 0.02, "stuck at {:?}", b.pos);
            assert!(along > moved * 0.5, "{dx} {dy}");
            assert!(dx > 0.0 && dy > 0.0, "{dx} {dy}");
            last = b.pos;
        }
    }

    #[test]
    fn a_sprint_straight_into_a_wall_stops_and_says_so() {
        let t = Tuning::default();
        let g = Hills(|x, _| (x < 5.0).then_some(0.0));
        let mut b = Body::new([0.0, 0.0, 0.0], 0.0);
        let input = Input {
            dir: Some([1.0, 0.0]),
            ..Input::default()
        };
        for _ in 0..240 {
            b.step(input, &g, &t, 1.0 / 60.0);
        }
        assert!(b.pos[0] < 5.0);
        assert!(b.speed() < 0.5, "it runs on the spot at {}", b.speed());
    }

    #[test]
    fn a_hurried_roll_comes_round_sooner_and_ends_the_same_way() {
        let t = Tuning::default();
        let g = flat();
        let mut b = Body::new([0.0, 0.0, 0.0], 0.0);
        let dt = 1.0 / 60.0;
        let roll = Input {
            roll: true,
            ..Input::default()
        };
        b.step(roll, &g, &t, dt);
        for _ in 0..9 {
            b.step(Input::default(), &g, &t, dt);
        }
        let p = b.roll_progress(&t);
        assert!(p > 0.2 && p < 0.4, "{p}");
        b.hurry_roll();
        let mut steps = 0;
        while b.rolling() {
            b.step(Input::default(), &g, &t, dt);
            steps += 1;
            assert!(steps < 60);
        }
        let plain = ((1.0 - p) * t.roll_time / dt) as usize;
        assert!(steps * 2 < plain + 2, "{steps} steps, against {plain}");
        // As far as ever: the whole of the roll's distance.
        assert!((b.pos[0] - t.roll_distance).abs() < 0.05, "{:?}", b.pos);
    }

    /// A lower floor over a gap, with a railing along the edge that only the room's model has.
    struct Railed;

    impl Ground for Railed {
        fn floor(&self, x: f32, _: f32, max_z: f32) -> Option<f32> {
            let f = if x < 5.0 {
                Some(2.0)
            } else if x > 5.4 {
                Some(0.0)
            } else {
                None
            };
            f.filter(|f| *f <= max_z)
        }
        fn blocked(&self, _: [f32; 3], _: [f32; 3]) -> bool {
            false
        }
        fn barrier(&self, a: [f32; 2], b: [f32; 2], low: f32, high: f32) -> bool {
            // A thin rail 0.9 m over the upper floor: a ray at a chosen height would go under or over it.
            a[0].min(b[0]) < 5.2 && a[0].max(b[0]) > 5.2 && low < 2.9 && high > 2.9
        }
    }

    #[test]
    fn a_railing_only_the_model_has_stops_a_fall_at_waist_height() {
        let t = Tuning::default();
        let mut b = Body::new([3.0, 0.0, 2.0], 0.0);
        let input = Input {
            dir: Some([1.0, 0.0]),
            ..Input::default()
        };
        for _ in 0..240 {
            b.step(input, &Railed, &t, 1.0 / 60.0);
        }
        assert!(b.grounded && b.pos[2] == 2.0, "fell: {:?}", b.pos);
        assert!(b.pos[0] < 5.2, "{:?}", b.pos);
    }

    fn jump_from(b: &mut Body, g: &dyn Ground, t: &Tuning, input: Input) -> f32 {
        assert!(b.jump(t));
        let start = b.pos[2];
        let mut top = start;
        for _ in 0..600 {
            b.step(input, g, t, 1.0 / 60.0);
            top = top.max(b.pos[2]);
            if b.grounded {
                break;
            }
        }
        assert!(b.grounded, "{:?}", b.pos);
        top - start
    }

    #[test]
    fn a_jump_rises_its_height_and_comes_down_in_an_arc() {
        let t = Tuning::default();
        let g = flat();
        let mut b = Body::new([0.0, 0.0, 0.0], 0.0);
        assert!(b.jump(&t));
        let (mut heights, mut steps) = (vec![0.0f32], 0);
        while !b.grounded && steps < 300 {
            b.step(Input::default(), &g, &t, 1.0 / 60.0);
            heights.push(b.pos[2]);
            steps += 1;
        }
        let top = heights.iter().cloned().fold(0.0f32, f32::max);
        assert!((top - t.jump_height).abs() < 0.06, "{top}");
        // Up and down take as long, about 0.7 s in all.
        let time = steps as f32 / 60.0;
        assert!((time - 0.705).abs() < 0.05, "{time}");
        let peak = heights.iter().position(|h| *h == top).unwrap() as f32;
        assert!(
            (peak / steps as f32 - 0.5).abs() < 0.08,
            "{peak} of {steps}"
        );
        assert_eq!(b.pos[2], 0.0);
    }

    #[test]
    fn a_jump_keeps_a_sprint_and_lands_with_a_crouch() {
        let t = Tuning::default();
        let g = flat();
        let mut b = Body::new([0.0, 0.0, 0.0], 0.0);
        let input = Input {
            dir: Some([1.0, 0.0]),
            ..Input::default()
        };
        for _ in 0..60 {
            b.step(input, &g, &t, 1.0 / 60.0);
        }
        let before = b.pos[0];
        jump_from(&mut b, &g, &t, input);
        assert!(b.pos[0] - before > 4.0, "{}", b.pos[0] - before);
        assert!(b.landing(), "the crouch");
        assert!((b.speed() - t.sprint).abs() < 0.01, "{}", b.speed());
        b.step(input, &g, &t, 1.0 / 60.0);
        assert!((b.speed() - t.sprint).abs() < 0.01, "{}", b.speed());
    }

    #[test]
    fn a_body_jumps_only_from_the_floor_on_its_feet() {
        let t = Tuning::default();
        let g = flat();
        let mut b = Body::new([0.0, 0.0, 0.0], 0.0);
        assert!(b.jump(&t));
        assert!(b.jump_progress(&t).is_some());
        assert!(!b.jump(&t), "not in the air");
        b.step(Input::default(), &g, &t, 1.0 / 60.0);
        assert!(!b.jump(&t), "not in the air");
        let mut b = Body::new([0.0, 0.0, 0.0], 0.0);
        let roll = Input {
            roll: true,
            ..Input::default()
        };
        b.step(roll, &g, &t, 1.0 / 60.0);
        assert!(b.rolling());
        assert!(!b.jump(&t), "not in a roll");
    }

    #[test]
    fn a_roll_waits_for_the_ground_after_a_jump() {
        let t = Tuning::default();
        let g = flat();
        let mut b = Body::new([0.0, 0.0, 0.0], 0.0);
        assert!(b.jump(&t));
        let roll = Input {
            roll: true,
            ..Input::default()
        };
        b.step(roll, &g, &t, 1.0 / 60.0);
        assert!(!b.rolling());
    }

    #[test]
    fn a_hard_fall_after_a_jump_is_still_a_hard_landing() {
        let t = Tuning::default();
        let g = Hills(|x, _| Some(if x < 1.0 { 3.0 } else { 0.0 }));
        let mut b = Body::new([0.8, 0.0, 3.0], 0.0);
        b.vel = [3.0, 0.0];
        jump_from(
            &mut b,
            &g,
            &t,
            Input {
                dir: Some([1.0, 0.0]),
                walk: true,
                ..Input::default()
            },
        );
        assert_eq!(b.pos[2], 0.0);
        assert!(b.landing());
        assert!(!b.land_light);
    }

    #[test]
    fn a_jump_does_not_go_over_a_rail_it_clears() {
        let t = Tuning::default();
        let input = Input {
            dir: Some([1.0, 0.0]),
            ..Input::default()
        };
        // Up to the rail, then jumps, and again: 1.1 m would clear a rail 0.9 m high.
        let mut b = Body::new([3.0, 0.0, 2.0], 0.0);
        for _ in 0..120 {
            b.step(input, &Railed, &t, 1.0 / 60.0);
        }
        for _ in 0..3 {
            let up = jump_from(&mut b, &Railed, &t, input);
            assert!(up > 1.0, "{up}");
            assert!(b.pos[2] == 2.0 && b.pos[0] < 5.2, "{:?}", b.pos);
        }
    }

    /// A low floor and a high one with no walk between: a jump does not strand the body up there.
    struct Shelf;

    impl Ground for Shelf {
        fn floor(&self, x: f32, _: f32, max_z: f32) -> Option<f32> {
            let f = if x < 3.0 { 0.0 } else { 0.8 };
            let f = if f <= max_z { f } else { 0.0 };
            Some(f)
        }
        fn blocked(&self, _: [f32; 3], _: [f32; 3]) -> bool {
            false
        }
        fn way_back(&self, a: [f32; 3], b: [f32; 3]) -> bool {
            (a[2] - b[2]).abs() < 0.1
        }
    }

    #[test]
    fn a_jump_lands_nowhere_there_is_no_walk_back_from() {
        let t = Tuning::default();
        let input = Input {
            dir: Some([1.0, 0.0]),
            ..Input::default()
        };
        let mut b = Body::new([2.0, 0.0, 0.0], 0.0);
        jump_from(&mut b, &Shelf, &t, input);
        assert_eq!(b.pos[2], 0.0, "{:?}", b.pos);
    }

    #[test]
    fn stairs_rise_without_a_jump_or_a_dip() {
        let t = Tuning::default();
        // Steps 0.3 m long and 0.18 m high.
        let g = Hills(|x, _| Some((x / 0.3).floor().max(0.0) * 0.18));
        let mut b = Body::new([0.0, 0.0, 0.0], 0.0);
        let input = Input {
            dir: Some([1.0, 0.0]),
            ..Input::default()
        };
        let mut last = 0.0f32;
        for _ in 0..120 {
            b.step(input, &g, &t, 1.0 / 60.0);
            let dz = b.pos[2] - last;
            assert!(b.grounded);
            assert!(dz > -0.01, "it dipped by {dz} at {:?}", b.pos);
            // The stair's own slope at a sprint is 0.07 m a step, and the snap never lets it fall further behind than its lag.
            assert!(dz < t.snap_lag, "it jumped by {dz} at {:?}", b.pos);
            last = b.pos[2];
        }
    }

    /// A wall that is clear at chest height where the body sets out, and in the way lower down.
    struct LowWall;

    impl Ground for LowWall {
        fn floor(&self, x: f32, _: f32, max_z: f32) -> Option<f32> {
            let f = if x < 5.0 {
                Some(2.0)
            } else if x > 5.5 {
                Some(0.0)
            } else {
                None
            };
            f.filter(|f| *f <= max_z)
        }
        fn blocked(&self, _: [f32; 3], _: [f32; 3]) -> bool {
            false
        }
        fn wall(&self, a: [f32; 3], _: [f32; 3]) -> bool {
            a[2] < 2.5
        }
    }

    /// A floor only a little below, across a wide gap: it must not fall past it on the way over.
    struct WideGap;

    impl Ground for WideGap {
        fn floor(&self, x: f32, _: f32, max_z: f32) -> Option<f32> {
            let f = if x < 5.0 {
                Some(0.6)
            } else if x > 6.0 {
                Some(0.0)
            } else {
                None
            };
            f.filter(|f| *f <= max_z)
        }
        fn blocked(&self, _: [f32; 3], _: [f32; 3]) -> bool {
            false
        }
    }

    #[test]
    fn a_body_does_not_fall_past_a_near_floor_across_a_wide_gap() {
        let t = Tuning::default();
        for speed in [Some(false), Some(true)] {
            let mut b = Body::new([4.5, 0.0, 0.6], 0.0);
            let input = Input {
                dir: Some([1.0, 0.0]),
                walk: speed == Some(true),
                ..Input::default()
            };
            for _ in 0..240 {
                b.step(input, &WideGap, &t, 1.0 / 60.0);
                assert!(b.pos[2] > -0.01, "{:?}", b.pos);
            }
            assert!(b.grounded && b.pos[2] == 0.0, "{:?}", b.pos);
        }
    }

    /// In the real game this hung over nothing, 20 m down and falling, on Kashyyyk, Korriban and Unknown World.
    #[test]
    fn a_body_that_sets_out_over_a_gap_always_reaches_the_floor_beyond() {
        let t = Tuning::default();
        let mut b = Body::new([4.0, 0.0, 2.0], 0.0);
        let input = Input {
            dir: Some([1.0, 0.0]),
            ..Input::default()
        };
        let mut fell = false;
        for _ in 0..120 {
            b.step(input, &LowWall, &t, 1.0 / 60.0);
            fell |= !b.grounded;
            assert!(b.pos[2] > -0.01, "it went through the floor: {:?}", b.pos);
        }
        assert!(fell, "it went over");
        assert!(
            b.grounded && b.pos[2] == 0.0 && b.pos[0] > 5.5,
            "{:?}",
            b.pos
        );
    }
}
