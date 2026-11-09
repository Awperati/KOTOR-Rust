//! Classic or Modern, the Single Player choice: Classic's tank controls and pauses, Modern's play that only Escape holds.
use kotor_gpu::stage::Stage;

const DT: f32 = 1.0 / 60.0;

fn stage(name: &str) -> Option<(Stage, std::path::PathBuf)> {
    kotor_core::install_dir()?;
    let mut s = Stage::open("danm14aa", true).unwrap();
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let file = std::env::temp_dir().join(format!("kotor_mode_{name}_{}.txt", std::process::id()));
    std::fs::write(&file, "").unwrap();
    s.use_settings_file(file.clone());
    s.set_view((640, 360));
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

fn run(s: &mut Stage, frames: usize, steer: (f32, f32)) {
    for _ in 0..frames {
        s.steer = (steer.0, steer.1, false);
        s.update(DT);
    }
}

fn gone(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

#[test]
fn classic_has_tank_controls_left_and_right_turn_and_there_is_no_strafe() {
    let Some((mut s, file)) = stage("tank") else {
        return;
    };
    s.set_movement_modern(false);
    run(&mut s, 5, (0.0, 0.0));
    // D: the body and the camera turn right, and the player does not step aside.
    let (yaw, at) = (s.yaw, s.game.world.pc_mover.pos);
    run(&mut s, 30, (0.0, 1.0));
    assert!(s.yaw < yaw - 0.5, "turned right: {yaw} to {}", s.yaw);
    assert!(gone(at, s.game.world.pc_mover.pos) < 0.05, "no strafe");
    // The body turned with it, standing: the camera is still behind it (the preview of 9 Nov had it in front).
    let tau = std::f32::consts::TAU;
    let off = |a: f32, b: f32| ((a - b + tau / 2.0).rem_euclid(tau) - tau / 2.0).abs();
    let heading = s.game.world.pc_mover.heading;
    assert!(
        off(heading, s.yaw) < 0.05,
        "body {heading}, camera {}",
        s.yaw
    );
    let pc = s.game.world.pc;
    let drawn = s.game.poses().iter().find(|p| p.id == pc).unwrap().heading;
    assert!(off(drawn, s.yaw) < 0.05, "drawn {drawn}, camera {}", s.yaw);
    // W goes the way it now faces.
    let at = s.game.world.pc_mover.pos;
    run(&mut s, 40, (1.0, 0.0));
    let now = s.game.world.pc_mover.pos;
    assert!(gone(at, now) > 0.5);
    let way = (now[1] - at[1]).atan2(now[0] - at[0]);
    let tau = std::f32::consts::TAU;
    let off = ((way - s.yaw + tau / 2.0).rem_euclid(tau) - tau / 2.0).abs();
    assert!(off < 0.2, "along the facing: {off}");
    // Modern movement strafes, as before.
    s.set_movement_modern(true);
    let (yaw, at) = (s.yaw, s.game.world.pc_mover.pos);
    run(&mut s, 30, (0.0, 1.0));
    assert!((s.yaw - yaw).abs() < 0.05, "the camera stays");
    assert!(gone(at, s.game.world.pc_mover.pos) > 0.3, "a strafe");
    std::fs::remove_file(&file).ok();
}

#[test]
fn modern_mode_has_no_pause_but_escape_and_classic_pauses_as_ever() {
    let Some((mut s, file)) = stage("pause") else {
        return;
    };
    // Classic: the pause key pauses, and again goes on.
    s.set_combat_modern(false);
    run(&mut s, 40, (0.0, 0.0));
    s.pause_key();
    assert!(s.paused(), "Classic pauses");
    s.pause_key();
    assert!(!s.paused());
    // Modern: movement and combat both. The pause key and the hourglass do nothing.
    s.set_movement_modern(true);
    s.set_combat_modern(true);
    assert!(s.modern_mode());
    s.pause_key();
    assert!(!s.paused(), "no pause key in Modern");
    s.toggle_pause();
    assert!(!s.paused(), "no hourglass in Modern");
    let t = s.game.world.time;
    run(&mut s, 10, (0.0, 0.0));
    assert!(s.game.world.time > t, "the game goes on");
    // Escape's menu holds the world, in Modern too.
    s.escape();
    s.update(DT);
    let t = s.game.world.time;
    run(&mut s, 10, (0.0, 0.0));
    assert_eq!(s.game.world.time, t, "held by Escape");
    s.escape();
    s.update(DT);
    let t = s.game.world.time;
    run(&mut s, 10, (0.0, 0.0));
    assert!(s.game.world.time > t);
    std::fs::remove_file(&file).ok();
}
