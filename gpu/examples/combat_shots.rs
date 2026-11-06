//! Frames for the combat builder's changes, rendered offscreen (no window, no screen capture):
//! Force Push throwing a soldier back, and the camera tilted right up under the ceiling.
//!
//!   cargo run --release --example combat_shots -- [out_dir] [width] [height]
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

const DT: f32 = 1.0 / 30.0;

fn shot(gpu: &mut GpuRenderer, s: &mut Stage, path: &std::path::Path) {
    let scene = s.dynamic_scene();
    let cam = s.camera();
    gpu.set_particles(s.particles());
    let mut img = gpu.render(&scene, &mut |n| s.texture(n), &cam).unwrap();
    if let Some(o) = s.overlay() {
        composite(&mut img, o);
    }
    std::fs::write(path, kotor_image::to_png(&img)).unwrap();
    println!("wrote {}", path.display());
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).map_or("docs/shots/combat", String::as_str));
    let width: u32 = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(1280);
    let height: u32 = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(720);
    std::fs::create_dir_all(&out)?;
    let tag = format!("{height}p");
    let mut s = Stage::open("end_m01aa", true)?;
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let mut gpu = GpuRenderer::new(width, height)?;
    let scene = s.static_scene().clone();
    gpu.set_static_scene(&scene, &mut |n| s.texture(n));
    // Through the opening, clicking on, until the player is free.
    for _ in 0..3000 {
        s.update(DT);
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        }
    }
    // A soldier stands in front of the player, and Force Push sends him back.
    let (pc, at, heading) = {
        let w = &s.game.world;
        (w.pc, w.object(w.pc).unwrap().position, w.pc_mover.heading)
    };
    let spot = [
        at[0] + heading.cos() * 1.6,
        at[1] + heading.sin() * 1.6,
        at[2],
    ];
    // From the side, so that the soldier is not behind the player.
    s.yaw = heading + std::f32::consts::FRAC_PI_2;
    let w = &mut s.game.world;
    let foe = w.spawn_creature("n_calonord", spot);
    w.object_mut(foe).unwrap().hp = 900;
    for _ in 0..10 {
        s.update(DT);
    }
    shot(&mut gpu, &mut s, &out.join(format!("push_0_before_{tag}.png")));
    let w = &mut s.game.world;
    w.object_mut(pc).unwrap().spells.insert(kotor_game::FORCE_PUSH);
    w.object_mut(pc).unwrap().force = 50;
    let _ = w.cast_spell(pc, kotor_game::FORCE_PUSH, foe, true);
    for i in 1..=3 {
        for _ in 0..8 {
            s.update(DT);
        }
        shot(
            &mut gpu,
            &mut s,
            &out.join(format!("push_{i}_{tag}.png")),
        );
    }
    let a = s.game.world.object(foe).unwrap().position;
    println!(
        "foe moved {:.2} m, disabled {}",
        (a[0] - spot[0]).hypot(a[1] - spot[1]),
        s.game.world.is_disabled(foe)
    );
    // The camera tilted right up.
    s.pitch = 1.2;
    s.distance = 6.0;
    shot(&mut gpu, &mut s, &out.join(format!("camera_tilted_{tag}.png")));
    Ok(())
}
