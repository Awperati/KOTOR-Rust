//! Creatures of a joined neighbour that walk a route before the player gets there. No scripts and no world: a route of
//! waypoints (`WP_<tag>_01`, `_02`...) walked in a loop, standing a few seconds at each, as a pure function of the clock.
//! See `docs/SEAMLESS_CREATURES.md`.

use crate::seamless_world::Figure;

/// How fast a creature walks, in metres a second.
pub const SPEED: f32 = 1.4;
/// How near the player must be for a creature standing still to turn to look, in metres.
pub const LOOK_RANGE: f32 = 6.0;
/// How fast a creature turns, in radians a second.
pub const TURN: f32 = 4.0;

/// Where a figure is, and which way it faces, at a time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub at: [f32; 3],
    pub heading: f32,
    pub moving: bool,
}

fn hash(tag: &str) -> u32 {
    tag.bytes().fold(2166136261u32, |h, b| {
        (h ^ b.to_ascii_lowercase() as u32).wrapping_mul(16777619)
    })
}

/// How long a creature stands at each waypoint, in seconds: 2 to 5, its own.
fn wait_of(f: &Figure) -> f32 {
    2.0 + (hash(&f.tag) % 30) as f32 / 10.0
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn bearing(a: [f32; 3], b: [f32; 3]) -> f32 {
    (b[1] - a[1]).atan2(b[0] - a[0])
}

/// The turn from `from` towards `to` by at most `step`, the short way round.
pub fn turn_towards(from: f32, to: f32, step: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let d = (to - from + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI;
    from + d.clamp(-step, step)
}

/// The pose of `f` at clock time `t`. A figure with no route stands where it is.
pub fn pose(f: &Figure, t: f32) -> Pose {
    let still = Pose {
        at: f.at,
        heading: f.heading,
        moving: false,
    };
    if f.route.is_empty() {
        return still;
    }
    let mut pts = vec![f.at];
    pts.extend(f.route.iter().copied());
    let wait = wait_of(f);
    let n = pts.len();
    let leg = |i: usize| dist(pts[i], pts[(i + 1) % n]) / SPEED;
    let cycle: f32 = (0..n).map(|i| wait + leg(i)).sum();
    if cycle <= 0.0 {
        return still;
    }
    let phase = (hash(&f.tag) % 1000) as f32 / 1000.0 * cycle;
    let mut u = (t + phase).rem_euclid(cycle);
    for i in 0..n {
        let arrived = if i == 0 {
            f.heading
        } else {
            bearing(pts[i - 1], pts[i])
        };
        if u < wait {
            return Pose {
                at: pts[i],
                heading: arrived,
                moving: false,
            };
        }
        u -= wait;
        let len = leg(i);
        if u < len {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            let k = if len > 0.0 { u / len } else { 0.0 };
            return Pose {
                at: [
                    a[0] + (b[0] - a[0]) * k,
                    a[1] + (b[1] - a[1]) * k,
                    a[2] + (b[2] - a[2]) * k,
                ],
                heading: bearing(a, b),
                moving: true,
            };
        }
        u -= len;
    }
    still
}

/// The heading a figure takes, one step on: towards the player when it stands still within `LOOK_RANGE` of them, and
/// along its pose otherwise. `now` is the heading it has, `player` is in the same space as the pose.
pub fn facing(p: &Pose, now: f32, player: [f32; 3], dt: f32) -> f32 {
    let want = if !p.moving && dist(p.at, player) < LOOK_RANGE {
        bearing(p.at, player)
    } else {
        p.heading
    };
    turn_towards(now, want, TURN * dt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walker(route: Vec<[f32; 3]>) -> Figure {
        Figure {
            appearance: 0,
            template: "t".into(),
            armour: None,
            at: [0.0, 0.0, 0.0],
            heading: 1.0,
            underwear: String::new(),
            tag: "guard".into(),
            route,
        }
    }

    #[test]
    fn a_figure_with_no_route_stands_still() {
        let f = walker(vec![]);
        for t in [0.0, 5.0, 99.0] {
            let p = pose(&f, t);
            assert_eq!((p.at, p.heading, p.moving), ([0.0; 3], 1.0, false));
        }
    }

    #[test]
    fn a_walker_goes_along_its_route_and_loops() {
        let f = walker(vec![[10.0, 0.0, 0.0], [10.0, 10.0, 0.0]]);
        let mut seen_moving = false;
        let mut far = 0.0f32;
        let mut last = pose(&f, 0.0).at;
        for i in 1..4000 {
            let p = pose(&f, i as f32 * 0.05);
            seen_moving |= p.moving;
            far = far.max(dist(p.at, [0.0; 3]));
            // A walker never steps faster than its speed.
            assert!(dist(p.at, last) <= SPEED * 0.05 + 1e-3, "step at {i}");
            last = p.at;
        }
        assert!(seen_moving);
        assert!(far > 13.0, "reaches the far waypoint: {far}");
        // Loops: the pose repeats after the whole cycle.
        let cycle = 3.0 * wait_of(&f) + (10.0 + 10.0 + 14.142_136) / SPEED;
        let (a, b) = (pose(&f, 7.0), pose(&f, 7.0 + cycle));
        assert!(dist(a.at, b.at) < 1e-2, "{a:?} {b:?}");
    }

    #[test]
    fn a_standing_walker_turns_to_the_player_in_range_and_not_out_of_it() {
        let f = walker(vec![[10.0, 0.0, 0.0]]);
        let t = (0..2000)
            .map(|i| i as f32 * 0.05)
            .find(|t| !pose(&f, *t).moving)
            .expect("it stands somewhere");
        let p = pose(&f, t);
        let near = [p.at[0], p.at[1] + 3.0, 0.0];
        let mut h = p.heading;
        for _ in 0..100 {
            h = facing(&p, h, near, 0.05);
        }
        assert!(
            (h - std::f32::consts::FRAC_PI_2).abs() < 0.01,
            "faces +y: {h}"
        );
        let far = [p.at[0], p.at[1] + 30.0, 0.0];
        let mut h = p.heading + 0.5;
        for _ in 0..100 {
            h = facing(&p, h, far, 0.05);
        }
        assert!((h - p.heading).abs() < 0.01, "back to its own: {h}");
    }
}
