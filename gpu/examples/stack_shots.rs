//! Frames of a footlocker's list, for its rows of one kind of item. Offscreen: no window, no screen capture, no saves.
//!
//!   cargo run --release --example stack_shots -- [out_dir] [name] [width] [height]
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

const DT: f32 = 1.0 / 30.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).map_or("audit", String::as_str));
    let name = args.get(2).map_or("locker", String::as_str).to_string();
    let width: u32 = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(1280);
    let height: u32 = args.get(4).and_then(|a| a.parse().ok()).unwrap_or(720);
    let size = (width, height);
    std::fs::create_dir_all(&out)?;

    let mut s = Stage::open("end_m01aa", true)?;
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    s.set_view(size);
    let mut gpu = GpuRenderer::new(width, height)?;
    let scene = s.static_scene().clone();
    gpu.set_static_scene(&scene, &mut |n| s.texture(n));
    for _ in 0..(50.0 / DT) as usize {
        s.update(DT);
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        }
    }
    let mut shot = |s: &mut Stage, name: &str| {
        s.update(DT);
        let scene = s.dynamic_scene();
        let cam = s.camera();
        gpu.set_particles(s.particles());
        let mut img = gpu.render(&scene, &mut |n| s.texture(n), &cam).unwrap();
        if let Some(o) = s.overlay() {
            composite(&mut img, o);
        }
        let p = out.join(format!("{name}_{height}.png"));
        std::fs::write(&p, kotor_image::to_png(&img)).unwrap();
        println!("wrote {}", p.display());
    };

    s.stand_near("end_locker01");
    s.update(DT);
    let locker = s.game.world.by_tag("end_locker01")[0];
    s.game.world.click(locker);
    s.update(DT);
    println!("rows: {:?}", s.game.world.container_items());
    shot(&mut s, &name);
    Ok(())
}
