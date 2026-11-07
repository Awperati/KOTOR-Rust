//! Draws a module's first view with each anti-aliasing and motion blur level, and says what each costs a frame.
//!
//!   cargo run --release --example aa_shots -- <module> <out_dir> [width] [height] [frames]
//!
//! Writes `<module>_<height>p_aa{0..3}.png` (anti-aliasing off, 4x, 8x, 16x; motion blur off),
//! `<module>_<height>p_mb{0..3}.png` (a camera turning 90 degrees a second; anti-aliasing off) and
//! `<module>_<height>p_mb2_{30,144}fps.png` (the same turn, drawn at 30 and at 144 frames a second, at Moderate: they
//! should look alike). Prints the median of `frames` frames (default 60) for a set of settings, timed to the GPU
//! finishing, with no read-back, the camera turning every frame.
use kotor_gpu::stage::Stage;
use kotor_gpu::GpuRenderer;
use kotor_render::Camera;

const DT: f32 = 1.0 / 60.0;

/// The camera turned `deg` degrees about the straight-up axis, on the spot.
fn turned(cam: &Camera, deg: f32) -> Camera {
    let a = deg.to_radians();
    let d = [
        cam.target[0] - cam.eye[0],
        cam.target[1] - cam.eye[1],
        cam.target[2] - cam.eye[2],
    ];
    let (s, c) = a.sin_cos();
    Camera {
        eye: cam.eye,
        target: [
            cam.eye[0] + d[0] * c - d[1] * s,
            cam.eye[1] + d[0] * s + d[1] * c,
            cam.eye[2] + d[2],
        ],
        fov: cam.fov,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let module = args.get(1).cloned().unwrap_or_else(|| "end_m01aa".into());
    let out = std::path::PathBuf::from(args.get(2).cloned().unwrap_or_else(|| ".".into()));
    let width: u32 = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(1280);
    let height: u32 = args.get(4).and_then(|a| a.parse().ok()).unwrap_or(720);
    let frames: usize = args.get(5).and_then(|a| a.parse().ok()).unwrap_or(60);
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
    println!(
        "{module} {width}x{height}: the card has up to {} samples a pixel",
        gpu.max_samples()
    );
    let save = |name: String, img: &kotor_image::Image| -> Result<(), Box<dyn std::error::Error>> {
        std::fs::write(
            out.join(format!("{module}_{height}p_{name}.png")),
            kotor_image::to_png(img),
        )?;
        Ok(())
    };

    // Anti-aliasing, a still view.
    gpu.set_motion_blur(0);
    for level in 0..=3u8 {
        gpu.set_anti_aliasing(level);
        let img = gpu.render(&scene, &mut |n| s.texture(n), &cam)?;
        save(format!("aa{level}"), &img)?;
        println!("aa{level}: {} samples a pixel", gpu.samples());
    }

    // Motion blur: the view turning 90 degrees a second. One frame to remember, then the one that is kept.
    gpu.set_anti_aliasing(0);
    for (fps, tag) in [
        (60.0f32, None),
        (30.0, Some("30fps")),
        (144.0, Some("144fps")),
    ] {
        let dt = 1.0 / fps;
        let step = 90.0 * dt;
        for level in if tag.is_some() { 2..=2u8 } else { 0..=3u8 } {
            gpu.set_motion_blur(level);
            gpu.set_frame_time(Some(dt));
            gpu.set_motion_blur_suspended(true);
            gpu.set_motion_blur_suspended(false);
            gpu.render(&scene, &mut |n| s.texture(n), &turned(&cam, 0.0))?;
            let img = gpu.render(&scene, &mut |n| s.texture(n), &turned(&cam, step))?;
            match tag {
                Some(t) => save(format!("mb{level}_{t}"), &img)?,
                None => save(format!("mb{level}"), &img)?,
            }
        }
    }
    gpu.set_frame_time(Some(DT));

    // Cost, with the camera turning each frame so that the blur has something to do.
    let configs: [(&str, u8, u8); 6] = [
        ("off/off", 0, 0),
        ("4x/off", 1, 0),
        ("4x/subtle (default)", 1, 1),
        ("8x/moderate", 2, 2),
        ("16x/intense", 3, 3),
        ("off/intense", 0, 3),
    ];
    let mut times: Vec<Vec<f32>> = vec![Vec::new(); configs.len()];
    let mut yaw = 0.0f32;
    // In blocks, three rounds over the settings so that the machine's drift lands on all of them.
    let block = (frames / 3).max(10);
    for _round in 0..3 {
        for (i, (_, aa, mb)) in configs.iter().enumerate() {
            gpu.set_anti_aliasing(*aa);
            gpu.set_motion_blur(*mb);
            for k in 0..block + 5 {
                yaw += 1.5;
                let ms = gpu.draw_ms(&scene, &mut |n| s.texture(n), &turned(&cam, yaw % 360.0));
                if k >= 5 {
                    times[i].push(ms);
                }
            }
        }
    }
    for (i, (name, _, _)) in configs.iter().enumerate() {
        let t = &mut times[i];
        t.sort_by(|a, b| a.total_cmp(b));
        let p99 = t[(t.len() * 99 / 100).min(t.len() - 1)];
        let med = t[t.len() / 2];
        let best = t[t.len() / 10];
        println!(
            "{width}x{height} {name:<22} best-tenth {best:6.2} ms  median {med:6.2} ms ({:5.0} fps)  p99 {p99:6.2} ms ({:5.0} fps)",
            1000.0 / med,
            1000.0 / p99
        );
    }
    Ok(())
}
