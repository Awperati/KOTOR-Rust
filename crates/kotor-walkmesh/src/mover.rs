//! Click-to-move: a character that turns towards the next waypoint of a route and walks it.
//!
//! **Assumptions, not measured from the game:** walking speed defaults to 1.6 m/s (the `walkdist` of a
//! human body in `appearance.2da` over a one second walk cycle); the turn rate defaults to 4.0 rad/s
//! (about 230 degrees per second); a character only walks while within 30 degrees of its target and
//! otherwise turns on the spot. The original engine's values may differ and have not been checked.

use crate::NavMesh;

/// Movement constants. The defaults are assumptions (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoveParams {
    /// Metres per second.
    pub speed: f32,
    /// Radians per second.
    pub turn_rate: f32,
    /// Walk only when the heading is within this many radians of the target direction.
    pub face_tolerance: f32,
}

impl Default for MoveParams {
    fn default() -> Self {
        MoveParams {
            speed: 1.6,
            turn_rate: 4.0,
            face_tolerance: 30.0f32.to_radians(),
        }
    }
}

/// What a call to [`Mover::step`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveStatus {
    /// No route.
    Idle,
    /// Turning on the spot towards the next waypoint.
    Turning,
    /// Walking (possibly while still finishing a turn).
    Moving,
    /// Reached the destination on this step.
    Arrived,
}

/// Position, heading and route of one character on a [`NavMesh`].
#[derive(Clone, Debug)]
pub struct Mover {
    pub pos: [f32; 3],
    /// Radians, 0 = +x, counter-clockwise.
    pub heading: f32,
    route: Vec<[f32; 3]>,
    next: usize,
}

fn wrap(a: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let r = (a + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI;
    if r <= -std::f32::consts::PI {
        r + tau
    } else {
        r
    }
}

impl Mover {
    pub fn new(pos: [f32; 3], heading: f32) -> Mover {
        Mover {
            pos,
            heading,
            route: Vec::new(),
            next: 0,
        }
    }

    /// Plan a route to `to`. Returns false (and keeps the old route) if there is none.
    pub fn go_to(&mut self, nav: &NavMesh, to: [f32; 3]) -> bool {
        match nav.find_path(self.pos, to) {
            Some(route) => {
                self.route = route;
                self.next = 1; // the first point is where we already are
                true
            }
            None => false,
        }
    }

    /// Follow a ready-made route (its first point is taken to be where the mover already is).
    pub fn follow(&mut self, route: Vec<[f32; 3]>) {
        self.route = route;
        self.next = 1;
    }

    /// Stop and forget the route.
    pub fn stop(&mut self) {
        self.route.clear();
        self.next = 0;
    }

    /// Whether there is still route to walk.
    pub fn has_route(&self) -> bool {
        self.next < self.route.len()
    }

    /// Metres of route left, measured through the remaining waypoints.
    pub fn remaining(&self) -> f32 {
        let mut p = self.pos;
        let mut total = 0.0;
        for q in self.route.iter().skip(self.next) {
            total += ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2)).sqrt();
            p = *q;
        }
        total
    }

    /// Advance by `dt` seconds. Turns first (at most `turn_rate * dt`), then walks at most `speed * dt`
    /// towards the next waypoint, staying on the floor.
    pub fn step(&mut self, nav: &NavMesh, params: &MoveParams, dt: f32) -> MoveStatus {
        if self.next >= self.route.len() {
            return MoveStatus::Idle;
        }
        let target = self.route[self.next];
        let (dx, dy) = (target[0] - self.pos[0], target[1] - self.pos[1]);
        let dist = (dx * dx + dy * dy).sqrt();
        if dist > 1e-4 {
            let err = wrap(dy.atan2(dx) - self.heading);
            let turn = err.clamp(-params.turn_rate * dt, params.turn_rate * dt);
            self.heading = wrap(self.heading + turn);
            if (err - turn).abs() > params.face_tolerance {
                return MoveStatus::Turning;
            }
            let go = (params.speed * dt).min(dist);
            self.pos[0] += dx / dist * go;
            self.pos[1] += dy / dist * go;
            if let Some(z) = nav.height_at(self.pos[0], self.pos[1], self.pos[2] + 0.4) {
                self.pos[2] = z;
            }
            if go < dist - 1e-6 {
                return MoveStatus::Moving;
            }
        }
        // Reached this waypoint.
        self.pos[0] = target[0];
        self.pos[1] = target[1];
        self.next += 1;
        if self.next >= self.route.len() {
            MoveStatus::Arrived
        } else {
            MoveStatus::Moving
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bwm::{Bwm, Face};

    fn flat(x0: f32, y0: f32, x1: f32, y1: f32) -> Bwm {
        let vertices = vec![[x0, y0, 0.0], [x1, y0, 0.0], [x1, y1, 0.0], [x0, y1, 0.0]];
        let faces = [[0, 1, 2], [0, 2, 3]]
            .map(|v| Face {
                vertices: v,
                material: 1,
                normal: [0.0, 0.0, 1.0],
                plane_distance: 0.0,
                adjacent: None,
            })
            .to_vec();
        Bwm {
            kind: 1,
            position: [0.0; 3],
            use_positions: [[0.0; 3]; 4],
            vertices,
            faces,
            aabbs: vec![],
            edges: vec![],
            perimeters: vec![],
            tree_refit: false,
        }
    }

    #[test]
    fn wrap_is_in_range() {
        for a in [-10.0f32, -3.2, 0.0, 3.2, 9.0] {
            let w = wrap(a);
            assert!(
                w > -std::f32::consts::PI - 1e-6 && w <= std::f32::consts::PI + 1e-6,
                "{a} -> {w}"
            );
        }
        assert!((wrap(2.0 * std::f32::consts::PI)).abs() < 1e-5);
    }

    #[test]
    fn turns_in_place_then_walks_and_arrives() {
        let nav = NavMesh::from_meshes(&[flat(0.0, 0.0, 20.0, 4.0)]);
        // Facing +x but the goal is behind (-x): must turn about 180 degrees first.
        let mut m = Mover::new([10.0, 2.0, 0.0], 0.0);
        assert!(m.go_to(&nav, [2.0, 2.0, 0.0]));
        let p = MoveParams::default();
        let (mut t, mut turned_in_place, mut last) = (0.0f32, 0.0f32, MoveStatus::Idle);
        while last != MoveStatus::Arrived && t < 30.0 {
            let before = m.pos;
            last = m.step(&nav, &p, 0.05);
            if last == MoveStatus::Turning {
                assert_eq!(m.pos, before, "moved while turning");
                turned_in_place += 0.05;
            }
            t += 0.05;
        }
        assert_eq!(last, MoveStatus::Arrived);
        assert!((m.pos[0] - 2.0).abs() < 1e-3 && (m.pos[1] - 2.0).abs() < 1e-3);
        // About pi / 4 rad/s of turning before the first step, then 8 m at 1.6 m/s = 5 s of walking.
        assert!(
            turned_in_place > 0.5 && turned_in_place < 1.0,
            "{turned_in_place}"
        );
        assert!((t - 5.0 - turned_in_place).abs() < 0.3, "total {t}");
        assert_eq!(m.step(&nav, &p, 0.05), MoveStatus::Idle);
    }

    #[test]
    fn unreachable_goal_keeps_the_old_route() {
        let nav = NavMesh::from_meshes(&[flat(0.0, 0.0, 4.0, 4.0)]);
        let mut m = Mover::new([1.0, 1.0, 0.0], 0.0);
        assert!(!m.go_to(&nav, [50.0, 50.0, 0.0]));
        assert_eq!(m.step(&nav, &MoveParams::default(), 0.1), MoveStatus::Idle);
    }
}
