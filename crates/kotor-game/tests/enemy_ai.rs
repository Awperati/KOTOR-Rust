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
