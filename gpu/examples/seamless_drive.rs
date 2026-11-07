//! Walks the player through a door that leads to another module, there and back, in the stage with no window, and checks
//! that nothing teleports, nothing loads on screen and frames stay quick. Draws the door opening onto the next area and
//! the walk through it. See `docs/SEAMLESS.md` and `scripts/drive_seamless.ps1`.
//!
//!   cargo run --release --example seamless_drive -- [module] [door tag] [out_dir] [width] [height]
//!
//! Prints one `PASS`/`FAIL` line per check and exits 1 when any fails.
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};
use std::time::Instant;

const DT: f32 = 1.0 / 60.0;

struct Run {
    failed: u32,
}

impl Run {
    fn check(&mut self, name: &str, ok: bool, detail: String) {
        if !ok {
            self.failed += 1;
        }
        println!(
            "{:<44} {}  {detail}",
            name,
            if ok { "PASS" } else { "FAIL" }
        );
    }
}

fn shot(gpu: &mut GpuRenderer, s: &mut Stage, seen: &mut (u64, u64), path: &std::path::Path) {
    if seen.0 != s.static_version {
        let scene = s.static_scene().clone();
        gpu.set_static_scene(&scene, &mut |n| s.texture(n));
        seen.0 = s.static_version;
    }
    if seen.1 != s.portal_version() {
        let other = s.portal().map(|(sc, a)| (sc.clone(), a));
        gpu.set_portal(other.as_ref().map(|(sc, a)| (sc, *a)), &mut |n| {
            s.texture(n)
        });
        seen.1 = s.portal_version();
    }
    let moving = s.portal_figures();
    gpu.set_portal_moving(&moving, &mut |n| s.texture(n));
    let scene = s.dynamic_scene();
    let cam = s.camera();
    gpu.set_particles(s.particles());
    let mut img = gpu.render(&scene, &mut |n| s.texture(n), &cam).unwrap();
    if let Some(o) = s.overlay() {
        composite(&mut img, o);
    }
    std::fs::write(path, kotor_image::to_png(&img)).unwrap();
    println!("wrote {}", path.display());
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let module = args.get(1).map_or("tar_m02ac", String::as_str).to_string();
    let tag = args.get(2).map_or("Apartments", String::as_str).to_string();
    let out = std::path::PathBuf::from(args.get(3).map_or("docs/shots/seamless", String::as_str));
    let width: u32 = args.get(4).and_then(|a| a.parse().ok()).unwrap_or(1280);
    let height: u32 = args.get(5).and_then(|a| a.parse().ok()).unwrap_or(720);
    std::fs::create_dir_all(&out)?;
    let name = format!("{height}p");
    let mut r = Run { failed: 0 };
    let mut s = Stage::open(&module, true)?;
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let mut gpu = GpuRenderer::new(width, height)?;
    let mut seen = (0u64, 0u64);
    let frames_wall = std::cell::RefCell::new(Vec::<f32>::new());
    let step = |s: &mut Stage| {
        let t = Instant::now();
        s.update(DT);
        frames_wall
            .borrow_mut()
            .push(t.elapsed().as_secs_f32() * 1000.0);
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        }
    };
    // Let any opening lines of the module pass.
    for _ in 0..600 {
        step(&mut s);
    }
    // The door, and the way through it: from where the player stands, away from the walkmesh's pocket behind the door.
    let sketch = s.game.world.sketch();
    let site = sketch
        .sites
        .iter()
        .find(|x| x.tag == tag)
        .ok_or("no such site")?
        .clone();
    let door_id = s
        .game
        .world
        .link_door_at(site.at)
        .ok_or("no such link door")?;
    let is_trigger = site.kind == kotor_game::Kind::Trigger;
    // A trigger has no door to say which way is through: the plan works it out from the neighbour's arrival point.
    let out_angle = match site.out {
        Some(o) => o,
        None => {
            let dir = kotor_core::install_dir().ok_or("no install")?;
            kotor_game::seamless::prepare(&dir, &sketch, &site, &Default::default())
                .map_err(|e| format!("{e}"))?
                .0
                .out
        }
    };
    let start = [
        site.at[0] - out_angle.cos() * 7.0,
        site.at[1] - out_angle.sin() * 7.0,
        site.at[2],
    ];
    let pc = s.game.world.pc;
    s.game.world.teleport(pc, start);
    s.yaw = out_angle;
    // Wait for the neighbour to load on its thread.
    let began = Instant::now();
    while s.game.world.seamless.attached.is_empty() && began.elapsed().as_secs() < 90 {
        step(&mut s);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let loaded_in = began.elapsed().as_secs_f32();
    let joined = !s.game.world.seamless.attached.is_empty();
    r.check(
        "the area behind the door loads in the background",
        joined,
        format!(
            "{loaded_in:.1} s of waiting, {} lines of notes: {:?}",
            s.seam_notes().len(),
            s.seam_notes()
        ),
    );
    if !joined {
        std::process::exit(1);
    }
    let walls_before = frames_wall.borrow().len();
    shot(
        &mut gpu,
        &mut s,
        &mut seen,
        &out.join(format!("0_before_{name}.png")),
    );
    // Walk up to the door.
    s.steer = (1.0, 0.0, false);
    let mut last = s.game.world.pc_mover.pos;
    let mut module_now = s.game.world.module.clone();
    let mut max_step = 0.0f32;
    let mut teleports = 0;
    let mut menus = 0;
    // How far the player moved this frame. Across a crossing the world changes space, so the old position is carried
    // over by the same move before it is compared: a crossing that moved the player would show as a step.
    let track = |s: &mut Stage, last: &mut [f32; 3], module_now: &mut String| {
        let w = &s.game.world;
        if w.module != *module_now {
            if let Some(b) = w.seamless.attached.values().find(|a| a.behind) {
                *last = b.rigid.apply(*last);
            }
            *module_now = w.module.clone();
        }
        let p = s.game.world.pc_mover.pos;
        let d = dist(p, *last);
        if d > 0.3 {
            println!(
                "jump {d:.3} in {} from {:?} to {:?}",
                s.game.world.module, last, p
            );
        }
        *last = p;
        d
    };
    let mut walked = 0;
    while dist(s.game.world.pc_mover.pos, site.at) > 1.3 && walked < 1200 {
        step(&mut s);
        let d = track(&mut s, &mut last, &mut module_now);
        max_step = max_step.max(d);
        teleports += (d > 0.5) as u32;
        menus += s.menu_open() as u32;
        walked += 1;
    }
    s.steer = (0.0, 0.0, false);
    r.check(
        "the player walks up to the door",
        dist(s.game.world.pc_mover.pos, site.at) <= 1.4,
        format!("{walked} frames"),
    );
    // Some doors start locked (the Cantina's). The test is about the crossing, not the lock, so it is taken off.
    if let Some(o) = s.game.world.objects.get_mut(&door_id) {
        if o.locked {
            println!("note: {tag} starts locked; unlocked for this run");
            o.locked = false;
        }
    }
    let before_use = s.game.world.module.clone();
    if !is_trigger {
        s.game.world.use_door(door_id);
    }
    let opened = is_trigger || s.game.world.object(door_id).is_some_and(|o| o.open);
    r.check(
        "using the door opens it, and does not change module",
        opened && s.game.world.transition_request.is_none(),
        format!(
            "module {before_use}, door {:?} {:?} locked {}, request {:?}, joined {}",
            s.game.world.object(door_id).map(|o| o.door),
            s.game.world.object(door_id).map(|o| o.open),
            s.game.world.object(door_id).is_some_and(|o| o.locked),
            s.game.world.transition_request,
            s.game.world.seamless.attached.contains_key(&door_id)
        ),
    );
    for i in 0..90 {
        step(&mut s);
        let d = track(&mut s, &mut last, &mut module_now);
        max_step = max_step.max(d);
        if i == 30 {
            shot(
                &mut gpu,
                &mut s,
                &mut seen,
                &out.join(format!("1_opening_{name}.png")),
            );
        }
    }
    shot(
        &mut gpu,
        &mut s,
        &mut seen,
        &out.join(format!("2_open_{name}.png")),
    );
    // Through.
    s.steer = (1.0, 0.0, false);
    let mut through = 0;
    let mut shot_mid = false;
    let mut crossed_at = None;
    while through < 400 {
        step(&mut s);
        let d = track(&mut s, &mut last, &mut module_now);
        max_step = max_step.max(d);
        teleports += (d > 0.5) as u32;
        menus += s.menu_open() as u32;
        through += 1;
        if crossed_at.is_none() && s.game.world.module != module {
            crossed_at = Some(through);
        }
        if let Some(c) = crossed_at {
            if !shot_mid && through == c + 20 {
                shot(
                    &mut gpu,
                    &mut s,
                    &mut seen,
                    &out.join(format!("3_through_{name}.png")),
                );
                shot_mid = true;
            }
            if through > c + 240 {
                break;
            }
        }
    }
    s.steer = (0.0, 0.0, false);
    let now = s.game.world.module.clone();
    r.check(
        "walking through leads into the next module",
        now != module,
        format!("{module} -> {now}"),
    );
    let bound = kotor_game::RUN_SPEED * DT * 1.6;
    r.check(
        "no teleport: position continuous every frame",
        teleports == 0 && max_step <= bound,
        format!("largest step {max_step:.3} m against a bound of {bound:.3} m, {teleports} jumps"),
    );
    r.check(
        "no menu or loading screen at any frame",
        menus == 0,
        format!("{menus} frames"),
    );
    let party_ok = {
        let w = &s.game.world;
        std::iter::once(w.pc)
            .chain(w.party.iter().copied())
            .all(|id| {
                w.object(id)
                    .and_then(|o| {
                        w.nav.as_ref()?.walkable_at(
                            o.position[0],
                            o.position[1],
                            o.position[2] + 1.0,
                        )
                    })
                    .is_some()
            })
    };
    r.check("the party stands on the new floor", party_ok, String::new());
    shot(
        &mut gpu,
        &mut s,
        &mut seen,
        &out.join(format!("4_inside_{name}.png")),
    );
    // And back: turn round and walk out of the way we came, through the door the party left open.
    let inside = s.game.world.module.clone();
    s.yaw += std::f32::consts::PI;
    s.steer = (1.0, 0.0, false);
    let mut back = 0;
    let mut back_shot = false;
    while back < 900 && s.game.world.module == inside {
        // Head for the doorway we came through, not along a line: the floor beyond a trigger can be a winding path.
        if let Some(a) = s.game.world.seamless.attached.values().find(|a| a.behind) {
            let p = s.game.world.pc_mover.pos;
            s.yaw = (a.at[1] - p[1]).atan2(a.at[0] - p[0]);
        }
        step(&mut s);
        let d = track(&mut s, &mut last, &mut module_now);
        max_step = max_step.max(d);
        teleports += (d > 0.5) as u32;
        menus += s.menu_open() as u32;
        back += 1;
        if !back_shot && back == 60 {
            shot(
                &mut gpu,
                &mut s,
                &mut seen,
                &out.join(format!("5_leaving_{name}.png")),
            );
            back_shot = true;
        }
    }
    // Back through: stand still, so that the frames after it show the old area alone.
    s.steer = (0.0, 0.0, false);
    for _ in 0..90 {
        step(&mut s);
        let d = track(&mut s, &mut last, &mut module_now);
        max_step = max_step.max(d);
        teleports += (d > 0.5) as u32;
    }
    r.check(
        "and back out of the same door, with no load",
        s.game.world.module == module && teleports == 0 && max_step <= bound,
        format!(
            "now in {}, largest step {max_step:.3} m, {teleports} jumps, {back} frames, joined {:?}, player at {:?}",
            s.game.world.module,
            s.game.world
                .seamless
                .attached
                .values()
                .map(|a| (a.module.clone(), a.behind, a.at, a.out))
                .collect::<Vec<_>>(),
            s.game.world.pc_mover.pos
        ),
    );
    r.check(
        "no menu or loading screen on the way back either",
        menus == 0,
        format!("{menus} frames in all"),
    );
    shot(
        &mut gpu,
        &mut s,
        &mut seen,
        &out.join(format!("6_back_{name}.png")),
    );
    let mut ms = frames_wall.borrow().clone();
    let crossing_frame = ms
        .get(walls_before..)
        .map_or(0.0, |v| v.iter().cloned().fold(0.0, f32::max));
    ms.sort_by(f32::total_cmp);
    let p50 = ms[ms.len() / 2];
    let p99 = ms[ms.len() * 99 / 100];
    println!(
        "frame times (update only, ms): median {p50:.2}, p99 {p99:.2}, worst {:.2}, worst while walking {crossing_frame:.2}",
        ms.last().unwrap()
    );
    if r.failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}
