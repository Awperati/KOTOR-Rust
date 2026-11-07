//! A thrown grenade flies before it bursts: frames of the throw, the flight and the blast, offscreen (no window, no screen
//! capture). Two throws: the player's at a soldier 8 m off, and the Endar Spire opening's, where a Sith soldier throws at a
//! Republic soldier and the pilot plays the game up to it. Each frame says where the grenade is and how much of the blast
//! is on screen.
//!
//!   cargo run --release --example grenade_strip -- [out_dir] [width] [height]
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

const DT: f32 = 1.0 / 30.0;
const EVERY: usize = 6;
const FRAMES: usize = 14;

fn frame(
    s: &mut Stage,
    gpu: &mut GpuRenderer,
    out: &std::path::Path,
    name: &str,
    k: usize,
    height: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let sc = s.dynamic_scene();
    let cam = s.camera();
    gpu.set_particles(s.particles());
    let live: usize = s.particles().iter().map(|b| b.vertices.len()).sum();
    let mut img = gpu.render(&sc, &mut |n| s.texture(n), &cam).unwrap();
    if let Some(o) = s.overlay() {
        composite(&mut img, o);
    }
    let p = out.join(format!("grenade_{name}_{k:02}_{height}p.png"));
    std::fs::write(&p, kotor_image::to_png(&img))?;
    let flying = s.game.world.grenades_in_flight();
    println!(
        "{name} {k:2}: {live:5} particle vertices, grenade {}",
        flying
            .first()
            .map_or("not in the air".to_string(), |g| format!(
                "at {:.1}, {:.1}, {:.2}",
                g.position[0], g.position[1], g.position[2]
            ))
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).map_or("docs/shots/combat", String::as_str));
    let width: u32 = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(1280);
    let height: u32 = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(720);
    std::fs::create_dir_all(&out)?;
    let mut gpu = GpuRenderer::new(width, height)?;

    // 1. The player's throw.
    let mut s = Stage::open("end_m01aa", true)?;
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
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
    let (pc, at, heading) = {
        let w = &s.game.world;
        (w.pc, w.object(w.pc).unwrap().position, w.pc_mover.heading)
    };
    s.yaw = heading + 0.5;
    let w = &mut s.game.world;
    let foe = w.spawn_creature(
        "n_sithsoldier003",
        [
            at[0] + heading.cos() * 5.5,
            at[1] + heading.sin() * 5.5,
            at[2],
        ],
    );
    let f = w.object_mut(foe).unwrap();
    (f.hp, f.max_hp, f.faction) = (900, 900, 1);
    let grenade = w.spawn_item("g_w_fraggren01");
    w.carry(pc, grenade);
    w.object_mut(pc).unwrap().min_one_hp = true;
    w.use_item_on(pc, grenade, foe).unwrap();
    for k in 0..FRAMES {
        for _ in 0..EVERY {
            s.update(DT);
            if s.game.world.in_dialogue() {
                s.number_key(1);
                s.continue_line();
            }
        }
        frame(&mut s, &mut gpu, &out, "player", k, height)?;
    }

    // 2. The opening's throw.
    let mut s = Stage::open("end_m01aa", true)?;
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    s.pilot = Some(kotor_game::Autopilot::new());
    let scene = s.static_scene().clone();
    gpu.set_static_scene(&scene, &mut |n| s.texture(n));
    for _ in 0..(400.0 / DT as f64) as usize {
        s.update(DT);
        if s.game
            .world
            .events
            .iter()
            .any(|e| e.contains("casts power 87"))
        {
            println!("the throw begins at game time {:.1}", s.game.world.time);
            break;
        }
    }
    for k in 0..FRAMES {
        for _ in 0..EVERY {
            s.update(DT);
        }
        frame(&mut s, &mut gpu, &out, "opening", k, height)?;
    }
    Ok(())
}
