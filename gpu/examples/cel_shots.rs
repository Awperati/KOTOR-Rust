//! Draws a module's first view three times, with cel shading off, subtle and strong, and once close to the player, and
//! says what each costs a frame.
//!
//!   cargo run --release --example cel_shots -- <module> <out_dir> [width] [height] [frames]
//!
//! Writes `<module>_<height>p_cel0.png` (and `1`, `2`) and `<module>_<height>p_close_cel0.png` (and `1`, `2`), and prints
//! the median time of `frames` frames (default 40) for each level. A frame is timed to the GPU finishing it, with no read-back.
use kotor_gpu::stage::Stage;
use kotor_gpu::GpuRenderer;

const DT: f32 = 1.0 / 60.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let module = args.get(1).cloned().unwrap_or_else(|| "end_m01aa".into());
    let out = std::path::PathBuf::from(args.get(2).cloned().unwrap_or_else(|| ".".into()));
    let width: u32 = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(1280);
    let height: u32 = args.get(4).and_then(|a| a.parse().ok()).unwrap_or(720);
    let frames: usize = args.get(5).and_then(|a| a.parse().ok()).unwrap_or(40);
    std::fs::create_dir_all(&out)?;
    let mut s = Stage::open(&module, true)?;
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let mut gpu = GpuRenderer::new(width, height)?;
    for _ in 0..400 {
        s.update(DT);
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        }
    }
    let scene_static = s.static_scene().clone();
    gpu.set_static_scene(&scene_static, &mut |n| s.texture(n));
    let scene = s.dynamic_scene();
    let cam = s.camera();
    // Close to the player, in front of the face.
    let p = s.game.world.pc_mover.pos;
    let h = s.game.world.pc_mover.heading;
    let close = kotor_render::Camera {
        eye: [p[0] + h.cos() * 1.9, p[1] + h.sin() * 1.9, p[2] + 1.45],
        target: [p[0], p[1], p[2] + 1.2],
        fov: 35.0,
    };
    for (what, cam) in [("", cam), ("close_", close)] {
        for level in 0..=2u8 {
            gpu.set_cel_shading(level);
            let img = gpu.render(&scene, &mut |n| s.texture(n), &cam)?;
            std::fs::write(
                out.join(format!("{module}_{height}p_{what}cel{level}.png")),
                kotor_image::to_png(&img),
            )?;
        }
        // The three levels in turn, round after round, so that the machine's drift lands on all of them.
        let mut times: [Vec<f32>; 3] = Default::default();
        for round in 0..frames + 5 {
            for level in 0..=2u8 {
                gpu.set_cel_shading(level);
                let ms = gpu.draw_ms(&scene, &mut |n| s.texture(n), &cam);
                if round >= 5 {
                    times[level as usize].push(ms);
                }
            }
        }
        let medians: Vec<f32> = times
            .iter_mut()
            .map(|t| {
                t.sort_by(|a, b| a.total_cmp(b));
                t[t.len() / 2]
            })
            .collect();
        println!(
            "{module} {width}x{height} {}: median frame, off {:.2} ms, subtle {:.2} ms, strong {:.2} ms",
            if what.is_empty() { "view" } else { "close-up" },
            medians[0],
            medians[1],
            medians[2]
        );
    }
    Ok(())
}
