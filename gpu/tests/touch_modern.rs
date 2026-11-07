//! Modern combat's on-screen buttons, without a window: Aim held, Attack that fires or strikes, Lock on, and the Force wheel.
use kotor_gpu::stage::Stage;
use kotor_ui::{TouchKey, TouchPart};

const SIZE: (u32, u32) = (1280, 720);
const DT: f32 = 1.0 / 60.0;
const SIZES: [(u32, u32); 5] = [
    (1280, 720),
    (1920, 1080),
    (1024, 768),
    (640, 480),
    (720, 1280),
];

fn stage(name: &str) -> Option<(Stage, std::path::PathBuf)> {
    kotor_core::install_dir()?;
    let mut s = Stage::open("danm14aa", true).unwrap();
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let file = std::env::temp_dir().join(format!("kotor_touchm_{name}_{}.txt", std::process::id()));
    std::fs::write(&file, "").unwrap();
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
    let pc = s.game.world.pc;
    let me = s.game.world.object_mut(pc).unwrap();
    (me.hp, me.max_hp) = (90000, 90000);
    s.toggle_touch();
    assert!(s.touch_on());
    Some((s, file))
}

fn run(s: &mut Stage, frames: usize) {
    for _ in 0..frames {
        s.update(DT);
    }
}

fn at(s: &mut Stage, k: TouchKey) -> Option<(f32, f32)> {
    s.touch_pixel(TouchPart::Key(k), SIZE)
}

fn tap(s: &mut Stage, k: TouchKey) {
    let p = at(s, k).expect("the button");
    s.finger_down(1, p, SIZE);
    s.finger_up(1, p, SIZE);
}

/// A foe of the module's, `ahead` metres in front of the view, that stands there.
fn foe(s: &mut Stage, ahead: f32) -> u32 {
    let at = s.game.world.pc_mover.pos;
    let pc = s.game.world.pc;
    let way = s.yaw;
    let w = &mut s.game.world;
    let id = w
        .objects
        .values()
        .find(|o| {
            o.kind == kotor_game::Kind::Creature
                && o.id != pc
                && !o.dead
                && !o.party_member
                && o.hp < 1000
        })
        .map(|o| o.id)
        .expect("a creature");
    w.teleport(
        id,
        [at[0] + way.cos() * ahead, at[1] + way.sin() * ahead, at[2]],
    );
    let f = w.object_mut(id).unwrap();
    (f.hp, f.max_hp, f.faction) = (90000, 90000, 1);
    id
}

fn mine_on(s: &Stage, id: u32, from: usize) -> usize {
    let w = &s.game.world;
    w.combat.log[from..]
        .iter()
        .filter(|l| l.attacker == w.pc && l.target == id)
        .count()
}

#[test]
fn modern_adds_aim_lock_and_force_and_they_stay_off_each_other_at_every_size() {
    let Some((mut s, file)) = stage("places") else {
        return;
    };
    for k in [TouchKey::Aim, TouchKey::Lock, TouchKey::Force] {
        assert!(at(&mut s, k).is_none(), "Classic has no {k:?}");
    }
    s.set_combat_modern(true);
    let pc = s.game.world.pc;
    let o = s.game.world.object_mut(pc).unwrap();
    o.spells.clear();
    o.spells.extend([10, 17, 50, 4, 8]);
    let over = |a: [i32; 4], b: [i32; 4]| {
        a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
    };
    for size in SIZES {
        s.set_view(size);
        let width = s.ui_layout_width() as i32;
        let (across, down) = (size.0 as f32 / width as f32, size.1 as f32 / 480.0);
        for wheel in [false, true] {
            let open = s.touch_key_word(TouchKey::Force).1;
            if open != wheel {
                let p = s
                    .touch_pixel(TouchPart::Key(TouchKey::Force), size)
                    .unwrap();
                s.finger_down(1, p, size);
                s.finger_up(1, p, size);
            }
            let (layer, hud) = s.touch_boxes().unwrap();
            assert_eq!(layer.len(), 12, "{size:?} wheel {wheel}");
            for (n, a) in layer.iter().enumerate() {
                assert!(
                    a[0] >= 0 && a[1] >= 0 && a[0] + a[2] <= width && a[1] + a[3] <= 480,
                    "{size:?}: {a:?} is off the window"
                );
                let (wide, high) = (a[2] as f32 * across, a[3] as f32 * down);
                assert!(
                    wide >= 44.0 && high >= 44.0,
                    "{size:?}: {a:?} is {wide} by {high}"
                );
                for b in &layer[n + 1..] {
                    assert!(!over(*a, *b), "{size:?}: {a:?} and {b:?}");
                }
                for (tag, h) in &hud {
                    assert!(!over(*a, *h), "{size:?}: {a:?} is on {tag} {h:?}");
                }
            }
        }
    }
    s.set_view(SIZE);
    std::fs::remove_file(&file).ok();
}

#[test]
fn aim_held_aims_and_attack_held_fires_at_the_weapon_s_pace() {
    let Some((mut s, file)) = stage("fire") else {
        return;
    };
    s.set_combat_modern(true);
    let id = foe(&mut s, 6.0);
    let pc = s.game.world.pc;
    let w = &mut s.game.world;
    let gun = w.spawn_item("g_w_blstrpstl001");
    w.equip(pc, gun, Some(4)).unwrap();
    run(&mut s, 2);
    assert_eq!(s.touch_key_word(TouchKey::Attack).0, "Attack");
    let aim = at(&mut s, TouchKey::Aim).unwrap();
    s.finger_down(1, aim, SIZE);
    run(&mut s, 20);
    assert!(s.aiming(), "held, it aims");
    assert_eq!(s.touch_key_word(TouchKey::Attack).0, "Fire");
    // The camera's ray passes the shoulder: turn it onto the foe.
    for _ in 0..200 {
        let c = s.camera();
        let to = s.game.world.object(id).unwrap().position;
        let want = (to[1] - c.eye[1]).atan2(to[0] - c.eye[0]);
        let pi = std::f32::consts::PI;
        s.yaw += ((want - s.yaw + pi).rem_euclid(2.0 * pi) - pi) * 0.5;
        s.update(DT);
    }
    assert!(s.aim_hot(), "on the foe");
    // A second finger holds Attack for two seconds: a shot each half second.
    let logged = s.game.world.combat.log.len();
    let fire = at(&mut s, TouchKey::Attack).unwrap();
    s.finger_down(2, fire, SIZE);
    run(&mut s, 120);
    s.finger_up(2, fire, SIZE);
    let shots = mine_on(&s, id, logged);
    assert!((3..=5).contains(&shots), "{shots} shots in 2 s");
    run(&mut s, 60);
    assert_eq!(mine_on(&s, id, logged), shots, "let go, it stops");
    s.finger_up(1, aim, SIZE);
    run(&mut s, 2);
    assert!(!s.aiming(), "let go, it does not aim");
    std::fs::remove_file(&file).ok();
}

#[test]
fn with_a_blade_attack_held_chains_blows_and_aim_is_the_guard() {
    let Some((mut s, file)) = stage("blade") else {
        return;
    };
    s.set_combat_modern(true);
    let id = foe(&mut s, 1.5);
    let pc = s.game.world.pc;
    let sabre = s.game.world.spawn_item("g_w_lghtsbr01");
    // The character has not the feat for one: this is not about that.
    let me = s.game.world.object_mut(pc).unwrap();
    me.equipped.insert(4, sabre);
    s.attack_mode_key();
    run(&mut s, 2);
    assert_eq!(s.touch_key_word(TouchKey::Aim).0, "Guard");
    assert_eq!(s.touch_key_word(TouchKey::Attack).0, "Strike");
    let logged = s.game.world.combat.log.len();
    let fire = at(&mut s, TouchKey::Attack).unwrap();
    s.finger_down(2, fire, SIZE);
    let mut clips = Vec::new();
    for _ in 0..150 {
        s.update(DT);
        let name = s
            .game
            .clip(pc)
            .map(|c| c.name.to_string())
            .unwrap_or_default();
        if name.starts_with("c2a") && clips.last() != Some(&name) {
            clips.push(name);
        }
    }
    s.finger_up(2, fire, SIZE);
    assert!(clips.len() >= 4, "a chain: {clips:?}");
    assert!(mine_on(&s, id, logged) >= 4);
    std::fs::remove_file(&file).ok();
}

#[test]
fn lock_on_locks_and_lets_go_and_is_lit_while_it_holds() {
    let Some((mut s, file)) = stage("lock") else {
        return;
    };
    s.set_combat_modern(true);
    let id = foe(&mut s, 5.0);
    run(&mut s, 2);
    tap(&mut s, TouchKey::Lock);
    assert_eq!(s.locked_on(), Some(id));
    assert!(s.touch_key_word(TouchKey::Lock).1, "lit");
    tap(&mut s, TouchKey::Lock);
    assert_eq!(s.locked_on(), None);
    assert!(!s.touch_key_word(TouchKey::Lock).1);
    std::fs::remove_file(&file).ok();
}

#[test]
fn force_opens_a_wheel_of_the_first_powers_known_and_one_casts_and_shuts_it() {
    let Some((mut s, file)) = stage("wheel") else {
        return;
    };
    s.set_combat_modern(true);
    let id = foe(&mut s, 6.0);
    let pc = s.game.world.pc;
    let o = s.game.world.object_mut(pc).unwrap();
    (o.force, o.max_force) = (1000, 1000);
    o.spells.clear();
    o.spells.extend([50, 10, 17]);
    run(&mut s, 2);
    s.lock_key();
    assert_eq!(s.locked_on(), Some(id));
    assert!(at(&mut s, TouchKey::Power(0)).is_none(), "shut");
    tap(&mut s, TouchKey::Force);
    assert!(s.touch_key_word(TouchKey::Force).1, "lit while open");
    // In the Force page's order, and only those known.
    for n in 0..3 {
        assert!(!s.touch_key_word(TouchKey::Power(n)).0.is_empty(), "{n}");
    }
    assert!(at(&mut s, TouchKey::Power(3)).is_none());
    assert!(
        at(&mut s, TouchKey::Jump).is_none(),
        "the wheel is in its place"
    );
    // Force Wound, the third, goes to the lock.
    s.game.world.last_power.remove(&pc);
    tap(&mut s, TouchKey::Power(2));
    assert_eq!(s.game.world.last_power.get(&pc).map(|p| p.0), Some(50));
    assert!(at(&mut s, TouchKey::Jump).is_some(), "shut again");
    assert!(!s.touch_key_word(TouchKey::Force).1);
    // Too few Force points: greyed.
    s.game.world.object_mut(pc).unwrap().force = 0;
    tap(&mut s, TouchKey::Force);
    assert!(s.touch_key_greyed(TouchKey::Power(2)));
    // Classic: none of it.
    s.set_combat_modern(false);
    assert!(at(&mut s, TouchKey::Power(0)).is_none());
    assert!(at(&mut s, TouchKey::Force).is_none());
    std::fs::remove_file(&file).ok();
}
