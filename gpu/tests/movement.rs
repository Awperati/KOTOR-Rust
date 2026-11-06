//! Modern movement through the window's loop: the settings that choose it, the roll key, the camera, and the look button.
use kotor_game::MovementMode;
use kotor_gpu::stage::Stage;

const SIZE: (u32, u32) = (640, 360);
const DT: f32 = 1.0 / 60.0;

/// A game with no pauses or pop-ups, its settings in a file of its own with these lines.
fn stage(name: &str, settings: &str) -> Option<(Stage, std::path::PathBuf)> {
    kotor_core::install_dir()?;
    let mut s = Stage::open("danm14aa", true).unwrap();
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
