//! Rides every lift door, in Classic and in Modern, once with the E key and once with a mouse click on the door, there and
//! back, in the stage with no window. For each ride it checks that the door starts a ride, the module changes to the door's,
//! the player can walk away from where it lands, and no body that was already dead falls again. One `PASS`/`FAIL` line each;
//! exits 1 when any fails. With an out folder it also writes three frames of each ride: before, in the car, arrived.
//!
//!   cargo run --release --example lift_drive -- [module:tag[>to] ...] [--modes classic,modern] [--inputs e,click] [--out dir]
//!
//! With no lifts named it rides every lift door of the install: those whose tag or template says elevator or lift, and those
//! `docs/lifts.tsv` makes lifts (the Upper City's, Davik's estate's and the Star Forge's, named "Elevator" in the game).
use kotor_game::{GameMode, Kind, ObjId};
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};
use kotor_render::Camera;

const DT: f32 = 1.0 / 60.0;
const SIZE: (u32, u32) = (1280, 720);

const LIFTS: &[&str] = &[
    "tar_m02ab:tar02_lowdoor",
    "tar_m03aa:tar04_elevdoor>tar_m02ab",
    "tar_m03aa:tar04_elevdoor>tar_m04aa",
    "tar_m04aa:tar04_elevdoor",
    "tar_m05ab:tar05_elevator",
    "tar_m10aa:tar10_elev02",
    "tar_m10aa:tar10_elev03",
    "tar_m10ab:tar10_elevator",
    "tar_m10ac:tar10_elevator",
    "tar_m09aa:ptar_elevdoor",
    "tar_m09ab:tar09_elevdoor",
    "tar_m11aa:ptar_secdoor>tar_m11ab",
    "sta_m45ab:sta45b_sta45c",
    "sta_m45ac:sta45c_sta45d",
];

fn keep_alive(s: &mut Stage) {
    let mut ids = s.game.world.party.clone();
    ids.push(s.game.world.pc);
    for id in ids {
        if let Some(o) = s.game.world.object_mut(id) {
            o.plot = true;
            if o.hp < o.max_hp / 2 || o.down || o.dead {
                o.hp = o.max_hp.max(1);
                o.down = false;
                o.dead = false;
            }
        }
    }
    s.game.world.game_over = false;
}

/// One step of the game, with any line said and any menu the scripts put up closed.
fn step(s: &mut Stage) {
    keep_alive(s);
    s.update(DT);
    keep_alive(s);
    if s.game.world.in_dialogue() {
        if std::env::var_os("DRIVE_DEBUG").is_some() {
            println!("talk: {:?}", s.game.world.dialogue_view());
        }
        let n = std::env::var("DRIVE_REPLY")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);
        s.number_key(n);
        s.continue_line();
    }
    if s.menu_open() {
        for tag in ["BTN_DONE", "BTN_OK", "BTN_BACK"] {
            if let Some(p) = s.menu_pixel(tag, None, SIZE) {
                s.hover(p, SIZE);
                s.click(p, SIZE);
            }
        }
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: [f32; 3]) -> [f32; 3] {
    let l = dot(a, a).sqrt().max(1e-6);
    [a[0] / l, a[1] / l, a[2] / l]
}

/// Where point `p` shows in a window of `SIZE`, or None behind the camera.
fn project(cam: &Camera, p: [f32; 3]) -> Option<(f32, f32)> {
    let f = norm(sub(cam.target, cam.eye));
    let r = norm(cross(f, [0.0, 0.0, 1.0]));
    let u = cross(r, f);
    let v = sub(p, cam.eye);
    let z = dot(v, f);
    if z < 0.1 {
        return None;
    }
    let t = (cam.fov.to_radians() / 2.0).tan();
    let aspect = SIZE.0 as f32 / SIZE.1 as f32;
    let (nx, ny) = (dot(v, r) / (z * t * aspect), dot(v, u) / (z * t));
    Some((
        (nx + 1.0) / 2.0 * SIZE.0 as f32,
        (1.0 - ny) / 2.0 * SIZE.1 as f32,
    ))
}

/// The door of `module` with `tag` (leading to `to`, when given).
fn find_door(s: &Stage, tag: &str, to: Option<&str>) -> Option<ObjId> {
    s.game.world.by_tag(tag).into_iter().find(|d| {
        s.game.world.object(*d).is_some_and(|o| {
            o.kind == Kind::Door
                && !o.link_module.is_empty()
                && to.is_none_or(|t| o.link_module.eq_ignore_ascii_case(t))
                // A lift's controls, as the Leviathan's: a talk whose reply moves the party.
                || o.kind == Kind::Placeable && o.useable
        })
    })
}

/// The floor's height under `p`, if it is walkable there.
fn floor_at(s: &Stage, p: [f32; 3]) -> Option<f32> {
    s.game
        .world
        .nav
        .as_ref()?
        .walkable_at(p[0], p[1], p[2] + 2.0)
}

/// Stand the player a little way in front of `door`, on the side of the walkmesh, facing it, the camera behind.
fn stand_before(s: &mut Stage, door: ObjId) -> bool {
    let at = s.game.world.object(door).unwrap().position;
    let pc = s.game.world.pc;
    let mut best = None;
    for i in 0..16 {
        let a = i as f32 * std::f32::consts::TAU / 16.0;
        let p = [at[0] + a.cos() * 2.2, at[1] + a.sin() * 2.2, at[2]];
        if let Some(z) = floor_at(s, p) {
            if (z - at[2]).abs() < 1.0 {
                // The side with most floor further out is the room's.
                let q = [at[0] + a.cos() * 4.0, at[1] + a.sin() * 4.0, at[2]];
                let room = floor_at(s, q).is_some();
                let score = if room { 2 } else { 1 };
                if best.is_none_or(|(sc, _, _)| score > sc) {
                    best = Some((score, [p[0], p[1], z], a));
                }
            }
        }
    }
    let Some((_, p, a)) = best else {
        return false;
    };
    s.game.world.teleport(pc, p);
    // Face the door: the heading from the player to it.
    let heading = a + std::f32::consts::PI;
    s.game.world.pc_mover.heading = heading;
    for _ in 0..3 {
        step(s);
        let cam = s.camera();
        let look = (cam.target[1] - cam.eye[1]).atan2(cam.target[0] - cam.eye[0]);
        let mut d = heading - look;
        while d > std::f32::consts::PI {
            d -= std::f32::consts::TAU;
        }
        while d < -std::f32::consts::PI {
            d += std::f32::consts::TAU;
        }
        s.yaw += d;
    }
    step(s);
    true
}

/// Use `door` the way `input` says: the E key, or a click on it where it shows.
fn use_door(s: &mut Stage, door: ObjId, input: &str) -> Result<(), String> {
    if input == "e" {
        s.interact();
        return Ok(());
    }
    let at = s.game.world.object(door).unwrap().position;
    let cam = s.camera();
    // Where on the door a click finds the door: from waist height up and down, as a player aims at what shows of it.
    let mut hits = Vec::new();
    for h in [1.0f32, 1.4, 0.6, 1.8, 0.3, 2.2] {
        let Some(p) = project(&cam, [at[0], at[1], at[2] + h]) else {
            continue;
        };
        if p.0 < 0.0 || p.1 < 0.0 || p.0 > SIZE.0 as f32 || p.1 > SIZE.1 as f32 {
            hits.push(format!("{h} m off screen"));
            continue;
        }
        let (o, d) = kotor_game::pixel_ray(cam.eye, cam.target, cam.fov, SIZE, p);
        let hit = s.game.world.pick(o, d);
        if hit == Some(door) {
            s.hover(p, SIZE);
            s.click(p, SIZE);
            return Ok(());
        }
        let what = hit
            .and_then(|h| s.game.world.object(h))
            .map_or("nothing".to_string(), |o| format!("{:?} {}", o.kind, o.tag));
        hits.push(format!("{h} m finds {what}"));
    }
    Err(format!("no click finds the door: {}", hits.join(", ")))
}

/// Dead creatures whose fall is playing from its start.
fn falling_again(s: &Stage) -> Vec<String> {
    let w = &s.game.world;
    w.objects
        .values()
        .filter(|o| o.kind == Kind::Creature && o.dead && o.id != w.pc)
        .filter(|o| {
            s.game
                .clip(o.id)
                .is_some_and(|c| c.name == "die" && c.time < 3.0)
        })
        .map(|o| o.tag.clone())
        .collect()
}

/// Whether the player can walk off from where it stands: one of eight ways moves it half a metre in two seconds.
fn can_walk(s: &mut Stage) -> bool {
    let pc = s.game.world.pc;
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::TAU / 8.0;
        let from = s.game.world.object(pc).unwrap().position;
        let to = [from[0] + a.cos() * 3.0, from[1] + a.sin() * 3.0, from[2]];
        if floor_at(s, to).is_none() {
            continue;
        }
        s.game.world.walk_to(to);
        for _ in 0..120 {
            step(s);
        }
        let now = s.game.world.object(pc).unwrap().position;
        if (now[0] - from[0]).hypot(now[1] - from[1]) > 0.5 {
            return true;
        }
    }
    false
}

struct Shots<'a> {
    gpu: Option<GpuRenderer>,
    seen: u64,
    dir: Option<&'a std::path::Path>,
}

impl Shots<'_> {
    fn take(&mut self, s: &mut Stage, name: &str) {
        let (Some(gpu), Some(dir)) = (self.gpu.as_mut(), self.dir) else {
            return;
        };
        if self.seen != s.static_version {
            let scene = s.static_scene().clone();
            gpu.set_static_scene(&scene, &mut |n| s.texture(n));
            self.seen = s.static_version;
        }
        let scene = s.dynamic_scene();
        let cam = s.camera();
        let mut img = gpu.render(&scene, &mut |n| s.texture(n), &cam).unwrap();
        if let Some(o) = s.overlay() {
            composite(&mut img, o);
        }
        let path = dir.join(format!("{name}.png"));
        std::fs::write(&path, kotor_image::to_png(&img)).unwrap();
        println!("wrote {}", path.display());
    }
}

/// One ride through `door` and the checks on it. Returns the failures, and the module it ended in.
fn ride(
    s: &mut Stage,
    door: ObjId,
    input: &str,
    shots: &mut Shots,
    name: &str,
) -> (Vec<String>, String) {
    let mut fails = Vec::new();
    let from = s.game.world.module.clone();
    let (to, lift) = {
        let o = s.game.world.object(door).unwrap();
        (o.link_module.clone(), s.game.world.is_lift_door(door))
    };
    let controls = s.game.world.object(door).unwrap().kind == Kind::Placeable;
    if !lift && !controls {
        fails.push("the door is not taken for a lift's".to_string());
    }
    if let Some(o) = s.game.world.object_mut(door) {
        o.locked = false;
    }
    if !stand_before(s, door) {
        fails.push("no floor in front of the door".to_string());
        return (fails, from);
    }
    let dead_before = falling_again(s);
    shots.take(s, &format!("{name}_0_before"));
    if let Err(e) = use_door(s, door, input) {
        fails.push(e);
        return (fails, from);
    }
    let (mut rode, mut shot_mid, mut arrived_at) = (false, false, None);
    for i in 0..60 * 25 {
        step(s);
        if let Some(r) = s.game.world.lift_ride() {
            rode |= !r.dip;
            if !shot_mid && r.progress() > 0.5 {
                shot_mid = true;
                shots.take(s, &format!("{name}_1_riding"));
            }
        }
        let there = if controls {
            !s.game.world.module.eq_ignore_ascii_case(&from)
        } else {
            s.game.world.module.eq_ignore_ascii_case(&to)
        };
        if there && s.game.world.lift_ride().is_none() {
            arrived_at = Some(i);
            break;
        }
    }
    if !rode && !controls {
        fails.push("no ride: the lift did not start".to_string());
    }
    let Some(_) = arrived_at else {
        let pc = s.game.world.pc;
        let p = s.game.world.object(pc).unwrap().position;
        fails.push(format!(
            "never arrived: still in {} at {p:?}",
            s.game.world.module
        ));
        return (fails, s.game.world.module.clone());
    };
    for _ in 0..90 {
        step(s);
    }
    shots.take(s, &format!("{name}_2_arrived"));
    let again: Vec<String> = falling_again(s)
        .into_iter()
        .filter(|t| !dead_before.contains(t))
        .collect();
    if !again.is_empty() {
        fails.push(format!("bodies fall again: {again:?}"));
    }
    if !can_walk(s) {
        let pc = s.game.world.pc;
        let p = s.game.world.object(pc).unwrap().position;
        fails.push(format!("stuck where it landed, at {p:?}"));
    }
    let to = if controls {
        s.game.world.module.clone()
    } else {
        to
    };
    (fails, to)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut lifts: Vec<String> = Vec::new();
    let mut modes = vec!["classic".to_string(), "modern".to_string()];
    let mut inputs = vec!["e".to_string(), "click".to_string()];
    let mut out = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--modes" => {
                modes = args
                    .next()
                    .unwrap()
                    .split(',')
                    .map(str::to_string)
                    .collect()
            }
            "--inputs" => {
                inputs = args
                    .next()
                    .unwrap()
                    .split(',')
                    .map(str::to_string)
                    .collect()
            }
            "--out" => out = Some(std::path::PathBuf::from(args.next().unwrap())),
            _ => lifts.push(a),
        }
    }
    if lifts.is_empty() {
        lifts = LIFTS.iter().map(|s| s.to_string()).collect();
    }
    if let Some(d) = &out {
        std::fs::create_dir_all(d)?;
    }
    let mut shots = Shots {
        gpu: out
            .as_ref()
            .map(|_| GpuRenderer::new(SIZE.0, SIZE.1))
            .transpose()?,
        seen: 0,
        dir: out.as_deref(),
    };
    let mut failed = 0;
    for lift in &lifts {
        let (module, rest) = lift.split_once(':').ok_or("module:tag")?;
        let (tag, to) = match rest.split_once('>') {
            Some((t, to)) => (t, Some(to)),
            None => (rest, None),
        };
        for mode in &modes {
            for input in &inputs {
                let mut s = Stage::open(module, true)?;
                s.enable_tutorials(false);
                s.enable_auto_pause(false);
                s.set_view(SIZE);
                s.set_game_mode(if mode == "modern" {
                    GameMode::Modern
                } else {
                    GameMode::Classic
                });
                s.set_click_to_move(true);
                for _ in 0..300 {
                    step(&mut s);
                }
                let name = format!("{module}_{tag}_{mode}_{input}");
                let Some(door) = find_door(&s, tag, to) else {
                    println!("{name:<60} FAIL  no such door");
                    failed += 1;
                    continue;
                };
                let (mut fails, there) = ride(&mut s, door, input, &mut shots, &name);
                let mut one_way = false;
                // And back: the lift door of where it landed that leads home, nearest the player.
                if fails.is_empty() {
                    let pc = s.game.world.pc;
                    let here = s.game.world.object(pc).unwrap().position;
                    let back = s
                        .game
                        .world
                        .objects
                        .values()
                        .filter(|o| {
                            o.kind == Kind::Door && o.link_module.eq_ignore_ascii_case(module)
                                || o.kind == Kind::Placeable && o.tag.eq_ignore_ascii_case(tag)
                        })
                        .min_by(|a, b| {
                            let da = (a.position[0] - here[0]).hypot(a.position[1] - here[1]);
                            let db = (b.position[0] - here[0]).hypot(b.position[1] - here[1]);
                            da.total_cmp(&db)
                        })
                        .map(|o| o.id);
                    match back {
                        Some(b) => {
                            let (f, _) =
                                ride(&mut s, b, input, &mut shots, &format!("{name}_back"));
                            fails.extend(f.into_iter().map(|f| format!("back: {f}")));
                        }
                        // A lift the game made one way, as tar_m10ab's: the way back is by other means.
                        None => one_way = true,
                    }
                }
                if fails.is_empty() && one_way {
                    println!(
                        "{name:<60} PASS  to {there}, one way: it has no door back to {module}"
                    );
                } else if fails.is_empty() {
                    println!("{name:<60} PASS  to {there} and back");
                } else {
                    failed += 1;
                    println!("{name:<60} FAIL  {}", fails.join("; "));
                }
            }
        }
    }
    println!("lift drive: {failed} failed");
    if failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}
