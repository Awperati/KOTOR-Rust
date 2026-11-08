//! A Force push over a rail in Upper City South, offscreen: the soldier stands at the edge, the push, and then the soldier is gone
//! over the side. The edge is one of the listed rows of docs/rails.tsv.
//!
//!   cargo run --release --example rail_shots -- [out_dir] [width] [height]
use kotor_game::FORCE_PUSH;
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

const DT: f32 = 1.0 / 30.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).map_or("docs/shots/combat", String::as_str));
    let width: u32 = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(1280);
    let height: u32 = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(720);
    std::fs::create_dir_all(&out)?;
    let mut s = Stage::open("tar_m02ac", true)?;
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let mut gpu = GpuRenderer::new(width, height)?;
    let scene = s.static_scene().clone();
    gpu.set_static_scene(&scene, &mut |n| s.texture(n));
    for _ in 0..90 {
        s.update(DT);
    }
    let pc = s.game.world.pc;
    // An open edge in the listed stretch: floor, a step to the edge and nothing beyond, with room behind it.
    let (at, way) = {
        let w = &s.game.world;
        let nav = w.nav.as_ref().unwrap();
        let mut found = None;
        'scan: for ix in 0..60 {
            for iy in 0..40 {
                let (x, y) = (120.0 + ix as f32, 90.0 + iy as f32);
                let Some(z) = nav.walkable_at(x, y, 3.0) else {
                    continue;
                };
                for (dx, dy) in [(0.0_f32, 1.0_f32), (0.0, -1.0), (1.0, 0.0), (-1.0, 0.0)] {
                    let edge_open = (1..=10).all(|k| {
                        nav.height_at(x + dx * k as f32, y + dy * k as f32, z + 0.4)
                            .is_none()
                    }) && !nav
                        .sight_blocked([x, y, z + 0.6], [x + dx * 4.0, y + dy * 4.0, z + 0.6]);
                    let room = (1..=4).all(|k| {
                        nav.walkable_at(x - dx * k as f32, y - dy * k as f32, z + 0.4)
                            .is_some()
                    });
                    if edge_open && room && (x - 139.0).hypot(y - 107.2) < 40.0 {
                        found = Some(([x, y, z], [dx, dy]));
                        break 'scan;
                    }
                }
            }
        }
        found.ok_or("no open edge near the first listed row")?
    };
    let w = &mut s.game.world;
    let others: Vec<_> = w
        .objects
        .values()
        .filter(|o| o.kind == kotor_game::Kind::Creature && o.id != pc)
        .map(|o| o.id)
        .collect();
    for id in others {
        w.object_mut(id).unwrap().dead = true;
        w.teleport(id, [0.0, 0.0, -300.0]);
    }
    w.combat.target.clear();
    {
        let p = w.object_mut(pc).unwrap();
        p.min_one_hp = true;
        (p.force, p.max_force) = (200, 200);
        p.spells.insert(FORCE_PUSH);
    }
    let victim = w.spawn_creature("end_trask", at);
    {
        let v = w.object_mut(victim).unwrap();
        (v.hp, v.max_hp, v.faction) = (900, 900, 1);
    }
    w.teleport(pc, [at[0] - way[0] * 2.0, at[1] - way[1] * 2.0, at[2]]);
    // Look at the pair from the side, a little above.
    let heading = way[1].atan2(way[0]);
    s.yaw = heading;
    s.distance = 7.0;
    s.pitch = 0.2;
    let shot = |s: &mut Stage, gpu: &mut GpuRenderer, name: &str| {
        s.update(DT);
        let sc = s.dynamic_scene();
        let cam = s.camera();
        gpu.set_particles(s.particles());
        let mut img = gpu.render(&sc, &mut |n| s.texture(n), &cam).unwrap();
        if let Some(o) = s.overlay() {
            composite(&mut img, o);
        }
        let p = out.join(format!("rail_{name}_{height}p.png"));
        std::fs::write(&p, kotor_image::to_png(&img)).unwrap();
        let v = s.game.world.object(victim).unwrap();
        println!(
            "{name}: soldier at {:.1} {:.1} {:.1}, hp {}, dead {}",
            v.position[0], v.position[1], v.position[2], v.hp, v.dead
        );
    };
    for _ in 0..10 {
        s.update(DT);
    }
    shot(&mut s, &mut gpu, "0_at_the_edge");
    s.game.world.cast_spell(pc, FORCE_PUSH, victim, true).ok();
    for _ in 0..8 {
        s.update(DT);
    }
    shot(&mut s, &mut gpu, "1_pushed");
    for _ in 0..10 {
        s.update(DT);
    }
    shot(&mut s, &mut gpu, "2_going_over");
    for _ in 0..90 {
        s.update(DT);
    }
    shot(&mut s, &mut gpu, "3_gone");
    Ok(())
}
