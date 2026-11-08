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

/// The size of the drift after `frames` frames.
fn sway_after(s: &mut Stage, frames: usize) -> f32 {
    run(s, frames);
    s.aim_sway()
}

#[test]
fn the_sway_is_by_the_clock_and_grows_with_speed_and_a_shot() {
    // The same clock, the same place, and it does move.
    assert_eq!(kotor_game::sway_path(12.5), kotor_game::sway_path(12.5));
    assert_ne!(kotor_game::sway_path(12.5), kotor_game::sway_path(13.5));
    let mut paths = Vec::new();
    for name in ["clock1", "clock2"] {
        let Some((mut s, file)) = stage(name, "") else {
            return;
        };
        if !blaster_in_hand(&mut s) {
            return;
        }
        aim_at(&mut s);
        let mut path = Vec::new();
        for _ in 0..30 {
            run(&mut s, 5);
            path.push(s.aim_drift());
        }
        paths.push(path);
        std::fs::remove_file(&file).ok();
    }
    assert_eq!(paths[0], paths[1], "the same clock gives the same path");
    let Some((mut s, file)) = stage("speed", "") else {
        return;
    };
    if !blaster_in_hand(&mut s) {
        return;
    }
    aim_at(&mut s);
    let still = sway_after(&mut s, 120);
    assert!(still > 0.0);
    // Walking.
    s.steer = (1.0, 0.0, false);
    let walking = sway_after(&mut s, 120);
    assert!(walking > still * 2.0, "{walking} against {still}");
    s.steer = (0.0, 0.0, false);
    let back = sway_after(&mut s, 180);
    assert!((back - still).abs() < still * 0.1, "{back} against {still}");
    // Just fired, and it goes again.
    s.click((320.0, 180.0), SIZE);
    let fired = sway_after(&mut s, 8);
    assert!(fired > still * 1.3, "{fired} against {still}");
    let back = sway_after(&mut s, 180);
    assert!((back - still).abs() < still * 0.1, "{back} against {still}");
    std::fs::remove_file(&file).ok();
}

#[test]
fn a_rifle_is_steadier_a_heavy_weapon_less_and_a_feat_shrinks_the_sway() {
    let Some((mut s, file)) = stage("size", "") else {
        return;
    };
    let w = &mut s.game.world;
    let pc = w.pc;
    let size = |w: &mut kotor_game::World, item: &str| {
        let gun = w.spawn_item(item);
        w.equip(pc, gun, Some(4)).unwrap();
        w.sway_size(10.0)
    };
    let pistol = size(w, "g_w_blstrpstl001");
    let rifle = size(w, "g_w_blstrrfl001");
    let heavy = size(w, "g_w_rptnblstr01");
    assert!(rifle < pistol, "{rifle} against {pistol}");
    assert!(heavy > pistol, "{heavy} against {pistol}");
    // Rifle feats: Weapon Focus, and Specialization on top of it.
    size(w, "g_w_blstrrfl001");
    w.object_mut(pc).unwrap().feats.insert(33);
    let focus = w.sway_size(10.0);
    assert!(focus < rifle, "{focus} against {rifle}");
    w.object_mut(pc).unwrap().feats.insert(47);
    assert!(w.sway_size(10.0) < focus);
    std::fs::remove_file(&file).ok();
}

#[test]
fn the_shot_follows_the_drift_that_shows() {
    let Some((mut s, file)) = stage("follow", "") else {
        return;
    };
    if !blaster_in_hand(&mut s) {
        return;
    }
    aim_at(&mut s);
    let foe = aim_yaw_onto(&mut s, 6.0);
    // Walk, for a wide drift, at a moment it lies well off the middle.
    s.game.world.object_mut(foe).unwrap().faction = 5;
    s.steer = (0.0, 1.0, false);
    let mut seen = false;
    let (mid_x, mid_y) = ((480.0 * SIZE.0 as f32 / SIZE.1 as f32) as i32 / 2, 240);
    for _ in 0..600 {
        s.update(DT);
        let (a, b) = s.aim_drift();
        let c = s.camera();
        let d = [0, 1, 2].map(|k| c.target[k] - c.eye[k]);
        let long = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let (_, ray) = s.aim_ray();
        let along: f32 = (0..3).map(|k| ray[k] * d[k] / long).sum();
        let hair = s.crosshair_view().unwrap();
        // The ray is off the camera's middle by what the drift says, and the crosshair by the same way.
        let want = (a * a + b * b).sqrt();
        assert!(
            (along.min(1.0).acos() - want).abs() < 0.004,
            "{along} {want}"
        );
        if a.abs() > 0.01 && b.abs() > 0.01 {
            assert_eq!(hair.x > mid_x, a > 0.0, "{} {a}", hair.x);
            assert_eq!(hair.y < mid_y, b > 0.0, "{} {b}", hair.y);
            seen = true;
        }
    }
    assert!(seen, "the drift never went wide");
    std::fs::remove_file(&file).ok();
}

/// Every row of the Force page, as the settings file names it.
const POWERS: [i32; 42] = [
    4, 7, 8, 9, 10, 11, 12, 13, 15, 16, 17, 18, 19, 20, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
    33, 34, 35, 36, 37, 38, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50,
];

#[test]
fn every_power_has_a_row_and_casts_from_its_key_at_what_the_crosshair_is_on() {
    let Some((mut s, file)) = stage("powers", "") else {
        return;
    };
    assert!(blaster_in_hand(&mut s));
    aim_at(&mut s);
    let foe = aim_yaw_onto(&mut s, 2.0);
    let pc = s.game.world.pc;
    assert!(s.aim_hot());
    let stands = s.game.world.object(foe).unwrap().position;
    for power in POWERS {
        let action = format!("Force{power}");
        // The last power may have thrown it across the room, which takes a moment, and it may have hit back.
        run(&mut s, 90);
        let me = s.game.world.object_mut(pc).unwrap();
        (me.hp, me.max_hp, me.dead, me.down) = (5000, 5000, false, false);
        s.game.world.teleport(foe, stands);
        run(&mut s, 2);
        let w = &mut s.game.world;
        let f = w.object_mut(foe).unwrap();
        (f.hp, f.max_hp, f.dead, f.down, f.faction) = (5000, 5000, false, false, 1);
        let o = w.object_mut(pc).unwrap();
        (o.force, o.max_force, o.dead, o.down) = (1000, 1000, false, false);
        o.spells.insert(power);
        w.last_power.remove(&pc);
        // A stronger power of its line left on from before would refuse it.
        for p in POWERS {
            w.end_power(pc, p);
        }
        assert_eq!(s.power_key(&action), None, "{action}");
        let w = &s.game.world;
        assert_eq!(w.last_power.get(&pc).map(|p| p.0), Some(power), "{action}");
        assert!(w.object(pc).unwrap().force <= 1000, "{action}");
        run(&mut s, 1);
    }
    std::fs::remove_file(&file).ok();
}

#[test]
fn a_power_key_says_why_it_does_nothing_and_only_works_in_modern() {
    let Some((mut s, file)) = stage("nopower", "") else {
        return;
    };
    assert!(blaster_in_hand(&mut s));
    let pc = s.game.world.pc;
    // Force Wound, row 50: for an enemy, at reach. Cure, row 10: on the player.
    let (wound, cure) = ("Force50", "Force10");
    let o = s.game.world.object_mut(pc).unwrap();
    (o.force, o.max_force) = (1000, 1000);
    o.spells.remove(&50);
    // Classic: nothing, not a word.
    assert_eq!(s.power_key(wound), None);
    assert!(!s.game.world.last_power.contains_key(&pc));
    aim_at(&mut s);
    // Not known.
    let said = s.power_key(wound).unwrap();
    assert!(said.ends_with("not known"), "{said}");
    // No one on the crosshair.
    s.game.world.object_mut(pc).unwrap().spells.insert(50);
    let said = s.power_key(wound).unwrap();
    assert!(said.ends_with("no target"), "{said}");
    assert!(!s.game.world.last_power.contains_key(&pc));
    assert_eq!(s.game.world.object(pc).unwrap().force, 1000);
    // Not enough Force.
    let foe = aim_yaw_onto(&mut s, 3.0);
    s.game.world.object_mut(pc).unwrap().force = 0;
    let said = s.power_key(wound).unwrap();
    assert!(said.ends_with("Force points needed"), "{said}");
    // Out of reach of the power, on the crosshair still.
    s.game.world.object_mut(pc).unwrap().force = 1000;
    let foe_far = s.game.world.object(foe).unwrap().position;
    let (x, y) = (s.yaw.cos(), s.yaw.sin());
    s.game.world.teleport(
        foe,
        [foe_far[0] + x * 20.0, foe_far[1] + y * 20.0, foe_far[2]],
    );
    aim_back(&mut s, foe);
    if s.aim_hot() {
        let said = s.power_key(wound).unwrap();
        assert!(said.ends_with("out of reach"), "{said}");
    }
    // A power on the player needs no crosshair, and a second press ends one that lasts.
    s.game.world.object_mut(pc).unwrap().spells.insert(10);
    s.game.world.object_mut(pc).unwrap().spells.insert(17);
    assert_eq!(s.power_key(cure), None);
    assert_eq!(s.game.world.last_power.get(&pc).map(|p| p.0), Some(10));
    assert_eq!(s.power_key("Force17"), None);
    run(&mut s, 2);
    assert!(s.game.world.power_active(pc, 17));
    assert_eq!(s.power_key("Force17"), None);
    assert!(!s.game.world.power_active(pc, 17));
    std::fs::remove_file(&file).ok();
}

#[test]
fn each_self_power_reads_active_alone_and_a_weaker_one_under_a_stronger_says_so_and_costs_nothing()
{
    let Some((mut s, file)) = sturdy_stage("lines") else {
        return;
    };
    let pc = s.game.world.pc;
    // Force Aura, Force Shield, Resist Force, Resist Energy: each alone.
    for power in [18, 24, 41, 42] {
        let o = s.game.world.object_mut(pc).unwrap();
        (o.force, o.max_force) = (1000, 1000);
        o.spells.insert(power);
        assert_eq!(s.power_key(&format!("Force{power}")), None);
        run(&mut s, 30);
        assert!(s.game.world.power_active(pc, power), "{power}");
        s.game.world.end_power(pc, power);
    }
    // Under Force Armour, Aura is refused, and under Force Immunity, Resist Force.
    for (strong, weak) in [(17, 18), (17, 24), (20, 41)] {
        let o = s.game.world.object_mut(pc).unwrap();
        (o.force, o.max_force) = (1000, 1000);
        o.spells.extend([strong, weak]);
        assert_eq!(s.power_key(&format!("Force{strong}")), None);
        run(&mut s, 30);
        assert!(s.game.world.power_active(pc, strong), "{strong}");
        let force = s.game.world.object(pc).unwrap().force;
        s.game.world.last_power.remove(&pc);
        let said = s.power_key(&format!("Force{weak}")).unwrap();
        assert!(said.ends_with("is on"), "{said}");
        assert_eq!(
            s.game.world.object(pc).unwrap().force,
            force,
            "nothing spent"
        );
        assert!(!s.game.world.last_power.contains_key(&pc));
        s.game.world.end_power(pc, strong);
        // With the stronger one gone, it casts.
        assert_eq!(s.power_key(&format!("Force{weak}")), None);
        run(&mut s, 30);
        assert!(s.game.world.power_active(pc, weak), "{weak}");
        s.game.world.end_power(pc, weak);
    }
    std::fs::remove_file(&file).ok();
}

/// Three enemies in front of the player: on the left, in the middle, and on the right of the view, each `ahead` metres off.
fn three_foes(s: &mut Stage, ahead: f32) -> [u32; 3] {
    let (pc, at) = (s.game.world.pc, s.game.world.pc_mover.pos);
    let mut out = [0; 3];
    for (n, off) in [0.45f32, 0.0, -0.45].into_iter().enumerate() {
        let way = s.yaw + off;
        let at = [at[0] + way.cos() * ahead, at[1] + way.sin() * ahead, at[2]];
        let id = s.game.world.spawn_creature("n_calonord", at);
        let f = s.game.world.object_mut(id).unwrap();
        (f.hp, f.max_hp, f.faction) = (9000, 9000, 1);
        out[n] = id;
    }
    let me = s.game.world.object_mut(pc).unwrap();
    (me.hp, me.max_hp) = (9000, 9000);
    out
}

fn sturdy_stage(name: &str) -> Option<(Stage, std::path::PathBuf)> {
    let (mut s, file) = stage(name, "")?;
    s.set_combat_modern(true);
    run(&mut s, 2);
    Some((s, file))
}

#[test]
fn the_lock_takes_the_nearest_to_the_middle_lets_go_and_the_wheel_goes_round() {
    let Some((mut s, file)) = sturdy_stage("lock") else {
        return;
    };
    // Nothing to lock on to, and Classic has none.
    s.lock_key();
    assert_eq!(s.locked_on(), None);
    let [left, middle, right] = three_foes(&mut s, 6.0);
    s.set_combat_modern(false);
    s.lock_key();
    assert_eq!(s.locked_on(), None, "Classic");
    s.set_combat_modern(true);
    s.lock_key();
    assert_eq!(s.locked_on(), Some(middle));
    assert!(s.crosshair_view().is_some(), "a mark on it");
    // The wheel goes round, and stops at the ends.
    s.lock_wheel(1.0);
    assert_eq!(s.locked_on(), Some(left));
    s.lock_wheel(-1.0);
    assert_eq!(s.locked_on(), Some(middle));
    s.lock_wheel(-1.0);
    assert_eq!(s.locked_on(), Some(right));
    // The key lets go.
    s.lock_key();
    assert_eq!(s.locked_on(), None);
    assert!(s.crosshair_view().is_none());
    std::fs::remove_file(&file).ok();
}

#[test]
fn locked_the_camera_comes_round_the_body_faces_it_and_circles_and_aiming_wins() {
    let Some((mut s, file)) = sturdy_stage("circle") else {
        return;
    };
    let [left, _, _] = three_foes(&mut s, 6.0);
    s.lock_key();
    s.lock_wheel(1.0);
    assert_eq!(s.locked_on(), Some(left));
    let way_to = |s: &Stage, from: [f32; 3]| {
        let at = s.game.world.object(left).unwrap().position;
        (at[1] - from[1]).atan2(at[0] - from[0])
    };
    let apart = |a: f32, b: f32| {
        let tau = std::f32::consts::TAU;
        ((a - b + tau / 2.0).rem_euclid(tau) - tau / 2.0).abs()
    };
    run(&mut s, 90);
    let eye = s.camera().eye;
    let off = apart(s.yaw, way_to(&s, eye));
    assert!(
        off > 0.05 && off < 0.3,
        "the camera came round, to one side: {off}"
    );
    let me = s.game.world.pc_mover.pos;
    let heading = s.game.world.pc_mover.heading;
    assert!(apart(heading, way_to(&s, me)) < 0.2, "the body faces it");
    // A strafe goes round it and the body keeps facing it.
    let start = way_to(&s, me);
    s.steer = (0.0, 1.0, false);
    run(&mut s, 60);
    s.steer = (0.0, 0.0, false);
    let me = s.game.world.pc_mover.pos;
    assert!(apart(way_to(&s, me), start) > 0.1, "it went round");
    let heading = s.game.world.pc_mover.heading;
    assert!(apart(heading, way_to(&s, me)) < 0.3, "still facing it");
    // Aiming wins: the camera is the player's, and the lock is still there.
    s.set_aim_held(true);
    run(&mut s, 30);
    assert!(s.aiming());
    let yaw = s.yaw + 1.0;
    s.yaw = yaw;
    run(&mut s, 30);
    assert!(
        apart(s.yaw, yaw) < 0.001,
        "the camera stays where the mouse put it"
    );
    assert_eq!(s.locked_on(), Some(left));
    s.set_aim_held(false);
    run(&mut s, 120);
    let eye = s.camera().eye;
    assert!(apart(s.yaw, way_to(&s, eye)) < 0.3, "and comes back");
    std::fs::remove_file(&file).ok();
}

#[test]
fn the_lock_ends_when_it_dies_is_far_or_stays_out_of_sight() {
    let Some((mut s, file)) = sturdy_stage("lose") else {
        return;
    };
    let [left, middle, right] = three_foes(&mut s, 6.0);
    // Dead.
    s.lock_key();
    assert_eq!(s.locked_on(), Some(middle));
    s.game.world.object_mut(middle).unwrap().dead = true;
    run(&mut s, 1);
    assert_eq!(s.locked_on(), None, "dead");
    // Far away.
    s.lock_key();
    let id = s.locked_on().expect("another");
    assert!(id == left || id == right);
    let (x, y) = (s.yaw.cos(), s.yaw.sin());
    let p = s.game.world.object(id).unwrap().position;
    s.game
        .world
        .teleport(id, [p[0] + x * 60.0, p[1] + y * 60.0, p[2]]);
    run(&mut s, 2);
    assert_eq!(s.locked_on(), None, "far");
    // Behind a closed door for a moment, and then for longer.
    let other = if id == left { right } else { left };
    s.yaw = {
        let me = s.game.world.pc_mover.pos;
        let at = s.game.world.object(other).unwrap().position;
        (at[1] - me[1]).atan2(at[0] - me[0])
    };
    run(&mut s, 1);
    s.lock_key();
    assert_eq!(s.locked_on(), Some(other));
    let me = s.game.world.pc_mover.pos;
    let at = s.game.world.object(other).unwrap().position;
    let (mx, my) = ((me[0] + at[0]) / 2.0, (me[1] + at[1]) / 2.0);
    let (dx, dy) = (at[0] - me[0], at[1] - me[1]);
    let l = dx.hypot(dy);
    let (sx, sy) = (-dy / l * 5.0, dx / l * 5.0);
    s.game
        .world
        .nav
        .as_mut()
        .unwrap()
        .block_line([mx + sx, my + sy, me[2]], [mx - sx, my - sy, me[2]]);
    run(&mut s, 60);
    assert_eq!(s.locked_on(), Some(other), "a moment");
    run(&mut s, 120);
    assert_eq!(s.locked_on(), None, "too long");
    std::fs::remove_file(&file).ok();
}

#[test]
fn a_power_goes_to_the_lock_with_no_crosshair() {
    let Some((mut s, file)) = sturdy_stage("lockpower") else {
        return;
    };
    let [_, middle, _] = three_foes(&mut s, 6.0);
    s.lock_key();
    assert_eq!(s.locked_on(), Some(middle));
    let pc = s.game.world.pc;
    let o = s.game.world.object_mut(pc).unwrap();
    (o.force, o.max_force) = (1000, 1000);
    o.spells.insert(50);
    // Force Wound, row 50, with no crosshair.
    assert_eq!(s.power_key("Force50"), None);
    assert_eq!(s.game.world.last_power.get(&pc).map(|p| p.0), Some(50));
    let cost = s.game.world.power_cost(pc, 50).unwrap();
    assert_eq!(s.game.world.object(pc).unwrap().force, 1000 - cost);
    std::fs::remove_file(&file).ok();
}

#[test]
fn a_click_while_aiming_does_not_walk() {
    let Some((mut s, file)) = stage("click", "click_to_walk=true\n") else {
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

#[test]
fn aiming_on_the_move_the_legs_go_the_way_of_the_body_and_the_upper_body_keeps_the_stance() {
    let Some((mut s, file)) = stage("legs", "") else {
        return;
    };
    if !blaster_in_hand(&mut s) {
        return;
    }
    aim_at(&mut s);
    assert!(s.aiming());
    run(&mut s, 5);
    assert_eq!(
        s.aim_legs_now(),
        None,
        "standing, the stance clip has it all"
    );
    // Each way round the aim: the hips' turn, and whether the walk plays backwards.
    for (ahead, right, turn, back) in [
        (1.0, 0.0, 0.0, false),
        (0.7, 0.7, -45.0, false),
        (0.0, 1.0, -75.0, false),
        (-0.7, 0.7, 45.0, true),
        (-1.0, 0.0, 0.0, true),
        (0.0, -1.0, 75.0, false),
    ] {
        for _ in 0..40 {
            s.steer = (ahead, right, false);
            s.update(DT);
        }
        let (t, b, ready) = s.aim_legs_now().expect("moving");
        assert!((t - turn).abs() < 3.0, "{ahead},{right}: {t}");
        assert_eq!(b, back, "{ahead},{right}");
        assert_eq!(ready, "g5r1", "the pistol's stance");
    }
    // Not aiming, the walk is the walk.
    s.set_aim_held(false);
    s.attack_mode_key();
    for _ in 0..40 {
        s.steer = (1.0, 0.0, false);
        s.update(DT);
    }
    assert!(!s.aiming());
    assert_eq!(s.aim_legs_now(), None);
    std::fs::remove_file(&file).ok();
}
