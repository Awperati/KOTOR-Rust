//! Frames of the main menu with the owner's title card. Offscreen: no window, no screen capture, no saves.
//!
//!   cargo run --release --example title_shots -- [out_dir] [width] [height]
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

const DT: f32 = 1.0 / 30.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).map_or("audit", String::as_str));
    let width: u32 = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(1280);
    let height: u32 = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(720);
    std::fs::create_dir_all(&out)?;
    let mut s = Stage::menu("end_m01aa", true)?;
    s.set_view((width, height));
    let mut gpu = GpuRenderer::new(width, height)?;
    for _ in 0..30 {
        s.update(DT);
    }
    let (moving, cam, figure) = s.frame_scene();
    gpu.set_figure_only(figure);
    gpu.set_particles(s.frame_particles());
    let mut img = gpu.render(&moving, &mut |n| s.texture(n), &cam)?;
    if let Some(o) = s.overlay() {
        composite(&mut img, o);
    }
    let p = out.join(format!("title_{height}.png"));
    std::fs::write(&p, kotor_image::to_png(&img))?;
    println!("wrote {}", p.display());
    Ok(())
}
