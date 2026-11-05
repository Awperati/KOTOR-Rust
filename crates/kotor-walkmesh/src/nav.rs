//! Navigation over the walkable faces of several walkmeshes: a face graph welded across room borders,
//! A* over it, and string pulling (funnel) to get a short polyline. Doors are treated as open.

use crate::bwm::Bwm;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

type V3 = [f32; 3];

type EdgeKey = ([i32; 3], [i32; 3]);

/// Quantisation of vertex positions when welding edges between faces, in metres.
const WELD: f32 = 0.02;

#[derive(Clone, Copy, Debug)]
struct Link {
    to: usize,
    /// The shared edge, as endpoints in world space.
    a: V3,
    b: V3,
}

/// Walkable faces of one or more walkmeshes (world space) with their adjacency.
pub struct NavMesh {
    tris: Vec<[V3; 3]>,
    centroids: Vec<V3>,
    links: Vec<Vec<Link>>,
}

fn key(p: V3) -> [i32; 3] {
    p.map(|v| (v / WELD).round() as i32)
}

fn dist(a: V3, b: V3) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>().sqrt()
}

fn tri_height(t: &[V3; 3], x: f32, y: f32) -> Option<f32> {
    let (a, b, c) = (t[0], t[1], t[2]);
    let d = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
    if d.abs() < 1e-9 {
        return None;
    }
    let l1 = ((b[1] - c[1]) * (x - c[0]) + (c[0] - b[0]) * (y - c[1])) / d;
    let l2 = ((c[1] - a[1]) * (x - c[0]) + (a[0] - c[0]) * (y - c[1])) / d;
    let l3 = 1.0 - l1 - l2;
    let eps = -1e-4;
    (l1 >= eps && l2 >= eps && l3 >= eps).then(|| l1 * a[2] + l2 * b[2] + l3 * c[2])
}

/// Heap entry ordered by lowest `f` first.
struct Open {
    f: f32,
    node: usize,
}
impl PartialEq for Open {
    fn eq(&self, o: &Self) -> bool {
        self.f == o.f
    }
}
impl Eq for Open {}
impl PartialOrd for Open {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Open {
    fn cmp(&self, o: &Self) -> Ordering {
        o.f.total_cmp(&self.f)
    }
}

impl NavMesh {
    /// Collect the walkable faces of `meshes` and weld edges that coincide (within 2 cm), which joins
    /// faces inside a mesh and across the borders of neighbouring rooms.
    pub fn from_meshes(meshes: &[Bwm]) -> NavMesh {
        let mut tris = Vec::new();
        for m in meshes {
            for f in m.faces.iter().filter(|f| f.walkable()) {
                tris.push(f.vertices.map(|i| m.vertices[i as usize]));
            }
        }
        let centroids: Vec<V3> = tris
            .iter()
            .map(|t| [0, 1, 2].map(|k| (t[0][k] + t[1][k] + t[2][k]) / 3.0))
            .collect();
        let mut by_edge: HashMap<EdgeKey, Vec<(usize, V3, V3)>> = HashMap::new();
        for (i, t) in tris.iter().enumerate() {
            for e in 0..3 {
                let (a, b) = (t[e], t[(e + 1) % 3]);
                let (ka, kb) = (key(a), key(b));
                if ka == kb {
                    continue;
                }
                let k = if ka < kb { (ka, kb) } else { (kb, ka) };
                by_edge.entry(k).or_default().push((i, a, b));
            }
        }
        let mut links = vec![Vec::new(); tris.len()];
        for sharers in by_edge.values() {
            for (x, &(i, a, b)) in sharers.iter().enumerate() {
                for &(j, ..) in &sharers[x + 1..] {
                    if i != j {
                        links[i].push(Link { to: j, a, b });
                        links[j].push(Link { to: i, a, b });
                    }
                }
            }
        }
        NavMesh {
            tris,
            centroids,
            links,
        }
    }

    pub fn face_count(&self) -> usize {
        self.tris.len()
    }

    /// Centre of face `i`.
    pub fn face_centroid(&self, i: usize) -> V3 {
        self.centroids[i]
    }

    /// Faces adjacent to face `i`.
    pub fn neighbours(&self, i: usize) -> impl Iterator<Item = usize> + '_ {
        self.links[i].iter().map(|l| l.to)
    }

    /// Walkable face under `(x, y)` at or below `max_z`, choosing the highest one, and its height.
    pub fn locate(&self, x: f32, y: f32, max_z: f32) -> Option<(usize, f32)> {
        let mut best: Option<(usize, f32)> = None;
        for (i, t) in self.tris.iter().enumerate() {
            if x < t[0][0].min(t[1][0]).min(t[2][0]) - 1e-3
                || x > t[0][0].max(t[1][0]).max(t[2][0]) + 1e-3
                || y < t[0][1].min(t[1][1]).min(t[2][1]) - 1e-3
                || y > t[0][1].max(t[1][1]).max(t[2][1]) + 1e-3
            {
                continue;
            }
            if let Some(z) = tri_height(t, x, y) {
                if z <= max_z && best.is_none_or(|(_, bz)| z > bz) {
                    best = Some((i, z));
                }
            }
        }
        best
    }

    /// Height of the floor under `(x, y)` at or below `max_z`.
    pub fn height_at(&self, x: f32, y: f32, max_z: f32) -> Option<f32> {
        self.locate(x, y, max_z).map(|(_, z)| z)
    }

    /// Face containing `p`, or else the face with the nearest centroid within `snap` metres.
    fn face_near(&self, p: V3, snap: f32) -> Option<usize> {
        if let Some((i, _)) = self.locate(p[0], p[1], p[2] + 0.5) {
            return Some(i);
        }
        self.centroids
            .iter()
            .enumerate()
            .map(|(i, c)| (i, dist(*c, p)))
            .filter(|(_, d)| *d <= snap)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    /// Faces from the face under `from` to the face under `to` (A* over face centres), if connected.
    /// A point not on a face snaps to the nearest face centre within 1.5 m.
    pub fn face_path(&self, from: V3, to: V3) -> Option<Vec<usize>> {
        let (start, goal) = (self.face_near(from, 1.5)?, self.face_near(to, 1.5)?);
        let mut g = vec![f32::MAX; self.tris.len()];
        let mut came = vec![usize::MAX; self.tris.len()];
        let mut open = BinaryHeap::new();
        g[start] = 0.0;
        open.push(Open {
            f: dist(self.centroids[start], self.centroids[goal]),
            node: start,
        });
        while let Some(Open { node, .. }) = open.pop() {
            if node == goal {
                let mut path = vec![goal];
                while *path.last()? != start {
                    path.push(came[*path.last()?]);
                }
                path.reverse();
                return Some(path);
            }
            for l in &self.links[node] {
                let ng = g[node] + dist(self.centroids[node], self.centroids[l.to]);
                if ng < g[l.to] {
                    g[l.to] = ng;
                    came[l.to] = node;
                    let h = dist(self.centroids[l.to], self.centroids[goal]);
                    open.push(Open {
                        f: ng + h,
                        node: l.to,
                    });
                }
            }
        }
        None
    }

    /// Shortest-looking route from `from` to `to` as a polyline of world points: A* over faces, then string
    /// pulling through the shared edges so the route cuts corners instead of visiting face centres.
    /// Corners are exactly on face edges, so a mover following it brushes walls; there is no agent radius.
    pub fn find_path(&self, from: V3, to: V3) -> Option<Vec<V3>> {
        let faces = self.face_path(from, to)?;
        // Start and end sit on their faces (snapped heights).
        let zs = |p: V3, f: usize| self.height_in(f, p[0], p[1]).map_or(p, |z| [p[0], p[1], z]);
        let (start, end) = (zs(from, faces[0]), zs(to, *faces.last()?));
        // Portals between consecutive faces, ordered (left, right) for a traveller crossing them.
        let mut portals: Vec<(V3, V3)> = vec![(start, start)];
        for w in faces.windows(2) {
            let l = self.links[w[0]].iter().find(|l| l.to == w[1])?;
            let c = self.centroids[w[0]];
            let mid = [(l.a[0] + l.b[0]) / 2.0, (l.a[1] + l.b[1]) / 2.0];
            let dir = [mid[0] - c[0], mid[1] - c[1]];
            let side = |p: V3| dir[0] * (p[1] - c[1]) - dir[1] * (p[0] - c[0]);
            portals.push(if side(l.a) > 0.0 {
                (l.a, l.b)
            } else {
                (l.b, l.a)
            });
        }
        portals.push((end, end));
        Some(funnel(&portals))
    }

    fn height_in(&self, face: usize, x: f32, y: f32) -> Option<f32> {
        tri_height(&self.tris[face], x, y)
    }
}

/// Twice the signed area of triangle `abc` in the XY plane (positive when `c` is left of `a -> b`).
fn area2(a: V3, b: V3, c: V3) -> f32 {
    (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])
}

fn same(a: V3, b: V3) -> bool {
    (a[0] - b[0]).abs() < 1e-5 && (a[1] - b[1]).abs() < 1e-5
}

/// "Simple stupid funnel" string pulling over `(left, right)` portals; first and last portal are the end points.
fn funnel(portals: &[(V3, V3)]) -> Vec<V3> {
    let mut path = vec![portals[0].0];
    let (mut apex, mut left, mut right) = (portals[0].0, portals[0].0, portals[0].1);
    let (mut left_i, mut right_i) = (0usize, 0usize);
    let mut i = 1;
    let mut guard = 0;
    while i < portals.len() {
        guard += 1;
        if guard > portals.len() * portals.len() + 16 {
            break; // cannot loop forever on degenerate input
        }
        let (pl, pr) = portals[i];
        // Tighten from the right.
        if area2(apex, right, pr) >= 0.0 {
            if same(apex, right) || area2(apex, left, pr) < 0.0 {
                right = pr;
                right_i = i;
            } else {
                path.push(left);
                apex = left;
                right = apex;
                right_i = left_i;
                i = left_i + 1;
                continue;
            }
        }
        // Tighten from the left.
        if area2(apex, left, pl) <= 0.0 {
            if same(apex, left) || area2(apex, right, pl) > 0.0 {
                left = pl;
                left_i = i;
            } else {
                path.push(right);
                apex = right;
                left = apex;
                left_i = right_i;
                i = right_i + 1;
                continue;
            }
        }
        i += 1;
    }
    let end = portals[portals.len() - 1].0;
    if !same(*path.last().unwrap_or(&end), end) || path.len() == 1 {
        path.push(end);
    }
    path.dedup_by(|b, a| same(*a, *b));
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bwm::{Bwm, Face};

    /// A flat floor from rectangles, each split into two walkable triangles.
    fn floor(rects: &[[f32; 4]]) -> Bwm {
        let mut vertices = Vec::new();
        let mut faces = Vec::new();
        for r in rects {
            let base = vertices.len() as u32;
            vertices.extend([
                [r[0], r[1], 0.0],
                [r[2], r[1], 0.0],
                [r[2], r[3], 0.0],
                [r[0], r[3], 0.0],
            ]);
            for idx in [[0, 1, 2], [0, 2, 3]] {
                faces.push(Face {
                    vertices: idx.map(|i| base + i),
                    material: 1,
                    normal: [0.0, 0.0, 1.0],
                    plane_distance: 0.0,
                    adjacent: None,
                });
            }
        }
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
    fn straight_corridor_is_a_straight_line() {
        // Two rooms side by side, welded along x = 10.
        let a = floor(&[[0.0, 0.0, 10.0, 4.0]]);
        let b = floor(&[[10.0, 0.0, 20.0, 4.0]]);
        let nav = NavMesh::from_meshes(&[a, b]);
        assert_eq!(nav.face_count(), 4);
        let p = nav.find_path([1.0, 2.0, 0.0], [19.0, 2.0, 0.0]).unwrap();
        assert!(p.len() <= 3, "{p:?}");
        let len: f32 = p.windows(2).map(|w| dist(w[0], w[1])).sum();
        assert!((len - 18.0).abs() < 0.01, "{len}");
    }

    #[test]
    fn path_bends_around_a_corner_at_the_corner() {
        // An L: a horizontal arm and a vertical arm meeting in the corner square [0,4]x[0,4].
        let a = floor(&[[0.0, 0.0, 4.0, 4.0], [4.0, 0.0, 12.0, 4.0]]);
        let b = floor(&[[0.0, 4.0, 4.0, 12.0]]);
        let nav = NavMesh::from_meshes(&[a, b]);
        let p = nav.find_path([11.0, 3.5, 0.0], [3.5, 11.0, 0.0]).unwrap();
        // The only wall corner the route must brush is the inner corner at (4, 4).
        assert!(
            p.iter()
                .any(|q| (q[0] - 4.0).abs() < 1e-4 && (q[1] - 4.0).abs() < 1e-4),
            "{p:?}"
        );
        let len: f32 = p.windows(2).map(|w| dist(w[0], w[1])).sum();
        // Optimal: (11, 3.5) -> inner corner (4, 4) -> (3.5, 11), two legs of about 7.02 m.
        let optimal =
            dist([11.0, 3.5, 0.0], [4.0, 4.0, 0.0]) + dist([4.0, 4.0, 0.0], [3.5, 11.0, 0.0]);
        assert!((len - optimal).abs() < 0.01, "{len} vs {optimal}");
    }

    #[test]
    fn disconnected_floors_have_no_path() {
        let a = floor(&[[0.0, 0.0, 4.0, 4.0]]);
        let b = floor(&[[20.0, 0.0, 24.0, 4.0]]);
        let nav = NavMesh::from_meshes(&[a, b]);
        assert!(nav.find_path([1.0, 1.0, 0.0], [21.0, 1.0, 0.0]).is_none());
        // A point far from any floor does not snap.
        assert!(nav.find_path([1.0, 1.0, 0.0], [10.0, 1.0, 0.0]).is_none());
    }

    #[test]
    fn height_and_locate() {
        let nav = NavMesh::from_meshes(&[floor(&[[0.0, 0.0, 4.0, 4.0]])]);
        assert_eq!(nav.height_at(2.0, 2.0, 1.0), Some(0.0));
        assert_eq!(nav.height_at(9.0, 2.0, 1.0), None);
        assert_eq!(nav.height_at(2.0, 2.0, -1.0), None); // floor is above max_z
    }
}
