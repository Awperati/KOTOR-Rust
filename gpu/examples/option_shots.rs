//! Every option page at 1280x720 and 3840x2160, for looking at, with settings and saves of its own.
//!
//!   cd gpu; cargo run --release --example option_shots -- [out_dir]
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

const SIZES: [(u32, u32); 2] = [(1280, 720), (3840, 2160)];

/// A page's button, what the cursor rests on for its help, and the pages under it with theirs.
type Page = (
    &'static str,
    &'static str,
    &'static [(&'static str, &'static str)],
);

fn shot(gpu: &mut GpuRenderer, s: &mut Stage, name: &str, out: &std::path::Path) {
    let scene = s.static_scene().clone();
    gpu.set_static_scene(&scene, &mut |n| s.texture(n));
    for size in SIZES {
        gpu.resize(size.0, size.1);
        s.words_fit();
        s.set_view(size);
        s.update(0.0);
        let (moving, cam, figure) = s.frame_scene();
        gpu.set_figure_only(figure);
        gpu.set_particles(s.frame_particles());
        let mut img = gpu.render(&moving, &mut |n| s.texture(n), &cam).unwrap();
        if let Some(o) = s.overlay() {
            composite(&mut img, o);
        }
        let (_, too_big) = s.words_fit();
        if !too_big.is_empty() {
            println!("does not fit on {name} at {}: {too_big:?}", size.1);
        }
        let path = out.join(format!("{name}_{}.png", size.1));
        std::fs::write(&path, kotor_image::to_png(&img)).unwrap();
        println!("wrote {}", path.display());
    }
    s.set_view(SIZES[0]);
}

fn click(s: &mut Stage, tag: &str) {
    let size = SIZES[0];
    s.set_view(size);
    let p = s
        .menu_pixel(tag, None, size)
        .unwrap_or_else(|| panic!("no {tag} on {}", s.menu_name()));
    s.hover(p, size);
    s.click(p, size);
    s.update(0.0);
}

/// The cursor on `tag`, so its help shows.
fn hover(s: &mut Stage, tag: &str) {
    let size = SIZES[0];
    if let Some(p) = s.menu_pixel(tag, None, size) {
        s.hover(p, size);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).map_or("docs/shots/options", String::as_str));
    std::fs::create_dir_all(&out)?;
    let mut gpu = GpuRenderer::new(SIZES[0].0, SIZES[0].1)?;
    let mut s = Stage::menu("end_m01aa", true)?;
    let saves = std::env::temp_dir().join(format!("kotor_rs_option_shots_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&saves);
    s.set_saves_dir(saves.clone());
    s.use_settings_file(saves.join("settings.txt"));
    for _ in 0..300 {
        s.update(1.0 / 30.0);
    }
    click(&mut s, "BTN_OPTIONS");
    shot(&mut gpu, &mut s, "options", &out);
    let pages: [Page; 5] = [
        (
            "BTN_GAMEPLAY",
            "CB_CLICK_TO_MOVE",
            &[("BTN_MOUSE", "CB_TOUCH"), ("BTN_KEYMAP", "BTN_Filter_Game")],
        ),
        ("BTN_FEEDBACK", "BTN_DEFAULT", &[]),
        ("BTN_AUTOPAUSE", "CB_ENDROUND", &[]),
        (
            "BTN_GRAPHICS",
            "BTN_MODPACKS",
            &[("BTN_ADVANCED", "CB_VSYNC")],
        ),
        ("BTN_SOUND", "SLI_MUSIC", &[]),
    ];
    for (page, help, under) in pages {
        click(&mut s, page);
        let name = s.menu_name();
        hover(&mut s, help);
        shot(&mut gpu, &mut s, name, &out);
        for (sub, help) in under {
            click(&mut s, sub);
            hover(&mut s, help);
            let sub_name = s.menu_name();
            shot(&mut gpu, &mut s, sub_name, &out);
            while s.menu_name() != name {
                s.escape();
            }
        }
        s.escape();
    }
    let _ = std::fs::remove_dir_all(&saves);
    Ok(())
}
