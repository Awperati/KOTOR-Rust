//! Walks through a joined door and reports what the world and the picture hold afterwards: creatures by tag, and what each
//! neighbour joined would draw.
//!   cargo run --release --example probe_cross -- tar_m02ac Apartments [out.png]
use kotor_gpu::stage::Stage;
use kotor_gpu::GpuRenderer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let module = args.get(1).cloned().unwrap_or_else(|| "tar_m02ac".into());
    let tag = args.get(2).cloned().unwrap_or_else(|| "Apartments".into());
    let mut s = Stage::open(&module, true)?;
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let sketch = s.game.world.sketch();
    let site = sketch
        .sites
        .iter()
        .find(|x| x.tag == tag)
        .ok_or("no such door")?
        .clone();
    let out = site.out.ok_or("no side")?;
    let pc = s.game.world.pc;
    s.game.world.teleport(
        pc,
        [
            site.at[0] - out.cos() * 3.0,
            site.at[1] - out.sin() * 3.0,
            site.at[2],
        ],
    );
    s.yaw = out;
    for _ in 0..600 {
        s.update(1.0 / 60.0);
        std::thread::sleep(std::time::Duration::from_millis(2));
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        }
    }
    let report = |s: &Stage, what: &str| {
        println!("-- {what}: in {}", s.game.world.module);
        let mut counts = std::collections::BTreeMap::<String, usize>::new();
        for o in s
            .game
            .world
            .objects
            .values()
            .filter(|o| o.kind == kotor_game::Kind::Creature)
        {
            *counts.entry(o.template.clone()).or_default() += 1;
        }
        println!(
            "   world creatures: {:?}",
            counts.iter().filter(|(_, n)| **n > 1).collect::<Vec<_>>()
        );
        println!(
            "   larrims in the world: {}",
            s.game
                .world
                .objects
                .values()
                .filter(|o| o.tag.to_lowercase().contains("larrim"))
                .count()
        );
        for o in s
            .game
            .world
            .objects
            .values()
            .filter(|o| o.tag.to_lowercase().contains("larrim"))
        {
            println!(
                "   {:?} #{} tag {} template {} at {:?} spawned {}",
                o.kind, o.id, o.tag, o.template, o.position, o.spawned
            );
        }
        for a in s.game.world.seamless.attached.values() {
            println!(
                "   attached {} behind {} portal {} larrims {}",
                a.module,
                a.behind,
                a.portal,
                a.scenery
                    .figures
                    .iter()
                    .filter(|f| f.template.contains("larrim"))
                    .count()
            );
        }
    };
    report(&s, "before");
    if let Ok(to) = std::env::var("TRANSITION") {
        // The old way: a loading screen into the very module that is joined.
        s.game
            .world
            .transition(&to, "")
            .map_err(|e| e.to_string())?;
    } else {
        s.game.world.use_door(
            s.game
                .world
                .objects
                .values()
                .find(|o| o.tag == tag)
                .map(|o| o.id)
                .ok_or("no door")?,
        );
    }
    s.steer = (1.0, 0.0, false);
    for _ in 0..900 {
        s.update(1.0 / 60.0);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    s.steer = (0.0, 0.0, false);
    for _ in 0..300 {
        s.update(1.0 / 60.0);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    report(&s, "after");
    if let Some(path) = args.get(3) {
        let mut gpu = GpuRenderer::new(1280, 720)?;
        let scene_static = s.static_scene().clone();
        gpu.set_static_scene(&scene_static, &mut |n| s.texture(n));
        let scene = s.dynamic_scene();
        let mut cam = s.camera();
        if let Some(l) = s
            .game
            .world
            .objects
            .values()
            .find(|o| o.tag == "tar02_larrim")
            .map(|o| o.position)
        {
            cam = kotor_render::Camera {
                eye: [l[0] + 2.0, l[1] - 2.0, l[2] + 1.6],
                target: [l[0], l[1], l[2] + 1.1],
                fov: 40.0,
            };
        }
        let img = gpu.render(&scene, &mut |n| s.texture(n), &cam)?;
        std::fs::write(path, kotor_image::to_png(&img))?;
    }
    Ok(())
}
