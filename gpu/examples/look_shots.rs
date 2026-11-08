//! Frames of a Custom character's Appearance page. Offscreen: no window, no screen capture, no saves.
//!
//!   cargo run --release --example look_shots -- [out_dir] [width] [height]
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
    shot(&mut s, "look_steps");
    click(&mut s, "BTN_STEPNAME1");
    shot(&mut s, "look_page");
    if let Some(p) = s.menu_step_pixel("LB_OPTIONS", 1, 1, size) {
        s.hover(p, size);
        s.click(p, size);
    }
    shot(&mut s, "look_large");

    // Stage 1: a drag turns the figure, and the rows give views, poses and lights.
    let row = |s: &mut Stage, name: &str| {
        let rows = s.menu_rows("LB_OPTIONS");
        rows.iter().position(|r| r.text == name).unwrap_or_else(|| {
            panic!(
                "no {name} on {}: {:?}",
                s.menu_name(),
                rows.iter().map(|r| r.text.clone()).collect::<Vec<_>>()
            )
        })
    };
    // The list shows six rows: it goes to the top, and then down until this one is in it.
    let bring = |s: &mut Stage, row: usize| {
        s.hover((width as f32 * 0.25, height as f32 * 0.4), size);
        for _ in 0..20 {
            s.menu_wheel(1.0);
        }
        for _ in 0..20 {
            let shown = s
                .menu_step_pixel("LB_OPTIONS", row, 1, size)
                .is_some_and(|p| p.1 < 345.0 * height as f32 / 480.0);
            if shown {
                break;
            }
            s.menu_wheel(-1.0);
        }
    };
    let step = |s: &mut Stage, row: usize| {
        bring(s, row);
        if let Some(p) = s.menu_step_pixel("LB_OPTIONS", row, 1, size) {
            s.hover(p, size);
            s.click(p, size);
        }
    };
    let middle = (455.0 * width as f32 / 640.0, 240.0 * height as f32 / 480.0);
    s.mouse_down_on_layer(middle, size);
    s.mouse_moved_on_layer((middle.0 + width as f32 * 0.06, middle.1), size);
    s.mouse_up_on_layer();
    shot(&mut s, "look_turned");
    let view = row(&mut s, "View");
    step(&mut s, view);
    shot(&mut s, "look_upper");
    step(&mut s, view);
    shot(&mut s, "look_head");
    let light = row(&mut s, "Lighting");
    for name in ["look_head_front", "look_head_side", "look_head_low"] {
        step(&mut s, light);
        shot(&mut s, name);
    }
    step(&mut s, light);
    step(&mut s, view);
    let pose = row(&mut s, "Pose");
    step(&mut s, pose);
    step(&mut s, pose);
    s.update(2.0);
    shot(&mut s, "look_pose3");
    println!("{:?}", s.look_preview());

    // Stage 2: the body's sliders, all at one end and then the other, seen whole and from the side.
    let pose = row(&mut s, "Pose");
    step(&mut s, pose);
    step(&mut s, pose);
    let sliders = [
        "Height",
        "Shoulders",
        "Chest",
        "Waist",
        "Hips",
        "Arms",
        "Legs",
    ];
    shot(&mut s, "look_body_plain");
    for (name, by) in [("look_body_most", 1), ("look_body_least", -1)] {
        for slider in sliders {
            let n = row(&mut s, slider);
            bring(&mut s, n);
            for _ in 0..20 {
                if let Some(p) = s.menu_step_pixel("LB_OPTIONS", n, by, size) {
                    s.hover(p, size);
                    s.click(p, size);
                }
            }
        }
        shot(&mut s, name);
        s.mouse_down_on_layer(middle, size);
        s.mouse_moved_on_layer((middle.0 + width as f32 * 0.12, middle.1), size);
        s.mouse_up_on_layer();
        shot(&mut s, &format!("{name}_side"));
        s.mouse_down_on_layer(middle, size);
        s.mouse_moved_on_layer((middle.0 - width as f32 * 0.12, middle.1), size);
        s.mouse_up_on_layer();
    }
    for slider in sliders {
        let n = row(&mut s, slider);
        println!("{slider}: {:?}", s.menu_rows("LB_OPTIONS")[n].choice);
    }

    // Stage 3: the face's sliders, seen close, from the front and a little turned. A face slider brings the view to the head.
    let face = ["Brow", "Nose", "Jaw", "Cheeks", "Eyes", "Mouth"];
    let turn = |s: &mut Stage, by: f32| {
        s.mouse_down_on_layer(middle, size);
        s.mouse_moved_on_layer((middle.0 + width as f32 * by, middle.1), size);
        s.mouse_up_on_layer();
    };
    // Back to the front.
    turn(&mut s, -0.06);
    let view = row(&mut s, "View");
    step(&mut s, view);
    step(&mut s, view);
    shot(&mut s, "look_face_plain");
    turn(&mut s, 0.04);
    shot(&mut s, "look_face_plain_turned");
    turn(&mut s, -0.04);
    for (name, by) in [("look_face_most", 1), ("look_face_least", -1)] {
        for slider in face {
            let n = row(&mut s, slider);
            bring(&mut s, n);
            for _ in 0..20 {
                if let Some(p) = s.menu_step_pixel("LB_OPTIONS", n, by, size) {
                    s.hover(p, size);
                    s.click(p, size);
                }
            }
        }
        shot(&mut s, name);
        turn(&mut s, 0.04);
        shot(&mut s, &format!("{name}_turned"));
        turn(&mut s, -0.04);
    }
    for slider in face {
        let n = row(&mut s, slider);
        println!("{slider}: {:?}", s.menu_rows("LB_OPTIONS")[n].choice);
    }
    println!("{:?}", s.look_preview());
    Ok(())
}
