//! Frames of Medication Sickness: the consumables slot with no cooldown, the cooldown full and half gone, and the question.
//! Offscreen: no window, no screen capture, and a settings file of its own.
//!
//!   cargo run --release --example med_shots -- [out_dir] [width] [height]
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).map_or("audit", String::as_str));
    let width: u32 = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(1280);
    let height: u32 = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(720);
    let size = (width, height);
    std::fs::create_dir_all(&out)?;

    let mut s = Stage::open("end_m01aa", true)?;
    s.set_view(size);
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let settings = std::env::temp_dir().join(format!("kotor_med_shots_{}.txt", std::process::id()));
    std::fs::write(&settings, "")?;
    s.use_settings_file(settings);
    let pc = s.game.world.pc;
    let packs: Vec<u32> = (0..4)
        .map(|_| s.game.world.spawn_item("g_i_medeqpmnt01"))
        .collect();
    let o = s.game.world.object_mut(pc).unwrap();
    o.inventory.extend(packs);
    o.hp = 1;
    s.update(1.0 / 30.0);
    let mut gpu = GpuRenderer::new(width, height)?;
    let scene = s.static_scene().clone();
    gpu.set_static_scene(&scene, &mut |n| s.texture(n));
    let mut shot = |s: &mut Stage, name: &str| {
        s.update(1.0 / 1000.0);
        let scene = s.dynamic_scene();
        let cam = s.camera();
        let mut img = gpu.render(&scene, &mut |n| s.texture(n), &cam).unwrap();
        if let Some(o) = s.overlay() {
            composite(&mut img, o);
        }
        let p = out.join(format!("{name}_{height}.png"));
        std::fs::write(&p, kotor_image::to_png(&img)).unwrap();
        println!(
            "wrote {} ({}) veil={:?}",
            p.display(),
            s.menu_name(),
            s.quick_slot_shown(0).map(|q| q.veil)
        );
    };
    let slot = s
        .hud_pixel("BTN_ACTION1", size)
        .expect("the consumables slot");
    shot(&mut s, "med_slot_0");
    s.hover(slot, size);
    s.click(slot, size);
    s.hover((width as f32 * 0.5, height as f32 * 0.3), size);
    shot(&mut s, "med_slot_100");
    s.game.world.time += 10.0;
    shot(&mut s, "med_slot_50");
    s.click(slot, size);
    shot(&mut s, "med_warning");
    s.capture_key("KeyY");
    s.open_character();
    shot(&mut s, "med_character");
    Ok(())
}
