use crate::{Model, Node};
use std::fmt::Write;

type Mat = [[f32; 4]; 4];

fn identity() -> Mat {
    [
        [1., 0., 0., 0.],
        [0., 1., 0., 0.],
        [0., 0., 1., 0.],
        [0., 0., 0., 1.],
    ]
}

fn mul(a: &Mat, b: &Mat) -> Mat {
    let mut o = [[0.0; 4]; 4];
    for (i, row) in o.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (0..4).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    o
}

/// Local transform: rotation from `[w, x, y, z]`, then translation. Column-vector convention.
fn local(n: &Node) -> Mat {
    let [w, x, y, z] = n.orientation;
    let len = (w * w + x * x + y * y + z * z).sqrt();
    let (w, x, y, z) = if len > 0.0 {
        (w / len, x / len, y / len, z / len)
    } else {
        (1., 0., 0., 0.)
    };
    [
        [
            1. - 2. * (y * y + z * z),
            2. * (x * y - z * w),
            2. * (x * z + y * w),
            n.position[0],
        ],
        [
            2. * (x * y + z * w),
            1. - 2. * (x * x + z * z),
            2. * (y * z - x * w),
            n.position[1],
        ],
        [
            2. * (x * z - y * w),
            2. * (y * z + x * w),
            1. - 2. * (x * x + y * y),
            n.position[2],
        ],
        [0., 0., 0., 1.],
    ]
}

fn apply(m: &Mat, p: [f32; 3]) -> [f32; 3] {
    let f = |r: usize| m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2] + m[r][3];
    [f(0), f(1), f(2)]
}

fn emit(n: &Node, parent: &Mat, out: &mut String, base: &mut usize) {
    let m = mul(parent, &local(n));
    if let Some(mesh) = &n.mesh {
        if mesh.render && !mesh.positions.is_empty() {
            let _ = writeln!(out, "o {}", n.name);
            if let Some(t) = &mesh.texture {
                let _ = writeln!(out, "usemtl {t}");
            }
            for p in &mesh.positions {
                let q = apply(&m, *p);
                let _ = writeln!(out, "v {} {} {}", q[0], q[1], q[2]);
            }
            for uv in &mesh.uvs {
                let _ = writeln!(out, "vt {} {}", uv[0], uv[1]);
            }
            let has_uv = mesh.uvs.len() == mesh.positions.len();
            for f in &mesh.faces {
                let [a, b, c] = f.indices.map(|i| i as usize + *base);
                if has_uv {
                    let _ = writeln!(out, "f {a}/{a} {b}/{b} {c}/{c}");
                } else {
                    let _ = writeln!(out, "f {a} {b} {c}");
                }
            }
            *base += mesh.positions.len();
        }
    }
    for c in &n.children {
        emit(c, &m, out, base);
    }
}

/// Wavefront OBJ text of the model's rest pose (all render-flagged meshes, world space).
pub fn to_obj(model: &Model) -> String {
    let mut out = format!(
        "# {} (exported from the owner's installed game data)\n",
        model.name
    );
    let mut base = 1;
    emit(&model.root, &identity(), &mut out, &mut base);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Mesh;

    #[test]
    fn exports_translated_triangle() {
        let mesh = Mesh {
            render: true,
            positions: vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]],
            faces: vec![crate::MeshFace {
                indices: [0, 1, 2],
                material: 0,
                normal: [0., 0., 1.],
            }],
            ..Mesh::default()
        };
        let node = Node {
            name: "tri".into(),
            flags: crate::NODE_HEADER | crate::NODE_MESH,
            position: [10., 0., 0.],
            orientation: [1., 0., 0., 0.],
            controllers: vec![],
            mesh: Some(mesh),
            light: None,
            emitter: None,
            children: vec![],
        };
        let model = Model {
            name: "t".into(),
            classification: 0,
            supermodel: None,
            bbox: [[0.; 3]; 2],
            radius: 0.,
            anim_scale: 1.,
            node_count: 1,
            root: node,
            animations: vec![],
        };
        let obj = to_obj(&model);
        assert!(obj.contains("v 10 0 0"));
        assert!(obj.contains("f 1 2 3"));
    }
}
