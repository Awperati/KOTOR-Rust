//! Frames of the swoop race for looking at, with no window and no screen capture. The pilot drives.
//!
//!   cargo run --offline --release --manifest-path gpu\Cargo.toml --example race_shots -- [out_dir] [width height]
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

const DT: f32 = 1.0 / 30.0;
/// Seconds since the module began: the lights, then the clock's own times, whose first is at 5 s.
const SHOTS: [f32; 11] = [
    0.6, 2.0, 3.5, 4.5, 5.75, 7.75, 9.75, 13.75, 17.75, 23.7, 28.7,
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).map_or("docs/shots/race", String::as_str));
    let number =
        |i: usize, default: u32| args.get(i).and_then(|v| v.parse().ok()).unwrap_or(default);
    let size = (number(2, 1280), number(3, 720));
    std::fs::create_dir_all(&out)?;
    let mut gpu = GpuRenderer::new(size.0, size.1)?;
    let mut s = Stage::new("tar_m03mg")?;
    s.set_view(size);
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    s.pilot = Some(kotor_game::Autopilot::new());
    let scene = s.static_scene().clone();
    gpu.set_static_scene(&scene, &mut |n| s.texture(n));
    for at in SHOTS {
        loop {
            s.update(DT);
            let Some(game) = s.game.world.race.as_ref() else {
                println!("the race ended before {at} s");
                return Ok(());
            };
            if game.time >= at {
                break;
            }
        }
        let (moving, cam, figure) = s.frame_scene();
        gpu.set_figure_only(figure);
        gpu.set_particles(s.frame_particles());
        let mut img = gpu.render(&moving, &mut |n| s.texture(n), &cam)?;
        if let Some(o) = s.overlay() {
            composite(&mut img, o);
        }
        let path = out.join(format!("race_{:05.2}.png", at));
        std::fs::write(&path, kotor_image::to_png(&img))?;
        let game = s.game.world.race.as_ref().unwrap();
        println!(
            "{} speed {:.0} at {:?} eye {:?} target {:?}",
            path.display(),
            game.speed,
            game.position(),
            cam.eye,
            cam.target
        );
    }
    Ok(())
}
