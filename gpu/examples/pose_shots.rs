//! Frames of two animation sets, offscreen: the player with a two-handed sword, ready and attacking, and the player using a
//! medpac. Each frame says which clip is playing.
//!
//!   cargo run --release --example pose_shots -- [out_dir] [width] [height]
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

const DT: f32 = 1.0 / 30.0;

fn shot(
    s: &mut Stage,
    gpu: &mut GpuRenderer,
    out: &std::path::Path,
    name: &str,
    height: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let sc = s.dynamic_scene();
    let cam = s.camera();
    gpu.set_particles(s.particles());
    let mut img = gpu.render(&sc, &mut |n| s.texture(n), &cam).unwrap();
    if let Some(o) = s.overlay() {
        composite(&mut img, o);
    }
    let p = out.join(format!("{name}_{height}p.png"));
    std::fs::write(&p, kotor_image::to_png(&img))?;
    let pc = s.game.world.pc;
    println!(
        "wrote {} (player clip {})",
        p.display(),
        s.game.clip(pc).map_or("-", |c| c.name)
    );
    Ok(())
}

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
    for _ in 0..3600 {
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
    s.yaw = heading + 2.4;

    // 1. A two-handed sword: ready, then a blow.
    let w = &mut s.game.world;
    w.object_mut(pc).unwrap().min_one_hp = true;
    let sword = w.spawn_item("g_w_dblswrd001");
    w.equip(pc, sword, Some(4)).unwrap();
    let foe = w.spawn_creature(
        "n_sithsoldier003",
        [
            at[0] + heading.cos() * 1.6,
            at[1] + heading.sin() * 1.6,
            at[2],
        ],
    );
    let f = w.object_mut(foe).unwrap();
    (f.hp, f.max_hp, f.faction) = (900, 900, 1);
    w.attack(pc, foe);
    w.attack(foe, pc);
    let (mut ready, mut hit) = (false, false);
    for _ in 0..(12.0 / DT) as usize {
        s.update(DT);
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        }
        let clip = s.game.clip(pc).map_or("-", |c| c.name);
        if !ready && clip == "g3r1" {
            ready = true;
            shot(&mut s, &mut gpu, &out, "twohand_ready", height)?;
        }
        if !hit && clip == "c3a1" && s.game.clip(pc).is_some_and(|c| c.time > 0.5) {
            hit = true;
            shot(&mut s, &mut gpu, &out, "twohand_attack", height)?;
        }
        if ready && hit {
            break;
        }
    }
    let w = &mut s.game.world;
    w.stop_attacking(pc);
    w.stop_attacking(foe);
    w.object_mut(foe).unwrap().position = [0.0, 0.0, -100.0];
    w.object_mut(foe).unwrap().dead = true;
    for _ in 0..900 {
        s.update(DT);
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        }
    }

    // 2. A medpac.
    let w = &mut s.game.world;
    let medpac = w.spawn_item("g_i_medeqpmnt01");
    w.carry(pc, medpac);
    w.use_item_on(pc, medpac, pc).unwrap();
    for (k, steps) in [(0, 6), (1, 9), (2, 9)] {
        for _ in 0..steps {
            s.update(DT);
            if s.game.world.in_dialogue() {
                s.number_key(1);
                s.continue_line();
            }
        }
        shot(&mut s, &mut gpu, &out, &format!("medpac_{k}"), height)?;
    }
    Ok(())
}
