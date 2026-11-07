//! Frames of the four quick slots and the Solo button, to put beside the owner's pictures of the original. Offscreen: no window, no
//! screen capture, no saves.
//!
//!   cargo run --release --example slots_shots -- [out_dir] [width] [height]
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

const DT: f32 = 1.0 / 30.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).map_or("audit", String::as_str));
    let width: u32 = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(1280);
    let height: u32 = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(720);
    let size = (width, height);
    std::fs::create_dir_all(&out)?;

    let mut s = Stage::open("end_m01aa", true)?;
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    s.set_view(size);
    let mut gpu = GpuRenderer::new(width, height)?;
    let scene = s.static_scene().clone();
    gpu.set_static_scene(&scene, &mut |n| s.texture(n));
    for _ in 0..(50.0 / DT) as usize {
        s.update(DT);
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        }
    }
    let mut shot = |s: &mut Stage, name: &str| {
        s.update(DT);
        let scene = s.dynamic_scene();
        let cam = s.camera();
        gpu.set_particles(s.particles());
        let mut img = gpu.render(&scene, &mut |n| s.texture(n), &cam).unwrap();
        if let Some(o) = s.overlay() {
            composite(&mut img, o);
        }
        let p = out.join(format!("{name}_{height}.png"));
        std::fs::write(&p, kotor_image::to_png(&img)).unwrap();
        println!("wrote {}", p.display());
    };

    // Nobody to be the target.
    let w = &mut s.game.world;
    let pc = w.pc;
    let others: Vec<u32> = w
        .objects
        .values()
        .filter(|o| o.id != pc && o.kind == kotor_game::Kind::Creature)
        .map(|o| o.id)
        .collect();
    for id in others {
        let at = w.object(id).unwrap().position;
        w.teleport(id, [at[0] + 500.0, at[1], at[2]]);
    }
    for o in w.objects.values_mut() {
        o.useable = false;
    }
    shot(&mut s, "slots_soldier_empty");

    // A soldier with a medpac, a stim and a shield: the far left is Pickup and the far right is empty, as in the owner's picture.
    let w = &mut s.game.world;
    for template in ["g_i_medeqpmnt01", "g_i_adrnaline003", "g_i_frarmbnds01"] {
        let item = w.spawn_item(template);
        w.object_mut(pc).unwrap().inventory.push(item);
        if template == "g_i_frarmbnds01" {
            println!("worn in {:?}", w.equip(pc, item, None));
        }
    }
    shot(&mut s, "slots_soldier");

    // A Jedi: Heal and Cure at the far left, and the lasting ones at the far right.
    let o = s.game.world.object_mut(pc).unwrap();
    o.spells.extend([28, 10, 8, 17]);
    (o.force, o.max_force) = (100, 100);
    shot(&mut s, "slots_jedi");

    // The far right on, and the far left's name under the cursor.
    let slot = s.hud_pixel("BTN_ACTION3", size).expect("the slot");
    s.click(slot, size);
    let slot = s.hud_pixel("BTN_ACTION0", size).expect("the slot");
    s.hover(slot, size);
    shot(&mut s, "slots_jedi_on");

    // Solo, on.
    s.game.world.solo_mode = true;
    shot(&mut s, "slots_solo_on");
    Ok(())
}
