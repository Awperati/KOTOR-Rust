//! Animation sampling: controller keys to a pose, and a pose to world transforms.
//!
//! Position (controller 8) and orientation (controller 20) are sampled; scale and the light/emitter
//! controllers are ignored. Nodes an animation does not key keep their rest transform. Position keys
//! are offsets added to the rest position.

use crate::obj::{apply, identity, matrix, mul, Mat};
use crate::{Animation, Controller, Model, Node};
use std::collections::HashMap;

/// Decode one packed orientation key: returns `[w, x, y, z]` and the squared length of the
/// decoded x, y, z. A correct layout never gives a squared length above 1 (apart from rounding),
/// which is what the tests check.
pub fn unpack_quat(word: f32) -> ([f32; 4], f32) {
    let bits = word.to_bits();
    let x = (bits & 0x7FF) as f32 / 1023.0 - 1.0;
    let y = ((bits >> 11) & 0x7FF) as f32 / 1023.0 - 1.0;
    let z = (bits >> 22) as f32 / 511.0 - 1.0;
    let n2 = x * x + y * y + z * z;
    let w = if n2 < 1.0 { (1.0 - n2).sqrt() } else { 0.0 };
    ([w, x, y, z], n2)
}

fn slerp(a: [f32; 4], mut b: [f32; 4], t: f32) -> [f32; 4] {
    let mut dot: f32 = (0..4).map(|i| a[i] * b[i]).sum();
    if dot < 0.0 {
        dot = -dot;
        b = b.map(|v| -v);
    }
    let (wa, wb) = if dot > 0.9995 {
        (1.0 - t, t)
    } else {
        let theta = dot.clamp(-1.0, 1.0).acos();
        let s = theta.sin();
        (((1.0 - t) * theta).sin() / s, (t * theta).sin() / s)
    };
    let q = [0, 1, 2, 3].map(|i| wa * a[i] + wb * b[i]);
    let len = q.iter().map(|v| v * v).sum::<f32>().sqrt();
    if len > 0.0 {
        q.map(|v| v / len)
    } else {
        a
    }
}

impl Controller {
    /// Values per key row in the file's `values`, and how many of them are the key's own value.
    /// Bezier keys (bit 0x10) are assumed to store the value first, then two tangents per column:
    /// **unverified**, no stock position key uses them in the checks here.
    fn layout(&self) -> (usize, usize) {
        if self.kind == 20 && self.columns == 2 {
            (1, 1)
        } else if self.columns & 0x10 != 0 {
            let n = (self.columns & 0x0F) as usize;
            (n * 3, n)
        } else {
            (self.columns as usize, self.columns as usize)
        }
    }

    /// Bracket `t` among the key times: `(low row, high row, blend)`.
    fn bracket(&self, t: f32) -> Option<(usize, usize, f32)> {
        let n = self.times.len();
        if n == 0 {
            return None;
        }
        if n == 1 || t <= self.times[0] {
            return Some((0, 0, 0.0));
        }
        if t >= self.times[n - 1] {
            return Some((n - 1, n - 1, 0.0));
        }
        let hi = self.times.partition_point(|&k| k <= t);
        let lo = hi - 1;
        let span = self.times[hi] - self.times[lo];
        let f = if span > 0.0 {
            (t - self.times[lo]) / span
        } else {
            0.0
        };
        Some((lo, hi, f))
    }

    fn row(&self, i: usize) -> Option<&[f32]> {
        let (stride, used) = self.layout();
        self.values.get(i * stride..i * stride + used)
    }

    /// Linear interpolation of a 3-value controller (position).
    pub fn sample_vec3(&self, t: f32) -> Option<[f32; 3]> {
        let (lo, hi, f) = self.bracket(t)?;
        let (a, b) = (self.row(lo)?, self.row(hi)?);
        if a.len() < 3 || b.len() < 3 {
            return None;
        }
        Some([0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * f))
    }

    /// Spherical interpolation of an orientation controller; returns `[w, x, y, z]`.
    pub fn sample_quat(&self, t: f32) -> Option<[f32; 4]> {
        if self.kind != 20 {
            return None;
        }
        let (lo, hi, f) = self.bracket(t)?;
        let key = |i: usize| -> Option<[f32; 4]> {
            let r = self.row(i)?;
            match r.len() {
                1 => Some(unpack_quat(r[0]).0),
                4 => Some([r[3], r[0], r[1], r[2]]), // stored x, y, z, w
                _ => None,
            }
        };
        Some(slerp(key(lo)?, key(hi)?, f))
    }

    /// Linear interpolation of a one-value controller (alpha).
    pub fn sample_scalar(&self, t: f32) -> Option<f32> {
        let (lo, hi, f) = self.bracket(t)?;
        let (a, b) = (*self.row(lo)?.first()?, *self.row(hi)?.first()?);
        Some(a + (b - a) * f)
    }
}

/// Controller 132: how much of a node's mesh shows. A panel's screen and the plate that covers it are two meshes in
/// one plane, and this says which of them is there.
const ALPHA: u32 = 132;

impl Node {
    /// What the model file gives as this node's alpha, 1 if it gives none.
    pub fn rest_alpha(&self) -> f32 {
        self.controllers
            .iter()
            .find(|c| c.kind == ALPHA)
            .and_then(|c| c.sample_scalar(0.0))
            .unwrap_or(1.0)
    }
}

/// Position and orientation of one node at a point in time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodePose {
    pub position: [f32; 3],
    /// `[w, x, y, z]`.
    pub orientation: [f32; 4],
    /// `None` when the animation leaves the node's alpha alone.
    pub alpha: Option<f32>,
}

/// A node's lower-case name, its world transform and its parent's.
pub type JointFrame = (String, [[f32; 4]; 4], [[f32; 4]; 4]);

/// Per-node pose keyed by lower-case node name.
pub type Pose = HashMap<String, NodePose>;

/// `from` turned into `to` by `weight_to` (0 to 1): positions in a line, orientations along the shorter arc.
/// A node only one of them has stays as it is.
pub fn blend(from: &Pose, to: &Pose, weight_to: f32) -> Pose {
    let w = weight_to.clamp(0.0, 1.0);
    let mut out = to.clone();
    for (name, a) in from {
        if let Some(b) = to.get(name) {
            out.insert(
                name.clone(),
                NodePose {
                    position: [0, 1, 2]
                        .map(|i| a.position[i] + (b.position[i] - a.position[i]) * w),
                    orientation: slerp(a.orientation, b.orientation, w),
                    alpha: match (a.alpha, b.alpha) {
                        (Some(x), Some(y)) => Some(x + (y - x) * w),
                        (x, y) => y.or(x),
                    },
                },
            );
        } else {
            out.insert(name.clone(), *a);
        }
    }
    out
}

impl Animation {
    /// Sample every node this animation animates at time `t` (seconds; clamped to the first/last key).
    /// A node whose animation has no position or orientation key takes `rest` for that channel.
    pub fn sample(&self, rest: &Model, t: f32) -> Pose {
        let mut pose = Pose::new();
        self.root.walk(&mut |n: &Node| {
            let base = rest.root.find(&n.name);
            // Position keys are offsets from the node's rest position, not absolute positions: the
            // pelvis root rests at z = 1.063 while its idle keys are about 0.02.
            let pos = n
                .controllers
                .iter()
                .find(|c| c.kind == 8)
                .and_then(|c| c.sample_vec3(t))
                .map(|k| match base {
                    Some(b) => [0, 1, 2].map(|i| b.position[i] + k[i]),
                    None => k,
                })
                .or_else(|| base.map(|b| b.position));
            let rot = n
                .controllers
                .iter()
                .find(|c| c.kind == 20)
                .and_then(|c| c.sample_quat(t))
                .or_else(|| base.map(|b| b.orientation));
            if let (Some(position), Some(orientation)) = (pos, rot) {
                pose.insert(
                    n.name.to_ascii_lowercase(),
                    NodePose {
                        position,
                        orientation,
                        alpha: n
                            .controllers
                            .iter()
                            .find(|c| c.kind == ALPHA)
                            .and_then(|c| c.sample_scalar(t)),
                    },
                );
            }
        });
        pose
    }
}

impl Model {
    /// The animation with this name (case-insensitive), if the model carries it.
    pub fn animation(&self, name: &str) -> Option<&Animation> {
        self.animations
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case(name))
    }

    /// World transform of every node, parents first, applying `pose` overrides where present.
    pub fn world_matrices(&self, pose: Option<&Pose>) -> Vec<(String, [[f32; 4]; 4])> {
        fn go(n: &Node, parent: &Mat, pose: Option<&Pose>, out: &mut Vec<(String, Mat)>) {
            let (p, o) = match pose.and_then(|p| p.get(&n.name.to_ascii_lowercase())) {
                Some(np) => (np.position, np.orientation),
                None => (n.position, n.orientation),
            };
            let m = mul(parent, &matrix(p, o));
            out.push((n.name.clone(), m));
            for c in &n.children {
                go(c, &m, pose, out);
            }
        }
        let mut out = Vec::new();
        go(&self.root, &identity(), pose, &mut out);
        out
    }

    /// Every node as the model itself has it, standing: the skeleton's own pose, with no animation.
    pub fn bind_pose(&self) -> Pose {
        let mut pose = Pose::new();
        self.root.walk(&mut |n: &Node| {
            pose.insert(
                n.name.to_ascii_lowercase(),
                NodePose {
                    position: n.position,
                    orientation: n.orientation,
                    alpha: None,
                },
            );
        });
        pose
    }

    /// Lower-case name, world transform and parent's world transform of every node, applying `pose` overrides.
    pub fn joint_frames(&self, pose: Option<&Pose>) -> Vec<JointFrame> {
        fn go(n: &Node, parent: &Mat, pose: Option<&Pose>, out: &mut Vec<JointFrame>) {
            let name = n.name.to_ascii_lowercase();
            let (p, o) = match pose.and_then(|p| p.get(&name)) {
                Some(np) => (np.position, np.orientation),
                None => (n.position, n.orientation),
            };
            let m = mul(parent, &matrix(p, o));
            out.push((name, m, *parent));
            for c in &n.children {
                go(c, &m, pose, out);
            }
        }
        let mut out = Vec::new();
        go(&self.root, &identity(), pose, &mut out);
        out
    }

    /// Every node that carries a render-flagged mesh and has some alpha under `pose`, with its world matrix.
    pub fn mesh_nodes(&self, pose: Option<&Pose>) -> Vec<(&Node, [[f32; 4]; 4])> {
        fn go<'a>(n: &'a Node, parent: &Mat, pose: Option<&Pose>, out: &mut Vec<(&'a Node, Mat)>) {
            let posed = pose.and_then(|p| p.get(&n.name.to_ascii_lowercase()));
            let (p, o) = match posed {
                Some(np) => (np.position, np.orientation),
                None => (n.position, n.orientation),
            };
            let m = mul(parent, &matrix(p, o));
            let alpha = posed
                .and_then(|np| np.alpha)
                .unwrap_or_else(|| n.rest_alpha());
            if alpha > 0.0 && n.mesh.as_ref().is_some_and(|m| m.render) {
                out.push((n, m));
            }
            for c in &n.children {
                go(c, &m, pose, out);
            }
        }
        let mut out = Vec::new();
        go(&self.root, &identity(), pose, &mut out);
        out
    }

    /// World-space position of a named node under `pose`.
    pub fn node_position(&self, name: &str, pose: Option<&Pose>) -> Option<[f32; 3]> {
        self.world_matrices(pose)
            .into_iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, m)| apply(&m, [0.0; 3]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos_ctrl() -> Controller {
        Controller {
            kind: 8,
            columns: 3,
            times: vec![0.0, 1.0, 3.0],
            values: vec![0., 0., 0., 2., 4., 6., 2., 4., 6.],
        }
    }

    #[test]
    fn position_interpolates_and_clamps() {
        let c = pos_ctrl();
        assert_eq!(c.sample_vec3(-1.0), Some([0., 0., 0.]));
        assert_eq!(c.sample_vec3(0.5), Some([1., 2., 3.]));
        assert_eq!(c.sample_vec3(2.0), Some([2., 4., 6.]));
        assert_eq!(c.sample_vec3(9.0), Some([2., 4., 6.]));
    }

    #[test]
    fn packed_identity_decodes_to_identity() {
        let bits = 1023u32 | (1023 << 11) | (511 << 22);
        let (q, n2) = unpack_quat(f32::from_bits(bits));
        assert!(n2 < 1e-6);
        assert!(
            (q[0] - 1.0).abs() < 1e-3
                && q[1].abs() < 1e-3
                && q[2].abs() < 1e-3
                && q[3].abs() < 1e-3
        );
    }

    #[test]
    fn blend_goes_from_one_pose_to_the_other() {
        let id = [1.0, 0.0, 0.0, 0.0];
        let at = |x: f32| NodePose {
            position: [x, 0.0, 0.0],
            orientation: id,
            alpha: None,
        };
        let (a, b) = (
            Pose::from([("n".to_string(), at(0.0)), ("only_a".to_string(), at(5.0))]),
            Pose::from([("n".to_string(), at(2.0))]),
        );
        assert_eq!(blend(&a, &b, 0.0)["n"].position[0], 0.0);
        assert_eq!(blend(&a, &b, 0.5)["n"].position[0], 1.0);
        assert_eq!(blend(&a, &b, 1.0)["n"].position[0], 2.0);
        assert!(blend(&a, &b, 0.5).contains_key("only_a"));
    }

    #[test]
    fn a_mesh_shows_by_its_own_alpha_until_an_animation_sets_another() {
        let alpha = |values: Vec<f32>| Controller {
            kind: ALPHA,
            columns: 1,
            times: (0..values.len()).map(|i| i as f32).collect(),
            values,
        };
        let id = [1.0, 0.0, 0.0, 0.0];
        let node = |name: &str, controllers: Vec<Controller>, children: Vec<Node>| Node {
            name: name.to_string(),
            index: 0,
            number: 0,
            flags: 0,
            position: [0.0; 3],
            orientation: id,
            controllers,
            mesh: Some(crate::Mesh {
                render: true,
                ..crate::Mesh::default()
            }),
            light: None,
            emitter: None,
            children,
        };
        let animation = |root: Node| Animation {
            name: String::new(),
            length: 1.0,
            transition: 0.0,
            root_name: String::new(),
            events: vec![],
            root,
        };
        let model = Model {
            name: String::new(),
            classification: 0,
            supermodel: None,
            bbox: [[0.0; 3]; 2],
            radius: 0.0,
            anim_scale: 1.0,
            node_count: 3,
            root: node(
                "root",
                vec![],
                vec![
                    node("screen", vec![alpha(vec![1.0])], vec![]),
                    node("plate", vec![alpha(vec![0.0])], vec![]),
                ],
            ),
            animations: vec![],
        };
        let shown = |pose: Option<&Pose>| {
            let mut names: Vec<String> = model
                .mesh_nodes(pose)
                .iter()
                .map(|(n, _)| n.name.clone())
                .collect();
            names.sort();
            names.join(" ")
        };
        assert_eq!(shown(None), "root screen");
        // The animation that switches the panel off swaps the two.
        let off = animation(node(
            "root",
            vec![],
            vec![
                node("screen", vec![alpha(vec![1.0, 0.0])], vec![]),
                node("plate", vec![alpha(vec![0.0, 1.0])], vec![]),
            ],
        ));
        assert_eq!(shown(Some(&off.sample(&model, 0.0))), "root screen");
        assert_eq!(shown(Some(&off.sample(&model, 1.0))), "plate root");
        // One that does not key alpha leaves it as the file has it.
        let idle = animation(node("root", vec![], vec![node("plate", vec![], vec![])]));
        assert_eq!(shown(Some(&idle.sample(&model, 0.0))), "root screen");
    }

    #[test]
    fn slerp_halfway_and_shortest_path() {
        let id = [1.0, 0.0, 0.0, 0.0];
        let rz90 = [
            (std::f32::consts::FRAC_PI_4).cos(),
            0.0,
            0.0,
            (std::f32::consts::FRAC_PI_4).sin(),
        ];
        let mid = slerp(id, rz90, 0.5);
        let half = std::f32::consts::FRAC_PI_8;
        assert!((mid[0] - half.cos()).abs() < 1e-5 && (mid[3] - half.sin()).abs() < 1e-5);
        // The negated target is the same rotation; the result must not swing the long way round.
        let mid2 = slerp(id, rz90.map(|v| -v), 0.5);
        assert!((mid2[0] - half.cos()).abs() < 1e-5);
    }

    #[test]
    fn float_quaternion_keys_are_reordered() {
        let c = Controller {
            kind: 20,
            columns: 4,
            times: vec![0.0],
            values: vec![0.0, 0.0, 0.0, 1.0], // x y z w
        };
        assert_eq!(c.sample_quat(0.0), Some([1.0, 0.0, 0.0, 0.0]));
    }
}
