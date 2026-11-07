//! Navigation over the walkable faces of several walkmeshes: a face graph welded across room borders,
//! A* over it, and string pulling (funnel) to get a short polyline. Doors are treated as open.

use crate::bwm::Bwm;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

type V3 = [f32; 3];

type EdgeKey = ([i32; 3], [i32; 3]);

/// Quantisation of vertex positions when welding edges between faces, in metres.
pub(crate) const WELD: f32 = 0.02;

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
    /// Faces covered by a closed door or other obstacle; routes cannot enter them. A count, so
    /// overlapping obstacles can be lifted independently.
    blocked: Vec<u16>,
    /// Lines that nothing may cross.
    lines: Vec<(V3, V3)>,
    /// Faces by the grid cells their bounds touch, so that finding the face under a point need not try them all.
    cells: HashMap<(i32, i32), Vec<u32>>,
    /// Upright faces that are not floor (walls and the sides of obstacles), for sight, and the cells they touch.
    walls: Vec<[V3; 3]>,
    wall_cells: HashMap<(i32, i32), Vec<u32>>,
    /// For each face, a number shared with every face a walk can reach from it.
    islands: Vec<u32>,
}

/// Size of a cell in `NavMesh::cells`, in metres.
const CELL: f32 = 4.0;

fn cell(x: f32, y: f32) -> (i32, i32) {
    ((x / CELL).floor() as i32, (y / CELL).floor() as i32)
}

fn key(p: V3) -> [i32; 3] {
    p.map(|v| (v / WELD).round() as i32)
}

fn dist(a: V3, b: V3) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>().sqrt()
}

fn dist_to_segment(p: V3, a: V3, b: V3) -> f32 {
    let ab: [f32; 3] = std::array::from_fn(|i| b[i] - a[i]);
    let len2 = ab.iter().map(|v| v * v).sum::<f32>();
    if len2 < 1e-9 {
        return dist(p, a);
    }
    let t = (0..3).map(|i| (p[i] - a[i]) * ab[i]).sum::<f32>() / len2;
    let t = t.clamp(0.0, 1.0);
    dist(p, std::array::from_fn(|i| a[i] + ab[i] * t))
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

/// Whether the segment `p` to `q` passes through the triangle (Moller and Trumbore).
fn segment_hits_triangle(p: V3, q: V3, a: V3, b: V3, c: V3) -> bool {
    let sub = |u: V3, v: V3| [u[0] - v[0], u[1] - v[1], u[2] - v[2]];
    let cross = |u: V3, v: V3| {
        [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ]
    };
    let dot = |u: V3, v: V3| u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
    let d = sub(q, p);
    let (e1, e2) = (sub(b, a), sub(c, a));
    let h = cross(d, e2);
    let det = dot(e1, h);
    if det.abs() < 1e-9 {
        return false;
    }
    let f = 1.0 / det;
    let s = sub(p, a);
    let u = f * dot(s, h);
    if !(0.0..=1.0).contains(&u) {
        return false;
    }
    let qv = cross(s, e1);
    let v = f * dot(d, qv);
    if v < 0.0 || u + v > 1.0 {
        return false;
    }
    let t = f * dot(e2, qv);
    (0.0..=1.0).contains(&t)
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
        let mut walls = Vec::new();
        for m in meshes {
            for f in m.faces.iter().filter(|f| f.walkable()) {
                tris.push(f.vertices.map(|i| m.vertices[i as usize]));
            }
            // Upright faces only: a floor that cannot be walked on is no wall to sight.
            for f in m
                .faces
                .iter()
                .filter(|f| !f.walkable() && f.normal[2].abs() < 0.7)
            {
                walls.push(f.vertices.map(|i| m.vertices[i as usize]));
            }
        }
        let mut wall_cells: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for (i, t) in walls.iter().enumerate() {
            let low = cell(
                t[0][0].min(t[1][0]).min(t[2][0]) - 1e-3,
                t[0][1].min(t[1][1]).min(t[2][1]) - 1e-3,
            );
            let high = cell(
                t[0][0].max(t[1][0]).max(t[2][0]) + 1e-3,
                t[0][1].max(t[1][1]).max(t[2][1]) + 1e-3,
            );
            for cx in low.0..=high.0 {
                for cy in low.1..=high.1 {
                    wall_cells.entry((cx, cy)).or_default().push(i as u32);
                }
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
        let mut islands = vec![u32::MAX; tris.len()];
        for start in 0..tris.len() {
            if islands[start] != u32::MAX {
                continue;
            }
            islands[start] = start as u32;
            let mut open = vec![start];
            while let Some(f) = open.pop() {
                for l in &links[f] {
                    if islands[l.to] == u32::MAX {
                        islands[l.to] = start as u32;
                        open.push(l.to);
                    }
                }
            }
        }
        let blocked = vec![0; tris.len()];
        let mut cells: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for (i, t) in tris.iter().enumerate() {
            let low = cell(
                t[0][0].min(t[1][0]).min(t[2][0]) - 1e-3,
                t[0][1].min(t[1][1]).min(t[2][1]) - 1e-3,
            );
            let high = cell(
                t[0][0].max(t[1][0]).max(t[2][0]) + 1e-3,
                t[0][1].max(t[1][1]).max(t[2][1]) + 1e-3,
            );
            for cx in low.0..=high.0 {
                for cy in low.1..=high.1 {
                    cells.entry((cx, cy)).or_default().push(i as u32);
                }
            }
        }
        NavMesh {
            tris,
            centroids,
            links,
            blocked,
            lines: Vec::new(),
            cells,
            walls,
            wall_cells,
            islands,
        }
    }

    /// Whether a walk could go from the floor under `a` to the floor under `b`, doors counting as open.
    pub fn same_island(&self, a: V3, b: V3) -> bool {
        let island = |p: V3| {
            self.locate(p[0], p[1], p[2] + 0.01)
                .map(|f| self.islands[f.0])
        };
        matches!((island(a), island(b)), (Some(x), Some(y)) if x == y)
    }

    /// Number of upright non-floor faces kept for sight.
    pub fn wall_count(&self) -> usize {
        self.walls.len()
    }

    /// Whether a wall stands between `p` and `q` (both at eye height).
    pub fn sight_blocked(&self, p: V3, q: V3) -> bool {
        let len = ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2)).sqrt();
        let steps = (len / (CELL * 0.5)).ceil().max(1.0) as usize;
        let mut seen: Vec<u32> = Vec::new();
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let c = cell(p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    let Some(list) = self.wall_cells.get(&(c.0 + dx, c.1 + dy)) else {
                        continue;
                    };
                    for &w in list {
                        if seen.contains(&w) {
                            continue;
                        }
                        seen.push(w);
                        let [a, b, c3] = self.walls[w as usize];
                        if segment_hits_triangle(p, q, a, b, c3) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Block every face whose centre is within `radius` (metres, in 3D) of `centre`. Returns how many faces
    /// were affected. Undo with [`NavMesh::unblock_disc`] using the same arguments.
    pub fn block_disc(&mut self, centre: V3, radius: f32) -> usize {
        let mut n = 0;
        for (i, c) in self.centroids.iter().enumerate() {
            if dist(*c, centre) <= radius {
                self.blocked[i] = self.blocked[i].saturating_add(1);
                n += 1;
            }
        }
        n
    }

    pub fn unblock_disc(&mut self, centre: V3, radius: f32) -> usize {
        let mut n = 0;
        for (i, c) in self.centroids.iter().enumerate() {
            if dist(*c, centre) <= radius && self.blocked[i] > 0 {
                self.blocked[i] -= 1;
                n += 1;
            }
        }
        n
    }

    /// Nothing may cross this line on the floor, as across a closed door. Undo with [`NavMesh::unblock_line`] using the
    /// same arguments. A line and not faces, because a door's faces can reach metres to either side of it.
    pub fn block_line(&mut self, a: V3, b: V3) {
        self.lines.push((a, b));
    }

    pub fn unblock_line(&mut self, a: V3, b: V3) {
        if let Some(i) = self.lines.iter().position(|l| *l == (a, b)) {
            self.lines.swap_remove(i);
        }
    }

    /// No blocked lines at all.
    pub fn clear_lines(&mut self) {
        self.lines.clear();
    }

    /// Whether going straight from `p` to `q` crosses a blocked line on its level.
    pub fn crosses_block(&self, p: V3, q: V3) -> bool {
        // More than a step, less than a storey: walkways pass over and under each other.
        const LEVEL: f32 = 1.5;
        let side =
            |u: V3, v: V3, r: V3| (v[0] - u[0]) * (r[1] - u[1]) - (v[1] - u[1]) * (r[0] - u[0]);
        self.lines.iter().any(|&(a, b)| {
            (p[2] - a[2]).abs() <= LEVEL
                && side(a, b, p) * side(a, b, q) < 0.0
                && side(p, q, a) * side(p, q, b) < 0.0
        })
    }

    /// Like `crosses_block`, for a body that moves: the step is looked at `margin` metres further on, so that it never ends
    /// on a blocked line or a hair short of one. A step that ended exactly on a closed door's line (a roll slid up to it, and
    /// the product of the two sides came to zero) was not a crossing, and the next step walked through the door.
    pub fn crosses_block_near(&self, p: V3, q: V3, margin: f32) -> bool {
        let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
        let l = dx.hypot(dy);
        if l < 1e-6 {
            return self.crosses_block(p, q);
        }
        let far = [q[0] + dx / l * margin, q[1] + dy / l * margin, q[2]];
        self.crosses_block(p, q) || self.crosses_block(p, far)
    }

    /// The faces a walker can reach from `from` without crossing a blocked line or entering a blocked face. Told apart
    /// from the rest, they show what lies behind a closed door. Empty when `from` is not on the floor.
    pub fn reachable_from(&self, from: V3) -> Vec<bool> {
        self.reach(from, false)
    }

    /// Like `reachable_from`, but with creatures not in the way: a character standing in a doorway shuts no door.
    pub fn reachable_from_floor(&self, from: V3) -> Vec<bool> {
        self.reach(from, true)
    }

    fn reach(&self, from: V3, ignore_discs: bool) -> Vec<bool> {
        let mut seen = vec![false; self.tris.len()];
        let Some(start) = self.face_beside(from, 1.5) else {
            return seen;
        };
        let mut entry = vec![from; self.tris.len()];
        let mut stack = vec![start];
        seen[start] = true;
        while let Some(n) = stack.pop() {
            for l in &self.links[n] {
                if seen[l.to] || (!ignore_discs && self.blocked[l.to] != 0) {
                    continue;
                }
                let door = [
                    (l.a[0] + l.b[0]) / 2.0,
                    (l.a[1] + l.b[1]) / 2.0,
                    (l.a[2] + l.b[2]) / 2.0,
                ];
                if self.crosses_block(entry[n], door) {
                    continue;
                }
                seen[l.to] = true;
                entry[l.to] = door;
                stack.push(l.to);
            }
        }
        seen
    }

    /// Join the face under `a` to the face under `b`, as across a doorway whose two sides are different walkmeshes.
    /// `portal` is the stretch of the doorway the join covers, for string pulling. Returns whether both faces exist.
    pub fn bridge(&mut self, a: V3, b: V3, portal: (V3, V3)) -> bool {
        let (Some((i, _)), Some((j, _))) = (
            self.locate(a[0], a[1], a[2] + 0.5),
            self.locate(b[0], b[1], b[2] + 0.5),
        ) else {
            return false;
        };
        if i == j || self.links[i].iter().any(|l| l.to == j) {
            return false;
        }
        self.links[i].push(Link {
            to: j,
            a: portal.0,
            b: portal.1,
        });
        self.links[j].push(Link {
            to: i,
            a: portal.0,
            b: portal.1,
        });
        true
    }

    /// Like `bridge`, joining the face under `a` among those `ok_a` accepts to the face under `b` among those `ok_b` does.
    pub fn bridge_between(
        &mut self,
        a: V3,
        b: V3,
        portal: (V3, V3),
        ok_a: &dyn Fn(usize) -> bool,
        ok_b: &dyn Fn(usize) -> bool,
    ) -> bool {
        let (Some((i, _)), Some((j, _))) = (
            self.locate_if(a[0], a[1], a[2] + 0.5, ok_a),
            self.locate_if(b[0], b[1], b[2] + 0.5, ok_b),
        ) else {
            return false;
        };
        if i == j || self.links[i].iter().any(|l| l.to == j) {
            return false;
        }
        self.links[i].push(Link {
            to: j,
            a: portal.0,
            b: portal.1,
        });
        self.links[j].push(Link {
            to: i,
            a: portal.0,
            b: portal.1,
        });
        true
    }

    /// Number of faces currently blocked.
    pub fn blocked_count(&self) -> usize {
        self.blocked.iter().filter(|&&b| b > 0).count()
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
        self.locate_if(x, y, max_z, &|_| true)
    }

    /// Like `locate`, among the faces `ok` accepts.
    pub fn locate_if(
        &self,
        x: f32,
        y: f32,
        max_z: f32,
        ok: &dyn Fn(usize) -> bool,
    ) -> Option<(usize, f32)> {
        let mut best: Option<(usize, f32)> = None;
        for &i in self.cells.get(&cell(x, y)).into_iter().flatten() {
            let i = i as usize;
            if !ok(i) {
                continue;
            }
            if let Some(z) = tri_height(&self.tris[i], x, y) {
                if z <= max_z && best.is_none_or(|(_, bz)| z > bz) {
                    best = Some((i, z));
                }
            }
        }
        best
    }

    /// Like `height_at`, but only where a character may stand: `None` on a blocked face.
    pub fn walkable_at(&self, x: f32, y: f32, max_z: f32) -> Option<f32> {
        self.locate(x, y, max_z)
            .filter(|(i, _)| self.blocked[*i] == 0)
            .map(|(_, z)| z)
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

    /// Face containing `p`, or else the face whose edge is nearest within `snap` metres. For whoever walks: a route that
    /// cuts a corner can leave them just off the floor, and a large face's centre can be far from its edge. Carth stood
    /// beside a table in `tar_m11aa` and could go nowhere.
    fn face_beside(&self, p: V3, snap: f32) -> Option<usize> {
        if let Some((i, _)) = self.locate(p[0], p[1], p[2] + 0.5) {
            return Some(i);
        }
        self.tris
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let d = (0..3)
                    .map(|e| dist_to_segment(p, t[e], t[(e + 1) % 3]))
                    .fold(f32::MAX, f32::min);
                (i, d)
            })
            .filter(|(_, d)| *d <= snap)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    /// Faces from the face under `from` to the face under `to` (A* over face centres), if connected.
    /// A start not on a face snaps to the nearest face edge within 1.5 m, and an end to the nearest face centre within
    /// 1.5 m. The last face may be blocked, as a door's is to whoever walks up to it, and two doors side by side can share one.
    pub fn face_path(&self, from: V3, to: V3) -> Option<Vec<usize>> {
        let (start, goal) = (self.face_beside(from, 1.5)?, self.face_near(to, 1.5)?);
        let mut g = vec![f32::MAX; self.tris.len()];
        let mut came = vec![usize::MAX; self.tris.len()];
        // Where a walker comes onto each face. A face's centre can lie across a blocked line from where the walker is.
        let mut entry = self.centroids.clone();
        entry[start] = from;
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
            let here = entry[node];
            for l in self.links[node]
                .iter()
                .filter(|l| self.blocked[l.to] == 0 || l.to == goal)
            {
                let door = [
                    (l.a[0] + l.b[0]) / 2.0,
                    (l.a[1] + l.b[1]) / 2.0,
                    (l.a[2] + l.b[2]) / 2.0,
                ];
                if self.crosses_block(here, door) {
                    continue;
                }
                let ng = g[node] + dist(self.centroids[node], self.centroids[l.to]);
                if ng < g[l.to] {
                    g[l.to] = ng;
                    came[l.to] = node;
                    entry[l.to] = door;
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
        let route = funnel(&portals);
        // Faces can reach across a blocked line, so the route itself has to stay on its side.
        (!route.windows(2).any(|w| self.crosses_block(w[0], w[1]))).then_some(route)
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
    fn a_blocked_line_cuts_routes_across_it_but_not_up_to_it() {
        let parts = [
            [0.0, 0.0, 10.0, 4.0],
            [10.0, 0.0, 20.0, 4.0],
            [20.0, 0.0, 30.0, 4.0],
        ];
        let mut nav = NavMesh::from_meshes(&parts.map(|p| floor(&[p])));
        let (from, on_it, past) = ([1.0, 2.0, 0.0], [11.0, 2.0, 0.0], [29.0, 2.0, 0.0]);
        let (a, b) = ([15.0, -1.0, 0.0], [15.0, 5.0, 0.0]);
        nav.block_line(a, b);
        assert!(nav.find_path(from, past).is_none());
        assert!(nav.find_path(from, on_it).is_some());
        nav.unblock_line(a, b);
        assert!(nav.find_path(from, past).is_some());
    }

    #[test]
    fn blocked_faces_stop_routes_until_unblocked() {
        let a = floor(&[[0.0, 0.0, 10.0, 4.0]]);
        let b = floor(&[[10.0, 0.0, 20.0, 4.0]]);
        let mut nav = NavMesh::from_meshes(&[a, b]);
        let (from, to) = ([1.0, 2.0, 0.0], [19.0, 2.0, 0.0]);
        assert!(nav.find_path(from, to).is_some());
        let n = nav.block_disc([10.0, 2.0, 0.0], 5.0);
        assert!(n >= 2 && nav.blocked_count() == n);
        assert!(
            nav.find_path(from, to).is_none(),
            "a blocked doorway must cut the route"
        );
        // Two obstacles on the same faces are lifted one at a time.
        nav.block_disc([10.0, 2.0, 0.0], 5.0);
        nav.unblock_disc([10.0, 2.0, 0.0], 5.0);
        assert!(nav.find_path(from, to).is_none());
        nav.unblock_disc([10.0, 2.0, 0.0], 5.0);
        assert_eq!(nav.blocked_count(), 0);
        assert!(nav.find_path(from, to).is_some());
    }

    #[test]
    fn someone_just_off_the_floor_walks_back_onto_it() {
        // Eight squares around a hole. The faces' centres are all further than 1.5 m from the point in the hole.
        let squares = [
            [0.0, 0.0, 4.0, 4.0],
            [4.0, 0.0, 8.0, 4.0],
            [8.0, 0.0, 12.0, 4.0],
            [0.0, 4.0, 4.0, 8.0],
            [8.0, 4.0, 12.0, 8.0],
            [0.0, 8.0, 4.0, 12.0],
            [4.0, 8.0, 8.0, 12.0],
            [8.0, 8.0, 12.0, 12.0],
        ];
        let nav = NavMesh::from_meshes(&[floor(&squares)]);
        let start = [4.3, 6.0, 0.0];
        assert_eq!(nav.height_at(start[0], start[1], 1.0), None);
        let route = nav.find_path(start, [10.0, 6.0, 0.0]).unwrap();
        // It goes round the hole, not across it.
        for w in route.windows(2) {
            let mid = [(w[0][0] + w[1][0]) / 2.0, (w[0][1] + w[1][1]) / 2.0];
            assert!(
                !(4.5..7.5).contains(&mid[0]) || !(4.5..7.5).contains(&mid[1]),
                "{route:?}"
            );
        }
    }

    #[test]
    fn height_and_locate() {
        let nav = NavMesh::from_meshes(&[floor(&[[0.0, 0.0, 4.0, 4.0]])]);
        assert_eq!(nav.height_at(2.0, 2.0, 1.0), Some(0.0));
        assert_eq!(nav.height_at(9.0, 2.0, 1.0), None);
        assert_eq!(nav.height_at(2.0, 2.0, -1.0), None); // floor is above max_z
    }
}
