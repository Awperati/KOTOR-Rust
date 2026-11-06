//! The rooms' walls, ceilings and floors as something a camera can bump into. The walkmesh has no ceiling, and tilted up the
//! camera rose through one and showed the room from outside.

use kotor_render::Scene;
use std::collections::HashMap;

/// Metres across a cell of the grid the triangles are filed in.
const CELL: f32 = 4.0;
/// A triangle this wide is a sky, not a wall.
const WIDEST: f32 = 100.0;

type V3 = [f32; 3];

#[derive(Default)]
pub(super) struct RoomShell {
    triangles: Vec<[V3; 3]>,
    cells: HashMap<(i32, i32), Vec<u32>>,
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

    /// How far along `dir`, a unit vector, the first surface is within `reach` metres of `from`.
    pub(super) fn first_hit(&self, from: V3, dir: V3, reach: f32) -> Option<f32> {
        let to = [0, 1].map(|k| from[k] + dir[k] * reach);
        let mut nearest: Option<f32> = None;
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
}
