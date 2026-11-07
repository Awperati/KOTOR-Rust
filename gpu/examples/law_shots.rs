//! An authority sees a crime and turns on the player, offscreen: the player in attack mode strikes a civilian in the Upper City of
//! Taris with a Sith patrol in sight. Frames before, as the blow lands, as the patrol turns, and as it fights.
//!
//!   cargo run --release --example law_shots -- [out_dir] [width] [height]
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

const DT: f32 = 1.0 / 30.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).map_or("docs/shots/combat", String::as_str));
    let width: u32 = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(1280);
    let height: u32 = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(720);
    std::fs::create_dir_all(&out)?;
    let mut s = Stage::open("tar_m02ab", true)?;
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let mut gpu = GpuRenderer::new(width, height)?;
    let scene = s.static_scene().clone();
    gpu.set_static_scene(&scene, &mut |n| s.texture(n));
    for _ in 0..90 {
        s.update(DT);
    }
    let w = &mut s.game.world;
    let pc = w.pc;
    let at = w.object(pc).unwrap().position;
    w.object_mut(pc).unwrap().min_one_hp = true;
    w.set_attack_mode(true);
    let find = |w: &kotor_game::World, part: &str| {
        w.objects
            .values()
            .find(|o| o.template.contains(part) && !o.dead)
            .map(|o| o.id)
    };
    let (Some(patrol), Some(civ)) = (find(w, "sithpat"), find(w, "drunk")) else {
        println!("no patrol or no civilian here");
        return Ok(());
    };
    // Both within sight of the player, about 3 and 5 m away.
    let spot = |w: &kotor_game::World, metres: f32, from: usize| {
        (0..16).map(|k| (k + from) % 16).find_map(|k| {
            let a = k as f32 * std::f32::consts::TAU / 16.0;
            let (x, y) = (at[0] + a.cos() * metres, at[1] + a.sin() * metres);
            let z = w.nav.as_ref()?.height_at(x, y, at[2] + 0.4)?;
            w.can_see_along([x, y, z + 1.5], [at[0], at[1], at[2] + 1.5])
                .then_some([x, y, z])
        })
    };
    if let Some(p) = spot(w, 2.0, 0) {
        w.teleport(civ, p);
    }
    if let Some(p) = spot(w, 5.0, 8) {
        w.teleport(patrol, p);
    }
    {
        let (p, c) = (
            w.object(patrol).unwrap().position,
            w.object(civ).unwrap().position,
        );
        println!(
            "player {at:?} civilian {c:?} patrol {p:?}, patrol range {}",
            w.object(patrol).unwrap().perception
        );
    }
    println!(
        "module {:?} authority {} rep {} sight {}",
        w.module,
        w.is_authority(patrol),
        w.reputation(patrol, pc),
        w.can_see_along(
            [
                w.object(patrol).unwrap().position[0],
                w.object(patrol).unwrap().position[1],
                1.5
            ],
            [at[0], at[1], 1.5]
        )
    );
    let heading = {
        let (c, p) = (
            w.object(civ).unwrap().position,
            w.object(patrol).unwrap().position,
        );
        ((c[1] + p[1]) / 2.0 - at[1]).atan2((c[0] + p[0]) / 2.0 - at[0])
    };
    s.yaw = heading + 0.6;
    s.distance = 7.0;
    let f = s.game.world.object_mut(civ).unwrap();
    (f.hp, f.max_hp) = (200, 200);
    let mut shot = |s: &mut Stage, name: &str| -> Result<(), Box<dyn std::error::Error>> {
        let sc = s.dynamic_scene();
        let cam = s.camera();
        gpu.set_particles(s.particles());
        let mut img = gpu.render(&sc, &mut |n| s.texture(n), &cam).unwrap();
        if let Some(o) = s.overlay() {
            composite(&mut img, o);
        }
        let p = out.join(format!("law_{name}_{height}p.png"));
        std::fs::write(&p, kotor_image::to_png(&img))?;
        let w = &s.game.world;
        println!(
            "wrote {} (wanted {}, {} authorities hostile)",
            p.display(),
            w.wanted_level(),
            w.law.hostile.len()
        );
        Ok(())
    };
    for _ in 0..20 {
        s.update(DT);
    }
    shot(&mut s, "0_before")?;
    s.game.world.click(civ);
    for _ in 0..(6.0 / DT) as usize {
        s.update(DT);
        if s.game.world.wanted_level() > 0 {
            break;
        }
    }
    println!(
        "hostile {:?} witnessed {:?} events {:?}",
        s.game.world.law.hostile,
        s.game.world.law.witnessed,
        s.game.world.events.iter().rev().take(4).collect::<Vec<_>>()
    );
    shot(&mut s, "1_crime")?;
    for _ in 0..30 {
        s.update(DT);
    }
    shot(&mut s, "2_patrol_turns")?;
    for _ in 0..90 {
        s.update(DT);
    }
    shot(&mut s, "3_patrol_fights")?;
    Ok(())
}
