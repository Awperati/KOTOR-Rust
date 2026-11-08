//! The detection bars, offscreen: three enemies round the player notice them one after another, the bars fill from white to
//! yellow to red, and then drain when the enemies are taken away.
//!
//!   cargo run --release --example detection_shots -- [out_dir] [width] [height]
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
    w.object_mut(pc).unwrap().min_one_hp = true;
    // Three soldiers, ahead, to the left and behind, at different distances. They do not strike back.
    let spots = [(0.0_f32, 5.0_f32), (1.6, 9.0), (3.3, 7.0)];
    let mut foes = Vec::new();
    for (turn, far) in spots {
        let a = heading + turn;
        let id = w.spawn_creature(
            "end_trask",
            [home[0] + a.cos() * far, home[1] + a.sin() * far, home[2]],
        );
        let f = w.object_mut(id).unwrap();
        (f.hp, f.max_hp, f.faction) = (9000, 9000, 1);
        foes.push(id);
    }
    s.yaw = heading;
    s.distance = 7.0;
    s.pitch = 0.3;
    let shot = |s: &mut Stage, gpu: &mut GpuRenderer, name: &str| {
        s.update(DT);
        let sc = s.dynamic_scene();
        let cam = s.camera();
        gpu.set_particles(s.particles());
        let mut img = gpu.render(&sc, &mut |n| s.texture(n), &cam).unwrap();
        if let Some(o) = s.overlay() {
            composite(&mut img, o);
        }
        let p = out.join(format!("detection_{name}_{height}p.png"));
        std::fs::write(&p, kotor_image::to_png(&img)).unwrap();
        let marks: Vec<String> = s
            .game
            .world
            .detection_marks()
            .iter()
            .map(|m| format!("{}:{:.2}{}", m.who, m.fill, if m.sees { "" } else { "~" }))
            .collect();
        println!("{name}: {}", marks.join(" "));
    };
    for (name, frames) in [("0_noticed", 12), ("1_filling", 30), ("2_alerted", 120)] {
        for _ in 0..frames {
            s.update(DT);
            // They only look: whoever is told to attack is dropped again.
            s.game.world.combat.target.clear();
        }
        shot(&mut s, &mut gpu, name);
    }
    // The soldiers go; the bars drain and fade.
    for id in &foes {
        s.game.world.teleport(*id, [0.0, 0.0, -300.0]);
    }
    for (name, frames) in [("3_losing", 30), ("4_gone", 150)] {
        for _ in 0..frames {
            s.update(DT);
        }
        shot(&mut s, &mut gpu, name);
    }
    Ok(())
}
