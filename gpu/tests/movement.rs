//! Modern movement through the window's loop: the settings that choose it, the roll key, the camera, and the look button.
use kotor_game::MovementMode;
use kotor_gpu::stage::Stage;

const SIZE: (u32, u32) = (640, 360);
const DT: f32 = 1.0 / 60.0;

/// A game with no pauses or pop-ups, its settings in a file of its own with these lines.
fn stage(name: &str, settings: &str) -> Option<(Stage, std::path::PathBuf)> {
    stage_in("danm14aa", name, settings)
}

fn stage_in(module: &str, name: &str, settings: &str) -> Option<(Stage, std::path::PathBuf)> {
    kotor_core::install_dir()?;
    let mut s = Stage::open(module, true).unwrap();
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let file = std::env::temp_dir().join(format!("kotor_move_{name}_{}.txt", std::process::id()));
    std::fs::write(&file, settings).unwrap();
    s.use_settings_file(file.clone());
    // These are tests of Modern movement, unless the file says Classic. The game's own default is Classic, Modern movement
    // and combat go together for the player, and here the movement is set alone, with Classic's combat and pause as before.
    if !settings.contains("movement_modern=") {
        s.set_movement_modern(true);
    }
    s.set_view(SIZE);
    s.update(DT);
    // The opening conversation first.
    for _ in 0..600 {
        if !s.game.world.in_dialogue() {
            break;
        }
        s.continue_line();
        s.number_key(1);
        s.update(DT);
    }
    assert!(!s.game.world.in_dialogue());
    Some((s, file))
}

fn pc_at(s: &Stage) -> [f32; 3] {
    s.game.world.pc_mover.pos
}

fn run(s: &mut Stage, frames: usize) {
    for _ in 0..frames {
        s.update(DT);
    }
}

/// The way the floor is clear from the player, by trying each direction of the keys.
fn clear_way(s: &mut Stage) -> f32 {
    let at = pc_at(s);
    let nav = s.game.world.nav.as_ref().unwrap();
    let mut best = (0.0, 0.0f32);
    for k in 0..32 {
        let a = k as f32 * std::f32::consts::TAU / 32.0;
        let mut d: f32 = 0.0;
        while d < 12.0 {
            let (x, y) = (at[0] + a.cos() * (d + 0.25), at[1] + a.sin() * (d + 0.25));
            if nav.walkable_at(x, y, at[2] + 0.5).is_none() || nav.crosses_block(at, [x, y, at[2]])
            {
                break;
            }
            d += 0.25;
        }
        if d > best.1 {
            best = (a, d);
        }
    }
    best.0
}

#[test]
fn classic_is_the_default_and_modern_movement_has_the_roll() {
    // The game's own default is Classic: the tank walk and rounds. Single Player asks for each new game.
    let default = kotor_gpu::stage::Settings::default();
    assert!(!default.movement_modern && !default.combat_modern);
    let Some((mut s, file)) = stage("default", "") else {
        return;
    };
    assert!(s.settings().movement_modern);
    assert!(!s.settings().click_to_walk, "Click to move is off at first");
    assert_eq!(s.game.world.movement, MovementMode::Modern);
    std::fs::remove_file(&file).ok();

    let Some((mut c, file)) = stage("classic", "movement_modern=false\n") else {
        return;
    };
    assert!(!c.settings().movement_modern);
    assert_eq!(c.game.world.movement, MovementMode::Classic);
    // Classic has no roll.
    c.roll_key();
    assert!(!c.game.world.roll_request);
    std::fs::remove_file(&file).ok();
    s.roll_key();
    assert!(s.game.world.roll_request);
}

#[test]
fn the_roll_key_rolls_with_dust_a_sound_and_a_turned_body() {
    let Some((mut s, file)) = stage("roll", "") else {
        return;
    };
    s.yaw = clear_way(&mut s);
    let dust = s.effects_playing();
    s.steer = (1.0, 0.0, false);
    run(&mut s, 10);
    s.roll_key();
    s.update(DT);
    assert!(s.game.world.pc_body.rolling(), "the key rolled");
    assert!(s.effects_playing() > dust, "the dust is there");
    run(&mut s, 12);
    // The feet and the head come round without going through the floor.
    let mut sank = 0.0f32;
    for _ in 0..40 {
        if let Some(d) = s.bent_body_sinks() {
            sank = sank.max(d);
        }
        s.update(DT);
    }
    assert!(sank < 0.03, "the tucked body went {sank} m into the floor");
    // It is still drawn.
    assert!(!s.dynamic_scene().meshes.is_empty());
    run(&mut s, 60);
    assert!(!s.game.world.pc_body.rolling());
    std::fs::remove_file(&file).ok();
}

#[test]
fn the_camera_widens_and_lags_at_a_sprint_and_settles_at_rest() {
    let Some((mut s, file)) = stage("camera", "") else {
        return;
    };
    s.yaw = clear_way(&mut s);
    let rest = s.camera();
    s.steer = (1.0, 0.0, false);
    run(&mut s, 60);
    let moving = s.camera();
    assert!(
        moving.fov > rest.fov + 2.0,
        "{} then {}",
        rest.fov,
        moving.fov
    );
    let p = pc_at(&s);
    let behind = ((moving.target[0] - p[0]).hypot(moving.target[1] - p[1])).abs();
    assert!(behind > 0.05 && behind < 0.8, "focus {behind} m behind");
    s.steer = (0.0, 0.0, false);
    run(&mut s, 180);
    let settled = s.camera();
    assert!((settled.fov - rest.fov).abs() < 0.1);
    let p = pc_at(&s);
    assert!((settled.target[0] - p[0]).hypot(settled.target[1] - p[1]) < 0.02);
    std::fs::remove_file(&file).ok();
}

fn angle_between(a: f32, b: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    ((a - b + tau * 1.5).rem_euclid(tau) - tau / 2.0).abs()
}

/// The way the keys are asking the player to go now.
fn asked(s: &Stage) -> f32 {
    let (dir, _) = s.game.world.steer.expect("a key is held");
    dir[1].atan2(dir[0])
}

/// The owner's report: hold forward, look round, and the old direction was kept until the key came up.
#[test]
fn a_held_key_follows_the_camera_in_both_modes() {
    for (lines, modern) in [("", true), ("movement_modern=false\n", false)] {
        let Some((mut s, file)) = stage(if modern { "keys_m" } else { "keys_c" }, lines) else {
            return;
        };
        s.yaw = clear_way(&mut s);
        s.steer = (1.0, 0.0, false);
        run(&mut s, 45);
        assert!(
            angle_between(asked(&s), s.yaw) < 0.05,
            "{} {}",
            asked(&s),
            s.yaw
        );
        // The look button goes down, and the mouse drags a quarter turn right, with the key still held.
        s.set_looking(true);
        s.look(314.0, 0.0);
        let turned = s.yaw;
        run(&mut s, 1);
        let first = angle_between(asked(&s), turned);
        if modern {
            assert!(first > 0.3, "the path curves: {first} off after a frame");
        } else {
            assert!(first < 0.01, "Classic goes at once: {first} off");
        }
        // Within a third of a second it goes the new way, and it goes on doing so.
        let mut last = s.game.world.pc_mover.heading;
        for _ in 0..20 {
            run(&mut s, 1);
            let now = s.game.world.pc_mover.heading;
            if modern {
                // No faster than the body's turn: 18.8 radians a second.
                assert!(
                    angle_between(now, last) < 18.8 / 60.0 + 0.01,
                    "{last} to {now}"
                );
            }
            last = now;
        }
        assert!(angle_between(asked(&s), turned) < 0.06, "{}", asked(&s));
        assert!(
            angle_between(last, turned) < 0.2,
            "faces {last}, camera {turned}"
        );
        assert!((s.yaw - turned).abs() < 1e-4, "and the camera stays put");
        // Let go of everything, and press again: the same way, at once.
        s.set_looking(false);
        s.steer = (0.0, 0.0, false);
        run(&mut s, 30);
        s.steer = (1.0, 0.0, false);
        run(&mut s, 1);
        assert!(
            angle_between(asked(&s), s.yaw) < 0.01,
            "{} {}",
            asked(&s),
            s.yaw
        );
        // Backward and sideways go by the camera too.
        s.steer = (-1.0, 0.0, false);
        run(&mut s, 30);
        assert!(angle_between(asked(&s), s.yaw + std::f32::consts::PI) < 0.02);
        std::fs::remove_file(&file).ok();
    }
}

#[test]
fn looking_round_moves_nobody_by_itself() {
    for (lines, name) in [("", "still_m"), ("movement_modern=false\n", "still_c")] {
        let Some((mut s, file)) = stage(name, lines) else {
            return;
        };
        run(&mut s, 30);
        let a = pc_at(&s);
        s.set_looking(true);
        s.look(300.0, 40.0);
        run(&mut s, 30);
        s.set_looking(false);
        run(&mut s, 30);
        let b = pc_at(&s);
        assert!((b[0] - a[0]).hypot(b[1] - a[1]) < 0.01, "{a:?} {b:?}");
        std::fs::remove_file(&file).ok();
    }
}

#[test]
fn in_modern_a_strafe_does_not_turn_the_camera() {
    let Some((mut s, file)) = stage("strafe", "") else {
        return;
    };
    let yaw = s.yaw;
    s.steer = (0.0, 1.0, false);
    run(&mut s, 60);
    assert!((s.yaw - yaw).abs() < 1e-4, "{yaw} to {}", s.yaw);
    std::fs::remove_file(&file).ok();
}

/// Floor on screen, a metre or more from the player.
fn floor_pixel(s: &mut Stage) -> (f32, f32) {
    let before = pc_at(s);
    let cam = s.camera();
    for y in (SIZE.1 / 2..SIZE.1).rev().step_by(20) {
        for x in (40..SIZE.0 - 40).step_by(40) {
            let (o, d) =
                kotor_game::pixel_ray(cam.eye, cam.target, cam.fov, SIZE, (x as f32, y as f32));
            if s.game.world.pick(o, d).is_some() {
                continue;
            }
            if let Some(p) = s.floor_under_ray(o, d) {
                if (p[0] - before[0]).hypot(p[1] - before[1]) > 1.0 {
                    return (x as f32, y as f32);
                }
            }
        }
    }
    panic!("no floor on screen");
}

#[test]
fn click_to_walk_works_in_both_modes_and_can_be_turned_off() {
    for (lines, walks) in [
        ("click_to_walk=true\n", true),
        ("movement_modern=false\nclick_to_walk=true\n", true),
        ("click_to_walk=false\n", false),
        ("", false),
    ] {
        let name = format!("click_{}", lines.len());
        let Some((mut s, file)) = stage(&name, lines) else {
            return;
        };
        let px = floor_pixel(&mut s);
        let from = pc_at(&s);
        s.click(px, SIZE);
        assert_eq!(s.game.world.pc_walking(), walks, "{lines:?}");
        run(&mut s, 40);
        let to = pc_at(&s);
        assert_eq!(
            (to[0] - from[0]).hypot(to[1] - from[1]) > 0.5,
            walks,
            "{from:?} {to:?}"
        );
        std::fs::remove_file(&file).ok();
    }
}

#[test]
fn click_to_move_goes_round_the_way_and_a_movement_key_or_a_tap_acts_as_the_mouse_does() {
    let Some((mut s, file)) = stage("click_move", "click_to_walk=true\n") else {
        return;
    };
    // A click walks a route on the walkmesh, and W stops it where it is.
    let px = floor_pixel(&mut s);
    s.click(px, SIZE);
    assert!(s.game.world.pc_walking());
    run(&mut s, 5);
    s.steer = (1.0, 0.0, false);
    run(&mut s, 1);
    s.steer = (0.0, 0.0, false);
    assert!(!s.game.world.pc_walking(), "the key stopped the walk");
    // A tap is a click.
    s.finger_down(3, px, SIZE);
    s.finger_up(3, px, SIZE);
    assert!(s.game.world.pc_walking(), "a tap walks");
    run(&mut s, 2);
    // Off: neither walks.
    s.set_click_to_move(false);
    s.steer = (1.0, 0.0, false);
    run(&mut s, 1);
    s.steer = (0.0, 0.0, false);
    run(&mut s, 2);
    let px = floor_pixel(&mut s);
    s.click(px, SIZE);
    s.finger_down(4, px, SIZE);
    s.finger_up(4, px, SIZE);
    assert!(!s.game.world.pc_walking());
    std::fs::remove_file(&file).ok();
}

#[test]
fn the_settings_file_keeps_both_rows() {
    let Some((mut s, file)) = stage("keep", "movement_modern=false\nclick_to_walk=false\n") else {
        return;
    };
    assert!(!s.settings().movement_modern && !s.settings().click_to_walk);
    // Any change writes the file: the two rows are still off when it is read again.
    s.look(1.0, 0.0);
    s.save_settings_now();
    let again = std::fs::read_to_string(&file).unwrap();
    assert!(again.contains("movement_modern=false"), "{again}");
    assert!(again.contains("click_to_walk=false"), "{again}");
    std::fs::remove_file(&file).ok();
}

/// The left and the right joint of a pair are mirror images, to within the skeleton's own difference: the
/// female skeleton stands with its knees 3 cm apart front to back, and a bent knee carries that to the foot.
fn even(joints: &std::collections::HashMap<String, [f32; 3]>, left: &str, right: &str) {
    let (l, r) = (joints[left], joints[right]);
    let off = [
        (l[0] + r[0]).abs(),
        (l[1] - r[1]).abs(),
        (l[2] - r[2]).abs(),
    ];
    assert!(
        off.iter().all(|d| *d < 0.06),
        "{left} {l:?} and {right} {r:?}"
    );
}

#[test]
fn a_roll_bends_both_sides_alike_with_the_knees_up() {
    let Some((mut s, file)) = stage("even", "") else {
        return;
    };
    s.preview_roll(0.5);
    let j = s.bent_joints().unwrap();
    for (l, r) in [
        ("lshin_g", "rshin_g"),
        ("lfoot_g", "rfoot_g"),
        ("lforearm_g", "rforearm_g"),
        ("lhand_g", "rhand_g"),
    ] {
        even(&j, l, r);
    }
    // Knees over the hips, and in front.
    assert!(j["lshin_g"][2] > j["lthigh_g"][2], "{:?}", j["lshin_g"]);
    assert!(
        j["lshin_g"][1] > j["lthigh_g"][1] + 0.3,
        "{:?}",
        j["lshin_g"]
    );
    // Heels back under the knees.
    assert!(
        j["lfoot_g"][2] < j["lshin_g"][2] - 0.3,
        "{:?}",
        j["lfoot_g"]
    );
    // Neither side of the body is crossed. Where the hands go is `the_tuck_has_its_elbows_on_the_thighs_and_its_hands_by_the_head`.
    assert!(j["lhand_g"][0] < 0.0 && j["rhand_g"][0] > 0.0);
    std::fs::remove_file(file).ok();
}

#[test]
fn a_landing_bends_both_sides_alike_and_keeps_the_feet_down() {
    let Some((mut s, file)) = stage("land_even", "") else {
        return;
    };
    s.preview_landing(1.0);
    let standing = s.bent_joints().unwrap();
    for p in [0.0, 0.5] {
        s.preview_landing(p);
        let j = s.bent_joints().unwrap();
        // Only at the deepest is the idle clip, which has one foot forward, gone.
        if p == 0.0 {
            even(&j, "lshin_g", "rshin_g");
            even(&j, "lfoot_g", "rfoot_g");
            even(&j, "lhand_g", "rhand_g");
        }
        for foot in ["lfoot_g", "rfoot_g"] {
            let lifted = (j[foot][2] - standing[foot][2]).abs();
            assert!(lifted < 0.05, "{foot} moved {lifted} m up or down at {p}");
        }
        assert!(j["pelvis_g"][2] < standing["pelvis_g"][2] - 0.05);
    }
    std::fs::remove_file(file).ok();
}

#[test]
fn f8_reads_the_pose_files_again_and_a_mistake_gives_the_default() {
    let Some((mut s, file)) = stage("reload", "") else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("kotor_pose_reload_{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    s.use_pose_folder(dir.clone());
    assert!(s.pose_problems().is_empty(), "{:?}", s.pose_problems());
    let roll = dir.join("roll_pose.txt");
    let default_text = std::fs::read_to_string(&roll).unwrap();
    assert!(dir.join("landing_pose.txt").exists());
    s.preview_roll(0.5);
    let knee = |s: &mut Stage| s.bent_joints().unwrap()["lshin_g"];
    let usual = knee(&mut s);

    // Legs straight: the knee comes down.
    std::fs::write(&roll, "turns = 1\n[key]\nat = 0\ntuck = 1\n").unwrap();
    s.reload_poses();
    assert!(s.pose_problems().is_empty(), "{:?}", s.pose_problems());
    let straight = knee(&mut s);
    assert!(straight[2] < usual[2] - 0.3, "{straight:?} {usual:?}");

    // A mistake: the default, and it says where.
    std::fs::write(&roll, "turns = 1\n[key]\nat = 0\nlthigh_g = 999\n").unwrap();
    s.reload_poses();
    assert!(
        s.pose_problems()[0].contains("line 4"),
        "{:?}",
        s.pose_problems()
    );
    assert_eq!(knee(&mut s), usual);
    let said = std::fs::read_to_string(dir.join("pose_problems.txt")).unwrap();
    assert!(said.contains("line 4"), "{said}");
    // The owner's file is left as it was.
    assert!(std::fs::read_to_string(&roll).unwrap().contains("999"));

    // Fixed again.
    std::fs::write(&roll, &default_text).unwrap();
    s.reload_poses();
    assert!(s.pose_problems().is_empty());
    assert_eq!(knee(&mut s), usual);
    std::fs::remove_dir_all(&dir).ok();
    std::fs::remove_file(file).ok();
}

/// The switch takes hold in the game, without a restart, and a body that was in Classic carries on from where it is.
#[test]
fn the_movement_setting_applies_at_once() {
    let Some((mut s, file)) = stage("live", "") else {
        return;
    };
    run(&mut s, 2);
    assert_eq!(s.game.world.movement, MovementMode::Modern);
    s.set_movement_modern(false);
    run(&mut s, 2);
    assert_eq!(s.game.world.movement, MovementMode::Classic);
    let at = pc_at(&s);
    s.set_movement_modern(true);
    run(&mut s, 2);
    assert_eq!(s.game.world.movement, MovementMode::Modern);
    let now = pc_at(&s);
    assert!(
        (now[0] - at[0]).hypot(now[1] - at[1]) < 0.01,
        "{at:?} {now:?}"
    );
    std::fs::remove_file(&file).ok();
}

/// The owner on the last one: "Roll animation looks like a frog now". Knees together, hands in front of the shins, and on the
/// floor all the way round, with the head going over first.
#[test]
fn a_roll_has_the_knees_together_and_stays_on_the_floor() {
    let Some((mut s, file)) = stage("frog", "") else {
        return;
    };
    s.preview_roll(0.5);
    let j = s.bent_joints().unwrap();
    let knees = j["rshin_g"][0] - j["lshin_g"][0];
    assert!(knees < 0.2, "the knees are {knees} m apart");
    // In front of the hips and up at the chest.
    assert!(
        j["lshin_g"][1] > j["lthigh_g"][1] + 0.25,
        "{:?}",
        j["lshin_g"]
    );
    assert!(
        j["lshin_g"][2] > j["lthigh_g"][2] + 0.1,
        "{:?}",
        j["lshin_g"]
    );
    // Hands nearer the middle than the shoulders, and in front of the shins' line back to the hips.
    assert!(j["rhand_g"][0] - j["lhand_g"][0] < 0.35);
    assert!(
        j["lhand_g"][1] > j["lthigh_g"][1] + 0.25,
        "{:?}",
        j["lhand_g"]
    );
    // Chin down: the head ahead of the chest.
    assert!(j["headhook"][1] > j["torsoupr_g"][1], "{:?}", j["headhook"]);
    // Touching the floor all the way round, neither floating nor sinking.
    for k in 6..=14 {
        s.preview_roll(k as f32 / 20.0);
        let sinks = s.bent_body_sinks().unwrap();
        assert!(sinks.abs() < 0.03, "{sinks} m into the floor at {k}/20");
    }
    std::fs::remove_file(&file).ok();
}

fn camera_away(s: &Stage) -> f32 {
    let c = s.camera();
    let d = [0, 1, 2].map(|i| c.eye[i] - c.target[i]);
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// The owner's report: the camera pulled in very close after a fall. Off the ledge on Taris the driver uses.
#[test]
fn the_camera_stays_out_through_a_fall_and_its_landing() {
    let Some((mut s, file)) = stage_in("tar_m10aa", "fall_cam", "") else {
        return;
    };
    s.place_player([106.01, 129.28, 2.25], 1.963);
    s.yaw = 1.963;
    run(&mut s, 180);
    let start = camera_away(&s);
    assert!(start > 2.5, "{start}");
    s.steer = (1.0, 0.0, false);
    let (mut fell, mut nearest, mut last, mut most) = (false, start, start, 0.0f32);
    for _ in 0..240 {
        s.update(DT);
        fell |= !s.game.world.pc_body.grounded;
        if s.game.world.pc_body.grounded && fell {
            s.steer = (0.0, 0.0, false);
        }
        let now = camera_away(&s);
        nearest = nearest.min(now);
        most = most.max((now - last).abs());
        last = now;
    }
    assert!(fell, "it fell");
    assert!(s.game.world.pc_body.grounded, "and landed");
    // At the bottom the cliff stands about 2 m behind the player: the last time, this came in to 0.8 m.
    assert!(nearest > start * 0.6, "{start} m came in to {nearest} m");
    println!(
        "fall camera: start {start:.2} nearest {nearest:.2} end {last:.2} biggest step {most:.3}"
    );
    assert!(most < 0.6, "the camera jumped {most} m in a frame");
    assert!(last > start * 0.6, "{start} m ended at {last} m");
    std::fs::remove_file(&file).ok();
}

/// A frame takes as long as it takes at 2160p: what is on screen still goes along evenly.
#[test]
fn an_uneven_frame_rate_does_not_shake_a_sprint() {
    let Some((mut s, file)) = stage("uneven", "") else {
        return;
    };
    s.yaw = clear_way(&mut s);
    s.steer = (1.0, 0.0, false);
    run(&mut s, 60);
    // Between 24 and 60 frames a second, and never the same twice running.
    let times = [1.0 / 30.0, 1.0 / 41.0, 1.0 / 24.0, 1.0 / 60.0, 1.0 / 37.0];
    let (mut last, mut speeds) = (s.camera().target, Vec::new());
    for k in 0..40 {
        let dt = times[k % times.len()];
        s.update(dt);
        let now = s.camera().target;
        speeds.push((now[0] - last[0]).hypot(now[1] - last[1]) / dt);
        last = now;
    }
    if s.game.world.pc_body.speed() > 6.0 {
        let mean = speeds.iter().sum::<f32>() / speeds.len() as f32;
        let worst = speeds.iter().map(|v| (v - mean).abs()).fold(0.0, f32::max);
        assert!(
            worst < mean * 0.25,
            "{mean} m/s, off by {worst}: {speeds:?}"
        );
    }
    std::fs::remove_file(&file).ok();
}

/// A click during a roll: the roll comes round sooner, and then the player goes there.
#[test]
fn a_click_cuts_a_roll_short_and_the_walk_follows() {
    let Some((mut s, file)) = stage("cancel", "") else {
        return;
    };
    s.yaw = clear_way(&mut s);
    let frames = |s: &mut Stage| {
        let mut n = 0;
        while s.game.world.pc_body.rolling() && n < 200 {
            s.update(DT);
            n += 1;
        }
        n
    };
    s.roll_key();
    s.update(DT);
    assert!(s.game.world.pc_body.rolling());
    let whole = frames(&mut s);
    run(&mut s, 60);
    s.roll_key();
    s.update(DT);
    let px = floor_pixel(&mut s);
    s.set_click_to_move(true);
    s.click(px, SIZE);
    assert!(s.game.world.pc_walking(), "the click was taken");
    let cut = frames(&mut s);
    assert!(cut * 2 < whole + 2, "{cut} frames, against {whole}");
    let from = pc_at(&s);
    run(&mut s, 30);
    let to = pc_at(&s);
    assert!(
        (to[0] - from[0]).hypot(to[1] - from[1]) > 0.3,
        "{from:?} {to:?}"
    );
    std::fs::remove_file(&file).ok();
}

/// The walk clip keeps to the floor: it runs at its own speed at a walk, and faster on the way up to a run.
#[test]
fn the_walk_clip_goes_with_the_speed() {
    let Some((mut s, file)) = stage("walkclip", "") else {
        return;
    };
    s.yaw = clear_way(&mut s);
    s.steer = (1.0, 0.0, true);
    run(&mut s, 60);
    let pc = s.game.world.pc;
    let clip = |s: &mut Stage| {
        s.game
            .poses()
            .into_iter()
            .find(|p| p.id == pc)
            .and_then(|p| p.clip)
            .unwrap()
    };
    let a = clip(&mut s);
    assert_eq!(a.name, "walk");
    run(&mut s, 30);
    let b = clip(&mut s);
    assert_eq!(b.name, "walk");
    let rate = (b.time - a.time) / (30.0 * DT);
    assert!(
        (rate - 1.0).abs() < 0.1,
        "at a walk the clip goes at {rate}"
    );
    std::fs::remove_file(&file).ok();
}

/// The door's plane runs on past the door. On Upper City South's plaza, 97 m along the plane of the door to the north, an eye
/// past it lost the floor and the player and showed the area behind that door. It flipped as the camera swung.
#[test]
fn a_doorway_far_along_its_plane_takes_nothing_from_the_picture() {
    let Some((mut s, file)) = stage_in("tar_m02ac", "doorplane", "") else {
        return;
    };
    let Ok(mut gpu) = kotor_gpu::GpuRenderer::new(SIZE.0, SIZE.1) else {
        return;
    };
    let tag = "UpperNorth";
    let site = s
        .game
        .world
        .sketch()
        .sites
        .iter()
        .find(|x| x.tag == tag)
        .unwrap()
        .clone();
    let way = site.out.unwrap();
    let before = [
        site.at[0] - way.cos() * 7.0,
        site.at[1] - way.sin() * 7.0,
        site.at[2],
    ];
    s.place_player(before, way);
    let began = std::time::Instant::now();
    while s.portal().is_none() && began.elapsed().as_secs() < 60 {
        s.update(DT);
        // Nothing joined shows until a window has taken its textures.
        s.take_warm(64);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let (other, door) = s
        .portal()
        .map(|(sc, a)| (sc.clone(), a))
        .expect("the door's view");
    let scene = s.static_scene().clone();
    gpu.set_static_scene(&scene, &mut |t| s.texture(t));
    let frame = |s: &mut Stage, gpu: &mut kotor_gpu::GpuRenderer, with: bool| {
        gpu.set_portal(with.then_some((&other, door)), &mut |t| s.texture(t));
        let body = s.dynamic_scene();
        let cam = s.camera();
        (gpu.render(&body, &mut |n| s.texture(n), &cam).unwrap(), cam)
    };
    // Where the owner saw it: the corridor to the Apartments, with the camera at every turn and on either side of the plane.
    let spot = [90.0, 118.5, 0.0];
    assert!((spot[1] - door.at[1]) * way.sin() > 0.0, "past the plane");
    let mut past = 0;
    for turn in 0..8 {
        s.place_player(spot, way);
        s.pitch = 1.1;
        s.yaw = way + turn as f32 * std::f32::consts::FRAC_PI_4;
        run(&mut s, 10);
        let (with, cam) = frame(&mut s, &mut gpu, true);
        let (without, _) = frame(&mut s, &mut gpu, false);
        past += ((cam.eye[1] - door.at[1]) * way.sin() > 0.0) as i32;
        assert!(
            with.rgba == without.rgba,
            "turn {turn}, eye at {:?}",
            cam.eye
        );
    }
    assert!(
        past > 0 && past < 8,
        "{past} of 8 turns had the eye past the plane"
    );
    // And it is still there from in front of the door: the open door shows the other area.
    let door_id = s.game.world.link_door_at(site.at).unwrap();
    if let Some(o) = s.game.world.objects.get_mut(&door_id) {
        o.locked = false;
    }
    s.game.world.use_door(door_id);
    s.place_player(before, way);
    s.pitch = 0.3;
    s.yaw = way;
    run(&mut s, 120);
    let (with, _) = frame(&mut s, &mut gpu, true);
    let (without, _) = frame(&mut s, &mut gpu, false);
    assert!(with.rgba != without.rgba, "the doorway shows nothing");
    std::fs::remove_file(&file).ok();
}

/// The way the owner saw it on the Endar Spire: a doorway of Taris, still in the picture after a load, took the room and the
/// player away wherever the eye was past its plane, and showed the Dark Jedi's talk from outside the ship.
#[test]
fn a_doorway_goes_with_the_module_it_was_in() {
    let Some((mut s, file)) = stage_in("tar_m02ac", "doorgoes", "") else {
        return;
    };
    let site = s
        .game
        .world
        .sketch()
        .sites
        .iter()
        .find(|x| x.tag == "UpperNorth")
        .unwrap()
        .clone();
    let way = site.out.unwrap();
    let before = [
        site.at[0] - way.cos() * 7.0,
        site.at[1] - way.sin() * 7.0,
        site.at[2],
    ];
    s.place_player(before, way);
    let began = std::time::Instant::now();
    while s.portal().is_none() && began.elapsed().as_secs() < 60 {
        s.update(DT);
        // Nothing joined shows until a window has taken its textures.
        s.take_warm(64);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(s.portal().is_some(), "the door's view");
    let version = s.portal_version();
    s.game.world.transition_request = Some(("end_m01aa".to_string(), String::new()));
    run(&mut s, 5);
    assert_eq!(s.game.world.module, "end_m01aa");
    assert!(s.portal().is_none(), "a door of Taris on the Endar Spire");
    assert!(s.portal_version() != version, "the window is told");
    std::fs::remove_file(&file).ok();
}

/// The owner's words: "Make spacebar perform a jump, not pause the game, make pausing the game the X key."
/// The owner's words: "Disable the ability to jump while walking and not running".
#[test]
fn space_does_not_jump_at_a_walk_and_does_at_a_run_or_from_a_stand() {
    let Some((mut s, file)) = stage("nowalkjump", "") else {
        return;
    };
    s.yaw = clear_way(&mut s);
    let leaves_the_floor = |s: &mut Stage| {
        // Past the 0.1 s wind-up.
        s.jump_key();
        let mut left = false;
        for _ in 0..10 {
            s.update(DT);
            left |= !s.game.world.pc_body.grounded;
        }
        run(s, 70);
        left
    };
    // Walking: nothing, and the press is not kept for later.
    s.steer = (1.0, 0.0, true);
    run(&mut s, 30);
    assert!(!leaves_the_floor(&mut s), "at a walk");
    s.steer = (1.0, 0.0, false);
    s.update(DT);
    assert!(s.game.world.pc_body.grounded, "not kept for the run");
    // Running.
    run(&mut s, 30);
    assert!(leaves_the_floor(&mut s), "at a run");
    // Standing still, and with Shift held standing still.
    s.steer = (0.0, 0.0, false);
    run(&mut s, 45);
    assert!(leaves_the_floor(&mut s), "from a stand");
    s.steer = (0.0, 0.0, true);
    run(&mut s, 5);
    assert!(leaves_the_floor(&mut s), "from a stand with the walk held");
    std::fs::remove_file(&file).ok();
}

#[test]
fn space_jumps_and_b_pauses() {
    let Some((mut s, file)) = stage("spacejump", "") else {
        return;
    };
    assert_eq!(s.action_for_key("Space"), Some("Jump"));
    assert_eq!(s.action_for_key("KeyB"), Some("Pause"));
    run(&mut s, 60);
    let floor = pc_at(&s)[2];
    s.jump_key();
    assert!(!s.paused(), "Space does not pause");
    let mut top = floor;
    let mut air = 0;
    for _ in 0..120 {
        s.update(DT);
        top = top.max(pc_at(&s)[2]);
        air += !s.game.world.pc_body.grounded as i32;
    }
    assert!((top - floor - 1.1).abs() < 0.1, "{top} from {floor}");
    assert!(
        (air as f32 * DT - 0.7).abs() < 0.1,
        "{air} steps in the air"
    );
    assert_eq!(pc_at(&s)[2], floor, "and back down");
    // In the air it does not jump again.
    s.jump_key();
    run(&mut s, 8);
    assert!(!s.game.world.pc_body.grounded, "up after the wind-up");
    let rising = s.game.world.pc_body.vz;
    s.jump_key();
    s.update(DT);
    assert!(s.game.world.pc_body.vz < rising, "{rising}");
    run(&mut s, 120);
    // X pauses and goes on, and Space does nothing in a pause.
    s.pause_key();
    assert!(s.paused());
    s.jump_key();
    assert!(s.paused(), "Space does not end a pause");
    s.update(DT);
    assert!(s.game.world.pc_body.grounded, "nor jump in one");
    s.pause_key();
    assert!(!s.paused());
    run(&mut s, 10);
    assert!(
        s.game.world.pc_body.grounded,
        "and no jump was kept for after it"
    );
    std::fs::remove_file(&file).ok();
}

/// Not in a conversation, in Classic movement, or while down.
#[test]
fn a_jump_is_only_for_the_modern_body_on_its_feet() {
    let Some((mut s, file)) = stage("nojump", "") else {
        return;
    };
    run(&mut s, 60);
    let pc = s.game.world.pc;
    let z = pc_at(&s)[2];
    s.set_movement_modern(false);
    s.jump_key();
    run(&mut s, 10);
    assert_eq!(pc_at(&s)[2], z, "in Classic");
    s.set_movement_modern(true);
    run(&mut s, 2);
    s.jump_key();
    run(&mut s, 10);
    assert!(pc_at(&s)[2] > z, "it does jump");
    run(&mut s, 120);
    assert_eq!(pc_at(&s)[2], z);
    s.game.world.objects.get_mut(&pc).unwrap().down = true;
    s.jump_key();
    run(&mut s, 10);
    assert_eq!(pc_at(&s)[2], z, "while down");
    assert!(s.game.world.pc_body.grounded, "while down");
    std::fs::remove_file(&file).ok();
}

fn apart(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// How far `p` is from the line from `a` to `b`, and the nearest place on it.
fn from_bone(p: [f32; 3], a: [f32; 3], b: [f32; 3]) -> (f32, [f32; 3]) {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let dot = (p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1] + (p[2] - a[2]) * ab[2];
    let t = (dot / (ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2])).clamp(0.0, 1.0);
    let q = [a[0] + ab[0] * t, a[1] + ab[1] * t, a[2] + ab[2] * t];
    (apart(p, q), q)
}

/// The owner's words: "elbows are tucked too close and intersect with the knees, the elbows need to be touching the outer
/// thighs with the arms positioned up and tucked around the head, the neck is tilted down as well."
#[test]
fn the_tuck_has_its_elbows_on_the_thighs_and_its_hands_by_the_head() {
    let Some((mut s, file)) = stage("tuckarms", "") else {
        return;
    };
    let joints: std::collections::HashMap<String, [f32; 3]> =
        s.roll_joints(0.5).into_iter().collect();
    let at = |n: &str| joints[n];
    let head = at("mesh head");
    let (l, r) = (at("mesh eyeLA"), at("mesh eyeRA"));
    let eyes = [0, 1, 2].map(|k| (l[k] + r[k]) / 2.0 - head[k]);
    let down = (-eyes[2]).atan2(eyes[1]).to_degrees();
    println!("the face looks {down:.1} degrees below straight ahead");
    assert!((15.0..=20.0).contains(&down), "{down}");
    for (side, way) in [("l", 1.0), ("r", -1.0)] {
        let j = |n: &str| at(&format!("{side}{n}"));
        let (elbow, wrist, knee) = (j("forearm_g"), j("hand_g"), j("shin_g"));
        let (on_thigh, near) = from_bone(elbow, j("thigh_g"), knee);
        let out = (elbow[0] - near[0]) * way * (at("lthigh_g")[0] - at("rthigh_g")[0]).signum();
        let (to_knee, to_head) = (apart(elbow, knee), apart(wrist, head));
        let beside = (wrist[0] - head[0]) * way * (at("lthigh_g")[0] - at("rthigh_g")[0]).signum();
        println!(
            "{side}: elbow {on_thigh:.3} from the thigh, {to_knee:.3} from the knee, {out:.3} outside it; wrist {to_head:.3} from the head, {beside:.3} beside it"
        );
        // A thigh is about 8 cm thick and an arm 4 cm: touching, and not inside.
        assert!(on_thigh < 0.15, "{side} {on_thigh}");
        assert!(on_thigh > 0.09, "{side} {on_thigh}");
        assert!(to_knee > 0.1, "{side} {to_knee}");
        assert!(out > 0.05, "{side} {out}");
        assert!(to_head < 0.2, "{side} {to_head}");
        assert!(beside > 0.06, "{side} {beside}");
    }
    std::fs::remove_file(&file).ok();
}

/// The pose files name the neck, and the head has to go with it.
#[test]
fn bending_the_neck_in_the_file_moves_the_head() {
    let Some((mut s, file)) = stage("neckbends", "") else {
        return;
    };
    let eyes = |s: &mut Stage| {
        let j: std::collections::HashMap<String, [f32; 3]> =
            s.roll_joints(0.5).into_iter().collect();
        let (e, h) = (j["mesh eyeLA"], j["mesh head"]);
        (h[2] - e[2]).atan2(e[1] - h[1]).to_degrees()
    };
    let as_it_is = eyes(&mut s);
    let dir = std::env::temp_dir().join(format!("kotor_neck_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("roll_pose.txt"),
        "turns = 0\n[key]\nat = 0\ntuck = 1\nnecklwr_g = 15\nneck_g = 15\n",
    )
    .unwrap();
    s.use_pose_folder(dir.clone());
    let down = eyes(&mut s);
    std::fs::write(
        dir.join("roll_pose.txt"),
        "turns = 0\n[key]\nat = 0\ntuck = 1\n",
    )
    .unwrap();
    s.reload_poses();
    let level = eyes(&mut s);
    assert!(
        (down - level - 30.0).abs() < 3.0,
        "{down} and {level}, from {as_it_is}"
    );
    std::fs::remove_dir_all(&dir).ok();
    std::fs::remove_file(&file).ok();
}

/// The owner's complaint: the player went through a closed Taris door. Slid along its line it came to lie on it, and was past.
#[test]
fn a_closed_taris_door_stops_the_player_however_it_comes() {
    let Some((mut s, file)) = stage_in("tar_m02aa", "doorshut", "") else {
        return;
    };
    for tag in ["tar02_aprtdoor", "ptar_stnddr"] {
        let (id, at, axis, half) = {
            let w = &s.game.world;
            let d = w.by_tag(tag)[0];
            let o = w.object(d).unwrap();
            (d, o.position, o.door_axis, o.door_half_width)
        };
        let out = [axis[1], -axis[0]];
        for facing in [1.0f32, -1.0] {
            let way = [out[0] * facing, out[1] * facing];
            for k in [-0.85f32, 0.0, 0.85] {
                for how in ["sprint", "roll", "jump", "classic"] {
                    let start = [
                        at[0] - way[0] * 2.5 + axis[0] * half * k,
                        at[1] - way[1] * 2.5 + axis[1] * half * k,
                        at[2],
                    ];
                    let yaw = way[1].atan2(way[0]);
                    s.set_movement_modern(how != "classic");
                    s.place_player(start, yaw);
                    s.steer = (0.0, 0.0, false);
                    s.update(DT);
                    s.yaw = yaw;
                    s.steer = (1.0, 0.0, false);
                    for f in 0..90 {
                        if how == "jump" && f % 45 == 5 {
                            s.jump_key();
                        }
                        if how == "roll" && f % 45 == 5 {
                            s.roll_key();
                        }
                        s.update(DT);
                        let p = pc_at(&s);
                        let side = (p[0] - at[0]) * way[0] + (p[1] - at[1]) * way[1];
                        let along = ((p[0] - at[0]) * axis[0] + (p[1] - at[1]) * axis[1]).abs();
                        let shut = s.game.world.object(id).is_some_and(|o| !o.open);
                        // Off the slab, which is 0.11 m each side of these doors' lines and 0.30 of the big ones'.
                        assert!(
                            !(shut && side > -0.4 && along < half),
                            "{tag} at {k}, facing {facing}, by {how}: {p:?} at frame {f}"
                        );
                    }
                }
            }
        }
    }
    std::fs::remove_file(&file).ok();
}

/// The camera stops at a closed door off its middle too, and not only within a metre of it.
#[test]
fn a_closed_door_stops_the_camera_across_its_whole_width() {
    let Some((mut s, file)) = stage_in("tar_m02ac", "doorcamera", "") else {
        return;
    };
    let site = s
        .game
        .world
        .sketch()
        .sites
        .iter()
        .find(|x| x.tag == "UpperNorth")
        .unwrap()
        .clone();
    let way = site.out.unwrap();
    let (c, n) = (way.cos(), way.sin());
    let half = s
        .game
        .world
        .object(s.game.world.by_tag("UpperNorth")[0])
        .unwrap()
        .door_half_width;
    for k in [-0.8f32, -0.4, 0.0, 0.4, 0.8] {
        // Backed up to the door, with the camera behind it.
        let at = [
            site.at[0] - c * 1.0 - n * half * k,
            site.at[1] - n * 1.0 + c * half * k,
            site.at[2],
        ];
        s.place_player(at, way + std::f32::consts::PI);
        s.pitch = 0.3;
        s.yaw = way + std::f32::consts::PI;
        run(&mut s, 30);
        let eye = s.camera().eye;
        let past = (eye[0] - site.at[0]) * c + (eye[1] - site.at[1]) * n;
        assert!(
            past < -0.35,
            "{k} of the way along: the eye is {past} past the door"
        );
    }
    std::fs::remove_file(&file).ok();
}

/// In the bunk room at the top of a jump the camera was inside the player's body: dark, with shards of skin.
#[test]
fn the_body_fades_out_as_the_camera_comes_into_it() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let mut s = Stage::open("end_m01aa", true).unwrap();
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let file = std::env::temp_dir().join(format!("kotor_move_bunk_{}.txt", std::process::id()));
    std::fs::write(&file, "").unwrap();
    s.use_settings_file(file.clone());
    s.set_view(SIZE);
    for _ in 0..4000 {
        s.update(DT);
        if s.game.world.in_dialogue() {
            s.continue_line();
            s.number_key(1);
        } else if s.game.steps > 60 {
            break;
        }
    }
    assert!(!s.game.world.in_dialogue());
    s.place_player([21.49, 21.356, -1.275], 0.0);
    run(&mut s, 40);
    s.jump_key();
    let mut most = 0.0f32;
    let mut away = f32::MAX;
    for _ in 0..50 {
        s.update(DT);
        let c = s.camera();
        let p = pc_at(&s);
        let up = (c.eye[2] - p[2]).clamp(0.0, 1.8);
        let off = (c.eye[0] - p[0])
            .hypot(c.eye[1] - p[1])
            .hypot(p[2] + up - c.eye[2]);
        let fade = s.player_fade();
        away = away.min(off);
        most = most.max(fade);
        if off <= 0.25 {
            assert_eq!(fade, 1.0, "{off} m off");
        } else if off >= 0.6 {
            assert_eq!(fade, 0.0, "{off} m off");
        } else {
            assert!(fade > 0.0 && fade < 1.0, "{off} m off: {fade}");
        }
        let scene = s.dynamic_scene();
        if fade > 0.0 && fade < 1.0 {
            assert!(
                scene.meshes.iter().any(|m| m.fade == fade),
                "the body takes it"
            );
        }
    }
    println!("the camera came to {away:.2} m, and the body faded {most:.2}");
    std::fs::remove_file(&file).ok();
}

/// Something narrow close behind the player: the camera steps to the side, and stands further out than the player's body.
#[test]
fn the_camera_steps_aside_where_that_gives_it_room() {
    let Some((mut s, file)) = stage_in("kas_m23aa", "aside", "") else {
        return;
    };
    s.place_player([66.33, 95.6, 44.04], 0.3);
    s.yaw = 0.3;
    run(&mut s, 90);
    let side = s.camera_side();
    assert!(side.abs() > 0.3, "{side}");
    let away = camera_away(&s);
    assert!(away > 1.0, "{away}");
    assert_eq!(s.player_fade(), 0.0);
    // Turned so that nothing stands behind it, it comes back to the middle.
    s.yaw = 0.3 + std::f32::consts::PI;
    let mut least = side.abs();
    for _ in 0..240 {
        s.update(DT);
        least = least.min(s.camera_side().abs());
    }
    println!("aside {side}, {away} m out; turned round, {least}");
    std::fs::remove_file(&file).ok();
}

/// The owner's words: "put one leg in front of the other and same with the respective upper limb". The paddling after the
/// top went: "Get rid of the paddling arms and legs part of the animation in the jump, it looks dumb".
#[test]
fn a_jump_strides_with_the_opposite_arm_and_closes_the_stride_with_no_paddling() {
    let Some((mut s, file)) = stage("stride", "") else {
        return;
    };
    let mut at = |p: f32| {
        s.preview_jump(p);
        s.bent_joints().unwrap()
    };
    // How far the left is ahead of the right: the knees, and the hands.
    let ahead =
        |j: &std::collections::HashMap<String, [f32; 3]>, l: &str, r: &str| j[l][1] - j[r][1];
    let stride = at(0.36);
    let (knees, hands) = (
        ahead(&stride, "lshin_g", "rshin_g"),
        ahead(&stride, "lhand_g", "rhand_g"),
    );
    assert!(knees > 0.25, "one leg in front of the other: {knees}");
    assert!(hands < -0.15, "the opposite arm forward: {hands}");
    // Over the top and down, nothing changes over: the same knee stays in front, and the same hand behind, each closing in.
    let moments: Vec<(f32, f32)> = (0..=9)
        .map(|n| {
            let j = at(0.5 + n as f32 * 0.05);
            (
                ahead(&j, "lshin_g", "rshin_g"),
                ahead(&j, "lhand_g", "rhand_g"),
            )
        })
        .collect();
    for (legs, arms) in &moments {
        assert!(*legs > -0.02, "the legs do not change over: {moments:?}");
        assert!(*arms < 0.02, "the arms do not change over: {moments:?}");
    }
    for pair in moments.windows(2) {
        assert!(
            pair[1].0 <= pair[0].0 + 0.02,
            "the stride closes: {moments:?}"
        );
    }
    // Coming down, the feet are nearly under the body again.
    let down = at(1.0);
    // The stride has not gone back to where it began (the owner, 9 Nov: no ping-pong): it is still the same leg ahead, closing for the
    // landing, which takes it on from here.
    let split = ahead(&down, "lshin_g", "rshin_g");
    assert!((-0.02..0.15).contains(&split), "{split}");
    std::fs::remove_file(&file).ok();
}

/// With `FRAME_STRAPS` set to a folder: the upper body of the player from the front, the back, the side and three quarters,
/// standing, running and at the top of a jump, close up. For the bra straps.
#[test]
fn frames_of_the_bra_straps() {
    let Ok(dir) = std::env::var("FRAME_STRAPS") else {
        return;
    };
    let Some((mut s, file)) = stage("strapframes", "") else {
        return;
    };
    s.yaw = clear_way(&mut s);
    s.set_view((640, 640));
    let pc = s.game.world.pc;
    let mut gpu = kotor_gpu::GpuRenderer::new(640, 640).unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    for pose in ["idle", "run", "jump"] {
        match pose {
            "run" => {
                s.steer = (1.0, 0.0, false);
                run(&mut s, 40);
            }
            "jump" => {
                s.steer = (0.0, 0.0, false);
                run(&mut s, 30);
                s.preview_jump(0.5);
            }
            _ => run(&mut s, 5),
        }
        s.update(DT);
        let at = s.game.world.object(pc).unwrap().position;
        let heading = s.game.world.pc_mover.heading;
        for (view, turn, up) in [
            ("front", 0.0f32, 0.25f32),
            ("threeq", 0.7, 0.25),
            ("side", std::f32::consts::FRAC_PI_2, 0.25),
            ("back", std::f32::consts::PI, 0.25),
            ("overfront", 0.0, 1.0),
            ("overback", std::f32::consts::PI, 1.0),
        ] {
            let a = heading + turn;
            let cam = kotor_render::Camera {
                eye: [
                    at[0] + a.cos() * 1.3,
                    at[1] + a.sin() * 1.3,
                    at[2] + 1.35 + up * 1.3,
                ],
                target: [at[0], at[1], at[2] + 1.35],
                fov: 40.0,
            };
            let still = s.static_scene().clone();
            gpu.set_static_scene(&still, &mut |n| s.texture(n));
            let moving = s.dynamic_scene();
            let img = gpu.render(&moving, &mut |n| s.texture(n), &cam).unwrap();
            std::fs::write(
                format!("{dir}/{pose}-{view}.png"),
                kotor_image::to_png(&img),
            )
            .unwrap();
        }
    }
    std::fs::remove_file(&file).ok();
}

/// Owner: the jump needs a crouch before it, "original-style and fluid". From the key press the body is drawn sinking into a crouch on
/// the floor for the wind-up, and it leaves the floor from that crouch.
#[test]
fn a_jump_crouches_on_the_floor_before_it_leaves() {
    let Some((mut s, file)) = stage("windup", "") else {
        return;
    };
    s.yaw = clear_way(&mut s);
    s.steer = (0.0, 0.0, false);
    run(&mut s, 30);
    let stand = s
        .game
        .world
        .pc_body
        .jump_windup_progress(&s.game.world.tuning);
    assert!(stand.is_none(), "not winding up before the key");
    s.jump_key();
    let mut deepest = 0.0f32;
    let mut grounded_bent = 0;
    for _ in 0..40 {
        s.update(DT);
        let body = &s.game.world.pc_body;
        if body.jump_windup_progress(&s.game.world.tuning).is_some() {
            assert!(body.grounded, "winding up on the floor");
            if let Some(j) = s.bent_joints() {
                grounded_bent += 1;
                deepest = deepest.max(j["lshin_g"][1] - j["lfoot_g"][1]);
            }
        }
    }
    assert!(
        grounded_bent >= 3,
        "the crouch is drawn: {grounded_bent} frames"
    );
    println!("crouch frames {grounded_bent}, shin ahead of foot {deepest:.3}");
    std::fs::remove_file(&file).ok();
}

/// From a run or from standing, through a jump and its landing, no hand or foot jumps from one frame to the next.
#[test]
fn a_jump_goes_smoothly_from_the_run_to_the_landing_and_back() {
    let Some((mut s, file)) = stage("smooth", "") else {
        return;
    };
    s.yaw = clear_way(&mut s);
    for forward in [1.0, 0.0] {
        s.steer = (forward, 0.0, false);
        run(&mut s, 45);
        s.jump_key();
        let mut last: Option<std::collections::HashMap<String, [f32; 3]>> = None;
        let (mut worst, mut seen, mut landed) = (0.0f32, 0, false);
        for _ in 0..90 {
            s.update(DT);
            landed |= s.game.world.pc_body.landing_from_jump();
            let Some(j) = s.bent_joints() else {
                last = None;
                continue;
            };
            seen += 1;
            if let Some(old) = &last {
                for n in [
                    "lfoot_g", "rfoot_g", "lhand_g", "rhand_g", "lshin_g", "rshin_g",
                ] {
                    let d = (0..3)
                        .map(|k| (j[n][k] - old[n][k]).powi(2))
                        .sum::<f32>()
                        .sqrt();
                    worst = worst.max(d);
                }
            }
            last = Some(j);
        }
        assert!(seen > 30 && landed, "{seen} bent frames, landed {landed}");
        println!("forward {forward}: at most {worst:.3} m in a frame");
        assert!(worst < 0.16, "a joint moved {worst} m in one frame");
    }
    std::fs::remove_file(&file).ok();
}

/// The owner, 8 Nov: "Jump animation is still off. Palms shouldn't be facing out." With `FRAME_JUMP` set to a folder, five moments of
/// a jump from the front and the side are written there.
#[test]
fn frames_of_a_jump_from_the_front_and_the_side() {
    let Ok(dir) = std::env::var("FRAME_JUMP") else {
        return;
    };
    let Some((mut s, file)) = stage("jumpframes", "") else {
        return;
    };
    s.set_view((480, 640));
    let pc = s.game.world.pc;
    let mut gpu = kotor_gpu::GpuRenderer::new(480, 640).unwrap();
    // FRAME_JUMP_COUNT moments (5 by default) from just off the floor to just before the landing.
    let count: usize = std::env::var("FRAME_JUMP_COUNT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    // FRAME_WINDUP_COUNT frames (0) of the crouch before it, from the key press to the last moment on the floor.
    let windup: usize = std::env::var("FRAME_WINDUP_COUNT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut moments: Vec<(bool, f32)> = (0..windup)
        .map(|k| (true, k as f32 / windup.max(2) as f32))
        .collect();
    moments.extend((0..count).map(|k| (false, 0.05 + 0.9 * k as f32 / (count.max(2) - 1) as f32)));
    for (n, (crouch, p)) in moments.iter().enumerate() {
        if *crouch {
            s.preview_windup(*p);
        } else {
            s.preview_jump(*p);
        }
        s.update(DT);
        let at = s.game.world.object(pc).unwrap().position;
        let heading = s.game.world.pc_mover.heading;
        for (view, ahead) in [("front", true), ("side", false)] {
            let (dx, dy) = if ahead {
                (heading.cos(), heading.sin())
            } else {
                (-heading.sin(), heading.cos())
            };
            let cam = kotor_render::Camera {
                eye: [at[0] + dx * 2.6, at[1] + dy * 2.6, at[2] + 1.2],
                target: [at[0], at[1], at[2] + 0.9],
                fov: 40.0,
            };
            let still = s.static_scene().clone();
            gpu.set_static_scene(&still, &mut |n| s.texture(n));
            let moving = s.dynamic_scene();
            let img = gpu.render(&moving, &mut |n| s.texture(n), &cam).unwrap();
            std::fs::write(
                format!("{dir}/jump-{n:03}-{view}.png"),
                kotor_image::to_png(&img),
            )
            .unwrap();
        }
    }
    std::fs::remove_file(&file).ok();
}
