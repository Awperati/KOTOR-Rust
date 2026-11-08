//! Where a fight frame's time goes, offscreen: a fight with several soldiers, each frame split into the game's update, the
//! dynamic scene's building and the GPU draw, with the worst of each, and where `World::step` spends its part.
//!
//!   cargo run --release --example fight_profile -- [width] [height] [foes]
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};
use std::time::Instant;

const DT: f32 = 1.0 / 60.0;

fn stats(name: &str, v: &mut [f64]) {
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    let sum: f64 = v.iter().sum();
    println!(
        "{name:<8} mean {:6.2}  median {:6.2}  p99 {:6.2}  worst {:6.2} ms",
        sum / n as f64,
        v[n / 2],
        v[(n as f64 * 0.99) as usize],
        v[n - 1]
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let width: u32 = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(3840);
    let height: u32 = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(2160);
    let foes: usize = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(4);
    let mut s = Stage::open("end_m01aa", true)?;
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let mut gpu = GpuRenderer::new(width, height)?;
    let scene = s.static_scene().clone();
    gpu.set_static_scene(&scene, &mut |n| s.texture(n));
    for _ in 0..3000 {
        s.update(1.0 / 30.0);
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        }
    }
    let (pc, at, heading) = {
        let w = &s.game.world;
        (w.pc, w.object(w.pc).unwrap().position, w.pc_mover.heading)
    };
    s.yaw = heading + 0.5;
    let w = &mut s.game.world;
    w.object_mut(pc).unwrap().min_one_hp = true;
    for k in 0..foes {
        let d = 4.0 + k as f32;
        let foe = w.spawn_creature(
            "n_sithsoldier003",
            [
                at[0] + heading.cos() * d,
                at[1] + heading.sin() * d + (k as f32 - 1.5) * 0.8,
                at[2],
            ],
        );
        let f = w.object_mut(foe).unwrap();
        (f.hp, f.max_hp, f.faction) = (900, 900, 1);
        w.attack(foe, pc);
    }
    w.step_profile = Some(Default::default());
    // Warm up: first sight of each model and texture.
    for _ in 0..120 {
        s.update(DT);
        let sc = s.dynamic_scene();
        let cam = s.camera();
        gpu.set_particles(s.particles());
        let _ = gpu.render(&sc, &mut |n| s.texture(n), &cam).unwrap();
    }
    s.game.world.step_profile = Some(Default::default());
    let (mut update, mut scene_ms, mut draw, mut hud, mut total) =
        (vec![], vec![], vec![], vec![], vec![]);
    for _ in 0..900 {
        let t0 = Instant::now();
        s.update(DT);
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        }
        let t1 = Instant::now();
        let sc = s.dynamic_scene();
        let t2 = Instant::now();
        let cam = s.camera();
        gpu.set_particles(s.particles());
        let mut img = gpu.render(&sc, &mut |n| s.texture(n), &cam).unwrap();
        let t3 = Instant::now();
        if let Some(o) = s.overlay() {
            composite(&mut img, o);
        }
        let t4 = Instant::now();
        let ms = |a: Instant, b: Instant| (b - a).as_secs_f64() * 1000.0;
        if ms(t1, t2) > 25.0 {
            let w = &s.game.world;
            let clips: Vec<String> = w
                .objects
                .values()
                .filter(|o| o.kind == kotor_game::Kind::Creature && !o.dead)
                .filter_map(|o| s.game.clip(o.id).map(|c| format!("{}:{}", o.id, c.name)))
                .collect();
            println!(
                "slow scene {:.1} ms at {:.2}s: {clips:?}",
                ms(t1, t2),
                w.time
            );
        }
        update.push(ms(t0, t1));
        scene_ms.push(ms(t1, t2));
        draw.push(ms(t2, t3));
        hud.push(ms(t3, t4));
        total.push(ms(t0, t4));
    }
    println!("{width}x{height}, {foes} soldiers, 900 frames of a fight");
    stats("update", &mut update);
    stats("scene", &mut scene_ms);
    stats("draw", &mut draw);
    stats("overlay", &mut hud);
    stats("total", &mut total);
    if let Some(p) = &s.game.world.step_profile {
        println!("World::step per call (count, mean ms, worst ms):");
        let mut rows: Vec<_> = p.iter().collect();
        rows.sort_by(|a, b| b.1 .1.total_cmp(&a.1 .1));
        for (name, (n, sum, worst)) in rows {
            println!("  {name:<12} {n:5}  {:7.3}  {:7.3}", sum / *n as f64, worst);
        }
    }
    Ok(())
}
