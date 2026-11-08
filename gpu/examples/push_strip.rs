//! Force Push in the open and against a wall, offscreen: a soldier thrown 8 m, and one thrown into the wall behind them. Frames at
//! set times after the cast, with how far the soldier has gone and what it is doing.
//!
//!   cargo run --release --example push_strip -- [out_dir] [width] [height]
use kotor_game::FORCE_PUSH;
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

const DT: f32 = 1.0 / 30.0;
const TIMES: [f32; 8] = [0.0, 0.25, 0.5, 0.8, 1.2, 2.0, 3.4, 4.4];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).map_or("docs/shots/combat", String::as_str));
    let width: u32 = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(1280);
    let height: u32 = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(720);
    std::fs::create_dir_all(&out)?;
    let mut s = Stage::open("end_m01aa", true)?;
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let mut gpu = GpuRenderer::new(width, height)?;
    let scene = s.static_scene().clone();
    gpu.set_static_scene(&scene, &mut |n| s.texture(n));
    for _ in 0..3000 {
        s.update(DT);
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        }
    }
    for _ in 0..600 {
        s.update(DT);
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        }
    }
    let pc = s.game.world.pc;
    let home = s.game.world.object(pc).unwrap().position;
    let w = &mut s.game.world;
    w.object_mut(pc).unwrap().spells.insert(FORCE_PUSH);
    w.object_mut(pc).unwrap().force = 500;
    w.object_mut(pc).unwrap().max_force = 500;
    let foe = w.spawn_creature("n_calonord", home);
    w.object_mut(foe).unwrap().hp = 900;
    w.object_mut(foe).unwrap().max_hp = 900;
    // Find a direction that ends in open air and one that ends on a wall, by throwing in each and watching where it lands.
    let (mut open, mut wall) = (None, None);
    for k in 0..24 {
        let a = k as f32 * std::f32::consts::TAU / 24.0;
        let from = [home[0] + a.cos(), home[1] + a.sin()];
        let Some(z) = w
            .nav
            .as_ref()
            .and_then(|n| n.height_at(from[0], from[1], home[2] + 0.4))
        else {
            continue;
        };
        w.teleport(foe, [from[0], from[1], z]);
        let _ = w.cast_spell(pc, FORCE_PUSH, foe, true);
        for _ in 0..80 {
            w.step(0.05);
        }
        let end = w.object(foe).unwrap().position;
        let moved = (end[0] - from[0]).hypot(end[1] - from[1]);
        let hit = std::mem::take(&mut w.thuds).first().is_some_and(|t| t.wall);
        if !hit && moved > 7.0 && open.is_none() {
            open = Some((a, [from[0], from[1], z]));
        }
        if hit && moved > 2.0 && wall.is_none() {
            wall = Some((a, [from[0], from[1], z]));
        }
        for _ in 0..80 {
            w.step(0.05);
        }
    }
    for (name, found) in [("open", open), ("wall", wall)] {
        let Some((a, spot)) = found else {
            println!("{name}: no direction found");
            continue;
        };
        let w = &mut s.game.world;
        w.teleport(foe, spot);
        w.object_mut(foe).unwrap().hp = 900;
        // The player stands behind the soldier, so that the throw goes away from the camera.
        let behind = [spot[0] - a.cos() * 1.0, spot[1] - a.sin() * 1.0, spot[2]];
        w.teleport(pc, behind);
        s.yaw = a;
        s.distance = 10.0;
        s.pitch = 0.5;
        for _ in 0..240 {
            s.update(DT);
            if s.game.world.in_dialogue() {
                s.number_key(1);
                s.continue_line();
            }
        }
        let _ = s.game.world.cast_spell(pc, FORCE_PUSH, foe, true);
        let mut t = 0.0;
        for (k, at) in TIMES.iter().enumerate() {
            while t < *at {
                s.update(DT);
                if s.game.world.in_dialogue() {
                    s.number_key(1);
                    s.continue_line();
                }
                t += DT;
            }
            let sc = s.dynamic_scene();
            let cam = s.camera();
            gpu.set_particles(s.particles());
            let mut img = gpu.render(&sc, &mut |n| s.texture(n), &cam).unwrap();
            if let Some(o) = s.overlay() {
                composite(&mut img, o);
            }
            let p = out.join(format!("push_{name}_{k}_{height}p.png"));
            std::fs::write(&p, kotor_image::to_png(&img))?;
            let pos = s.game.world.object(foe).unwrap().position;
            println!(
                "{name} {at:.2} s: {:.1} m from the start, clip {}",
                (pos[0] - spot[0]).hypot(pos[1] - spot[1]),
                s.game.clip(foe).map_or("-", |c| c.name)
            );
        }
        let w = &mut s.game.world;
        w.thuds.clear();
        for _ in 0..30 {
            s.update(DT);
        }
    }
    Ok(())
}
