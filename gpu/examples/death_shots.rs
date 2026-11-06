//! Frames of the player's death, rendered offscreen (no window, no screen capture): the fall, the fade and the
//! game-over screen.
//!
//!   cargo run --release --example death_shots -- [out_dir] [width] [height]
use kotor_game::{EffectRec, DURATION_INSTANT};
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

const DT: f32 = 1.0 / 30.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).map_or("docs/shots/combat", String::as_str));
    let width: u32 = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(1280);
    let height: u32 = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(720);
    std::fs::create_dir_all(&out)?;
    let mut s = Stage::open("end_m01aa", true)?;
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let mut gpu = GpuRenderer::new(width, height)?;
    let scene = s.static_scene().clone();
    gpu.set_static_scene(&scene, &mut |n| s.texture(n));
    for _ in 0..3000 {
        s.update(DT);
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        }
    }
    let w = &mut s.game.world;
    for id in w.standing_party() {
        w.object_mut(id).unwrap().min_one_hp = false;
        let mut e = EffectRec::new("EffectDamage");
        e.nums = vec![1000.0, 0.0, 0.0];
        w.effects.push(e);
        let h = w.effects.len() as u32;
        w.apply_effect(id, id, h, DURATION_INSTANT, 0.0);
    }
    let mut at = 0.0;
    for (seconds, name) in [
        (0.2, "death_0_falling"),
        (1.5, "death_1_down"),
        (4.0, "death_2_fading"),
        (6.5, "death_3_game_over"),
    ] {
        while at < seconds {
            s.update(DT);
            at += DT;
        }
        let sc = s.dynamic_scene();
        let cam = s.camera();
        gpu.set_particles(s.particles());
        let mut img = gpu.render(&sc, &mut |n| s.texture(n), &cam).unwrap();
        if let Some(o) = s.overlay() {
            composite(&mut img, o);
        }
        let p = out.join(format!("{name}_{height}p.png"));
        std::fs::write(&p, kotor_image::to_png(&img))?;
        println!("wrote {} (menu {})", p.display(), s.menu_name());
    }
    Ok(())
}
