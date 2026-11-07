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
fn modern_is_the_default_and_classic_is_a_setting() {
    let Some((mut s, file)) = stage("default", "") else {
        return;
    };
    assert!(s.settings().movement_modern);
    assert!(s.settings().click_to_walk);
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
        ("", true),
        ("movement_modern=false\n", true),
        ("click_to_walk=false\n", false),
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
fn a_roll_bends_both_sides_alike_with_the_knees_up_and_the_hands_on_the_shins() {
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
    // Each hand within a hand's length of its shin, and neither side of the body is crossed.
    for (hand, knee, foot) in [
        ("lhand_g", "lshin_g", "lfoot_g"),
        ("rhand_g", "rshin_g", "rfoot_g"),
    ] {
        let middle = [0, 1, 2].map(|i| (j[knee][i] + j[foot][i]) / 2.0);
        let d: f32 = (0..3)
            .map(|i| (j[hand][i] - middle[i]).powi(2))
            .sum::<f32>()
            .sqrt();
        assert!(d < 0.3, "{hand} is {d} m from its shin: {:?}", j[hand]);
    }
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

/// An area joined on at a door has walls of its own, which this area's rooms do not know of. The Apartments' stand over the
/// corridor up to their door, and the camera rose behind them and lost the player.
#[test]
fn the_camera_stops_at_the_walls_of_an_area_joined_on() {
    let Some((mut s, file)) = stage_in("tar_m02ac", "joinedwall", "") else {
        return;
    };
    let way = std::f32::consts::FRAC_PI_2;
    s.place_player([90.0, 122.0, 0.0], way);
    s.pitch = 1.1;
    s.yaw = way;
    let from = s.static_version;
    let began = std::time::Instant::now();
    while s.static_version == from && began.elapsed().as_secs() < 60 {
        s.update(DT);
        // Nothing joined shows until a window has taken its textures.
        s.take_warm(64);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(s.static_version != from, "the Apartments are joined on");
    run(&mut s, 60);
    let away = camera_away(&s);
    assert!(
        away < 3.2,
        "the camera is {away} m out, and 3.3 m is the whole arm"
    );
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
