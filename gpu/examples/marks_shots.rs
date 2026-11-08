//! Frames of the Appearance page's marks: the scars, the tattoos and the make-up, each group at full strength, from the front
//! and a little turned. Offscreen: no window, no screen capture, no saves.
//!
//!   cargo run --release --example marks_shots -- [out_dir] [width] [height] [head steps]
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).map_or("audit", String::as_str));
    let width: u32 = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(1280);
    let height: u32 = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(720);
    let heads: usize = args.get(4).and_then(|a| a.parse().ok()).unwrap_or(0);
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
        let (_, too_big) = s.words_fit();
        if !too_big.is_empty() {
            println!("does not fit on {name}: {too_big:?}");
        }
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
    let bring = |s: &mut Stage, row: usize| {
        s.hover((width as f32 * 0.25, height as f32 * 0.4), size);
        for _ in 0..40 {
            s.menu_wheel(1.0);
        }
        for _ in 0..40 {
            let shown = s
                .menu_step_pixel("LB_OPTIONS", row, 1, size)
                .is_some_and(|p| p.1 < 345.0 * height as f32 / 480.0);
            if shown {
                break;
            }
            s.menu_wheel(-1.0);
        }
    };
    let set = |s: &mut Stage, name: &str, by: i32, times: usize| {
        let n = row(s, name);
        bring(s, n);
        for _ in 0..times {
            if let Some(p) = s.menu_step_pixel("LB_OPTIONS", n, by, size) {
                s.hover(p, size);
                s.click(p, size);
            }
        }
    };
    let middle = (455.0 * width as f32 / 640.0, 240.0 * height as f32 / 480.0);
    let turn = |s: &mut Stage, by: f32| {
        s.mouse_down_on_layer(middle, size);
        s.mouse_moved_on_layer((middle.0 + width as f32 * by, middle.1), size);
        s.mouse_up_on_layer();
    };
    if heads > 0 {
        set(&mut s, "Preset", 1, heads);
    }
    let groups: [(&str, [&str; 3]); 3] = [
        ("marks_scars", ["Scar: nose", "Scar: lip", "Scar: chin"]),
        (
            "marks_tattoos",
            ["Tattoo: cheeks", "Tattoo: brow", "Tattoo: chin"],
        ),
        (
            "marks_paint",
            ["Make-up: eyes", "Make-up: lips", "Make-up: cheeks"],
        ),
    ];
    // The plain face first: one mark's arrow there and back brings the view to the head.
    set(&mut s, "Scar: nose", 1, 1);
    set(&mut s, "Scar: nose", -1, 1);
    shot(&mut s, "marks_plain");
    for (name, marks) in groups {
        for m in marks {
            set(&mut s, m, 1, 10);
        }
        shot(&mut s, name);
        turn(&mut s, 0.05);
        shot(&mut s, &format!("{name}_turned"));
        turn(&mut s, -0.10);
        shot(&mut s, &format!("{name}_other"));
        turn(&mut s, 0.05);
        for m in marks {
            let n = row(&mut s, m);
            println!("{m}: {:?}", s.menu_rows("LB_OPTIONS")[n].choice);
            set(&mut s, m, -1, 10);
        }
    }
    Ok(())
}
