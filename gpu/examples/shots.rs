//! Frames of the running game for looking at, without a window or a screen capture: rendered offscreen and written
//! as PNG: a line of the opening conversation, then the player walking with the HUD showing.
//!
//!   cargo run --offline --release --manifest-path gpu\Cargo.toml --example shots -- [out_dir]
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

const SIZE: (u32, u32) = (1280, 720);
const DT: f32 = 1.0 / 30.0;

fn shot(gpu: &mut GpuRenderer, s: &mut Stage, path: &std::path::Path) {
    let scene = s.dynamic_scene();
    let cam = s.camera();
    let mut img = gpu.render(&scene, &mut |n| s.texture(n), &cam).unwrap();
    if let Some(o) = s.overlay() {
        composite(&mut img, o);
    }
    std::fs::write(path, kotor_image::to_png(&img)).unwrap();
    println!("wrote {}", path.display());
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).map_or("docs/shots/play", String::as_str));
    std::fs::create_dir_all(&out)?;
    let mut s = Stage::open("end_m01aa", true)?;
    let mut gpu = GpuRenderer::new(SIZE.0, SIZE.1)?;
    let scene = s.static_scene().clone();
    gpu.set_static_scene(&scene, &mut |n| s.texture(n));
    // The opening conversation, then Trask's talk with its replies.
    let (mut talked, mut replied, mut asked) = (false, false, false);
    // Through the opening's own camera, a second into its first beat, and in its third.
    for _ in 0..60 {
        s.update(DT);
    }
    shot(&mut gpu, &mut s, &out.join("cutscene_1.png"));
    for _ in 0..300 {
        s.update(DT);
    }
    shot(&mut gpu, &mut s, &out.join("cutscene_2.png"));
    for _ in 0..6000 {
        s.update(DT);
        match s.game.world.dialogue_view() {
            Some((_, text, _, replies)) => {
                if !talked && !text.is_empty() {
                    talked = true;
                    shot(&mut gpu, &mut s, &out.join("dialogue_line.png"));
                }
                if !replied && !replies.is_empty() {
                    replied = true;
                    // Let the camera swing round to the speaker.
                    for _ in 0..90 {
                        s.update(DT);
                    }
                    shot(&mut gpu, &mut s, &out.join("dialogue_replies.png"));
                }
                s.number_key(1);
                s.continue_line();
            }
            None if talked && !asked => {
                asked = true;
                let trask = s.game.world.by_tag("end_trask")[0];
                s.game.world.talk_to(trask);
            }
            None if asked => break,
            None => {}
        }
    }
    let (w, h) = (SIZE.0 as f32, SIZE.1 as f32);
    // Click down the screen until one lands on the floor.
    let mut step = 0.6;
    while !s.game.world.pc_walking() && step > 0.45 {
        s.click((w * 0.5, h * step), SIZE);
        step -= 0.02;
    }
    for _ in 0..25 {
        s.update(DT);
    }
    shot(&mut gpu, &mut s, &out.join("walking_hud.png"));
    // The HUD with a target, its health bar and its action slot, then class 4 armour on the player.
    let pc = s.game.world.pc;
    if s.stand_near("end_locker01") {
        // Turn to where the camera has the most room.
        let here = s.game.world.object(pc).map_or([0.0; 3], |o| o.position);
        let room = |s: &Stage| {
            let e = s.camera().eye;
            (e[0] - here[0]).hypot(e[1] - here[1])
        };
        let mut best = (0.0, s.yaw);
        for k in 0..16 {
            s.yaw = k as f32 * std::f32::consts::TAU / 16.0;
            if room(&s) > best.0 {
                best = (room(&s), s.yaw);
            }
        }
        s.yaw = best.1;
        s.update(DT);
        s.update(DT);
        shot(&mut gpu, &mut s, &out.join("hud_target.png"));
        let armour = s.game.world.spawn_item("g_a_class4001");
        if let Some(o) = s.game.world.object_mut(pc) {
            o.equipped.insert(1, armour);
        }
        shot(&mut gpu, &mut s, &out.join("armour.png"));
    }
    println!(
        "player walking: {}; {} lines shown; {} world errors",
        s.game.world.pc_walking(),
        s.lines_shown,
        s.game.world.errors.len()
    );
    // A stock texture with alpha: the ivy of Dantooine.
    if let Ok(mut d) = Stage::open("danm13", true) {
        let scene = d.static_scene().clone();
        gpu.set_static_scene(&scene, &mut |n| d.texture(n));
        let ivy = [82.16, 106.1, 9.38];
        let cam = kotor_render::Camera {
            eye: [ivy[0] - 3.5, ivy[1] - 3.5, ivy[2] + 0.8],
            target: ivy,
            fov: 55.0,
        };
        let moving = kotor_render::Scene::default();
        let img = gpu.render(&moving, &mut |n| d.texture(n), &cam)?;
        std::fs::write(out.join("alpha_ivy.png"), kotor_image::to_png(&img))?;
        gpu.set_transparency(false);
        gpu.set_static_scene(&scene, &mut |n| d.texture(n));
        let img = gpu.render(&moving, &mut |n| d.texture(n), &cam)?;
        std::fs::write(out.join("alpha_ivy_off.png"), kotor_image::to_png(&img))?;
    }
    Ok(())
}
