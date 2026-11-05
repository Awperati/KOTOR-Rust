//! Render one area with the software renderer and the GPU renderer, write both PNGs and print how far apart they are.
//!
//!   cargo run --offline --manifest-path gpu\Cargo.toml --example compare -- [area] [out_dir]
use kotor_archive::Install;
use kotor_gpu::GpuRenderer;
use kotor_render::Camera;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let args: Vec<String> = std::env::args().collect();
    let area = args.get(1).map_or("m01aa", String::as_str);
    let out = std::path::PathBuf::from(args.get(2).map_or("docs/shots/gpu", String::as_str));
    std::fs::create_dir_all(&out)?;
    let dir = kotor_core::install_dir().ok_or("no KotOR install found")?;
    let mut inst = Install::open(dir)?;
    inst.load_module(args.get(3).map_or("end_m01aa", String::as_str))?;
    inst.load_texture_packs()?;
    let mut scene = kotor_render::load_area(&mut inst, area)?;
    kotor_render::add_area_objects(&mut scene, &mut inst, area)?;
    let mid = [0, 1, 2].map(|k| (scene.min[k] + scene.max[k]) / 2.0);
    let cam = Camera {
        eye: [mid[0], scene.min[1] - 2.0, mid[2] + 6.0],
        target: mid,
        fov: 70.0,
    };
    let (w, h) = (960, 540);
    let t = std::time::Instant::now();
    let soft = kotor_render::render(
        &scene,
        &mut |n| kotor_render::load_texture(&mut inst, n),
        &cam,
        w,
        h,
    );
    println!("software: {:.2}s", t.elapsed().as_secs_f32());
    let mut gpu = GpuRenderer::new(w, h)?;
    gpu.set_sky_at_infinity(false);
    gpu.set_transparency(false);
    let t = std::time::Instant::now();
    let img = gpu.render(
        &scene,
        &mut |n| kotor_render::load_texture(&mut inst, n),
        &cam,
    )?;
    println!("gpu: {:.2}s", t.elapsed().as_secs_f32());
    std::fs::write(out.join("soft.png"), kotor_image::to_png(&soft))?;
    std::fs::write(out.join("gpu.png"), kotor_image::to_png(&img))?;
    let (mut sum, mut far, mut n) = (0u64, 0u64, 0u64);
    for (a, b) in soft.rgba.chunks(4).zip(img.rgba.chunks(4)) {
        let d: i32 = (0..3).map(|k| (a[k] as i32 - b[k] as i32).abs()).sum();
        sum += d as u64;
        far += (d > 90) as u64;
        n += 1;
    }
    println!(
        "mean difference {:.2} of 765 per pixel; {:.2}% of pixels differ by more than 90",
        sum as f64 / n as f64,
        far as f64 * 100.0 / n as f64
    );
    Ok(())
}
