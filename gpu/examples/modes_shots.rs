//! Frames of the main menu with Single Player and Multiplayer, the Classic or Modern question, and the Multiplayer panel.
//! Offscreen: no window, no screen capture, and a settings file of its own.
//!
//!   cargo run --release --example modes_shots -- [out_dir] [width] [height]
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).map_or("audit", String::as_str));
    let width: u32 = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(1280);
    let height: u32 = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(720);
    let size = (width, height);
    std::fs::create_dir_all(&out)?;

    let mut gpu = GpuRenderer::new(width, height)?;
    let mut s = Stage::menu("end_m01aa", true)?;
    s.set_view(size);
    let settings =
        std::env::temp_dir().join(format!("kotor_modes_shots_{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&settings);
    s.use_settings_file(settings);
    s.enable_mode_choice(true);
    let hover = |s: &mut Stage, tag: &str| {
        let p = s.menu_pixel(tag, None, size).expect("the button");
        s.hover(p, size);
        s.update(0.0);
        p
    };
    let click = |s: &mut Stage, tag: &str| {
        let p = hover(s, tag);
        s.click(p, size);
        s.update(0.0);
    };
    let mut shot = |s: &mut Stage, name: &str| {
        s.update(1.0 / 30.0);
        let (_, too_big) = s.words_fit();
        if !too_big.is_empty() {
            println!("does not fit on {name}: {too_big:?}");
        }
        let scene = s.static_scene().clone();
        gpu.set_static_scene(&scene, &mut |n| s.texture(n));
        let (moving, cam, figure) = s.frame_scene();
        gpu.set_figure_only(figure);
        gpu.set_particles(s.frame_particles());
        let mut img = gpu.render(&moving, &mut |n| s.texture(n), &cam).unwrap();
        if let Some(o) = s.overlay() {
            composite(&mut img, o);
        }
        let p = out.join(format!("{name}_{height}.png"));
        std::fs::write(&p, kotor_image::to_png(&img)).unwrap();
        println!("wrote {} ({})", p.display(), s.menu_name());
    };
    hover(&mut s, "BTN_NEWGAME");
    shot(&mut s, "modes_main");
    click(&mut s, "BTN_NEWGAME");
    hover(&mut s, "QUICK_CHAR_BTN");
    shot(&mut s, "modes_choice_classic");
    let modern = hover(&mut s, "CUST_CHAR_BTN");
    shot(&mut s, "modes_choice_modern");
    // Where Modern is, in a 640 by 480 layout, for a driver.
    println!(
        "modern at layout {:.0},{:.0}",
        modern.0 * 640.0 / width as f32,
        modern.1 * 480.0 / height as f32
    );
    click(&mut s, "BTN_BACK");
    click(&mut s, "BTN_MOVIES");
    shot(&mut s, "modes_multiplayer");
    Ok(())
}
