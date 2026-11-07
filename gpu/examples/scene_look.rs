//! A module's first view with its particles, after some seconds of running: `scene_look <module> <out.png> [seconds] [width] [height]`.
use kotor_gpu::stage::Stage;
use kotor_gpu::GpuRenderer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let module = a.get(1).cloned().unwrap_or_else(|| "kas_m25aa".into());
    let out = a.get(2).cloned().unwrap_or_else(|| "look.png".into());
    let secs: f32 = a.get(3).and_then(|v| v.parse().ok()).unwrap_or(4.0);
    let (w, h): (u32, u32) = (
        a.get(4).and_then(|v| v.parse().ok()).unwrap_or(960),
        a.get(5).and_then(|v| v.parse().ok()).unwrap_or(540),
    );
    let mut s = Stage::open(&module, true)?;
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let mut gpu = GpuRenderer::new(w, h)?;
    if std::env::var_os("EMIT").is_some() {
        let mut by: std::collections::BTreeMap<String, Vec<[f32; 3]>> = Default::default();
        for (p, t) in s.effects.source_places() {
            by.entry(t).or_default().push(p);
        }
        for (t, ps) in by {
            println!(
                "emitters {t}: {} first {:?}",
                ps.len(),
                ps.iter().take(4).collect::<Vec<_>>()
            );
        }
    }
    if let Ok(at) = std::env::var("CAMAT") {
        let v: Vec<f32> = at.split(',').filter_map(|x| x.parse().ok()).collect();
        let pc = s.game.world.pc;
        s.game.world.teleport(pc, [v[0], v[1], v[2]]);
        if v.len() > 3 {
            s.yaw = v[3];
        }
    }
    for _ in 0..(secs * 60.0) as usize {
        s.update(1.0 / 60.0);
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        }
    }
    let scene_static = s.static_scene().clone();
    if std::env::var_os("TEX").is_some() {
        let mut t: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
        for m in &scene_static.meshes {
            let e = t
                .entry(format!("{:?} lm={:?}", m.texture, m.lightmap.is_some()))
                .or_default();
            e.0 += 1;
            e.1 += m.triangles.len();
        }
        for (k, (n, tr)) in t {
            println!("tex {k}: {n} meshes {tr} tris");
        }
    }
    if let Ok(name) = std::env::var("FIND") {
        for m in &scene_static.meshes {
            if m.texture
                .as_deref()
                .is_some_and(|t| t.to_ascii_lowercase().contains(&name.to_ascii_lowercase()))
            {
                let lo = |k: usize| m.positions.iter().map(|p| p[k]).fold(f32::MAX, f32::min);
                let hi = |k: usize| m.positions.iter().map(|p| p[k]).fold(f32::MIN, f32::max);
                let pc = s.game.world.pc_mover.pos;
                let mut near: Vec<[f32; 3]> = m.positions.clone();
                near.sort_by(|a, b| {
                    (a[0] - pc[0])
                        .hypot(a[1] - pc[1])
                        .total_cmp(&(b[0] - pc[0]).hypot(b[1] - pc[1]))
                });
                println!(
                    "nearest beam vertices to the player {:?}: {:?}",
                    pc,
                    &near[..4.min(near.len())]
                );
                println!(
                    "found {} {:?} x {:.0}..{:.0} y {:.0}..{:.0} z {:.0}..{:.0}",
                    m.name,
                    m.texture,
                    lo(0),
                    hi(0),
                    lo(1),
                    hi(1),
                    lo(2),
                    hi(2)
                );
            }
        }
    }
    if std::env::var_os("THIN").is_some() {
        let mut n = 0;
        for m in &scene_static.meshes {
            let lo = |k: usize| m.positions.iter().map(|p| p[k]).fold(f32::MAX, f32::min);
            let hi = |k: usize| m.positions.iter().map(|p| p[k]).fold(f32::MIN, f32::max);
            let (dx, dy, dz) = (hi(0) - lo(0), hi(1) - lo(1), hi(2) - lo(2));
            if dz > 5.0 && dx.min(dy) < 0.5 && dx.max(dy) > 0.0 {
                n += 1;
                if n <= 12 {
                    println!(
                        "thin {} {:?} size {:.2} x {:.2} x {:.1} at {:.0},{:.0},{:.0} tris {}",
                        m.name,
                        m.texture,
                        dx,
                        dy,
                        dz,
                        lo(0),
                        lo(1),
                        lo(2),
                        m.triangles.len()
                    );
                }
            }
        }
        println!("thin meshes: {n} of {}", scene_static.meshes.len());
    }
    if std::env::var_os("LIST").is_some() {
        let mut seen = std::collections::BTreeMap::new();
        for m in &scene_static.meshes {
            let c = m
                .positions
                .iter()
                .fold([0.0f32; 3], |a, p| [a[0] + p[0], a[1] + p[1], a[2] + p[2]]);
            let n = m.positions.len().max(1) as f32;
            seen.entry((m.name.clone(), m.texture.clone()))
                .or_insert_with(Vec::new)
                .push([c[0] / n, c[1] / n, c[2] / n]);
        }
        let pc = s.game.world.pc_mover.pos;
        for ((n, t), at) in &seen {
            let near: Vec<_> = at
                .iter()
                .filter(|p| (p[0] - pc[0]).hypot(p[1] - pc[1]) < 25.0)
                .collect();
            if !near.is_empty() {
                println!("{n} {:?} x{} near {:?}", t, near.len(), near[0]);
            }
        }
        println!("pc {:?}", s.game.world.pc_mover.pos);
    }
    gpu.set_static_scene(&scene_static, &mut |n| s.texture(n));
    let scene = s.dynamic_scene();
    let cam = s.camera();
    let batches = s.particles();
    if std::env::var_os("LIST").is_some() {
        for b in &batches {
            println!(
                "particles: {} additive {} vertices {}",
                b.texture,
                b.additive,
                b.vertices.len()
            );
        }
    }
    gpu.set_particles(batches);
    let img = gpu.render(&scene, &mut |n| s.texture(n), &cam)?;
    std::fs::write(&out, kotor_image::to_png(&img))?;
    println!("wrote {out}");
    Ok(())
}
