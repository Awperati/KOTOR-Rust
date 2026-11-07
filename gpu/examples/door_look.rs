//! Draws a module's door open, from some way in front of it, with the neighbour joined or not.
//!   cargo run --release --example door_look -- <module> <door tag> <out.png> <on|off> [metres back] [frames]
use kotor_gpu::stage::Stage;
use kotor_gpu::GpuRenderer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let module = args.get(1).cloned().unwrap_or_else(|| "tar_m02aa".into());
    let tag = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| "tar02_aprtdoor".into());
    let out = args.get(3).cloned().unwrap_or_else(|| "door.png".into());
    let on = args.get(4).is_none_or(|a| a == "on");
    let back: f32 = args.get(5).and_then(|a| a.parse().ok()).unwrap_or(4.0);
    let frames: usize = args.get(6).and_then(|a| a.parse().ok()).unwrap_or(300);
    let mut s = Stage::open(&module, true)?;
    let file = std::env::temp_dir().join("door_look_settings.txt");
    std::fs::write(&file, format!("seamless_areas={on}\n"))?;
    s.use_settings_file(file);
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let sketch = s.game.world.sketch();
    let site = sketch
        .sites
        .iter()
        .find(|x| x.tag == tag)
        .ok_or("no such door")?
        .clone();
    let out_angle = site.out.ok_or("no side")?;
    let pc = s.game.world.pc;
    s.game.world.teleport(
        pc,
        [
            site.at[0] - out_angle.cos() * back,
            site.at[1] - out_angle.sin() * back,
            site.at[2],
        ],
    );
    s.yaw = out_angle;
    let mut gpu = GpuRenderer::new(1280, 720)?;
    for _ in 0..frames {
        s.update(1.0 / 60.0);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    // The door open at once.
    let id = s
        .game
        .world
        .objects
        .values()
        .find(|o| o.tag == tag)
        .map(|o| o.id)
        .ok_or("no door object")?;
    if let Some(o) = s.game.world.objects.get_mut(&id) {
        o.open = true;
        o.door = kotor_game::DoorState::Open;
    }
    for _ in 0..30 {
        s.update(1.0 / 60.0);
    }
    let scene_static = s.static_scene().clone();
    gpu.set_static_scene(&scene_static, &mut |n| s.texture(n));
    let scene = s.dynamic_scene();
    let cam = s.camera();
    let img = gpu.render(&scene, &mut |n| s.texture(n), &cam)?;
    std::fs::write(&out, kotor_image::to_png(&img))?;
    println!(
        "wrote {out}, joined: {:?}",
        s.game
            .world
            .seamless
            .attached
            .values()
            .map(|a| a.module.clone())
            .collect::<Vec<_>>()
    );
    Ok(())
}
