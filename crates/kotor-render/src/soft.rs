use crate::Scene;
use kotor_image::Image;
use std::collections::HashMap;
use std::rc::Rc;

/// A perspective camera in the game's Z-up world.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    /// Vertical field of view in degrees.
    pub fov: f32,
}

type V3 = [f32; 3];

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
fn norm(a: V3) -> V3 {
    let l = dot(a, a).sqrt().max(1e-9);
    [a[0] / l, a[1] / l, a[2] / l]
}

/// A vertex after the view transform. `z` is the distance in front of the camera.
#[derive(Clone, Copy)]
struct Vtx {
    x: f32,
    y: f32,
    z: f32,
    uv: [f32; 2],
    uv2: [f32; 2],
    shade: f32,
}

fn lerp(a: Vtx, b: Vtx, t: f32) -> Vtx {
    let l = |p: f32, q: f32| p + (q - p) * t;
    Vtx {
        x: l(a.x, b.x),
        y: l(a.y, b.y),
        z: l(a.z, b.z),
        uv: [l(a.uv[0], b.uv[0]), l(a.uv[1], b.uv[1])],
        uv2: [l(a.uv2[0], b.uv2[0]), l(a.uv2[1], b.uv2[1])],
        shade: l(a.shade, b.shade),
    }
}

/// Clip a triangle against the near plane `z = near`.
fn clip_near(tri: [Vtx; 3], near: f32) -> Vec<[Vtx; 3]> {
    let inside: Vec<bool> = tri.iter().map(|v| v.z >= near).collect();
    let n_in = inside.iter().filter(|&&b| b).count();
    match n_in {
        0 => vec![],
        3 => vec![tri],
        _ => {
            // Walk the edges, keeping inside vertices and adding the crossing points.
            let mut poly: Vec<Vtx> = Vec::new();
            for i in 0..3 {
                let (a, b) = (tri[i], tri[(i + 1) % 3]);
                let (ia, ib) = (inside[i], inside[(i + 1) % 3]);
                if ia {
                    poly.push(a);
                }
                if ia != ib {
                    poly.push(lerp(a, b, (near - a.z) / (b.z - a.z)));
                }
            }
            (1..poly.len() - 1)
                .map(|i| [poly[0], poly[i], poly[i + 1]])
                .collect()
        }
    }
}

/// Bilinear sample with wrap-around; `v` is flipped so v = 0 is the bottom row, as in the game's UVs.
fn sample(img: &Image, uv: [f32; 2]) -> [f32; 3] {
    let (w, h) = (img.width as usize, img.height as usize);
    let u = uv[0] - uv[0].floor();
    let v = 1.0 - (uv[1] - uv[1].floor());
    let (fx, fy) = (u * w as f32 - 0.5, v * h as f32 - 0.5);
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0, fy - y0);
    let at = |x: i64, y: i64| -> [f32; 3] {
        let (x, y) = (
            x.rem_euclid(w as i64) as usize,
            y.rem_euclid(h as i64) as usize,
        );
        let p = &img.rgba[(y * w + x) * 4..];
        [p[0] as f32, p[1] as f32, p[2] as f32]
    };
    let (x0, y0) = (x0 as i64, y0 as i64);
    let (a, b, c, d) = (
        at(x0, y0),
        at(x0 + 1, y0),
        at(x0, y0 + 1),
        at(x0 + 1, y0 + 1),
    );
    let mut out = [0.0; 3];
    for k in 0..3 {
        let top = a[k] + (b[k] - a[k]) * tx;
        let bot = c[k] + (d[k] - c[k]) * tx;
        out[k] = (top + (bot - top) * ty) / 255.0;
    }
    out
}

/// Render `scene` from `cam` into a `width` x `height` image. `texture` maps a texture name to a
/// decoded image (called once per name). Shading is diffuse x lightmap, or diffuse x a soft
/// face-normal term where a mesh has no lightmap. Triangles are drawn double-sided.
pub fn render(
    scene: &Scene,
    texture: &mut dyn FnMut(&str) -> Option<Image>,
    cam: &Camera,
    width: u32,
    height: u32,
) -> Image {
    let mut cache: HashMap<String, Option<Rc<Image>>> = HashMap::new();
    let mut get = |name: &Option<String>| -> Option<Rc<Image>> {
        let n = name.as_ref()?.to_ascii_lowercase();
        cache
            .entry(n.clone())
            .or_insert_with(|| texture(&n).map(Rc::new))
            .clone()
    };

    let fwd = norm(sub(cam.target, cam.eye));
    let right = norm(cross(fwd, [0.0, 0.0, 1.0]));
    let up = cross(right, fwd);
    let f = 1.0 / (cam.fov.to_radians() / 2.0).tan();
    let aspect = width as f32 / height as f32;
    let near = 0.05;
    let light = norm([0.3, -0.5, 0.8]);

    let n = (width * height) as usize;
    let mut color = vec![[18u8, 20, 28]; n];
    let mut depth = vec![f32::MAX; n];

    for mesh in &scene.meshes {
        let diffuse = get(&mesh.texture);
        let lightmap = get(&mesh.lightmap);
        let view: Vec<(f32, f32, f32)> = mesh
            .positions
            .iter()
            .map(|&p| {
                let d = sub(p, cam.eye);
                (dot(d, right), dot(d, up), dot(d, fwd))
            })
            .collect();
        for tri in &mesh.triangles {
            let [a, b, c] = tri.map(|i| i as usize);
            if a >= view.len() || b >= view.len() || c >= view.len() {
                continue;
            }
            let face_n = norm(cross(
                sub(mesh.positions[b], mesh.positions[a]),
                sub(mesh.positions[c], mesh.positions[a]),
            ));
            let vtx = |i: usize| Vtx {
                x: view[i].0,
                y: view[i].1,
                z: view[i].2,
                uv: mesh.uvs.get(i).copied().unwrap_or([0.0; 2]),
                uv2: mesh.uvs2.get(i).copied().unwrap_or([0.0; 2]),
                shade: {
                    let nrm = mesh.normals.get(i).copied().map(norm).unwrap_or(face_n);
                    0.55 + 0.45 * dot(nrm, light).abs()
                },
            };
            for t in clip_near([vtx(a), vtx(b), vtx(c)], near) {
                raster(
                    &t,
                    f,
                    aspect,
                    (width, height),
                    diffuse.as_deref(),
                    lightmap.as_deref(),
                    &mut color,
                    &mut depth,
                );
            }
        }
    }
    let mut rgba = Vec::with_capacity(n * 4);
    for c in color {
        rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
    }
    Image {
        width,
        height,
        rgba,
    }
}

#[allow(clippy::too_many_arguments)]
fn raster(
    t: &[Vtx; 3],
    f: f32,
    aspect: f32,
    (w, h): (u32, u32),
    diffuse: Option<&Image>,
    lightmap: Option<&Image>,
    color: &mut [[u8; 3]],
    depth: &mut [f32],
) {
    // Perspective projection to pixel coordinates; keep 1/z for perspective-correct attributes.
    let proj = |v: &Vtx| {
        let nx = v.x * f / aspect / v.z;
        let ny = v.y * f / v.z;
        (
            (nx * 0.5 + 0.5) * w as f32,
            (0.5 - ny * 0.5) * h as f32,
            1.0 / v.z,
        )
    };
    let p = [proj(&t[0]), proj(&t[1]), proj(&t[2])];
    let area = (p[1].0 - p[0].0) * (p[2].1 - p[0].1) - (p[2].0 - p[0].0) * (p[1].1 - p[0].1);
    if area.abs() < 1e-6 {
        return;
    }
    let min_x = p
        .iter()
        .map(|q| q.0)
        .fold(f32::MAX, f32::min)
        .floor()
        .max(0.0) as i32;
    let max_x = p
        .iter()
        .map(|q| q.0)
        .fold(f32::MIN, f32::max)
        .ceil()
        .min(w as f32 - 1.0) as i32;
    let min_y = p
        .iter()
        .map(|q| q.1)
        .fold(f32::MAX, f32::min)
        .floor()
        .max(0.0) as i32;
    let max_y = p
        .iter()
        .map(|q| q.1)
        .fold(f32::MIN, f32::max)
        .ceil()
        .min(h as f32 - 1.0) as i32;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let w0 = ((p[1].0 - px) * (p[2].1 - py) - (p[2].0 - px) * (p[1].1 - py)) / area;
            let w1 = ((p[2].0 - px) * (p[0].1 - py) - (p[0].0 - px) * (p[2].1 - py)) / area;
            let w2 = 1.0 - w0 - w1;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let inv_z = w0 * p[0].2 + w1 * p[1].2 + w2 * p[2].2;
            let z = 1.0 / inv_z;
            let i = (y as u32 * w + x as u32) as usize;
            if z >= depth[i] {
                continue;
            }
            // Perspective-correct weights.
            let (c0, c1, c2) = (
                w0 * p[0].2 / inv_z,
                w1 * p[1].2 / inv_z,
                w2 * p[2].2 / inv_z,
            );
            let mix2 = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| {
                [
                    c0 * a[0] + c1 * b[0] + c2 * c[0],
                    c0 * a[1] + c1 * b[1] + c2 * c[1],
                ]
            };
            let shade = c0 * t[0].shade + c1 * t[1].shade + c2 * t[2].shade;
            let base = match diffuse {
                Some(img) => sample(img, mix2(t[0].uv, t[1].uv, t[2].uv)),
                None => [0.6, 0.6, 0.6],
            };
            let lit = match lightmap {
                Some(img) => sample(img, mix2(t[0].uv2, t[1].uv2, t[2].uv2)),
                None => [shade, shade, shade],
            };
            depth[i] = z;
            for k in 0..3 {
                color[i][k] = (base[k] * lit[k] * 255.0).clamp(0.0, 255.0) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SceneMesh;

    fn quad_scene() -> Scene {
        // A 4 x 4 square in the plane y = 5, facing the camera at the origin (Z up).
        let mesh = SceneMesh {
            name: "q".into(),
            positions: vec![[-2., 5., -2.], [2., 5., -2.], [2., 5., 2.], [-2., 5., 2.]],
            uvs: vec![[0., 0.], [1., 0.], [1., 1.], [0., 1.]],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
            texture: Some("red".into()),
            ..SceneMesh::default()
        };
        Scene {
            meshes: vec![mesh],
            min: [-2., 5., -2.],
            max: [2., 5., 2.],
        }
    }

    fn red() -> Option<Image> {
        Some(Image {
            width: 2,
            height: 2,
            rgba: [255, 0, 0, 255].repeat(4),
        })
    }

    #[test]
    fn square_in_front_is_drawn_in_the_middle() {
        let cam = Camera {
            eye: [0., 0., 0.],
            target: [0., 5., 0.],
            fov: 60.0,
        };
        let img = render(&quad_scene(), &mut |_| red(), &cam, 64, 64);
        let px = |x: usize, y: usize| &img.rgba[(y * 64 + x) * 4..(y * 64 + x) * 4 + 3];
        assert!(
            px(32, 32)[0] > 100 && px(32, 32)[1] == 0,
            "centre should be red: {:?}",
            px(32, 32)
        );
        assert_eq!(px(1, 1), &[18, 20, 28], "corner should be background");
    }

    #[test]
    fn nothing_behind_the_camera_is_drawn() {
        let cam = Camera {
            eye: [0., 0., 0.],
            target: [0., -5., 0.],
            fov: 60.0,
        };
        let img = render(&quad_scene(), &mut |_| red(), &cam, 32, 32);
        assert!(img.rgba.chunks(4).all(|p| p[..3] == [18, 20, 28]));
    }

    #[test]
    fn camera_inside_geometry_is_clipped_not_dropped() {
        // Eye 1 m from the quad plane but looking along it: the quad still straddles the near plane.
        let cam = Camera {
            eye: [0., 4.9, 0.],
            target: [0., 10., 0.],
            fov: 90.0,
        };
        let img = render(&quad_scene(), &mut |_| red(), &cam, 32, 32);
        assert!(img.rgba.chunks(4).any(|p| p[0] > 100));
    }

    #[test]
    fn nearer_surface_wins() {
        let mut s = quad_scene();
        let mut near = s.meshes[0].clone();
        near.positions.iter_mut().for_each(|p| p[1] = 3.0);
        near.texture = Some("green".into());
        s.meshes.push(near);
        let cam = Camera {
            eye: [0., 0., 0.],
            target: [0., 5., 0.],
            fov: 60.0,
        };
        let img = render(
            &s,
            &mut |n| {
                if n == "green" {
                    Some(Image {
                        width: 1,
                        height: 1,
                        rgba: vec![0, 255, 0, 255],
                    })
                } else {
                    red()
                }
            },
            &cam,
            64,
            64,
        );
        let c = &img.rgba[(32 * 64 + 32) * 4..];
        assert!(c[1] > 100 && c[0] == 0, "{:?}", &c[..3]);
    }
}
