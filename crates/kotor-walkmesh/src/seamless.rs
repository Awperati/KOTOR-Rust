//! Seamless areas: the rigid move that puts a neighbouring module beside this one, so that a door's far side is the
//! neighbour's arrival point, and the check that two modules never overlap in world space. See `docs/SEAMLESS.md`.

use crate::nav::WELD;
use crate::Bwm;
use std::collections::HashMap;

type V3 = [f32; 3];

/// A walkable triangle in world space.
pub type Tri = [V3; 3];

/// A turn about the vertical axis followed by a shift: `p' = R(yaw) p + shift`. The z shift is plain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rigid {
    pub yaw: f32,
    pub shift: V3,
}

impl Rigid {
    pub const IDENTITY: Rigid = Rigid {
        yaw: 0.0,
        shift: [0.0; 3],
    };

    /// Sine and cosine of the turn, exact for the quarter turns that placed modules mostly are: a vertex shared by two
    /// rooms must come out the same on both, and a quarter turn through the rounded value of pi/2 would not.
    fn trig(&self) -> (f32, f32) {
        let (s, c) = self.yaw.sin_cos();
        let snap = |v: f32| {
            if v.abs() < 1e-6 {
                0.0
            } else if (v.abs() - 1.0).abs() < 1e-6 {
                v.signum()
            } else {
                v
            }
        };
        (snap(s), snap(c))
    }

    pub fn apply(&self, p: V3) -> V3 {
        let (s, c) = self.trig();
        [
            c * p[0] - s * p[1] + self.shift[0],
            s * p[0] + c * p[1] + self.shift[1],
            p[2] + self.shift[2],
        ]
    }

    /// A direction (no shift).
    pub fn apply_dir(&self, d: V3) -> V3 {
        let (s, c) = self.trig();
        [c * d[0] - s * d[1], s * d[0] + c * d[1], d[2]]
    }

    pub fn inverse(&self) -> Rigid {
        let back = Rigid {
            yaw: -self.yaw,
            shift: [0.0; 3],
        };
        let t = back.apply(self.shift);
        Rigid {
            yaw: -self.yaw,
            shift: [-t[0], -t[1], -t[2]],
        }
    }

    /// First `other`, then `self`.
    pub fn after(&self, other: &Rigid) -> Rigid {
        Rigid {
            yaw: wrap(self.yaw + other.yaw),
            shift: self.apply(other.shift),
        }
    }
}

/// An angle in (-pi, pi].
pub fn wrap(a: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let r = (a + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI;
    if r <= -std::f32::consts::PI {
        std::f32::consts::PI
    } else {
        r
    }
}

/// Where the neighbour goes. `door` is our door and `out` the angle (radians, anticlockwise from +x) from our side
/// through it. `arrival` and `arrival_facing` are the waypoint the door's link names in the neighbour, and the angle it
/// faces: away from the neighbour's own door, so into the neighbour. The result puts that waypoint `gap` metres beyond
/// our door and turns the neighbour until it faces the way we walk.
pub fn place(door: V3, out: f32, arrival: V3, arrival_facing: f32, gap: f32) -> Rigid {
    let yaw = wrap(out - arrival_facing);
    let target = [
        door[0] + out.cos() * gap,
        door[1] + out.sin() * gap,
        door[2],
    ];
    let turn = Rigid {
        yaw,
        shift: [0.0; 3],
    };
    let r = turn.apply(arrival);
    Rigid {
        yaw,
        shift: [target[0] - r[0], target[1] - r[1], target[2] - r[2]],
    }
}

/// Put the neighbour's doorway on ours. `door` is our door and `out` the angle through it, into the neighbour. `theirs`
/// is the neighbour's own door back to us, and `theirs_out` the angle through it, from the neighbour toward us. The
/// result turns the neighbour until its way out points back along ours, and moves it until the two doors coincide.
pub fn join(door: V3, out: f32, theirs: V3, theirs_out: f32) -> Rigid {
    let turn = Rigid {
        yaw: wrap(out + std::f32::consts::PI - theirs_out),
        shift: [0.0; 3],
    };
    let at = turn.apply(theirs);
    let shift = [door[0] - at[0], door[1] - at[1], door[2] - at[2]];
    // Two rooms' faces are joined where their edges meet on the navigation mesh's weld grid. The edges of one module
    // that meet there do so in its own frame, so a move that is a quarter turn and a shift by whole grid cells keeps
    // the grid, and every join, exactly where it was. Any other move may land a seam across a cell line and open it.
    let grid = |v: f32| (v / WELD).round() * WELD;
    let quarter = std::f32::consts::FRAC_PI_2;
    let turned = (turn.yaw / quarter).round() * quarter;
    if (turn.yaw - turned).abs() < 1e-3 {
        return Rigid {
            yaw: wrap(turned),
            shift: shift.map(grid),
        };
    }
    Rigid {
        yaw: turn.yaw,
        shift,
    }
}

/// Where `p` lies relative to a doorway at `door` through which `out` leads: how far beyond it, and how far to the side.
pub fn beyond(door: V3, out: f32, p: V3) -> (f32, f32) {
    let (s, c) = out.sin_cos();
    let v = [p[0] - door[0], p[1] - door[1]];
    (v[0] * c + v[1] * s, (v[1] * c - v[0] * s).abs())
}

/// The walkable faces of a walkmesh in world space.
pub fn walkable_tris(m: &Bwm) -> Vec<Tri> {
    m.faces
        .iter()
        .filter(|f| f.walkable())
        .map(|f| f.vertices.map(|i| m.vertices[i as usize]))
        .collect()
}

pub fn move_tris(t: &[Tri], r: &Rigid) -> Vec<Tri> {
    t.iter().map(|tri| tri.map(|p| r.apply(p))).collect()
}

/// A walkmesh moved by `r`, for building a navigation mesh. Its search tree is dropped, which `NavMesh` does not use.
pub fn move_bwm(m: &Bwm, r: &Rigid) -> Bwm {
    let mut out = m.clone();
    out.vertices = m.vertices.iter().map(|p| r.apply(*p)).collect();
    out.position = r.apply(m.position);
    for f in &mut out.faces {
        f.normal = r.apply_dir(f.normal);
        let v = out.vertices[f.vertices[0] as usize];
        f.plane_distance = -(f.normal[0] * v[0] + f.normal[1] * v[1] + f.normal[2] * v[2]);
    }
    out.aabbs.clear();
    out
}

/// Two triangles on the same floor, separated by less than `eps` along some axis, do not count as overlapping: a
/// doorway's two sides touch.
fn tris_overlap(a: &Tri, b: &Tri, eps: f32) -> bool {
    // Each edge's normal in the plane.
    let axes = |t: &Tri| -> [[f32; 2]; 3] {
        std::array::from_fn(|i| {
            let (p, q) = (t[i], t[(i + 1) % 3]);
            [p[1] - q[1], q[0] - p[0]]
        })
    };
    for n in axes(a).into_iter().chain(axes(b)) {
        let len = n[0].hypot(n[1]);
        if len < 1e-9 {
            return false;
        }
        let proj = |t: &Tri| {
            t.iter()
                .map(|p| (p[0] * n[0] + p[1] * n[1]) / len)
                .fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)))
        };
        let (a0, a1) = proj(a);
        let (b0, b1) = proj(b);
        if (a1 - b0).min(b1 - a0) <= eps {
            return false;
        }
    }
    true
}

fn z_range(t: &Tri) -> (f32, f32) {
    (
        t[0][2].min(t[1][2]).min(t[2][2]),
        t[0][2].max(t[1][2]).max(t[2][2]),
    )
}

const GRID: f32 = 4.0;

/// The first pair of faces, one from each set, that overlap in plan and are within `z_tol` metres of each other in
/// height, when each penetrates the other by more than `eps`. Faces on different floors of the same shaft do not
/// count, which is what `z_tol` is for.
pub fn first_overlap(a: &[Tri], b: &[Tri], eps: f32, z_tol: f32) -> Option<(usize, usize)> {
    let mut grid: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
    let cells = |t: &Tri| {
        let lo = |k: usize| t.iter().map(|p| p[k]).fold(f32::MAX, f32::min);
        let hi = |k: usize| t.iter().map(|p| p[k]).fold(f32::MIN, f32::max);
        let (x0, x1) = ((lo(0) / GRID).floor() as i32, (hi(0) / GRID).floor() as i32);
        let (y0, y1) = ((lo(1) / GRID).floor() as i32, (hi(1) / GRID).floor() as i32);
        (x0..=x1).flat_map(move |x| (y0..=y1).map(move |y| (x, y)))
    };
    for (i, t) in a.iter().enumerate() {
        for c in cells(t) {
            grid.entry(c).or_default().push(i as u32);
        }
    }
    for (j, tb) in b.iter().enumerate() {
        let (bz0, bz1) = z_range(tb);
        for c in cells(tb) {
            for &i in grid.get(&c).into_iter().flatten() {
                let ta = &a[i as usize];
                let (az0, az1) = z_range(ta);
                if az0 > bz1 + z_tol || bz0 > az1 + z_tol {
                    continue;
                }
                if tris_overlap(ta, tb, eps) {
                    return Some((i as usize, j));
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    fn near(a: V3, b: V3) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-3)
    }

    #[test]
    fn inverse_undoes_apply() {
        let r = Rigid {
            yaw: 0.7,
            shift: [3.0, -4.0, 0.5],
        };
        let p = [1.0, 2.0, 3.0];
        assert!(near(r.inverse().apply(r.apply(p)), p));
        assert!(near(r.after(&r.inverse()).apply(p), p));
    }

    #[test]
    fn placed_arrival_sits_beyond_the_door_and_faces_out() {
        // Our door at (10, 0) and we walk along +x. The neighbour's waypoint is at (-5, 7) facing +y.
        let door = [10.0, 0.0, 1.0];
        let r = place(door, 0.0, [-5.0, 7.0, 1.0], FRAC_PI_2, 1.5);
        assert!(near(r.apply([-5.0, 7.0, 1.0]), [11.5, 0.0, 1.0]));
        // Its facing (+y) now points along +x.
        assert!(near(r.apply_dir([0.0, 1.0, 0.0]), [1.0, 0.0, 0.0]));
        // A point 3 m ahead of the waypoint in the neighbour is 3 m further out in ours.
        assert!(near(r.apply([-5.0, 10.0, 1.0]), [14.5, 0.0, 1.0]));
    }

    #[test]
    fn joined_doors_coincide_and_the_ways_through_oppose() {
        // Our door at (4, 0) leads east. The neighbour's door at (-2, 5) leads, for the neighbour, north-west.
        let ours = [4.0, 0.0, 0.5];
        let theirs = [-2.0, 5.0, 0.0];
        let r = join(ours, 0.0, theirs, FRAC_PI_2 + 0.5);
        assert!(near(r.apply(theirs), ours));
        let d = r.apply_dir([(FRAC_PI_2 + 0.5).cos(), (FRAC_PI_2 + 0.5).sin(), 0.0]);
        assert!(near(d, [-1.0, 0.0, 0.0]), "{d:?}");
    }

    #[test]
    fn beyond_measures_along_and_across() {
        let (along, across) = beyond([10.0, 0.0, 0.0], FRAC_PI_2, [11.5, 3.0, 0.0]);
        assert!((along - 3.0).abs() < 1e-4 && (across - 1.5).abs() < 1e-4);
        let (along, _) = beyond([10.0, 0.0, 0.0], 0.0, [8.0, 0.0, 0.0]);
        assert!((along + 2.0).abs() < 1e-4);
    }

    fn square(x: f32, y: f32, s: f32) -> Vec<Tri> {
        let (a, b, c, d) = (
            [x, y, 0.0],
            [x + s, y, 0.0],
            [x + s, y + s, 0.0],
            [x, y + s, 0.0],
        );
        vec![[a, b, c], [a, c, d]]
    }

    #[test]
    fn rooms_that_touch_do_not_overlap() {
        let a = square(0.0, 0.0, 4.0);
        let b = square(4.0, 0.0, 4.0);
        assert_eq!(first_overlap(&a, &b, 0.05, 1.0), None);
        // Sharing a corner or an edge with a 2 cm sliver of slop is still touching.
        let c = square(3.98, 0.0, 4.0);
        assert_eq!(first_overlap(&a, &c, 0.05, 1.0), None);
    }

    #[test]
    fn rooms_that_cross_do_overlap() {
        let a = square(0.0, 0.0, 4.0);
        let b = square(3.0, 1.0, 4.0);
        assert!(first_overlap(&a, &b, 0.05, 1.0).is_some());
        // A turned copy still overlaps where it should, and is clear once moved away.
        let turned = move_tris(
            &b,
            &Rigid {
                yaw: FRAC_PI_2,
                shift: [0.0; 3],
            },
        );
        assert!(first_overlap(&a, &turned, 0.05, 1.0).is_none());
        let moved = move_tris(
            &a,
            &Rigid {
                yaw: 0.3,
                shift: [100.0, 0.0, 0.0],
            },
        );
        assert!(first_overlap(&a, &moved, 0.05, 1.0).is_none());
    }

    #[test]
    fn another_floor_does_not_overlap_but_the_same_floor_does() {
        let a = square(0.0, 0.0, 4.0);
        let up = move_tris(
            &a,
            &Rigid {
                yaw: 0.0,
                shift: [0.0, 0.0, 6.0],
            },
        );
        assert_eq!(first_overlap(&a, &up, 0.05, 2.0), None);
        assert!(first_overlap(&a, &a, 0.05, 2.0).is_some());
    }

    #[test]
    fn a_big_triangle_inside_a_small_one_overlaps() {
        let big = vec![[[0.0, 0.0, 0.0], [20.0, 0.0, 0.0], [0.0, 20.0, 0.0]]];
        let small = vec![[[1.0, 1.0, 0.0], [2.0, 1.0, 0.0], [1.0, 2.0, 0.0]]];
        assert!(first_overlap(&big, &small, 0.05, 1.0).is_some());
    }
}
