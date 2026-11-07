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
/// Frames since the start, for what is done every few.
static FRAMES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

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

/// What the viewer does each frame before it draws: put a changed picture on the GPU, and the moving figures seen through a
/// doorway. Records what that cost, for the frame log.
fn sync(
    gpu: &mut GpuRenderer,
    s: &mut Stage,
    seen: &mut (u64, u64),
    log: &mut Vec<(f32, &'static str)>,
) {
    let warm = s.take_warm(3);
    if !warm.is_empty() {
        let t = Instant::now();
        gpu.warm_textures(&warm, &mut |n| s.texture(n));
        log.push((
            t.elapsed().as_secs_f32() * 1000.0,
            "textures go up, three a frame",
        ));
    }
    if seen.0 != s.static_version {
        let t = Instant::now();
        let scene = s.static_scene().clone();
        gpu.set_static_scene(&scene, &mut |n| s.texture(n));
        seen.0 = s.static_version;
        log.push((t.elapsed().as_secs_f32() * 1000.0, "the picture goes up"));
    }
    if seen.1 != s.portal_version() {
        let t = Instant::now();
        let other = s.portal().map(|(sc, a)| (sc.clone(), a));
        gpu.set_portal(other.as_ref().map(|(sc, a)| (sc, *a)), &mut |n| {
            s.texture(n)
        });
        seen.1 = s.portal_version();
        log.push((
            t.elapsed().as_secs_f32() * 1000.0,
            "the portal picture goes up",
        ));
    }
    // Bodies seen through a doorway are posed three frames in four from the last time: idling is slow.
    if s.portal().is_some()
        && FRAMES
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            .is_multiple_of(3)
    {
        let t = Instant::now();
        let moving = s.portal_figures();
        gpu.set_portal_moving(&moving, &mut |n| s.texture(n));
        log.push((
            t.elapsed().as_secs_f32() * 1000.0,
            "figures seen through the doorway",
        ));
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
    let mut uploads: Vec<(f32, &'static str)> = Vec::new();
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
    let sketch = s.game.world.sketch();
    // Let any opening lines of the module pass.
    for _ in 0..std::env::var("DRIVE_WAIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(600)
    {
        step(&mut s);
    }
    // The door, and the way through it: from where the player stands, away from the walkmesh's pocket behind the door.
    // (taken before the module's first frames: once a neighbour is joined the doorway no longer reads as a closed door)
    // `tag#2` is the second site with that tag, for modules whose doors share one.
    if std::env::var_os("DRIVE_DEBUG").is_some() {
        for x in &sketch.sites {
            println!(
                "site {} {:?} at {:?} out {:?} stub {}",
                x.tag,
                x.kind,
                x.at,
                x.out,
                x.stub.len()
            );
        }
        for o in s
            .game
            .world
            .objects
            .values()
            .filter(|o| o.kind == kotor_game::Kind::Door)
        {
            println!(
                "door {} at {:?} open {} locked {} link {}",
                o.tag, o.position, o.open, o.locked, o.link_module
            );
        }
    }
    let (tag_name, nth) = match tag.split_once('#') {
        Some((t, n)) => (t.to_string(), n.parse::<usize>().unwrap_or(1).max(1) - 1),
        None => (tag.clone(), 0),
    };
    let site = sketch
        .sites
        .iter()
        .filter(|x| x.tag == tag_name)
        .nth(nth)
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
    // As far back from the doorway as there is floor, up to 7 m, so that the walk up to it is a real one.
    let start = {
        let nav = s.game.world.nav.as_ref().ok_or("no floor")?;
        let mut best = None;
        let mut d = 7.0f32;
        while d >= 1.5 {
            let (x, y) = (
                site.at[0] - out_angle.cos() * d,
                site.at[1] - out_angle.sin() * d,
            );
            if let Some(z) = nav.walkable_at(x, y, site.at[2] + 2.0) {
                best = Some([x, y, z]);
                break;
            }
            d -= 0.5;
        }
        best.ok_or("no floor to walk up from")?
    };
    let pc = s.game.world.pc;
    s.game.world.teleport(pc, start);
    s.yaw = out_angle;
    // Wait for the neighbour to load on its thread.
    let began = Instant::now();
    while s.game.world.seamless.attached.is_empty() && began.elapsed().as_secs() < 90 {
        step(&mut s);
        sync(&mut gpu, &mut s, &mut seen, &mut uploads);
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
    // By the route the game itself finds, to a point just before the doorway: the floor before it can wind.
    s.steer = (0.0, 0.0, false);
    let mut pre = site.at;
    let mut routed = false;
    for d in [1.0f32, 1.5, 2.0, 3.0, 4.0] {
        let (x, y) = (
            site.at[0] - out_angle.cos() * d,
            site.at[1] - out_angle.sin() * d,
        );
        let Some(z) = s
            .game
            .world
            .nav
            .as_ref()
            .and_then(|n| n.walkable_at(x, y, site.at[2] + 2.0))
        else {
            continue;
        };
        pre = [x, y, z];
        if s.game.world.walk_to(pre) {
            routed = true;
            break;
        }
    }
    while dist(
        s.game.world.pc_mover.pos,
        if routed { pre } else { site.at },
    ) > 0.7
        && walked < 1500
    {
        step(&mut s);
        sync(&mut gpu, &mut s, &mut seen, &mut uploads);
        let d = track(&mut s, &mut last, &mut module_now);
        max_step = max_step.max(d);
        teleports += (d > 0.5) as u32;
        menus += s.menu_open() as u32;
        walked += 1;
    }
    s.steer = (0.0, 0.0, false);
    r.check(
        "the player walks up to the door",
        dist(s.game.world.pc_mover.pos, pre) <= 1.0,
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
        sync(&mut gpu, &mut s, &mut seen, &mut uploads);
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
    // Through, the way the doorway faces.
    s.yaw = out_angle;
    s.steer = (1.0, 0.0, false);
    let mut through = 0;
    let mut shot_mid = false;
    let mut crossed_at = None;
    while through < 400 {
        step(&mut s);
        sync(&mut gpu, &mut s, &mut seen, &mut uploads);
        let d = track(&mut s, &mut last, &mut module_now);
        max_step = max_step.max(d);
        teleports += (d > 0.5) as u32;
        menus += s.menu_open() as u32;
        through += 1;
        if through % 100 == 1 && std::env::var_os("DRIVE_DEBUG").is_some() {
            if let Some(nav) = s.game.world.nav.as_ref() {
                let row: String = (0..40)
                    .map(|k| {
                        let d = -2.0 + k as f32 * 0.25;
                        let (x, y) = (
                            site.at[0] + out_angle.cos() * d,
                            site.at[1] + out_angle.sin() * d,
                        );
                        if nav.walkable_at(x, y, site.at[2] + 2.0).is_some() {
                            '#'
                        } else {
                            '.'
                        }
                    })
                    .collect();
                println!("floor from 2 m before the doorway to 8 m past: {row}");
            }
            for a in s.game.world.seamless.attached.values() {
                println!(
                    "joined {} portal {} gaps {:.2} {:.2} half {:.2} out {:.2}",
                    a.module, a.portal, a.gap_before, a.gap_after, a.half, a.out
                );
            }
            println!(
                "through {through}: pc {:?} steer {:?} yaw {:.2} out {out_angle:.2}",
                s.game.world.pc_mover.pos, s.game.world.steer, s.yaw
            );
        }
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
    // A sprint is 1.6 times a run and a route is walked a little faster still: a step under 0.35 m is a stride, and a
    // teleport is metres.
    let bound = kotor_game::RUN_SPEED * DT * 3.9;
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
    s.steer = (0.0, 0.0, false);
    let mut back = 0;
    let mut back_shot = false;
    while back < 900 && s.game.world.module == inside {
        // Back by the game's own route to just past the doorway we came through.
        if back == 0 {
            if let Some(a) = s.game.world.seamless.attached.values().find(|a| a.behind) {
                let (x, y) = (a.at[0] + a.out.cos() * 1.5, a.at[1] + a.out.sin() * 1.5);
                let z = s
                    .game
                    .world
                    .nav
                    .as_ref()
                    .and_then(|n| n.walkable_at(x, y, a.at[2] + 2.0))
                    .unwrap_or(a.at[2]);
                s.steer = (0.0, 0.0, false);
                s.game.world.walk_to([x, y, z]);
            }
        }
        step(&mut s);
        sync(&mut gpu, &mut s, &mut seen, &mut uploads);
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
        sync(&mut gpu, &mut s, &mut seen, &mut uploads);
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
    // What each frame paid on top of the update to get the picture up: the worst few, by what they were.
    for (ms, what) in uploads.iter().filter(|u| u.0 > 8.0) {
        println!("frame log, over 8 ms: {ms:.1} ms for {what}");
    }
    uploads.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (ms, what) in uploads.iter().take(4) {
        println!("frame log, extra work before drawing: {ms:.1} ms for {what}");
    }
    let spent: f32 = uploads.iter().map(|u| u.0).sum();
    println!(
        "frame log: {} uploads in all, {spent:.0} ms together, none over {:.0} ms but the worst four above",
        uploads.len(),
        uploads.get(4).map_or(0.0, |u| u.0)
    );
    if r.failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}
