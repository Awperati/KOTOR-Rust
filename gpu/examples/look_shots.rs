//! Frames of a Custom character's Appearance page. Offscreen: no window, no screen capture, no saves.
//!
//!   cargo run --release --example look_shots -- [out_dir] [width] [height]
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
    s.enable_chargen(true);
    let click = |s: &mut Stage, tag: &str| {
        let p = s.menu_pixel(tag, None, size).expect("the button");
        s.hover(p, size);
        s.click(p, size);
        s.update(0.0);
    };
    let mut shot = |s: &mut Stage, name: &str| {
        s.update(0.0);
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
        println!("wrote {}", p.display());
    };
    click(&mut s, "BTN_NEWGAME");
    click(&mut s, "BTN_SEL4");
    click(&mut s, "CUST_CHAR_BTN");
    shot(&mut s, "look_steps");
    click(&mut s, "BTN_STEPNAME1");
    shot(&mut s, "look_page");
    if let Some(p) = s.menu_step_pixel("LB_OPTIONS", 1, 1, size) {
        s.hover(p, size);
        s.click(p, size);
    }
    shot(&mut s, "look_large");
    Ok(())
}
