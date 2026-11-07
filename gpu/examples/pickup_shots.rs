//! Frames of the Pickup button, its panel and the word when there is nothing. Offscreen: no window, no screen capture, no saves.
//!
//!   cargo run --release --example pickup_shots -- [out_dir] [width] [height]
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

    // Nobody about to be the target, and nothing to find.
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

    // The button, under the cursor.
    let slot = s.hud_pixel("BTN_ACTION0", size).expect("the slot");
    s.hover(slot, size);
    shot(&mut s, "pickup_button");
    s.hover((width as f32 / 2.0, height as f32 / 2.0), size);

    // Far from anything: a word over the player.
    s.pickup();
    println!("opened: {:?}", s.game.world.open_sources());
    println!("near nothing: {:?}", s.floats_shown());
    shot(&mut s, "pickup_nothing");

    // A footlocker and two sets of remains, with credits in each.
    s.stand_near("end_locker01");
    s.update(DT);
    let w = &mut s.game.world;
    let locker = w.by_tag("end_locker01")[0];
    w.object_mut(locker).unwrap().useable = true;
    let here = w.object(pc).unwrap().position;
    for (dx, template, count) in [(0.6, "g_i_medeqpmnt01", 2), (-0.6, "g_w_lngswrd01", 1)] {
        let body = w.spawn_creature("n_calonord", [here[0] + dx, here[1], here[2]]);
        let item = w.spawn_item(template);
        w.object_mut(item).unwrap().stack = count;
        let credits = w.spawn_item("g_i_credits001");
        w.object_mut(credits).unwrap().stack = 25;
        let o = w.object_mut(body).unwrap();
        o.dead = true;
        o.useable = true;
        o.equipped.clear();
        o.inventory = vec![credits, item];
    }
    s.pickup();
    println!("{}", s.state_line());
    shot(&mut s, "pickup_panel");
    Ok(())
}
