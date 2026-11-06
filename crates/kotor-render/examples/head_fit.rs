//! How a head sits on its body. The neck vertices of the head are picked at rest and followed by index into each pose,
//! since a body lying down has some other part of the head lowest. Also the colours either side of the join.
//! Usage: head_fit [appearance rows...]

use kotor_archive::Install;
use kotor_render::{rig_for_outfit, Rig, Scene};

fn is_head(tex: &Option<String>, head_tex: &str) -> bool {
    tex.as_deref()
        .is_some_and(|t| t.eq_ignore_ascii_case(head_tex))
}

fn apart(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f32>().sqrt()
}

/// (mesh, vertex) of the head within 3 cm of its lowest point at rest.
fn neck_vertices(rest: &Scene, head_tex: &str) -> Vec<(usize, usize)> {
    let bottom = rest
        .meshes
        .iter()
        .filter(|m| is_head(&m.texture, head_tex))
        .flat_map(|m| m.positions.iter())
        .fold(f32::MAX, |b, p| b.min(p[2]));
    let mut v = Vec::new();
    for (i, m) in rest
        .meshes
        .iter()
        .enumerate()
        .filter(|(_, m)| is_head(&m.texture, head_tex))
    {
        for (k, p) in m.positions.iter().enumerate() {
            if p[2] < bottom + 0.03 {
                v.push((i, k));
            }
        }
    }
    v
}

/// (mesh, vertex) of the body within 8 cm of the head's neck at rest: the collar.
fn collar_vertices(rest: &Scene, head_tex: &str, neck: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    for (i, m) in rest
        .meshes
        .iter()
        .enumerate()
        .filter(|(_, m)| !is_head(&m.texture, head_tex))
    {
        for (k, p) in m.positions.iter().enumerate() {
            if neck
                .iter()
                .any(|(a, b)| apart(rest.meshes[*a].positions[*b], *p) < 0.08)
            {
                v.push((i, k));
            }
        }
    }
    v
}

/// The largest distance from a neck vertex to its nearest collar vertex in a scene.
fn join(scene: &Scene, neck: &[(usize, usize)], collar: &[(usize, usize)]) -> f32 {
    let mut worst = 0.0f32;
    for (a, b) in neck {
        let p = scene.meshes[*a].positions[*b];
        let near = collar
            .iter()
            .map(|(c, d)| apart(scene.meshes[*c].positions[*d], p))
            .fold(f32::MAX, f32::min);
        worst = worst.max(near);
    }
    worst
}

fn scan(rig: &Rig, head_tex: &str) {
    let rest = rig.pose(None).unwrap();
    let neck = neck_vertices(&rest, head_tex);
    let collar = collar_vertices(&rest, head_tex, &neck);
    println!(
        "  {} neck and {} collar vertices; at rest the join is {:.3}",
        neck.len(),
        collar.len(),
        join(&rest, &neck, &collar)
    );
    let mut all: Vec<(f32, String, f32)> = Vec::new();
    for name in rig.animation_names() {
        let len = rig.animation_length(&name).unwrap_or(0.0);
        for k in 0..4 {
            let t = len * k as f32 / 4.0;
            if let Some(pose) = rig.sample(&name, t) {
                if let Ok(scene) = rig.posed(Some(&pose)) {
                    all.push((join(&scene, &neck, &collar), name.clone(), t));
                }
            }
        }
    }
    all.sort_by(|a, b| b.0.total_cmp(&a.0));
    println!("  {} samples; widest:", all.len());
    for (d, name, t) in all.iter().take(8) {
        println!("    {name:<14} t {t:.2}: {d:.3}");
    }
    for want in ["pause1", "walk", "run", "g8r1", "die3", "dead3", "dead"] {
        if let Some((d, name, t)) = all.iter().find(|a| a.1.eq_ignore_ascii_case(want)) {
            println!("    {name:<14} t {t:.2}: {d:.3}");
        }
    }
}

/// The average colour of a texture at the given vertices of a scene.
fn colour(inst: &mut Install, scene: &Scene, at: &[(usize, usize)]) -> [f32; 3] {
    let (mut sum, mut n) = ([0.0f32; 3], 0.0f32);
    for (m, k) in at {
        let mesh = &scene.meshes[*m];
        let (Some(name), Some(uv)) = (mesh.texture.as_ref(), mesh.uvs.get(*k)) else {
            continue;
        };
        let Some(img) = kotor_render::load_texture(inst, name) else {
            continue;
        };
        let x = (uv[0].rem_euclid(1.0) * (img.width - 1) as f32) as u32;
        let y = ((1.0 - uv[1].rem_euclid(1.0)) * (img.height - 1) as f32) as u32;
        let p = ((y * img.width + x) * 4) as usize;
        for (c, s) in sum.iter_mut().enumerate() {
            *s += f32::from(img.rgba[p + c]);
        }
        n += 1.0;
    }
    sum.map(|v| (v / n.max(1.0)).round())
}

fn main() {
    let rows: Vec<usize> = std::env::args()
        .skip(1)
        .filter_map(|a| a.parse().ok())
        .collect();
    let rows = if rows.is_empty() {
        vec![92, 107, 122, 137, 152, 167]
    } else {
        rows
    };
    let mut inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
    inst.set_prefer_rim(true);
    inst.load_texture_packs().unwrap();
    for row in rows {
        for letter in [Some('b'), None] {
            let rig = rig_for_outfit(&mut inst, row, 1, letter).unwrap();
            let head_tex = rig.head_texture().unwrap_or_default().to_string();
            let rest = rig.pose(None).unwrap();
            let neck = neck_vertices(&rest, &head_tex);
            let collar = collar_vertices(&rest, &head_tex, &neck);
            let body_tex = rest
                .meshes
                .iter()
                .find(|m| !is_head(&m.texture, &head_tex))
                .and_then(|m| m.texture.clone());
            println!(
                "row {row} column {letter:?}: head {head_tex} neck colour {:?}, body {body_tex:?} collar colour {:?}",
                colour(&mut inst, &rest, &neck),
                colour(&mut inst, &rest, &collar)
            );
            if letter.is_some() {
                scan(&rig, &head_tex);
            }
        }
    }
}
