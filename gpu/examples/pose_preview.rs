//! Pictures of the roll, the landing and the jump from the pose files, with no window and no screen capture: six moments from the
//! front, the side and above, for looking at after an edit. See `docs/MOVEMENT_POSE.md`.
//!
//!   cargo run --offline --release --manifest-path gpu\Cargo.toml --example pose_preview -- [out_dir] [--folder pose_folder]
//!
//! With no folder it reads the player's own files, in `%LOCALAPPDATA%\kotor-rs`.
use kotor_gpu::stage::Stage;
use kotor_gpu::GpuRenderer;
use kotor_image::Image;
use kotor_render::Camera;

const SIZE: (u32, u32) = (1280, 720);
/// Each moment's middle, cut from a frame.
const TILE: u32 = 400;
const DT: f32 = 1.0 / 60.0;
/// Open ground on Dantooine, and the way the player faces: the spot the movement driver runs from.
const MODULE: &str = "danm14aa";
const SPOT: [f32; 3] = [165.17, 136.66, 1.85];
const HEADING: f32 = 1.767;
/// How far the camera stands from the player, metres, and how high it looks.
const AWAY: f32 = 3.0;
const LOOK_AT: f32 = 0.85;
const FOV: f32 = 45.0;
const ROLL_MOMENTS: [f32; 6] = [0.08, 0.2, 0.35, 0.5, 0.65, 0.85];
const LANDING_MOMENTS: [f32; 6] = [0.0, 0.15, 0.3, 0.5, 0.7, 0.9];
const JUMP_MOMENTS: [f32; 6] = [0.22, 0.36, 0.5, 0.65, 0.8, 0.93];

#[derive(Clone, Copy)]
enum Move {
    Roll,
    Landing,
    Jump,
}

fn tile(frame: &Image) -> Image {
    let left = (frame.width - TILE) / 2;
    let mut rgba = Vec::with_capacity((TILE * frame.height * 4) as usize);
    for y in 0..frame.height {
        let from = ((y * frame.width + left) * 4) as usize;
        rgba.extend_from_slice(&frame.rgba[from..from + (TILE * 4) as usize]);
    }
    Image {
        width: TILE,
        height: frame.height,
        rgba,
    }
}

fn strip(tiles: &[Image]) -> Image {
    let height = tiles[0].height;
    let width = TILE * tiles.len() as u32;
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for t in tiles {
            let from = (y * t.width * 4) as usize;
            rgba.extend_from_slice(&t.rgba[from..from + (t.width * 4) as usize]);
        }
    }
    Image {
        width,
        height,
        rgba,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let folder_at = args.iter().position(|a| a == "--folder");
    let out = std::path::PathBuf::from(
        args.get(1)
            .filter(|a| !a.starts_with("--"))
            .map_or("pose_preview", String::as_str),
    );
    std::fs::create_dir_all(&out)?;
    let mut s = Stage::open(MODULE, true)?;
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let folder = folder_at
        .and_then(|i| args.get(i + 1))
        .map(std::path::PathBuf::from)
        .or_else(|| {
            kotor_gpu::stage::default_settings_file()?
                .parent()
                .map(std::path::Path::to_path_buf)
        });
    if let Some(folder) = folder {
        s.use_pose_folder(folder);
    }
    // Clips, from a folder named for them: without one it leaves the player's folder alone.
    if std::env::var_os("KOTOR_RS_ANIMS").is_some() {
        s.reload_custom_clips();
    }
    for p in s.pose_problems() {
        println!("mistake: {p}");
    }
    s.set_view(SIZE);
    s.update(DT);
    for _ in 0..600 {
        if !s.game.world.in_dialogue() {
            break;
        }
        s.continue_line();
        s.number_key(1);
        s.update(DT);
    }
    s.place_player(SPOT, HEADING);
    for _ in 0..30 {
        s.update(DT);
    }
    let mut gpu = GpuRenderer::new(SIZE.0, SIZE.1)?;
    let scene = s.static_scene().clone();
    gpu.set_static_scene(&scene, &mut |n| s.texture(n));
    let at = s.game.world.pc_mover.pos;
    let (c, n) = (HEADING.cos(), HEADING.sin());
    // In front of the player, at its right hand, and over its head: which way, and how high.
    let views = [
        ("front", [c, n], 0.3),
        ("side", [n, -c], 0.3),
        ("top", [c * 0.2, n * 0.2], AWAY),
    ];
    let moves = [
        ("roll", Move::Roll, ROLL_MOMENTS),
        ("landing", Move::Landing, LANDING_MOMENTS),
        ("jump", Move::Jump, JUMP_MOMENTS),
    ];
    for (what, kind, moments) in moves {
        for (view, way, high) in views {
            let cam = Camera {
                eye: [
                    at[0] + way[0] * AWAY,
                    at[1] + way[1] * AWAY,
                    at[2] + LOOK_AT + high,
                ],
                target: [at[0], at[1], at[2] + LOOK_AT],
                fov: FOV,
            };
            let mut tiles = Vec::new();
            for p in moments {
                match kind {
                    Move::Roll => s.preview_roll(p),
                    Move::Landing => s.preview_landing(p),
                    Move::Jump => s.preview_jump(p),
                }
                let body = s.dynamic_scene();
                let frame = gpu.render(&body, &mut |n| s.texture(n), &cam)?;
                tiles.push(tile(&frame));
            }
            let path = out.join(format!("{what}_{view}.png"));
            std::fs::write(&path, kotor_image::to_png(&strip(&tiles)))?;
            println!("wrote {}", path.display());
        }
    }
    Ok(())
}
