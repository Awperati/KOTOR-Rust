//! Frames of the Appearance page's hair colours on several heads: the head's own and each colour, close.
//! Offscreen: no window, no screen capture, no saves.
//!
//!   cargo run --release --example hair_shots -- [out_dir] [width] [height]
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).map_or("audit", String::as_str));
    let width: u32 = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(1280);
    let height: u32 = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(720);
    let size = (width, height);
    std::fs::create_dir_all(&out)?;

    let mut gpu = GpuRenderer::new(width, height)?;
    let mut s = Stage::menu("end_m01aa", true)?;
    s.set_view(size);
    s.enable_chargen(true);
    let click = |s: &mut Stage, tag: &str| {
        let p = s.menu_pixel(tag, None, size).expect("the button");
        s.hover(p, size);
        s.click(p, size);
        s.update(0.0);
    };
    let mut shot = |s: &mut Stage, name: &str| {
        s.update(0.0);
        let scene = s.static_scene().clone();
        gpu.set_static_scene(&scene, &mut |n| s.texture(n));
        let (moving, cam, figure) = s.frame_scene();
        gpu.set_figure_only(figure);
        gpu.set_particles(s.frame_particles());
        let mut img = gpu.render(&moving, &mut |n| s.texture(n), &cam).unwrap();
        if let Some(o) = s.overlay() {
            composite(&mut img, o);
        }
        let p = out.join(format!("{name}_{height}.png"));
        std::fs::write(&p, kotor_image::to_png(&img)).unwrap();
        println!("wrote {}", p.display());
    };
    click(&mut s, "BTN_NEWGAME");
    click(&mut s, "BTN_SEL4");
    click(&mut s, "CUST_CHAR_BTN");
    click(&mut s, "BTN_STEPNAME1");
    let row = |s: &mut Stage, name: &str| {
        let rows = s.menu_rows("LB_OPTIONS");
        rows.iter()
            .position(|r| r.text == name)
            .unwrap_or_else(|| panic!("no {name}"))
    };
    let step = |s: &mut Stage, name: &str, times: usize| {
        let n = row(s, name);
        s.hover((width as f32 * 0.25, height as f32 * 0.4), size);
        for _ in 0..40 {
            s.menu_wheel(1.0);
        }
        for _ in 0..times {
            if let Some(p) = s.menu_step_pixel("LB_OPTIONS", n, 1, size) {
                s.hover(p, size);
                s.click(p, size);
            }
        }
        s.menu_rows("LB_OPTIONS")[n]
            .choice
            .clone()
            .unwrap_or_default()
    };
    // Each head, then the colours round to the head's own again: 10 in all.
    for head in 0..4 {
        if head > 0 {
            step(&mut s, "Preset", 4);
        }
        for colour in 0..10 {
            let words = if colour == 0 {
                // The first arrow brings the view to the head; the head's own comes round again at the end.
                step(&mut s, "Hair colour", 0)
            } else {
                step(&mut s, "Hair colour", 1)
            };
            if [0, 1, 3, 5, 6, 7, 8].contains(&colour) {
                shot(&mut s, &format!("hair_h{head}_{colour}"));
            }
            println!("head {head}: {words}");
        }
        step(&mut s, "Hair colour", 1);
    }
    Ok(())
}
