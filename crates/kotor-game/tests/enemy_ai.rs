//! What an enemy does with the talents it has (GAPS row 8).

use kotor_archive::Install;
use kotor_game::{Kind, ObjId, World};

fn world() -> Option<(World, ObjId, ObjId)> {
    let mut base = Install::open(kotor_core::install_dir()?).ok()?;
    base.set_prefer_rim(true);
    let mut w = World::new(base).ok()?;
    w.load_module("end_m01aa").ok()?;
    let spec = w.recommended_character("Player", 0, 1)?;
    w.create_character(&spec).ok()?;
    w.enter_area();
    let pc = w.pc;
    w.object_mut(pc)?.min_one_hp = true;
    let template = w
        .objects
        .values()
        .find(|o| o.kind == Kind::Creature && w.reputation(pc, o.id) <= 10)?
        .template
        .clone();
    let p = w.object(pc)?.position;
    let foe = w.spawn_creature(&template, [p[0] + 4.0, p[1], p[2]]);
    Some((w, pc, foe))
}

#[test]
fn an_enemy_who_knows_a_power_uses_it_in_a_fight() {
    let Some((mut w, pc, foe)) = world() else {
        return;
    };
    let o = w.object_mut(foe).unwrap();
    // Force Push and Force Lightning-class powers: whatever it can pay for that is for an enemy.
    o.spells.extend([23, 20]);
    (o.force, o.max_force) = (200, 200);
    (o.hp, o.max_hp) = (900, 900);
    w.attack(foe, pc);
    for _ in 0..(40.0 / 0.05) as usize {
        w.step(0.05);
    }
    let cast = w
        .events
        .iter()
        .any(|e| e.contains("casts power") && e.contains("Player"));
    let force_left = w.object(foe).unwrap().force;
    println!("force left {force_left}");
    assert!(cast || force_left < 200, "it never used a power");
}

#[test]
fn a_hurt_enemy_with_a_medpac_heals_itself() {
    let Some((mut w, pc, foe)) = world() else {
        return;
    };
    let item = w.spawn_item("g_i_medeqpmnt01");
    w.carry(foe, item);
    let o = w.object_mut(foe).unwrap();
    (o.max_hp, o.hp) = (60, 8);
    w.attack(foe, pc);
    let mut healed = false;
    for _ in 0..(40.0 / 0.05) as usize {
        let before = w.object(foe).unwrap().hp;
        w.step(0.05);
        if w.object(foe).unwrap().hp > before + 5 {
            healed = true;
        }
    }
    let used: Vec<_> = w
        .events
        .iter()
        .filter(|e| e.contains("medpac") || e.contains("UseItem") || e.contains("ActionUse"))
        .take(5)
        .collect();
    println!("{used:?}");
    assert!(healed, "it never healed");
}

fn object_arg(v: u32) -> Vec<u8> {
    let mut b = vec![0x04, 0x06];
    b.extend_from_slice(&v.to_be_bytes());
    b
}

/// Run `ActionMoveAwayFromObject(from, run, range)` as the foe's own script.
fn flee_from(w: &mut World, foe: ObjId, from: ObjId, run: i32, range: f32) {
    let call = |name: &str, argc: u8, w: &World| {
        let mut b = vec![0x05, 0x00];
        b.extend_from_slice(&w.routine_id(name).unwrap().to_be_bytes());
        b.push(argc);
        b
    };
    let mut body = Vec::new();
    body.extend([0x04, 0x04]);
    body.extend_from_slice(&range.to_be_bytes());
    body.extend([0x04, 0x03]);
    body.extend_from_slice(&run.to_be_bytes());
    body.extend(object_arg(from));
    body.extend(call("ActionMoveAwayFromObject", 3, w));
    let mut b = vec![0x1E, 0x00, 0, 0, 0, 8, 0x20, 0x00];
    b.extend(body);
    b.extend([0x20, 0x00]);
    let mut v = b"NCS V1.0B".to_vec();
    v.extend_from_slice(&((13 + b.len()) as u32).to_be_bytes());
    v.extend(b);
    let prog = kotor_script::disassemble(&v).unwrap();
    w.run_program(&prog, foe).unwrap();
}

#[test]
fn a_creature_told_to_move_away_runs_from_the_thing_and_stops_on_the_floors_edge() {
    let Some((mut w, pc, foe)) = world() else {
        return;
    };
    w.interactive_dialogue = true;
    // Friendly, so that nothing but the order moves it.
    w.object_mut(foe).unwrap().faction = 2;
    let start = w.object(foe).unwrap().position;
    let from = w.object(pc).unwrap().position;
    let dist = |w: &World| {
        let p = w.object(foe).unwrap().position;
        (p[0] - from[0]).hypot(p[1] - from[1])
    };
    let before = dist(&w);
    flee_from(&mut w, foe, pc, 1, 40.0);
    for _ in 0..(8.0 / 0.05) as usize {
        w.step(0.05);
    }
    let after = dist(&w);
    assert!(after > before + 1.0, "{before} -> {after}");
    assert!(
        after < before + 12.5,
        "no more than the longest run: {before} -> {after}"
    );
    let end = w.object(foe).unwrap().position;
    assert!(
        w.nav
            .as_ref()
            .unwrap()
            .height_at(end[0], end[1], end[2] + 0.4)
            .is_some(),
        "it ends on the floor, from {start:?} to {end:?}"
    );
}
