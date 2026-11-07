//! Bug 18: a soldier in a fight keeps a fighting stance between rounds. Offscreen frames of one with a vibroblade and one with a
//! blaster, taken between two of its rounds, with the clip names it played and the seconds it stood idle.
//!
//!   cargo run --release --example stance_shots -- [out_dir] [width] [height]
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
    for (name, gun, distance) in [
        ("blade", None, 1.4),
        ("blaster", Some("g_w_blstrpstl001"), 5.0),
    ] {
        let w = &mut s.game.world;
        w.object_mut(pc).unwrap().min_one_hp = true;
        let foe = w.spawn_creature(
            "n_sithsoldier003",
            [
                at[0] + heading.cos() * distance,
                at[1] + heading.sin() * distance,
                at[2],
            ],
        );
        if let Some(g) = gun {
            let item = w.spawn_item(g);
            let f = w.object_mut(foe).unwrap();
            f.equipped.remove(&4);
            f.equipped.insert(4, item);
        }
        let f = w.object_mut(foe).unwrap();
        (f.hp, f.max_hp, f.faction) = (900, 900, 1);
        w.attack(foe, pc);
        let (mut idle, mut fighting, mut framed, mut names) =
            (0.0f32, 0.0f32, false, Vec::<String>::new());
        let log0 = s.game.world.combat.log.len();
        for i in 0..(14.0 / DT) as usize {
            s.update(DT);
            if s.game.world.in_dialogue() {
                s.number_key(1);
                s.continue_line();
            }
            let clip = s.game.clip(foe).map_or("-", |c| c.name);
            if names.last().map(String::as_str) != Some(clip) {
                names.push(clip.to_string());
            }
            let n = s.game.world.combat.log[log0..]
                .iter()
                .filter(|l| l.attacker == foe)
                .count();
            if n >= 1 {
                fighting += DT;
                if clip == "pause1" {
                    idle += DT;
                }
            }
            // Between two rounds: a second and a half after its first attack.
            if n >= 1 && !framed && i > 0 {
                let first = s.game.world.combat.log[log0..]
                    .iter()
                    .find(|l| l.attacker == foe)
                    .map(|l| l.round)
                    .unwrap();
                if s.game.world.combat.round > first && fighting > 1.5 {
                    framed = true;
                    let sc = s.dynamic_scene();
                    let cam = s.camera();
                    gpu.set_particles(s.particles());
                    let mut img = gpu.render(&sc, &mut |n| s.texture(n), &cam).unwrap();
                    if let Some(o) = s.overlay() {
                        composite(&mut img, o);
                    }
                    let p = out.join(format!("stance_{name}_{height}p.png"));
                    std::fs::write(&p, kotor_image::to_png(&img))?;
                    println!("wrote {} (clip {clip})", p.display());
                }
            }
        }
        println!("{name}: fought {fighting:.1} s, idle {idle:.2} s, clips {names:?}");
        s.game.world.stop_attacking(pc);
        s.game.world.object_mut(foe).unwrap().position = [0.0, 0.0, -100.0];
        s.game.world.object_mut(foe).unwrap().dead = true;
    }
    Ok(())
}
