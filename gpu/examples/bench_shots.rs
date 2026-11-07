//! Frames of the upgrade bench's four kinds, with nothing to upgrade and with a lightsaber chosen. Offscreen: no window, no screen
//! capture, no saves.
//!
//!   cargo run --release --example bench_shots -- [out_dir] [width] [height]
use kotor_gpu::stage::Stage;

const DT: f32 = 1.0 / 30.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).map_or("audit", String::as_str));
    let width: u32 = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(1280);
    let height: u32 = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(720);
    std::fs::create_dir_all(&out)?;

    let mut s = Stage::open("end_m01aa", true)?;
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    s.set_view((width, height));
    s.update(DT);
    let shot = |s: &mut Stage, name: &str| {
        s.update(DT);
        let p = out.join(format!("{name}_{height}.png"));
        let o = s.overlay().expect("the screen");
        std::fs::write(&p, kotor_image::to_png(o)).unwrap();
        println!("wrote {}", p.display());
    };
    s.open_bench();
    shot(&mut s, "bench_nothing");
    s.escape();
    let w = &mut s.game.world;
    let pc = w.pc;
    let saber = w.spawn_item("g_w_lghtsbr01");
    w.carry(pc, saber);
    s.open_bench();
    let chosen = s
        .menu_pixel("BTN_LIGHTSABER", None, (width, height))
        .expect("the button");
    s.click(chosen, (width, height));
    shot(&mut s, "bench_kinds");
    Ok(())
}
