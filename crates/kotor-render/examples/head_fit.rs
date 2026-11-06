//! How a head sits on its body, in every animation: how far the bottom of the head is from the nearest part of the
//! body, and how far body vertices have strayed from where the neighbouring ones went.
//! Usage: head_fit [appearance rows...]

use kotor_archive::Install;
use kotor_render::{rig_for_outfit, Rig, Scene};

/// The largest distance from a vertex of the lowest 3 cm of the head to the nearest vertex of the body, and the lowest
/// point of the head above the highest point of the body.
fn fit(scene: &Scene, head_tex: &str) -> (f32, f32) {
    let is_head = |t: &Option<String>| {
        t.as_deref()
            .is_some_and(|t| t.eq_ignore_ascii_case(head_tex))
    };
    let (mut bottom, mut top) = (f32::MAX, f32::MIN);
    for m in &scene.meshes {
        for p in &m.positions {
            if is_head(&m.texture) {
                bottom = bottom.min(p[2]);
            } else {
                top = top.max(p[2]);
            }
        }
    }
    let body: Vec<[f32; 3]> = scene
        .meshes
        .iter()
        .filter(|m| !is_head(&m.texture))
        .flat_map(|m| m.positions.iter().copied())
        .collect();
    let mut worst = 0.0f32;
    for m in scene.meshes.iter().filter(|m| is_head(&m.texture)) {
        for p in m.positions.iter().filter(|p| p[2] < bottom + 0.03) {
            let near = body
                .iter()
                .map(|b| (0..3).map(|k| (b[k] - p[k]).powi(2)).sum::<f32>())
                .fold(f32::MAX, f32::min)
                .sqrt();
            worst = worst.max(near);
        }
    }
    (worst, bottom - top)
}

/// How far the body's vertices near the head at rest end up from the bottom of the head, and how far any body vertex
/// moved compared with its neighbours in the mesh order (a torn mesh shows as a jump).
fn drift(rest: &Scene, posed: &Scene, head_tex: &str) -> (f32, f32) {
    let is_head = |t: &Option<String>| {
        t.as_deref()
            .is_some_and(|t| t.eq_ignore_ascii_case(head_tex))
    };
    let low = |s: &Scene| {
        let mut l = [0.0, 0.0, f32::MAX];
        for m in s.meshes.iter().filter(|m| is_head(&m.texture)) {
            for p in &m.positions {
                if p[2] < l[2] {
                    l = *p;
                }
            }
        }
        l
    };
    let d = |a: [f32; 3], b: [f32; 3]| (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f32>().sqrt();
    let (lr, lp) = (low(rest), low(posed));
    let mut worst = 0.0f32;
    let mut torn = 0.0f32;
    for (r, p) in rest
        .meshes
        .iter()
        .zip(&posed.meshes)
        .filter(|(m, _)| !is_head(&m.texture))
    {
        for i in 0..r.positions.len().min(p.positions.len()) {
            if d(r.positions[i], lr) < 0.08 {
                worst = worst.max(d(p.positions[i], lp));
            }
        }
        for f in &r.triangles {
            for k in 0..3 {
                let (a, b) = (f[k] as usize, f[(k + 1) % 3] as usize);
                if a < r.positions.len() && b < r.positions.len() {
                    torn = torn
                        .max(d(p.positions[a], p.positions[b]) - d(r.positions[a], r.positions[b]));
                }
            }
        }
    }
    (worst, torn)
}

fn scan_drift(rig: &Rig, head_tex: &str) {
    let rest = rig.pose(None).unwrap();
    let mut all: Vec<(f32, f32, String, f32)> = Vec::new();
    for name in rig.animation_names() {
        let len = rig.animation_length(&name).unwrap_or(0.0);
        for k in 0..4 {
            let t = len * k as f32 / 4.0;
            if let Some(pose) = rig.sample(&name, t) {
                if let Ok(scene) = rig.posed(Some(&pose)) {
                    let (w, torn) = drift(&rest, &scene, head_tex);
                    all.push((w, torn, name.clone(), t));
                }
            }
        }
    }
    all.sort_by(|a, b| b.1.total_cmp(&a.1));
    println!("  most stretched edges:");
    for (w, torn, name, t) in all.iter().take(10) {
        println!("    {name:<14} t {t:.2}: stretch {torn:.3}, collar {w:.3}");
    }
    all.sort_by(|a, b| b.0.total_cmp(&a.0));
    println!("  collar farthest:");
    for (w, torn, name, t) in all.iter().take(8) {
        println!("    {name:<14} t {t:.2}: collar {w:.3}, stretch {torn:.3}");
    }
}

fn scan(rig: &Rig, head_tex: &str) {
    scan_drift(rig, head_tex);
    let rest = fit(&rig.pose(None).unwrap(), head_tex);
    println!("  rest: farthest {:.3}, gap {:.3}", rest.0, rest.1);
    let mut all: Vec<(f32, f32, String, f32)> = Vec::new();
    for name in rig.animation_names() {
        let len = rig.animation_length(&name).unwrap_or(0.0);
        for k in 0..4 {
            let t = len * k as f32 / 4.0;
            if let Some(pose) = rig.sample(&name, t) {
                if let Ok(scene) = rig.posed(Some(&pose)) {
                    let (near, gap) = fit(&scene, head_tex);
                    all.push((near, gap, name.clone(), t));
                }
            }
        }
    }
    all.sort_by(|a, b| b.0.total_cmp(&a.0));
    println!("  {} samples; worst:", all.len());
    for (near, gap, name, t) in all.iter().take(12) {
        println!("    {name:<16} t {t:.2}: farthest {near:.3}, gap {gap:.3}");
    }
    for want in ["pause1", "pause2", "pause3", "walk", "run", "g8r1"] {
        if let Some((near, gap, name, t)) = all.iter().find(|a| a.2.eq_ignore_ascii_case(want)) {
            println!("    {name:<16} t {t:.2}: farthest {near:.3}, gap {gap:.3}");
        }
    }
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
    for row in rows {
        let rig = rig_for_outfit(&mut inst, row, 1, Some('b')).unwrap();
        let head_tex = rig.head_texture().unwrap_or_default().to_string();
        println!("row {row} head texture {head_tex}");
        scan(&rig, &head_tex);
    }
}
