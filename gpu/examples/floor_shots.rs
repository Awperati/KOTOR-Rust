//! A blaster aimed down at someone knocked flat, and a blade finishing them, offscreen from the side: frames a few moments apart.
//!
//!   cargo run --release --example floor_shots -- [out_dir] [width] [height]
use kotor_game::{EffectRec, DURATION_TEMPORARY};
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

const DT: f32 = 1.0 / 30.0;

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
    let (pc, home, heading) = {
        let w = &s.game.world;
        (w.pc, w.object(w.pc).unwrap().position, w.pc_mover.heading)
    };
    for (name, item, metres, times) in [
        (
            "blaster",
            "g_w_blstrpstl001",
            3.0f32,
            vec![0.2, 0.5, 0.9, 1.3],
        ),
        ("rifle", "g_w_blstrrfl001", 3.0, vec![0.2, 0.5, 0.9, 1.3]),
        ("blade", "g_w_lngswrd01", 1.4, vec![0.2, 0.6, 1.0, 1.4]),
    ] {
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
        let pcw = w.object_mut(pc).unwrap();
        pcw.equipped.remove(&5);
        pcw.min_one_hp = true;
        let it = w.spawn_item(item);
        w.object_mut(pc).unwrap().equipped.insert(4, it);
        let at = [
            home[0] + heading.cos() * metres,
            home[1] + heading.sin() * metres,
            home[2],
        ];
        let foe = w.spawn_creature("n_calonord", at);
        {
            let f = w.object_mut(foe).unwrap();
            (f.hp, f.max_hp, f.faction, f.natural_ac) = (9000, 9000, 1, 0);
        }
        w.effects.push(EffectRec::new("EffectKnockdown"));
        let handle = w.effects.len() as u32;
        w.apply_effect(pc, foe, handle, DURATION_TEMPORARY, 20.0);
        w.attack(pc, foe);
        s.yaw = heading + 2.6;
        s.distance = 7.0;
        s.pitch = 0.3;
        let mut t = 0.0;
        for (k, when) in times.iter().enumerate() {
            while t < *when {
                s.update(DT);
                t += DT;
            }
            let sc = s.dynamic_scene();
            let cam = s.camera();
            gpu.set_particles(s.particles());
            let mut img = gpu.render(&sc, &mut |n| s.texture(n), &cam).unwrap();
            if let Some(o) = s.overlay() {
                composite(&mut img, o);
            }
            let p = out.join(format!("floor_{name}_{k}_{height}p.png"));
            std::fs::write(&p, kotor_image::to_png(&img))?;
            let clip = s.game.clip(pc).cloned();
            println!(
                "{name} {when:.2} s: clip {} aim {:.0} on the floor {} foe clip {} dist {:.1}",
                clip.as_ref().map_or("-", |c| c.name),
                s.aim_down_of(pc, &clip),
                s.game.world.is_prone(foe),
                s.game
                    .clip(foe)
                    .map_or("-".to_string(), |c| format!("{}@{:.2}", c.name, c.time)),
                {
                    let (a, b) = (
                        s.game.world.object(pc).unwrap().position,
                        s.game.world.object(foe).unwrap().position,
                    );
                    (a[0] - b[0]).hypot(a[1] - b[1])
                }
            );
        }
        s.game.world.stop_attacking(pc);
        s.game.world.object_mut(foe).unwrap().dead = true;
    }
    Ok(())
}
