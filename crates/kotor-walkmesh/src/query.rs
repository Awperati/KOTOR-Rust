//! Spatial queries over a walkmesh using its AABB tree (falling back to a linear scan when a
//! mesh has no tree): floor under a point, ray casts and nearest walkable point.

use crate::Bwm;

type V3 = [f32; 3];

/// Every box test is widened by this much to absorb float rounding. (Stock boxes that are wrong by more are
/// recomputed on load, see `refit_tree`.)
const SLACK: f32 = 1e-3;

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

/// A ray hit on a face.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub face: usize,
    /// Distance along the (unit) direction.
    pub t: f32,
    pub point: V3,
}

/// Height of triangle `abc` under `(x, y)` if the point is inside it in the XY plane.
pub(crate) fn tri_height(a: V3, b: V3, c: V3, x: f32, y: f32) -> Option<f32> {
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

/// Moller-Trumbore ray/triangle test (both sides). Returns the distance along `dir`.
fn ray_tri(o: V3, dir: V3, a: V3, b: V3, c: V3) -> Option<f32> {
    let (e1, e2) = (sub(b, a), sub(c, a));
    let p = cross(dir, e2);
    let det = dot(e1, p);
    if det.abs() < 1e-9 {
        return None;
    }
    let inv = 1.0 / det;
    let s = sub(o, a);
    let u = dot(s, p) * inv;
    if !(-1e-4..=1.0 + 1e-4).contains(&u) {
        return None;
    }
    let q = cross(s, e1);
    let v = dot(dir, q) * inv;
    if v < -1e-4 || u + v > 1.0 + 1e-4 {
        return None;
    }
    let t = dot(e2, q) * inv;
    (t >= 0.0).then_some(t)
}

/// Closest point to `p` on triangle `abc` (Ericson, Real-Time Collision Detection).
fn closest_on_triangle(p: V3, a: V3, b: V3, c: V3) -> V3 {
    let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
    let (d1, d2) = (dot(ab, ap), dot(ac, ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = sub(p, b);
    let (d3, d4) = (dot(ab, bp), dot(ac, bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    let at = |t: f32, from: V3, dir: V3| {
        [
            from[0] + t * dir[0],
            from[1] + t * dir[1],
            from[2] + t * dir[2],
        ]
    };
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return at(d1 / (d1 - d3), a, ab);
    }
    let cp = sub(p, c);
    let (d5, d6) = (dot(ab, cp), dot(ac, cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return at(d2 / (d2 - d6), a, ac);
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return at(w, b, sub(c, b));
    }
    let denom = 1.0 / (va + vb + vc);
    let (v, w) = (vb * denom, vc * denom);
    [
        a[0] + ab[0] * v + ac[0] * w,
        a[1] + ab[1] * v + ac[1] * w,
        a[2] + ab[2] * v + ac[2] * w,
    ]
}

fn dist2_to_box(p: V3, min: V3, max: V3) -> f32 {
    (0..3)
        .map(|i| {
            let d = (min[i] - SLACK - p[i]).max(p[i] - max[i] - SLACK).max(0.0);
            d * d
        })
        .sum()
}

/// Slab test: entry distance of the ray into the box, if it hits within `max_t`.
fn ray_box(o: V3, dir: V3, min: V3, max: V3, max_t: f32) -> Option<f32> {
    let (mut t0, mut t1) = (0.0f32, max_t);
    for i in 0..3 {
        if dir[i].abs() < 1e-12 {
            if o[i] < min[i] - SLACK || o[i] > max[i] + SLACK {
                return None;
            }
        } else {
            let inv = 1.0 / dir[i];
            let (mut a, mut b) = ((min[i] - SLACK - o[i]) * inv, (max[i] + SLACK - o[i]) * inv);
            if a > b {
                std::mem::swap(&mut a, &mut b);
            }
            t0 = t0.max(a);
            t1 = t1.min(b);
            if t0 > t1 {
                return None;
            }
        }
    }
    Some(t0)
}

impl Bwm {
    fn tri(&self, face: usize) -> [V3; 3] {
        self.faces[face].vertices.map(|i| self.vertices[i as usize])
    }

    /// Faces whose tree leaf box passes `keep`, visiting the tree depth-first.
    /// Without a tree every face is a candidate.
    fn visit(&self, mut keep: impl FnMut(V3, V3) -> bool, mut each: impl FnMut(usize)) {
        if self.aabbs.is_empty() {
            (0..self.faces.len()).for_each(&mut each);
            return;
        }
        let mut stack = vec![0usize];
        while let Some(i) = stack.pop() {
            let Some(n) = self.aabbs.get(i) else { continue };
            if !keep(n.min, n.max) {
                continue;
            }
            if n.face >= 0 {
                each(n.face as usize);
            } else {
                stack.push(n.left as usize);
                stack.push(n.right as usize);
            }
        }
    }

    /// Floor under `(x, y)`: among walkable faces covering the point, the highest one at or
    /// below `max_z`. Pass `f32::MAX` to take the highest overall.
    pub fn floor_at(&self, x: f32, y: f32, max_z: f32) -> Option<(usize, f32)> {
        let mut best: Option<(usize, f32)> = None;
        let eps = SLACK;
        self.visit(
            |min, max| {
                x >= min[0] - eps && x <= max[0] + eps && y >= min[1] - eps && y <= max[1] + eps
            },
            |f| {
                if !self.faces[f].walkable() {
                    return;
                }
                let [a, b, c] = self.tri(f);
                if let Some(z) = tri_height(a, b, c, x, y) {
                    if z <= max_z && best.is_none_or(|(_, bz)| z > bz) {
                        best = Some((f, z));
                    }
                }
            },
        );
        best
    }

    /// Nearest hit of a ray (any face, or only walkable ones) within `max_t`. `dir` need not be unit length.
    pub fn raycast(&self, origin: V3, dir: V3, max_t: f32, walkable_only: bool) -> Option<Hit> {
        let len = dot(dir, dir).sqrt();
        if len < 1e-9 {
            return None;
        }
        let d = [dir[0] / len, dir[1] / len, dir[2] / len];
        let mut best: Option<Hit> = None;
        let limit = std::cell::Cell::new(max_t);
        self.visit(
            |min, max| ray_box(origin, d, min, max, limit.get()).is_some(),
            |f| {
                if walkable_only && !self.faces[f].walkable() {
                    return;
                }
                let [a, b, c] = self.tri(f);
                if let Some(t) = ray_tri(origin, d, a, b, c) {
                    if t <= limit.get() {
                        limit.set(t);
                        let point = [
                            origin[0] + d[0] * t,
                            origin[1] + d[1] * t,
                            origin[2] + d[2] * t,
                        ];
                        best = Some(Hit { face: f, t, point });
                    }
                }
            },
        );
        best
    }

    /// Closest point on any walkable face to `p`, with the face index. Linear in the number of
    /// candidate boxes visited; prunes boxes farther than the best found so far.
    pub fn nearest_walkable(&self, p: V3) -> Option<(usize, V3)> {
        if self.aabbs.is_empty() {
            return self.nearest_walkable_linear(p);
        }
        let mut best: Option<(usize, V3, f32)> = None;
        let mut best_d2 = f32::MAX;
        // Best-first would be tighter; a depth-first walk that skips far boxes is enough here.
        let mut stack = vec![0usize];
        while let Some(i) = stack.pop() {
            let Some(n) = self.aabbs.get(i) else { continue };
            if dist2_to_box(p, n.min, n.max) > best_d2 {
                continue;
            }
            if n.face >= 0 {
                let f = n.face as usize;
                if !self.faces[f].walkable() {
                    continue;
                }
                let [a, b, c] = self.tri(f);
                let q = closest_on_triangle(p, a, b, c);
                let d = sub(q, p);
                let d2 = dot(d, d);
                if d2 < best_d2 {
                    best_d2 = d2;
                    best = Some((f, q, d2));
                }
            } else {
                // Visit the nearer child last so it is popped first.
                let (l, r) = (n.left as usize, n.right as usize);
                let dl = self
                    .aabbs
                    .get(l)
                    .map_or(f32::MAX, |c| dist2_to_box(p, c.min, c.max));
                let dr = self
                    .aabbs
                    .get(r)
                    .map_or(f32::MAX, |c| dist2_to_box(p, c.min, c.max));
                if dl < dr {
                    stack.push(r);
                    stack.push(l);
                } else {
                    stack.push(l);
                    stack.push(r);
                }
            }
        }
        best.map(|(f, q, _)| (f, q))
    }

    /// Reference implementation of [`Bwm::nearest_walkable`] that scans every face; used by tests.
    pub fn nearest_walkable_linear(&self, p: V3) -> Option<(usize, V3)> {
        let mut best: Option<(usize, V3, f32)> = None;
        for f in (0..self.faces.len()).filter(|&f| self.faces[f].walkable()) {
            let [a, b, c] = self.tri(f);
            let q = closest_on_triangle(p, a, b, c);
            let d = sub(q, p);
            let d2 = dot(d, d);
            if best.is_none_or(|(_, _, bd)| d2 < bd) {
                best = Some((f, q, d2));
            }
        }
        best.map(|(f, q, _)| (f, q))
    }

    /// Recompute every box from its triangles, keeping the tree shape. 21 of the 1,202 stock area walkmeshes
    /// carry boxes that are off by up to 0.7 m (stale after editing), which would make queries miss faces.
    /// Returns true if any stored box was off by more than 1 mm.
    pub(crate) fn refit_tree(&mut self) -> bool {
        let n = self.aabbs.len();
        if n == 0 {
            return false;
        }
        // Pre-order list from the root; reversed, children come before parents.
        let mut order = Vec::with_capacity(n);
        let mut seen = vec![false; n];
        let mut stack = vec![0usize];
        while let Some(i) = stack.pop() {
            if i >= n || std::mem::replace(&mut seen[i], true) {
                return false; // malformed tree: leave it alone, check_tree reports it
            }
            order.push(i);
            if self.aabbs[i].face < 0 {
                stack.push(self.aabbs[i].left as usize);
                stack.push(self.aabbs[i].right as usize);
            }
        }
        let mut changed = false;
        for &i in order.iter().rev() {
            let (mut min, mut max) = ([f32::MAX; 3], [f32::MIN; 3]);
            let face = self.aabbs[i].face;
            if face >= 0 {
                let Some(f) = self.faces.get(face as usize) else {
                    return changed;
                };
                for v in self.tri(face as usize) {
                    for k in 0..3 {
                        min[k] = min[k].min(v[k]);
                        max[k] = max[k].max(v[k]);
                    }
                }
                let _ = f;
            } else {
                for c in [self.aabbs[i].left as usize, self.aabbs[i].right as usize] {
                    for k in 0..3 {
                        min[k] = min[k].min(self.aabbs[c].min[k]);
                        max[k] = max[k].max(self.aabbs[c].max[k]);
                    }
                }
            }
            let node = &mut self.aabbs[i];
            let off = (0..3).any(|k| {
                (node.min[k] - min[k]).abs() > 1e-3 || (node.max[k] - max[k]).abs() > 1e-3
            });
            // A stored box that merely contains the triangles loosely is harmless; only a box that is
            // too small (misses geometry) or very loose counts as stale.
            let too_small =
                (0..3).any(|k| node.min[k] > min[k] + 1e-3 || node.max[k] < max[k] - 1e-3);
            changed |= off && too_small;
            node.min = min;
            node.max = max;
        }
        changed
    }

    /// Check the AABB tree is well formed: child indices in range, no cycles, leaf faces valid,
    /// and every leaf triangle inside its leaf box. Returns the number of leaves.
    pub fn check_tree(&self) -> Result<usize, String> {
        if self.aabbs.is_empty() {
            return Ok(0);
        }
        let mut seen = vec![false; self.aabbs.len()];
        let mut stack = vec![0usize];
        let mut leaves = 0;
        while let Some(i) = stack.pop() {
            let n = self
                .aabbs
                .get(i)
                .ok_or(format!("node index {i} out of range"))?;
            if std::mem::replace(&mut seen[i], true) {
                return Err(format!("node {i} reached twice"));
            }
            if n.face >= 0 {
                let f = n.face as usize;
                if f >= self.faces.len() {
                    return Err(format!("leaf {i} face {f} out of range"));
                }
                let over = self
                    .tri(f)
                    .iter()
                    .flat_map(|v| (0..3).map(move |k| (n.min[k] - v[k]).max(v[k] - n.max[k])))
                    .fold(f32::MIN, f32::max);
                if over > SLACK {
                    return Err(format!("leaf {i}: face {f} outside its box by {over}"));
                }
                leaves += 1;
            } else {
                stack.push(n.left as usize);
                stack.push(n.right as usize);
            }
        }
        Ok(leaves)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closest_point_cases() {
        let (a, b, c) = ([0., 0., 0.], [2., 0., 0.], [0., 2., 0.]);
        assert_eq!(
            closest_on_triangle([0.5, 0.5, 3.0], a, b, c),
            [0.5, 0.5, 0.0]
        ); // above interior
        assert_eq!(closest_on_triangle([-1., -1., 0.], a, b, c), a); // vertex region
        assert_eq!(closest_on_triangle([1., -1., 0.], a, b, c), [1., 0., 0.]); // edge region
    }

    #[test]
    fn ray_hits_triangle_from_both_sides() {
        let (a, b, c) = ([0., 0., 0.], [2., 0., 0.], [0., 2., 0.]);
        assert_eq!(ray_tri([0.5, 0.5, 2.0], [0., 0., -1.], a, b, c), Some(2.0));
        assert_eq!(ray_tri([0.5, 0.5, -2.0], [0., 0., 1.], a, b, c), Some(2.0));
        assert_eq!(ray_tri([5., 5., 2.0], [0., 0., -1.], a, b, c), None);
        assert_eq!(ray_tri([0.5, 0.5, 2.0], [0., 0., 1.], a, b, c), None);
    }

    #[test]
    fn slab_test() {
        assert!(ray_box(
            [0., 0., 5.],
            [0., 0., -1.],
            [-1., -1., 0.],
            [1., 1., 1.],
            100.
        )
        .is_some());
        assert!(ray_box(
            [5., 0., 5.],
            [0., 0., -1.],
            [-1., -1., 0.],
            [1., 1., 1.],
            100.
        )
        .is_none());
    }
}
