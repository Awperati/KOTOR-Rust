//! Modern combat's aim through the window's loop: when it begins, the camera, the body and the sight.
use kotor_gpu::stage::Stage;

const SIZE: (u32, u32) = (640, 360);
const DT: f32 = 1.0 / 60.0;

fn stage(name: &str, settings: &str) -> Option<(Stage, std::path::PathBuf)> {
    kotor_core::install_dir()?;
    let mut s = Stage::open("danm14aa", true).unwrap();
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let file = std::env::temp_dir().join(format!("kotor_aim_{name}_{}.txt", std::process::id()));
    std::fs::write(&file, settings).unwrap();
    s.use_settings_file(file.clone());
    s.set_view(SIZE);
    s.update(DT);
    for _ in 0..600 {
        if !s.game.world.in_dialogue() {
            break;
        }
        s.continue_line();
        s.number_key(1);
        s.update(DT);
    }
    Some((s, file))
}

fn run(s: &mut Stage, frames: usize) {
    for _ in 0..frames {
        s.update(DT);
    }
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f32>().sqrt()
}

#[test]
fn the_look_button_aims_only_in_modern_combat_and_in_a_fight_or_with_attack_mode() {
    let Some((mut s, file)) = stage("when", "") else {
        return;
    };
    // Classic: the button is the camera's alone.
    s.set_aim_held(true);
    run(&mut s, 5);
    assert!(!s.aiming(), "Classic is the default");
    // Modern, with nothing to fight: still the camera's alone.
    s.set_combat_modern(true);
    run(&mut s, 5);
    if s.seen_enemy().is_none() {
        assert!(!s.aiming(), "no fight, no enemy, Attack mode off");
    }
    s.attack_mode_key();
    run(&mut s, 2);
    assert!(s.aiming(), "Attack mode on");
    // Let go, and a menu or Classic movement, end it.
    s.set_aim_held(false);
    run(&mut s, 2);
    assert!(!s.aiming());
    s.set_aim_held(true);
    run(&mut s, 2);
    assert!(s.aiming());
    s.escape();
    run(&mut s, 2);
    assert!(!s.aiming(), "a menu");
    s.escape();
    run(&mut s, 2);
    assert!(s.aiming());
    s.set_movement_modern(false);
    run(&mut s, 2);
    assert!(!s.aiming(), "Classic movement");
    std::fs::remove_file(&file).ok();
}

#[test]
fn aiming_brings_the_camera_over_the_right_shoulder_and_lets_it_go_again() {
    let Some((mut s, file)) = stage("camera", "") else {
        return;
    };
    s.set_combat_modern(true);
    s.attack_mode_key();
    run(&mut s, 30);
    let before = s.camera();
    let (dx, dy) = (s.yaw.cos(), s.yaw.sin());
    s.set_aim_held(true);
    run(&mut s, 60);
    assert!(s.aim_share() > 0.99, "{}", s.aim_share());
    let aimed = s.camera();
    let body = s.game.world.pc_mover.pos;
    // Nearer, narrower and higher up the body.
    assert!(aimed.fov < before.fov - 5.0, "{} {}", aimed.fov, before.fov);
    assert!(aimed.target[2] - body[2] > 1.4, "{:?}", aimed.target);
    let arm = distance(aimed.eye, aimed.target);
    assert!(arm < 2.2, "{arm}");
    assert!(arm < distance(before.eye, before.target));
    // To the player's right, if there is room: the keys' right is (dy, -dx). The opening room has it.
    let side = (aimed.eye[0] - body[0]) * dy - (aimed.eye[1] - body[1]) * dx;
    assert!(side > 0.3, "{side}");
    // Letting go gives it all back.
    s.set_aim_held(false);
    run(&mut s, 180);
    let after = s.camera();
    assert_eq!(s.aim_share(), 0.0);
    assert!((after.fov - before.fov).abs() < 0.5);
    assert!((distance(after.eye, after.target) - distance(before.eye, before.target)).abs() < 0.2);
    std::fs::remove_file(&file).ok();
}

#[test]
fn aiming_the_body_faces_the_aim_goes_slower_and_the_keys_go_by_the_aim() {
    let Some((mut s, file)) = stage("body", "") else {
        return;
    };
    s.set_combat_modern(true);
    s.attack_mode_key();
    s.set_aim_held(true);
    run(&mut s, 30);
    let start = s.game.world.pc_body.pos;
    // Strafing to the right, the body still faces the aim, and at no more than the aim's speed.
    s.steer = (0.0, 1.0, false);
    let mut fastest = 0.0f32;
    for _ in 0..40 {
        s.update(DT);
        fastest = fastest.max(s.game.world.pc_body.speed());
    }
    let off = (s.game.world.pc_body.heading - s.yaw + std::f32::consts::PI)
        .rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;
    assert!(off.abs() < 0.05, "faces the aim: {off}");
    assert!(fastest <= s.game.world.tuning.aim_speed + 0.01, "{fastest}");
    let now = s.game.world.pc_body.pos;
    let (dx, dy) = (s.yaw.cos(), s.yaw.sin());
    let right = (now[0] - start[0]) * dy - (now[1] - start[1]) * dx;
    let ahead = (now[0] - start[0]) * dx + (now[1] - start[1]) * dy;
    assert!(right > 0.3 && ahead.abs() < right * 0.2, "{right} {ahead}");
    // Let go, and the body turns to where it goes again.
    s.set_aim_held(false);
    run(&mut s, 40);
    assert!(s.game.world.pc_aim.is_none());
    assert!(s.game.world.pc_body.speed() > s.game.world.tuning.aim_speed);
    std::fs::remove_file(&file).ok();
}

#[test]
fn the_mouse_looks_lower_while_aiming_and_comes_back_when_it_ends() {
    let Some((mut s, file)) = stage("pitch", "") else {
        return;
    };
    s.set_combat_modern(true);
    s.attack_mode_key();
    s.set_aim_held(true);
    run(&mut s, 2);
    for _ in 0..50 {
        s.look(0.0, -100.0);
    }
    assert!(s.pitch < -0.5, "{}", s.pitch);
    s.set_aim_held(false);
    run(&mut s, 60);
    assert!((s.pitch + 0.2).abs() < 0.02, "{}", s.pitch);
    std::fs::remove_file(&file).ok();
}

#[test]
fn a_click_while_aiming_does_not_walk() {
    let Some((mut s, file)) = stage("click", "") else {
        return;
    };
    s.set_combat_modern(true);
    s.attack_mode_key();
    s.set_aim_held(true);
    run(&mut s, 2);
    s.click((320.0, 300.0), SIZE);
    assert!(!s.game.world.pc_walking());
    s.set_aim_held(false);
    run(&mut s, 2);
    s.click((320.0, 300.0), SIZE);
    assert!(s.game.world.pc_walking());
    std::fs::remove_file(&file).ok();
}
