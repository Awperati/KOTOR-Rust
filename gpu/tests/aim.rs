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

/// A creature made an enemy, standing `ahead` metres in front of the player, in the middle of the aim, with a blaster in hand.
fn foe_ahead(s: &mut Stage, ahead: f32) -> u32 {
    let w = &mut s.game.world;
    let pc = w.pc;
    let at = w.pc_mover.pos;
    let foe = w
        .objects
        .values()
        .find(|o| o.kind == kotor_game::Kind::Creature && o.id != pc && !o.dead && !o.party_member)
        .map(|o| o.id)
        .expect("a creature");
    let (x, y) = (s.yaw.cos(), s.yaw.sin());
    let w = &mut s.game.world;
    w.teleport(foe, [at[0] + x * ahead, at[1] + y * ahead, at[2]]);
    let f = w.object_mut(foe).unwrap();
    (f.hp, f.max_hp, f.faction) = (5000, 5000, 1);
    foe
}

/// A blaster from the player's own game, or the first item that is one.
fn blaster_in_hand(s: &mut Stage) -> bool {
    let w = &mut s.game.world;
    let pc = w.pc;
    let gun = w.spawn_item("g_w_blstrpstl001");
    w.equip(pc, gun, Some(4)).unwrap();
    w.weapon_of(pc).ranged
}

fn aim_at(s: &mut Stage) {
    // The camera's ray passes the shoulder: turn it onto what stands straight ahead of the body at this range.
    s.set_combat_modern(true);
    s.attack_mode_key();
    s.set_aim_held(true);
    run(s, 40);
}

#[test]
fn a_shot_on_the_body_always_lands_and_rolls_a_graze_a_hit_or_a_critical() {
    let Some((mut s, file)) = stage("shot", "") else {
        return;
    };
    if !blaster_in_hand(&mut s) {
        return;
    }
    aim_at(&mut s);
    let way = aim_yaw_onto(&mut s, 6.0);
    let foe = way;
    let hp = s.game.world.object(foe).unwrap().hp;
    let logged = s.game.world.combat.log.len();
    let bolts = s.effects.bolts_fired;
    for _ in 0..60 {
        s.click((320.0, 180.0), SIZE);
        run(&mut s, 31);
    }
    let log: Vec<_> = s.game.world.combat.log[logged..]
        .iter()
        .filter(|l| l.attacker == s.game.world.pc)
        .cloned()
        .collect();
    assert_eq!(log.len(), 60, "every click a shot");
    assert!(log.iter().all(|l| l.target == foe));
    assert!(log.iter().all(|l| l.outcome != kotor_game::Outcome::Miss));
    assert!(log.iter().all(|l| l.damage >= 1), "{log:?}");
    assert!(s.game.world.object(foe).unwrap().hp < hp);
    assert!(s.effects.bolts_fired >= bolts + 60);
    // The rule is the shot's own, not the no-misses setting's: with that off, a shot still never misses.
    let (eye, dir) = s.aim_ray();
    s.game.world.no_misses = false;
    let logged = s.game.world.combat.log.len();
    for _ in 0..200 {
        s.game.world.fire_aimed(eye, dir, 30.0).unwrap();
        s.game.world.object_mut(foe).unwrap().hp = 5000;
    }
    let w = &s.game.world;
    let shots = &w.combat.log[logged..];
    assert_eq!(shots.iter().filter(|l| l.attacker == w.pc).count(), 200);
    assert!(shots.iter().all(|l| l.outcome != kotor_game::Outcome::Miss));
    std::fs::remove_file(&file).ok();
}

/// Turn the camera so that the crosshair lies on a foe `ahead` metres off, and give back the foe.
fn aim_yaw_onto(s: &mut Stage, ahead: f32) -> u32 {
    let foe = foe_ahead(s, ahead);
    aim_back(s, foe);
    s.set_view(SIZE);
    foe
}

/// Turn the camera onto a foe, and give the ray.
fn aim_back(s: &mut Stage, foe: u32) -> ([f32; 3], [f32; 3]) {
    for _ in 0..200 {
        let c = s.camera();
        let at = s.game.world.object(foe).unwrap().position;
        let want = (at[1] - c.eye[1]).atan2(at[0] - c.eye[0]);
        let off = (want - s.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        s.yaw += off * 0.5;
        s.update(DT);
    }
    s.aim_ray()
}

#[test]
fn the_crosshair_warms_on_an_enemy_and_a_shot_past_it_or_too_soon_does_nothing() {
    let Some((mut s, file)) = stage("past", "") else {
        return;
    };
    if !blaster_in_hand(&mut s) {
        return;
    }
    aim_at(&mut s);
    let foe = aim_yaw_onto(&mut s, 6.0);
    assert!(s.aim_hot(), "on the foe");
    let hp = s.game.world.object(foe).unwrap().hp;
    let logged = s.game.world.combat.log.len();
    // Two clicks at once are one shot.
    s.click((320.0, 180.0), SIZE);
    s.click((320.0, 180.0), SIZE);
    assert_eq!(s.game.world.combat.log.len(), logged + 1);
    // Off to the side, it is cold and hits nothing.
    run(&mut s, 40);
    s.yaw += 1.2;
    run(&mut s, 2);
    assert!(!s.aim_hot(), "off the foe");
    let hp_now = s.game.world.object(foe).unwrap().hp;
    let logged = s.game.world.combat.log.len();
    s.click((320.0, 180.0), SIZE);
    assert_eq!(s.game.world.object(foe).unwrap().hp, hp_now);
    assert_eq!(s.game.world.combat.log.len(), logged);
    assert!(hp_now <= hp);
    std::fs::remove_file(&file).ok();
}

#[test]
fn a_closed_door_or_a_wall_stops_a_shot_and_a_friend_is_not_hit() {
    let Some((mut s, file)) = stage("wall", "") else {
        return;
    };
    if !blaster_in_hand(&mut s) {
        return;
    }
    aim_at(&mut s);
    let foe = aim_yaw_onto(&mut s, 6.0);
    // Standing behind a blocked line.
    let (a, b) = {
        let w = &s.game.world;
        (w.pc_gun(), w.object(foe).unwrap().position)
    };
    let (mid, side) = (
        [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, a[2]],
        [-(b[1] - a[1]), b[0] - a[0], 0.0],
    );
    let long = side[0].hypot(side[1]);
    let (p, q) = (
        [
            mid[0] + side[0] / long * 3.0,
            mid[1] + side[1] / long * 3.0,
            b[2],
        ],
        [
            mid[0] - side[0] / long * 3.0,
            mid[1] - side[1] / long * 3.0,
            b[2],
        ],
    );
    let (eye, dir) = s.aim_ray();
    assert_eq!(s.game.world.aim_hit(eye, dir, 30.0).who, Some(foe));
    // Nearer than the foe, the room's own surface stops it.
    assert_eq!(s.game.world.aim_hit(eye, dir, 2.0).who, None, "a wall");
    // On the player's team, it is not hit.
    let pc = s.game.world.pc;
    s.game.world.party.push(foe);
    assert_eq!(s.game.world.aim_hit(eye, dir, 30.0).who, None, "a friend");
    s.game.world.party.retain(|p| *p != foe);
    assert_eq!(s.game.world.aim_hit(eye, dir, 30.0).who, Some(foe));
    // The room's real surface: turn until the ray meets one, and stand the foe just beyond it.
    let mut wall = None;
    for _ in 0..64 {
        s.yaw += std::f32::consts::TAU / 64.0;
        run(&mut s, 2);
        let (e, d) = s.aim_ray();
        let r = s.aim_reach(e, d);
        if (1.0..25.0).contains(&r) {
            wall = Some((e, d, r));
            break;
        }
    }
    let (e, d, r) = wall.expect("the room has a surface");
    let t = r + 1.5;
    let beyond = [e[0] + d[0] * t, e[1] + d[1] * t, e[2] + d[2] * t - 1.0];
    s.game.world.teleport(foe, beyond);
    run(&mut s, 40);
    assert!(!s.aim_hot(), "a foe beyond the wall");
    let logged = s.game.world.combat.log.len();
    s.click((320.0, 180.0), SIZE);
    assert!(s.game.world.combat.log[logged..]
        .iter()
        .all(|l| l.attacker != pc));
    s.game.world.teleport(foe, b);
    run(&mut s, 1);
    let (eye, dir) = aim_back(&mut s, foe);
    // A closed door between them stops it, and the click logs nothing.
    s.game.world.nav.as_mut().unwrap().block_line(p, q);
    assert_eq!(s.game.world.aim_hit(eye, dir, 30.0).who, None, "a door");
    let logged = s.game.world.combat.log.len();
    s.click((320.0, 180.0), SIZE);
    assert!(s.game.world.combat.log[logged..]
        .iter()
        .all(|l| l.attacker != pc));
    std::fs::remove_file(&file).ok();
}

#[test]
fn a_neutral_is_hit_only_with_attack_mode_on() {
    let Some((mut s, file)) = stage("neutral", "") else {
        return;
    };
    if !blaster_in_hand(&mut s) {
        return;
    }
    aim_at(&mut s);
    let foe = aim_yaw_onto(&mut s, 6.0);
    let (eye, dir) = s.aim_ray();
    s.game.world.object_mut(foe).unwrap().faction = 5;
    assert!(s.game.world.reputation(s.game.world.pc, foe) > 10);
    assert_eq!(s.game.world.aim_hit(eye, dir, 30.0).who, Some(foe));
    // The shot is a blow like any other: the law hears it.
    let pc = s.game.world.pc;
    s.game.world.fire_aimed(eye, dir, 30.0).unwrap();
    let w = &s.game.world;
    assert!(w
        .combat
        .log
        .iter()
        .any(|l| l.attacker == pc && l.target == foe));
    assert_eq!(w.combat.last_hostile.get(&pc), Some(&foe));
    s.attack_mode_key();
    assert_eq!(s.game.world.aim_hit(eye, dir, 30.0).who, None);
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
