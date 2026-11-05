//! Tiny software renderer used to eyeball models without a window or GPU.

use kotor_image::Image;
use kotor_model::{Model, Node};

type Mat = [[f32; 4]; 4];

fn local(n: &Node) -> Mat {
    let [w, x, y, z] = n.orientation;
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

fn mul(a: &Mat, b: &Mat) -> Mat {
    let mut o = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            o[i][j] = (0..4).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    o
}

fn apply(m: &Mat, p: [f32; 3]) -> [f32; 3] {
    let f = |r: usize| m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2] + m[r][3];
    [f(0), f(1), f(2)]
}

fn collect(n: &Node, parent: &Mat, tris: &mut Vec<[[f32; 3]; 3]>) {
    let m = mul(parent, &local(n));
    if let Some(mesh) = &n.mesh {
        if mesh.render {
            for f in &mesh.faces {
                let p = f.indices.map(|i| {
                    mesh.positions
                        .get(i as usize)
                        .copied()
                        .map(|p| apply(&m, p))
                });
                if let [Some(a), Some(b), Some(c)] = p {
                    tris.push([a, b, c]);
                }
            }
        }
    }
    for c in &n.children {
        collect(c, &m, tris);
    }
}

/// Orthographic render looking along +Y (front view: x right, z up), flat shaded.
pub fn render(model: &Model, size: u32) -> Image {
    let id = [
        [1., 0., 0., 0.],
        [0., 1., 0., 0.],
        [0., 0., 1., 0.],
        [0., 0., 0., 1.],
    ];
    let mut tris = Vec::new();
    collect(&model.root, &id, &mut tris);
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for t in &tris {
        for v in t {
            for k in 0..3 {
                min[k] = min[k].min(v[k]);
                max[k] = max[k].max(v[k]);
            }
        }
    }
    let n = (size * size) as usize;
    let mut rgba = vec![0u8; n * 4];
    for px in rgba.chunks_mut(4) {
        px.copy_from_slice(&[24, 24, 32, 255]);
    }
    if tris.is_empty() {
        return Image {
            width: size,
            height: size,
            rgba,
        };
    }
    let span = (max[0] - min[0]).max(max[2] - min[2]).max(1e-3);
    let scale = (size as f32 - 8.0) / span;
    let to_screen = |v: [f32; 3]| {
        (
            4.0 + (v[0] - min[0]) * scale,
            size as f32 - 4.0 - (v[2] - min[2]) * scale,
            v[1],
        )
    };
    let mut depth = vec![f32::MAX; n];
    for t in &tris {
        let s = t.map(to_screen);
        let e1 = [t[1][0] - t[0][0], t[1][1] - t[0][1], t[1][2] - t[0][2]];
        let e2 = [t[2][0] - t[0][0], t[2][1] - t[0][1], t[2][2] - t[0][2]];
        let nrm = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];
        let len = (nrm[0] * nrm[0] + nrm[1] * nrm[1] + nrm[2] * nrm[2])
            .sqrt()
            .max(1e-9);
        let shade = ((nrm[1] / len).abs() * 0.7 + 0.3).min(1.0);
        let c = (60.0 + 190.0 * shade) as u8;
        let (x0, x1) = (
            s.iter()
                .map(|p| p.0)
                .fold(f32::MAX, f32::min)
                .floor()
                .max(0.0) as i32,
            s.iter()
                .map(|p| p.0)
                .fold(f32::MIN, f32::max)
                .ceil()
                .min(size as f32 - 1.0) as i32,
        );
        let (y0, y1) = (
            s.iter()
                .map(|p| p.1)
                .fold(f32::MAX, f32::min)
                .floor()
                .max(0.0) as i32,
            s.iter()
                .map(|p| p.1)
                .fold(f32::MIN, f32::max)
                .ceil()
                .min(size as f32 - 1.0) as i32,
        );
        let area = (s[1].0 - s[0].0) * (s[2].1 - s[0].1) - (s[2].0 - s[0].0) * (s[1].1 - s[0].1);
        if area.abs() < 1e-6 {
            continue;
        }
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let w0 = ((s[1].0 - px) * (s[2].1 - py) - (s[2].0 - px) * (s[1].1 - py)) / area;
                let w1 = ((s[2].0 - px) * (s[0].1 - py) - (s[0].0 - px) * (s[2].1 - py)) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                    let z = w0 * s[0].2 + w1 * s[1].2 + w2 * s[2].2;
                    let i = (y as u32 * size + x as u32) as usize;
                    if z < depth[i] {
                        depth[i] = z;
                        rgba[i * 4..i * 4 + 4].copy_from_slice(&[c, c, c.saturating_add(10), 255]);
                    }
                }
            }
        }
    }
    Image {
        width: size,
        height: size,
        rgba,
    }
}
