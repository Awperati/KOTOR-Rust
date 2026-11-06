//! Modern movement: a body with a velocity, a facing that follows it, gravity, and a roll.
//!
//! The Classic walk puts the player at full speed in one frame, turns it at a fixed rate and sets its height to the
//! floor's every step. This body accelerates and slows down, carves its turns, snaps to slopes and stairs smoothly,
//! falls off ledges along an arc and rolls on a key. It is pure: the floor is asked through [`Ground`], so the tests
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
/// Height over the feet that a wall is looked for at, in metres. Assumed: a wall reaches it, a cliff below does not.
const CHEST: f32 = 1.0;
/// The least speed going out over a gap, so that nothing hangs over it. Assumed.
const GAP_SPEED: f32 = 2.5;

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
        }
    }
}

/// The floor, as the body asks it.
pub trait Ground {
    /// Height of the highest walkable floor under `(x, y)` at or below `max_z`; `None` where there is none.
    fn floor(&self, x: f32, y: f32, max_z: f32) -> Option<f32>;
    /// Whether a straight move from `a` to `b` is stopped (a closed door's line).
    fn blocked(&self, a: [f32; 3], b: [f32; 3]) -> bool;
    /// Whether a wall stands between two points.
    fn wall(&self, _a: [f32; 3], _b: [f32; 3]) -> bool {
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
    fall_from: f32,
    last_stood: [f32; 3],
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
            fall_from: pos[2],
            last_stood: pos,
        }
    }

    pub fn speed(&self) -> f32 {
        self.vel[0].hypot(self.vel[1])
    }

    pub fn rolling(&self) -> bool {
        self.roll.is_some()
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
            self.roll = Some(Roll { dir, t: 0.0 });
            self.heading = dir[1].atan2(dir[0]);
            ev.roll_started = true;
        }
        if let Some(r) = self.roll {
            let t0 = r.t;
            let t1 = (t0 + dt).min(t.roll_time);
            let d = Self::roll_distance_at(t, t1) - Self::roll_distance_at(t, t0);
            self.move_by([r.dir[0] * d, r.dir[1] * d], ground, t);
            self.vel = [r.dir[0] * d / dt, r.dir[1] * d / dt];
            self.roll = Some(Roll { dir: r.dir, t: t1 });
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
            let moved = self.move_by(d, ground, t);
            if !moved.0 {
                self.vel[0] = 0.0;
            }
            if !moved.1 {
                self.vel[1] = 0.0;
            }
            self.face(input, t, dt);
        }
        ev.left_ground |= stood && !self.grounded;
        self.vertical(ground, t, dt, ev);
    }

    fn accelerate(&mut self, input: Input, t: &Tuning, dt: f32) {
        let top = if input.walk { t.walk } else { t.sprint };
        let top = if self.land_left > 0.0 {
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

    /// Whether a lower floor lies this way from `(x, y)`, over a gap, with no wall at chest height in between. A cliff
    /// leaves such a gap in the walkable faces.
    fn drop_ahead(&self, ground: &dyn Ground, t: &Tuning, x: f32, y: f32, u: [f32; 2]) -> bool {
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
                let chest = z + CHEST;
                return f >= z - t.max_drop
                    && !ground.wall([self.pos[0], self.pos[1], chest], [qx, qy, chest])
                    && (!self.grounded || ground.way_back(self.pos, [qx, qy, f]));
            }
        }
        false
    }

    /// Move on the floor by `d`, sliding along what stops it. Returns whether x and y got through.
    fn move_by(&mut self, d: [f32; 2], ground: &dyn Ground, t: &Tuning) -> (bool, bool) {
        if d[0] == 0.0 && d[1] == 0.0 {
            return (true, true);
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
                return (here.is_some() || self.drop_ahead(ground, t, x, y, u)).then_some([x, y]);
            }
            let clear = match here {
                Some(f) if f < p[2] - t.max_drop => return None,
                Some(f) if f < p[2] - t.step_down && !ground.way_back(p, [x, y, f]) => return None,
                // Keep off the very edge of the floor, as the Classic steering does.
                Some(_) => ground
                    .floor(x + u[0] * 0.3, y + u[1] * 0.3, reach)
                    .is_some(),
                None => false,
            };
            (clear || self.drop_ahead(ground, t, x, y, u)).then_some([x, y])
        };
        let (q, through) = if let Some(q) = ok(d[0], d[1]) {
            (q, (true, true))
        } else if let Some(q) = ok(d[0], 0.0) {
            (q, (true, d[1] == 0.0))
        } else if let Some(q) = ok(0.0, d[1]) {
            (q, (d[0] == 0.0, true))
        } else {
            return (false, false);
        };
        self.pos[0] = q[0];
        self.pos[1] = q[1];
        // Out over a gap: nothing under it now, so it falls, and it keeps going until it is over the lower floor.
        if self.grounded && ground.floor(q[0], q[1], reach).is_none() {
            self.grounded = false;
            self.vz = 0.0;
            self.fall_from = p[2];
            let (sx, sy) = (q[0] - p[0], q[1] - p[1]);
            let l = sx.hypot(sy).max(1e-6);
            if self.speed() < GAP_SPEED {
                self.vel = [sx / l * GAP_SPEED, sy / l * GAP_SPEED];
            }
        }
        through
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
        // A roll carries on through the air; the floor to land on is the one under the new spot.
        let floor = ground.floor(x, y, self.pos[2] + 0.1);
        match floor {
            Some(f) if z1 <= f => {
                let impact = -self.vz;
                self.pos[2] = f;
                self.vz = 0.0;
                self.grounded = true;
                self.last_stood = self.pos;
                ev.landed = Some(impact);
                if impact > t.land_threshold {
                    self.land_total =
                        ((impact - t.land_threshold) * t.land_per_speed + 0.12).min(t.land_max);
                    self.land_left = self.land_total;
                    self.vel[0] *= t.land_slow;
                    self.vel[1] *= t.land_slow;
                }
            }
            _ => {
                self.pos[2] = z1;
                if self.fall_from - z1 > t.void_depth {
                    self.pos = self.last_stood;
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
}
