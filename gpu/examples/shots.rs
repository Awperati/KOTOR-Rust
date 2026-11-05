//! Frames of the running game for looking at, without a window or a screen capture: rendered offscreen and written
//! as PNG: a line of the opening conversation, then the player walking with the HUD showing.
//!
//!   cargo run --offline --release --manifest-path gpu\Cargo.toml --example shots -- [out_dir]
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

const SIZE: (u32, u32) = (1280, 720);
const DT: f32 = 1.0 / 30.0;

fn shot(gpu: &mut GpuRenderer, s: &mut Stage, path: &std::path::Path) {
    let scene = s.dynamic_scene();
    let cam = s.camera();
    let mut img = gpu.render(&scene, &mut |n| s.texture(n), &cam).unwrap();
    if let Some(o) = s.overlay() {
        composite(&mut img, o);
    }
    std::fs::write(path, kotor_image::to_png(&img)).unwrap();
    println!("wrote {}", path.display());
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).map_or("docs/shots/play", String::as_str));
    std::fs::create_dir_all(&out)?;
    let mut s = Stage::open("end_m01aa", true)?;
    let mut gpu = GpuRenderer::new(SIZE.0, SIZE.1)?;
    let scene = s.static_scene().clone();
    gpu.set_static_scene(&scene, &mut |n| s.texture(n));
    // The opening conversation, then Trask's talk with its replies.
    let (mut talked, mut replied, mut asked) = (false, false, false);
    for _ in 0..6000 {
        s.update(DT);
        match s.game.world.dialogue_view() {
            Some((_, text, _, replies)) => {
                if !talked && !text.is_empty() {
                    talked = true;
                    shot(&mut gpu, &mut s, &out.join("dialogue_line.png"));
                }
                if !replied && !replies.is_empty() {
                    replied = true;
                    // Let the camera swing round to the speaker.
                    for _ in 0..90 {
                        s.update(DT);
                    }
                    shot(&mut gpu, &mut s, &out.join("dialogue_replies.png"));
                }
                s.number_key(1);
                s.continue_line();
            }
            None if talked && !asked => {
                asked = true;
                let trask = s.game.world.by_tag("end_trask")[0];
                s.game.world.talk_to(trask);
            }
            None if asked => break,
            None => {}
        }
    }
    let (w, h) = (SIZE.0 as f32, SIZE.1 as f32);
    // Click down the screen until one lands on the floor.
    let mut step = 0.6;
    while !s.game.world.pc_walking() && step > 0.45 {
        s.click((w * 0.5, h * step), SIZE);
        step -= 0.02;
    }
    for _ in 0..25 {
        s.update(DT);
    }
    shot(&mut gpu, &mut s, &out.join("walking_hud.png"));
    println!(
        "player walking: {}; {} lines shown; {} world errors",
        s.game.world.pc_walking(),
        s.lines_shown,
        s.game.world.errors.len()
    );
    Ok(())
}
