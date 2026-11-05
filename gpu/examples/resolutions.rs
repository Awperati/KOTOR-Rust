//! The same scene at each supported resolution, HUD and dialogue, rendered offscreen. Prints how long a frame takes.
//!
//!   cargo run --offline --release --manifest-path gpu\Cargo.toml --example resolutions -- [out_dir]
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

const DT: f32 = 1.0 / 30.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).map_or("docs/shots/res", String::as_str));
    std::fs::create_dir_all(&out)?;
    let mut s = Stage::open("end_m01aa", true)?;
    let mut gpu = GpuRenderer::new(1280, 720)?;
    let scene = s.static_scene().clone();
    gpu.set_static_scene(&scene, &mut |n| s.texture(n));
    // On to Trask's replies.
    for _ in 0..6000 {
        s.update(DT);
        match s.game.world.dialogue_view() {
            Some((_, _, _, r)) if !r.is_empty() => break,
            Some(_) => s.continue_line(),
            None => {}
        }
    }
    for _ in 0..30 {
        s.update(DT);
    }
    for (w, h) in [(1280, 720), (1920, 1080), (2560, 1440), (3840, 2160)] {
        gpu.resize(w, h);
        s.set_view((w, h));
        let t = std::time::Instant::now();
        s.update(DT);
        let refreshed = t.elapsed().as_secs_f32();
        let moving = s.dynamic_scene();
        let cam = s.camera();
        let t = std::time::Instant::now();
        let mut img = gpu.render(&moving, &mut |n| s.texture(n), &cam)?;
        let drawn = t.elapsed().as_secs_f32();
        if let Some(o) = s.overlay() {
            composite(&mut img, o);
        }
        std::fs::write(out.join(format!("talk_{h}.png")), kotor_image::to_png(&img))?;
        println!(
            "{w}x{h}: overlay and step {:.0} ms, draw and read back {:.0} ms",
            refreshed * 1000.0,
            drawn * 1000.0
        );
    }
    Ok(())
}
