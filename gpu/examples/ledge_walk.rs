//! Walks off the game's ledges to see where a fall is wrong, with no window and no screen capture. For each spot in a file
//! from `crates\kotor-render\examples\ledges.rs` it runs forward, says whether the player fell, and writes three frames.
//!
//!   cargo run --offline --release --manifest-path gpu\Cargo.toml --example ledge_walk -- spots.txt out_dir
//!
//! A line of the file: `module x y z heading HEADING drop DROP`. It writes no saves and no settings.
use kotor_gpu::stage::Stage;
use kotor_gpu::GpuRenderer;

const SIZE: (u32, u32) = (640, 360);
const DT: f32 = 1.0 / 60.0;
/// Seconds it runs for, at most, before it says it did not fall.
const RUN_SECONDS: f32 = 3.0;

fn away(s: &Stage) -> f32 {
    let c = s.camera();
    let d = [0, 1, 2].map(|i| c.eye[i] - c.target[i]);
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

fn shot(
    s: &mut Stage,
    gpu: &mut GpuRenderer,
    path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let body = s.dynamic_scene();
    let cam = s.camera();
    let frame = gpu.render(&body, &mut |n| s.texture(n), &cam)?;
    std::fs::write(path, kotor_image::to_png(&frame))?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let spots = std::fs::read_to_string(&args[1])?;
    let out = std::path::PathBuf::from(&args[2]);
    std::fs::create_dir_all(&out)?;
    let mut gpu = GpuRenderer::new(SIZE.0, SIZE.1)?;
    for (n, line) in spots.lines().enumerate() {
        let w: Vec<&str> = line.split_whitespace().collect();
        if w.len() < 8 {
            continue;
        }
        let module = w[0];
        let at = [w[1].parse::<f32>()?, w[2].parse()?, w[3].parse()?];
        let heading: f32 = w[5].parse()?;
        let mut s = match Stage::open(module, true) {
            Ok(s) => s,
            Err(e) => {
                println!("{n:03} {module} could not open: {e}");
                continue;
            }
        };
        s.enable_tutorials(false);
        s.enable_auto_pause(false);
        s.set_view(SIZE);
        s.update(DT);
        for _ in 0..300 {
            if !s.game.world.in_dialogue() {
                break;
            }
            s.continue_line();
            s.number_key(1);
            s.update(DT);
        }
        s.place_player(at, heading);
        s.yaw = heading;
        for _ in 0..30 {
            s.update(DT);
        }
        let scene = s.static_scene().clone();
        gpu.set_static_scene(&scene, &mut |t| s.texture(t));
        let (mut nearest, start) = (f32::MAX, away(&s));
        shot(&mut s, &mut gpu, &out.join(format!("{n:03}_a.png")))?;
        s.steer = (1.0, 0.0, false);
        let mut fell = false;
        for _ in 0..(RUN_SECONDS / DT) as usize {
            s.update(DT);
            nearest = nearest.min(away(&s));
            if s.game.world.in_dialogue() {
                s.continue_line();
                s.number_key(1);
            }
            if !s.game.world.pc_body.grounded {
                fell = true;
                break;
            }
        }
        let edge = s.game.world.pc_mover.pos;
        shot(&mut s, &mut gpu, &out.join(format!("{n:03}_b.png")))?;
        s.steer = (0.0, 0.0, false);
        for _ in 0..90 {
            s.update(DT);
            nearest = nearest.min(away(&s));
        }
        let end = s.game.world.pc_mover.pos;
        shot(&mut s, &mut gpu, &out.join(format!("{n:03}_c.png")))?;
        println!(
            "{n:03} {module} from {:.2},{:.2},{:.2} heading {heading:.3} {} at {:.2},{:.2},{:.2} ended {:.2},{:.2},{:.2} drop {:.2} camera {start:.2} nearest {nearest:.2}",
            at[0],
            at[1],
            at[2],
            if fell { "FELL" } else { "stayed" },
            edge[0],
            edge[1],
            edge[2],
            end[0],
            end[1],
            end[2],
            edge[2] - end[2],
        );
    }
    Ok(())
}
