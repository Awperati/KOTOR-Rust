//! Frames of the portrait made from the player's face: the head as drawn for it, and the HUD and the character sheet with
//! it. Offscreen: no window, no screen capture, and a settings file of its own.
//!
//!   cargo run --release --example portrait_shots -- [out_dir] [width] [height]
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
    let settings =
        std::env::temp_dir().join(format!("kotor_portrait_shots_{}.txt", std::process::id()));
    std::fs::write(&settings, "")?;
    s.use_settings_file(settings);
    let pc = s.game.world.pc;
    let o = s.game.world.object_mut(pc).unwrap();
    // `PORTRAIT_LOOK=race=2` for another look: the Jawa's, with `KOTOR_MOD_PACKS=on`.
    let look: Vec<(String, i32)> = match std::env::var("PORTRAIT_LOOK") {
        Ok(l) => l
            .split(',')
            .filter_map(|kv| kv.split_once('='))
            .filter_map(|(k, v)| Some((k.to_string(), v.parse().ok()?)))
            .collect(),
        Err(_) => [
            ("nose", 60),
            ("jaw", 40),
            ("tattoo_cheeks", 100),
            ("paint_lips", 80),
        ]
        .map(|(k, v)| (k.to_string(), v))
        .to_vec(),
    };
    for (k, v) in look {
        o.look.sliders.insert(k, v);
    }
    s.update(1.0 / 30.0);
    let mut gpu = GpuRenderer::new(width, height)?;
    let scene = s.static_scene().clone();
    gpu.set_static_scene(&scene, &mut |n| s.texture(n));

    let (name, head, cam) = s.face_portrait_wanted().expect("a face to draw");
    gpu.set_figure_only(true);
    let drawn = gpu.render(&head, &mut |n| s.texture(n), &cam)?;
    std::fs::write(
        out.join(format!("portrait_drawn_{height}.png")),
        kotor_image::to_png(&drawn),
    )?;
    s.set_face_portrait(&name, &drawn);
    println!("made {name}: the player's picture is {}", s.pc_portrait());

    let mut shot = |s: &mut Stage, name: &str| {
        s.update(1.0 / 1000.0);
        let (scene, cam, figure) = s.frame_scene();
        gpu.set_figure_only(figure);
        let mut img = gpu.render(&scene, &mut |n| s.texture(n), &cam).unwrap();
        if let Some(o) = s.overlay() {
            composite(&mut img, o);
        }
        let p = out.join(format!("{name}_{height}.png"));
        std::fs::write(&p, kotor_image::to_png(&img)).unwrap();
        println!("wrote {} ({})", p.display(), s.menu_name());
    };
    shot(&mut s, "portrait_hud");
    s.open_character();
    shot(&mut s, "portrait_character");
    Ok(())
}
