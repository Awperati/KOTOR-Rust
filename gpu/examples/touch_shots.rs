//! Frames of the on-screen controls at several window sizes. Offscreen: no window, no screen capture, no saves.
//!
//!   cargo run --release --example touch_shots -- [out_dir] [WxH ...]
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};
use kotor_ui::{TouchKey, TouchPart};

const DT: f32 = 1.0 / 30.0;
const SIZES: [(u32, u32); 4] = [(1280, 720), (1920, 1080), (720, 1280), (800, 1000)];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).map_or("audit", String::as_str));
    std::fs::create_dir_all(&out)?;
    let asked: Vec<(u32, u32)> = args
        .iter()
        .skip(2)
        .filter_map(|a| {
            let (w, h) = a.split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        })
        .collect();
    let sizes = if asked.is_empty() {
        SIZES.to_vec()
    } else {
        asked
    };

    let mut s = Stage::open("end_m01aa", true)?;
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    for _ in 0..(50.0 / DT) as usize {
        s.update(DT);
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        }
    }
    for size in sizes {
        s.set_view(size);
        let mut gpu = GpuRenderer::new(size.0, size.1)?;
        let scene = s.static_scene().clone();
        gpu.set_static_scene(&scene, &mut |n| s.texture(n));
        let mut shot = |s: &mut Stage, name: &str| {
            s.update(DT);
            let scene = s.dynamic_scene();
            let cam = s.camera();
            gpu.set_particles(s.particles());
            let mut img = gpu.render(&scene, &mut |n| s.texture(n), &cam).unwrap();
            if let Some(o) = s.overlay() {
                composite(&mut img, o);
            }
            let p = out.join(format!("{name}_{}x{}.png", size.0, size.1));
            std::fs::write(&p, kotor_image::to_png(&img)).unwrap();
            println!("wrote {}", p.display());
        };
        shot(&mut s, "touch_off");

        // The blue button, by a finger.
        let blue = s
            .hud_pixel(kotor_ui::TOUCH_BUTTON, size)
            .expect("the button");
        s.finger_down(1, blue, size);
        s.finger_up(1, blue, size);
        assert!(s.touch_on());
        shot(&mut s, "touch_on");

        // A thumb on the stick and one on Jump, at once.
        let stick = s.touch_pixel(TouchPart::Stick, size).expect("the stick");
        s.finger_down(2, stick, size);
        s.finger_moved(2, (stick.0 + 30.0, stick.1 - 60.0), size);
        let jump = s.touch_pixel(TouchPart::Key(TouchKey::Jump), size).unwrap();
        s.finger_down(3, jump, size);
        // And the other thumb on the look stick.
        let look = s
            .touch_pixel(TouchPart::Look, size)
            .expect("the look stick");
        s.finger_down(4, look, size);
        s.finger_moved(4, (look.0 - 40.0, look.1 + 25.0), size);
        println!("steer {:?}, {} fingers", s.touch_steer(), s.fingers_down());
        shot(&mut s, "touch_pressed");
        s.finger_up(2, stick, size);
        s.finger_up(3, jump, size);
        s.finger_up(4, look, size);

        let (layer, hud) = s.touch_boxes().expect("the layer");
        let overlaps = |a: [i32; 4], b: [i32; 4]| {
            a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
        };
        for l in &layer {
            for (tag, h) in &hud {
                if overlaps(*l, *h) {
                    println!("{l:?} is over {tag} {h:?}");
                }
            }
        }
        // Off again for the next size.
        s.finger_down(1, blue, size);
        s.finger_up(1, blue, size);
    }
    Ok(())
}
