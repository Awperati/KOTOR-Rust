//! The rooms' walls, ceilings and floors as something a camera can bump into. The walkmesh has no ceiling, and tilted up the
//! camera rose through one and showed the room from outside.

use kotor_render::Scene;
use std::collections::HashMap;
use std::sync::Arc;

/// Metres across a cell of the grid the triangles are filed in.
const CELL: f32 = 4.0;
/// A triangle this wide is a sky, not a wall.
const WIDEST: f32 = 100.0;

type V3 = [f32; 3];

#[derive(Default)]
pub(super) struct RoomShell {
    triangles: Vec<[V3; 3]>,
    cells: HashMap<(i32, i32), Vec<u32>>,
    /// Areas joined on to this one: their walls stop a camera too, though they are not in this area's walkmesh.
    joined: Vec<Arc<RoomShell>>,
}

fn cell(v: f32) -> i32 {
    (v / CELL).floor() as i32
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// How far along `dir` from `from` a triangle is, from either side.
fn hit(from: V3, dir: V3, t: &[V3; 3]) -> Option<f32> {
    let (e1, e2) = (sub(t[1], t[0]), sub(t[2], t[0]));
    let p = cross(dir, e2);
    let det = dot(e1, p);
    if det.abs() < 1e-7 {
        return None;
    }
    let to = sub(from, t[0]);
    let u = dot(to, p) / det;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = cross(to, e1);
    let v = dot(dir, q) / det;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let along = dot(e2, q) / det;
    (along > 0.0).then_some(along)
}

impl RoomShell {
    /// From a scene of rooms alone: a camera goes through what stands in them.
    pub(super) fn of(rooms: &Scene) -> RoomShell {
        let mut shell = RoomShell::default();
        for m in &rooms.meshes {
            for tri in &m.triangles {
                let t = tri.map(|i| m.positions[i as usize]);
                let low = |k: usize| t.iter().map(|p| p[k]).fold(f32::MAX, f32::min);
                let high = |k: usize| t.iter().map(|p| p[k]).fold(f32::MIN, f32::max);
                if high(0) - low(0) > WIDEST || high(1) - low(1) > WIDEST {
                    continue;
                }
                let n = shell.triangles.len() as u32;
                shell.triangles.push(t);
                for x in cell(low(0))..=cell(high(0)) {
                    for y in cell(low(1))..=cell(high(1)) {
                        shell.cells.entry((x, y)).or_default().push(n);
                    }
                }
            }
        }
        shell
    }

    /// Forget the areas joined on, for a new list of them.
    pub(super) fn unjoin(&mut self) {
        self.joined.clear();
    }

    /// An area joined on to this one, built on the thread that loaded it: the frame it joins pays nothing for it.
    pub(super) fn join(&mut self, other: Arc<RoomShell>) {
        self.joined.push(other);
    }

    /// How far along `dir`, a unit vector, the first surface is within `reach` metres of `from`.
    pub(super) fn first_hit(&self, from: V3, dir: V3, reach: f32) -> Option<f32> {
        let to = [0, 1].map(|k| from[k] + dir[k] * reach);
        let mut nearest = self
            .joined
            .iter()
            .filter_map(|o| o.first_hit(from, dir, reach))
            .min_by(f32::total_cmp);
        for x in cell(from[0].min(to[0]))..=cell(from[0].max(to[0])) {
            for y in cell(from[1].min(to[1]))..=cell(from[1].max(to[1])) {
                for n in self.cells.get(&(x, y)).into_iter().flatten() {
                    if let Some(d) = hit(from, dir, &self.triangles[*n as usize]) {
                        if d <= reach && nearest.is_none_or(|b| d < b) {
                            nearest = Some(d);
                        }
                    }
                }
            }
        }
        nearest
    }
}

/// Whether a triangle crosses the upright sheet on the line from `a` to `b`, between two heights.
fn crosses_sheet(t: &[V3; 3], a: [f32; 2], b: [f32; 2], low: f32, high: f32) -> bool {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let long = dx.hypot(dy);
    if long < 1e-4 {
        return false;
    }
    let (ux, uy) = (dx / long, dy / long);
    // Each corner: how far along the line, how far off it, and how high.
    let c = t.map(|p| {
        let (x, y) = (p[0] - a[0], p[1] - a[1]);
        (x * ux + y * uy, x * -uy + y * ux, p[2])
    });
    // Where the triangle's edges go through the sheet's plane.
    let mut points: Vec<(f32, f32)> = Vec::with_capacity(3);
    for k in 0..3 {
        let (p, q) = (c[k], c[(k + 1) % 3]);
        if (p.1 < 0.0) != (q.1 < 0.0) {
            let f = p.1 / (p.1 - q.1);
            points.push((p.0 + (q.0 - p.0) * f, p.2 + (q.2 - p.2) * f));
        }
    }
    let [p, q] = points[..] else {
        return false;
    };
    // That piece of the triangle, cut down to the sheet.
    let (mut from, mut to) = (0.0f32, 1.0f32);
    for (start, change, lo, hi) in [(p.0, q.0 - p.0, 0.0, long), (p.1, q.1 - p.1, low, high)] {
        if change.abs() < 1e-6 {
            if start < lo || start > hi {
                return false;
            }
            continue;
        }
        let (mut near, mut far) = ((lo - start) / change, (hi - start) / change);
        if near > far {
            std::mem::swap(&mut near, &mut far);
        }
        from = from.max(near);
        to = to.min(far);
        if from > to {
            return false;
        }
    }
    true
}

impl kotor_game::Barriers for RoomShell {
    fn across(&self, a: [f32; 2], b: [f32; 2], low: f32, high: f32) -> bool {
        for x in cell(a[0].min(b[0]))..=cell(a[0].max(b[0])) {
            for y in cell(a[1].min(b[1]))..=cell(a[1].max(b[1])) {
                for n in self.cells.get(&(x, y)).into_iter().flatten() {
                    if crosses_sheet(&self.triangles[*n as usize], a, b, low, high) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kotor_render::SceneMesh;

    /// A ceiling 3 m up, from -10 to 10 each way.
    fn ceiling() -> RoomShell {
        RoomShell::of(&Scene {
            meshes: vec![SceneMesh {
                positions: vec![
                    [-10.0, -10.0, 3.0],
                    [10.0, -10.0, 3.0],
                    [10.0, 10.0, 3.0],
                    [-10.0, 10.0, 3.0],
                ],
                triangles: vec![[0, 1, 2], [0, 2, 3]],
                ..SceneMesh::default()
            }],
            ..Scene::default()
        })
    }

    #[test]
    fn a_ray_stops_at_the_first_surface_within_its_reach() {
        let shell = ceiling();
        let up = [0.0, 0.0, 1.0];
        let d = shell.first_hit([1.0, 1.0, 1.0], up, 5.0).unwrap();
        assert!((d - 2.0).abs() < 1e-5, "{d}");
        assert_eq!(
            shell.first_hit([1.0, 1.0, 1.0], up, 1.0),
            None,
            "out of reach"
        );
        assert_eq!(
            shell.first_hit([1.0, 1.0, 1.0], [0.0, 0.0, -1.0], 5.0),
            None
        );
        assert_eq!(
            shell.first_hit([20.0, 1.0, 1.0], up, 5.0),
            None,
            "past its edge"
        );
    }

    #[test]
    fn a_camera_stops_at_an_area_joined_on_until_it_is_let_go() {
        let up = [0.0, 0.0, 1.0];
        let mut shell = RoomShell::default();
        let other = Scene {
            meshes: vec![SceneMesh {
                positions: vec![
                    [-10.0, -10.0, 2.0],
                    [10.0, -10.0, 2.0],
                    [10.0, 10.0, 2.0],
                    [-10.0, 10.0, 2.0],
                ],
                triangles: vec![[0, 1, 2], [0, 2, 3]],
                ..SceneMesh::default()
            }],
            ..Scene::default()
        };
        assert_eq!(shell.first_hit([0.0, 1.0, 0.0], up, 5.0), None);
        shell.join(Arc::new(RoomShell::of(&other)));
        let d = shell.first_hit([0.0, 1.0, 0.0], up, 5.0).unwrap();
        assert!((d - 2.0).abs() < 1e-3, "{d}");
        shell.unjoin();
        assert_eq!(shell.first_hit([0.0, 1.0, 0.0], up, 5.0), None);
    }

    /// A rail 2 cm thick and 0.9 m up, across the line of travel: rays at chosen heights go over and under it.
    fn rail() -> RoomShell {
        RoomShell::of(&Scene {
            meshes: vec![SceneMesh {
                positions: vec![
                    [1.0, -5.0, 0.89],
                    [1.0, 5.0, 0.89],
                    [1.0, 5.0, 0.91],
                    [1.0, -5.0, 0.91],
                ],
                triangles: vec![[0, 1, 2], [0, 2, 3]],
                ..SceneMesh::default()
            }],
            ..Scene::default()
        })
    }

    #[test]
    fn a_thin_rail_crosses_the_sheet_a_fall_looks_through() {
        use kotor_game::Barriers;
        let r = rail();
        assert!(r.across([0.0, 0.0], [2.0, 0.0], 0.25, 1.2));
        assert!(r.across([0.0, 0.3], [2.0, -0.2], 0.25, 1.2), "at an angle");
        assert!(
            r.across([2.0, 0.0], [0.0, 0.0], 0.25, 1.2),
            "from either side"
        );
        assert!(!r.across([0.0, 0.0], [0.9, 0.0], 0.25, 1.2), "short of it");
        assert!(
            !r.across([0.0, 0.0], [2.0, 0.0], 1.0, 1.2),
            "under the sheet"
        );
        assert!(
            !r.across([0.0, 0.0], [2.0, 0.0], 0.25, 0.8),
            "over the sheet"
        );
        assert!(!r.across([0.0, 6.0], [2.0, 6.0], 0.25, 1.2), "past its end");
    }
}
