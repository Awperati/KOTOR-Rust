//! Frames of the slots and nameplate over a target, on a tall and a short creature and a human. Offscreen: no window, no
//! screen capture, and a settings file of its own.
//!
//!   cargo run --release --example pills_shots -- [out_dir] [tag] [width] [height]
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

const DT: f32 = 1.0 / 30.0;
/// Clear floor on Dantooine, as `tests/plates.rs` has it.
const MODULE: &str = "danm14aa";
const SPOT: [f32; 3] = [165.17, 136.66, 1.85];
const FACING: f32 = 1.767;
/// Who stands there: a name, an appearance row (none for the template's own), metres ahead and to the side.
const WHO: [(&str, Option<i32>, f32, f32); 3] = [
    ("Human", None, 5.0, 1.6),
    ("Wookiee", Some(51), 5.0, -2.0),
    ("Jawa", Some(73), 5.0, 3.4),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).map_or("audit", String::as_str));
    let tag = args.get(2).map_or("now", String::as_str).to_string();
    let width: u32 = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(1280);
    let height: u32 = args.get(4).and_then(|a| a.parse().ok()).unwrap_or(720);
    let size = (width, height);
    std::fs::create_dir_all(&out)?;

    let mut s = Stage::open(MODULE, true)?;
    s.set_view(size);
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let settings =
        std::env::temp_dir().join(format!("kotor_pills_shots_{}.txt", std::process::id()));
    std::fs::write(&settings, "")?;
    s.use_settings_file(settings);
    s.place_player(SPOT, FACING);
    let (sin, cos) = FACING.sin_cos();
    let mut ids = Vec::new();
    for (name, look, ahead, side) in WHO {
        let at = [
            SPOT[0] + cos * ahead + sin * side,
            SPOT[1] + sin * ahead - cos * side,
            SPOT[2],
        ];
        let id = s.game.world.spawn_creature("n_calonord", at);
        let o = s.game.world.object_mut(id).unwrap();
        o.name = name.to_string();
        o.faction = 1;
        if let Some(row) = look {
            o.appearance = row;
        }
        ids.push((name, id));
    }
    // Nobody fights: they only stand there.
    s.game.world.combat.target.clear();
    for _ in 0..10 {
        s.update(DT);
    }
    let mut gpu = GpuRenderer::new(width, height)?;
    let scene = s.static_scene().clone();
    gpu.set_static_scene(&scene, &mut |n| s.texture(n));
    let wide = s.state_line();
    let layout_wide: f32 = wide
        .split(' ')
        .find_map(|p| p.strip_prefix("width="))
        .and_then(|w| w.parse().ok())
        .unwrap_or(640.0);
    for (name, id) in ids {
        // Off every plate for longer than a target keeps its slots, so that the next one takes them.
        s.hover((width as f32 * 0.25, height as f32 * 0.12), size);
        for _ in 0..30 {
            s.update(DT);
        }
        let Some((_, at)) = s.plates_shown().into_iter().find(|(who, _)| *who == id) else {
            println!("{name}: no plate");
            continue;
        };
        let p = (
            at.0 as f32 * width as f32 / layout_wide,
            at.1 as f32 * height as f32 / 480.0,
        );
        s.hover(p, size);
        s.update(DT);
        s.update(DT);
        s.game.world.combat.target.clear();
        let scene = s.dynamic_scene();
        let cam = s.camera();
        let mut img = gpu.render(&scene, &mut |n| s.texture(n), &cam)?;
        if let Some(o) = s.overlay() {
            composite(&mut img, o);
        }
        let file = out.join(format!("pills_{tag}_{}_{height}.png", name.to_lowercase()));
        std::fs::write(&file, kotor_image::to_png(&img))?;
        let slot = s.hud_place("BTN_TARGET0");
        println!(
            "wrote {} plate={at:?} slot={slot:?} head={:?}",
            file.display(),
            s.measured_head(id)
        );
    }
    Ok(())
}
