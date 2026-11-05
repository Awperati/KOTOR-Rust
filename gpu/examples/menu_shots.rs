//! Frames of the menus for looking at, at 1280x720 and 3840x2160, with no window and no screen capture.
//!
//!   cargo run --offline --release --manifest-path gpu\Cargo.toml --example menu_shots -- [out_dir]
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

const SIZES: [(u32, u32); 2] = [(1280, 720), (3840, 2160)];

fn shot(gpu: &mut GpuRenderer, s: &mut Stage, name: &str, out: &std::path::Path) {
    let scene = s.static_scene().clone();
    gpu.set_static_scene(&scene, &mut |n| s.texture(n));
    for size in SIZES {
        gpu.resize(size.0, size.1);
        s.set_view(size);
        s.update(0.0);
        let moving = s.dynamic_scene();
        let cam = s.camera();
        let mut img = gpu.render(&moving, &mut |n| s.texture(n), &cam).unwrap();
        if let Some(o) = s.overlay() {
            composite(&mut img, o);
        }
        let path = out.join(format!("{name}_{}.png", size.1));
        std::fs::write(&path, kotor_image::to_png(&img)).unwrap();
        println!("wrote {}", path.display());
    }
}

fn hover(s: &mut Stage, tag: &str) {
    let size = (1280, 720);
    s.set_view(size);
    if let Some(p) = s.menu_pixel(tag, None, size) {
        s.hover(p, size);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).map_or("docs/shots/menus", String::as_str));
    std::fs::create_dir_all(&out)?;
    let mut gpu = GpuRenderer::new(SIZES[0].0, SIZES[0].1)?;
    let mut s = Stage::menu("end_m01aa", true)?;
    hover(&mut s, "BTN_NEWGAME");
    shot(&mut gpu, &mut s, "main_menu", &out);
    Ok(())
}
