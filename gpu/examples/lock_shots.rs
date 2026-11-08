//! Frames of the button for a lock, plain and with its arrows. Offscreen: no window, no screen capture, no saves.
//!
//!   cargo run --release --example lock_shots -- [out_dir] [WxH ...]
//!
//! In `LOCK_MODULE`, else the Endar Spire.
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

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

    // `LOCK_MODULE=tar_m02aa` for another place: Taris's upper city.
    let module = std::env::var("LOCK_MODULE").unwrap_or_else(|_| "end_m01aa".to_string());
    let mut s = Stage::open(&module, true)?;
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    for _ in 0..(50.0 / DT) as usize {
        s.update(DT);
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        }
    }
    // A locked door beside the player, with nothing else in reach.
    let w = &mut s.game.world;
    let doors: Vec<u32> = w
        .objects
        .values()
        .filter(|o| o.kind == kotor_game::Kind::Door && o.link_module.is_empty())
        .map(|o| o.id)
        .collect();
    let door = doors
        .into_iter()
        .find(|d| w.lock_can_be_picked(*d))
        .expect("a door");
    for o in w.objects.values().filter(|o| o.kind == kotor_game::Kind::Door) {
        println!(
            "door {:?}: tag {:?}, template {:?}, locked {}, clickable {}{}",
            o.name,
            o.tag,
            o.template,
            o.locked,
            w.clickable(o.id),
            if o.id == door { ", the one shown" } else { "" }
        );
    }
    let at = w.object(w.pc).unwrap().position;
    for o in w.objects.values_mut() {
        o.useable = false;
    }
    let o = w.object_mut(door).unwrap();
    o.locked = true;
    o.open = false;
    o.useable = true;
    o.conversation.clear();
    o.scripts.clear();
    o.position = [at[0] - 0.8, at[1], at[2]];
    let pc = w.pc;
    let body = w.object_mut(pc).unwrap();
    body.skills = vec![0; 8];
    body.skills[6] = 20;

    for spike in [false, true] {
        if spike {
            let w = &mut s.game.world;
            let item = w.spawn_item("g_i_secspike02");
            w.object_mut(pc).unwrap().inventory.push(item);
        }
        let name = if spike { "lock_arrows" } else { "lock_plain" };
        for size in &sizes {
            s.set_view(*size);
            let mut gpu = GpuRenderer::new(size.0, size.1)?;
            let scene = s.static_scene().clone();
            gpu.set_static_scene(&scene, &mut |n| s.texture(n));
            for _ in 0..10 {
                s.yaw = std::f32::consts::PI;
                s.update(DT);
            }
            let scene = s.dynamic_scene();
            let cam = s.camera();
            gpu.set_particles(s.particles());
            let mut img = gpu.render(&scene, &mut |n| s.texture(n), &cam)?;
            if let Some(o) = s.overlay() {
                composite(&mut img, o);
            }
            let p = out.join(format!("{name}_{}.png", size.1));
            std::fs::write(&p, kotor_image::to_png(&img))?;
            println!(
                "wrote {} button {:?}",
                p.display(),
                s.hud_place("BTN_TARGET0")
            );
        }
    }
    Ok(())
}
