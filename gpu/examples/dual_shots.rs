//! A fight with a weapon in each hand, rendered offscreen (no window, no screen capture): two blaster pistols, then a
//! pair of vibroblades. Frames at the moment both hands have swung, and the attack rolls of each round printed.
//!
//!   cargo run --release --example dual_shots -- [out_dir] [width] [height]
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
    for _ in 0..3000 {
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
    for (name, right, left, distance) in [
        ("blasters", "g_w_blstrpstl001", "g_w_blstrpstl001", 4.0),
        ("short_lightsabers", "g_w_shortsbr01", "g_w_shortsbr01", 1.3),
    ] {
        let w = &mut s.game.world;
        let o = w.object_mut(pc).unwrap();
        o.equipped.remove(&4);
        o.equipped.remove(&5);
        let (r, l) = (w.spawn_item(right), w.spawn_item(left));
        let o = w.object_mut(pc).unwrap();
        o.equipped.insert(4, r);
        o.equipped.insert(5, l);
        o.min_one_hp = true;
        let foe = w.spawn_creature(
            "n_calonord",
            [
                at[0] + heading.cos() * distance,
                at[1] + heading.sin() * distance,
                at[2],
            ],
        );
        let f = w.object_mut(foe).unwrap();
        (f.hp, f.max_hp, f.faction) = (900, 900, 1);
        let log0 = w.combat.log.len();
        w.attack(pc, foe);
        let mut framed = false;
        for _ in 0..(10.0 / DT) as usize {
            s.update(DT);
            if s.game.world.in_dialogue() {
                s.number_key(1);
                s.continue_line();
            }
            let rounds: Vec<u32> = s.game.world.combat.log[log0..]
                .iter()
                .filter(|l| l.attacker == pc)
                .map(|l| l.round)
                .collect();
            let n = rounds.len();
            if n >= 2 && rounds[n - 1] == rounds[n - 2] && !framed {
                framed = true;
                for _ in 0..4 {
                    s.update(DT);
                }
                let sc = s.dynamic_scene();
                let cam = s.camera();
                gpu.set_particles(s.particles());
                let mut img = gpu.render(&sc, &mut |n| s.texture(n), &cam).unwrap();
                if let Some(o) = s.overlay() {
                    composite(&mut img, o);
                }
                let p = out.join(format!("dual_{name}_{height}p.png"));
                std::fs::write(&p, kotor_image::to_png(&img))?;
                println!("wrote {}", p.display());
            }
            if framed && n >= 6 {
                break;
            }
        }
        for l in s.game.world.combat.log[log0..]
            .iter()
            .filter(|l| l.attacker == pc)
            .take(6)
        {
            println!(
                "{name}: round {} d20 {} + {} vs {} {:?} {} damage",
                l.round, l.roll, l.bonus, l.defense, l.outcome, l.damage
            );
        }
        s.game.world.stop_attacking(pc);
        s.game.world.object_mut(foe).unwrap().position = [0.0, 0.0, -100.0];
        s.game.world.object_mut(foe).unwrap().dead = true;
    }
    Ok(())
}
