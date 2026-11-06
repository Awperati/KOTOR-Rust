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
    // It is still drawn: the turn itself is tested beside `tilt_matrix`.
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

#[test]
fn looking_round_does_not_steer_the_keys() {
    let Some((mut s, file)) = stage("look", "") else {
        return;
    };
    s.yaw = clear_way(&mut s);
    let way = s.yaw;
    s.steer = (1.0, 0.0, false);
    run(&mut s, 3);
    // The look button goes down, and the mouse drags right. The player keeps going the way it was.
    s.set_looking(true);
    s.look(400.0, 0.0);
    let camera_turned = s.yaw;
    assert!((camera_turned - way).abs() > 0.5, "the camera turned");
    let a = pc_at(&s);
    run(&mut s, 8);
    let b = pc_at(&s);
    let gone = (b[1] - a[1]).atan2(b[0] - a[0]);
    let off = (gone - way).sin().abs();
    assert!(off < 0.05, "went {gone}, keys say {way}");
    assert!(
        (s.yaw - camera_turned).abs() < 1e-4,
        "and the camera stays put"
    );
    // Let go: the keys go by the camera again.
    s.set_looking(false);
    run(&mut s, 30);
    let (a, b) = (b, pc_at(&s));
    let gone = (b[1] - a[1]).atan2(b[0] - a[0]);
    assert!((gone - way).sin().abs() > 0.1, "now it goes by the camera");
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
fn click_to_walk_can_be_turned_off() {
    for (lines, walks) in [("", true), ("click_to_walk=false\n", false)] {
        let Some((mut s, file)) = stage(if walks { "click_on" } else { "click_off" }, lines) else {
            return;
        };
        let px = floor_pixel(&mut s);
        s.click(px, SIZE);
        assert_eq!(s.game.world.pc_walking(), walks, "{lines:?}");
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
