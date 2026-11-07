//! Frames of the box for a save's name: the place offered, faint, and a name typed. No window, no screen capture. Saves go to a temp folder.
//!
//!   cargo run --release --example savename_shots -- [out_dir] [WxH ...]
use kotor_gpu::stage::Stage;

const DT: f32 = 1.0 / 30.0;
const SIZES: [(u32, u32); 2] = [(1280, 720), (3840, 2160)];

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
    s.set_saves_dir(std::env::temp_dir().join(format!("kotor_savename_{}", std::process::id())));
    for _ in 0..(50.0 / DT) as usize {
        s.update(DT);
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        }
    }
    for size in sizes {
        s.set_view(size);
        let press = |s: &mut Stage, tag: &str| {
            let p = s.menu_pixel(tag, None, size).expect(tag);
            s.hover(p, size);
            s.click(p, size);
        };
        s.escape();
        press(&mut s, "BTN_SAVEGAME");
        press(&mut s, "BTN_SAVELOAD");
        let shot = |s: &mut Stage, name: &str| {
            s.update(DT);
            let p = out.join(format!("{name}_{}.png", size.1));
            let img = s.overlay().expect("the screen");
            std::fs::write(&p, kotor_image::to_png(img)).unwrap();
            println!("wrote {}", p.display());
        };
        shot(&mut s, "savename_offered");
        s.type_text("Before the bridge");
        shot(&mut s, "savename_typed");
        s.escape();
        s.escape();
        s.escape();
    }
    Ok(())
}
