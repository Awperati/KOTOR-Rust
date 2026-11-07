//! Frames of the compass, its icons and a tracked quest at several window sizes. Offscreen: no window, no screen capture, no saves.
//!
//!   cargo run --release --example compass_shots -- [out_dir] [WxH ...]
use kotor_game::Kind;
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

const DT: f32 = 1.0 / 30.0;
const SIZES: [(u32, u32); 3] = [(1280, 720), (3840, 2160), (720, 1280)];

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
    // Something of each kind round the player, so that the frames show the icons.
    let w = &mut s.game.world;
    let at = w.pc_mover.pos;
    let pc = w.pc;
    for o in w.objects.values_mut().filter(|o| o.id != pc) {
        o.conversation.clear();
        o.scripts.clear();
    }
    w.delayed.clear();
    let yaw = s.yaw;
    let around = |right: f32, far: f32| {
        let b = yaw - right;
        [at[0] + b.cos() * far, at[1] + b.sin() * far, at[2]]
    };
    let w = &mut s.game.world;
    let door = w
        .objects
        .values()
        .find(|o| o.kind == Kind::Door)
        .map(|o| o.id)
        .expect("a door");
    let o = w.object_mut(door).unwrap();
    o.link_module = "somewhere".to_string();
    o.name = "Starboard Section".to_string();
    o.position = around(-0.5, 18.0);
    let store = w.spawn_creature("n_calonord", around(0.7, 12.0));
    let o = w.object_mut(store).unwrap();
    o.kind = Kind::Store;
    let foe = w.spawn_creature("n_calonord", around(-1.1, 14.0));
    w.object_mut(foe).unwrap().faction = 1;
    w.combat.perceived.insert((pc, foe));
    let person = w.spawn_creature("n_calonord", around(0.25, 8.0));
    let o = w.object_mut(person).unwrap();
    o.name = "Someone".to_string();
    o.conversation = "anything".to_string();
    o.faction = 5;
    o.useable = true;
    // A quest with a step to show. What knows the steps is built on a thread, the first time.
    let tags = w.quest_tags();
    let mut tag = tags.first().cloned().expect("a quest");
    'found: for _ in 0..120 {
        for t in &tags {
            w.journal.clear();
            w.journal.insert(t.clone(), 1);
            if w.next_step(t).is_some() {
                tag = t.clone();
                break 'found;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    w.journal.clear();
    w.journal.insert(tag.clone(), 1);
    w.journal_order = vec![tag.clone()];
    println!("tracking {tag}: {:?}", w.next_step(&tag));

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
            println!("wrote {} {:?}", p.display(), s.compass_place());
        };
        s.yaw = yaw;
        if s.tracked_quest().is_some() {
            s.toggle_track(&tag);
        }
        shot(&mut s, "compass_plain");

        s.toggle_track(&tag);
        let _ = s.compass_now();
        shot(&mut s, "compass_tracked");

        // The step behind the camera: the pin held at the end of the strip, and no arrow on the screen.
        let here = s.game.world.pc_mover.pos;
        if let Some(step) = s.game.world.next_step(&tag) {
            let bearing = (step.position[1] - here[1]).atan2(step.position[0] - here[0]);
            s.yaw = bearing + 2.4;
            let _ = s.compass_now();
            shot(&mut s, "compass_behind");
        }

        // The journal, with its buttons.
        s.yaw = yaw;
        s.open_journal();
        shot(&mut s, "compass_journal");
        s.open_journal();
    }
    Ok(())
}
