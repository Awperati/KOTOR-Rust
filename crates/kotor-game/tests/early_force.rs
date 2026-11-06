//! `docs/EARLYFORCE.md`: the owner's early Force rules, and that with them off nothing has changed.

use kotor_archive::Install;
use kotor_game::{CombatAction, World, FORCE_PUSH};

fn world(early: bool, wisdom: i32) -> Option<World> {
    let mut inst = Install::open(kotor_core::install_dir()?).ok()?;
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).ok()?;
    w.load_module("end_m01aa").ok()?;
    w.early_force = early;
    let mut spec = w.recommended_character("Player", 0, 1)?;
    spec.abilities[4] = wisdom;
    // The points the raised Wisdom needs come off Strength, however the sums fall.
    spec.abilities[0] = 8;
    w.create_character(&spec).unwrap();
    Some(w)
}

#[test]
fn with_the_rule_off_a_soldier_has_no_force_and_no_powers() {
    let Some(mut w) = world(false, 10) else {
        return;
    };
    let pc = w.pc;
    let o = w.object(pc).unwrap();
    assert_eq!((o.force, o.max_force), (0, 0));
    assert!(o.spells.is_empty());
    assert_eq!(w.power_slots_for(pc, 2), 0);
    assert_eq!(w.level_up(pc).unwrap().force, 0);
    assert!(w.combat_powers(pc).is_empty());
    let o = w.object_mut(pc).unwrap();
    o.force = 0;
    for _ in 0..600 {
        w.step(1.0 / 60.0);
    }
    assert_eq!(w.object(pc).unwrap().force, 0, "nothing comes back");
}

#[test]
fn force_push_is_in_the_powers_list_and_in_no_other_slot() {
    let Some(mut w) = world(true, 10) else {
        return;
    };
    let pc = w.pc;
    let power = CombatAction::Power(FORCE_PUSH);
    assert!(w.combat_powers(pc).iter().any(|c| c.action == power));
    assert!(w.combat_feats(pc).iter().all(|c| c.action != power));
    assert!(w
        .combat_powers(pc)
        .iter()
        .all(|c| matches!(c.action, CombatAction::Power(_))));
    assert!(w.combat_throwables(pc).is_empty());
}

#[test]
fn the_pool_starts_small_and_grows_with_level_and_wisdom() {
    let Some(mut w) = world(true, 10) else {
        return;
    };
    let pc = w.pc;
    assert_eq!(w.object(pc).unwrap().max_force, 12);
    assert_eq!(w.level_up(pc).unwrap().force, 2);
    assert_eq!(w.object(pc).unwrap().max_force, 14);
    let Some(mut wise) = world(true, 14) else {
        return;
    };
    let pc = wise.pc;
    assert_eq!(wise.object(pc).unwrap().max_force, 14, "Wisdom 14 adds 2");
    assert_eq!(wise.level_up(pc).unwrap().force, 4);
}

#[test]
fn force_push_is_known_costs_what_the_table_says_and_the_force_comes_back_after() {
    let Some(mut w) = world(true, 10) else {
        return;
    };
    let pc = w.pc;
    assert!(w.object(pc).unwrap().spells.contains(&FORCE_PUSH));
    let foe = w
        .objects
        .values()
        .find(|o| o.template == "n_sithsoldier003")
        .unwrap()
        .id;
    let at = w.object(pc).unwrap().position;
    w.teleport(foe, [at[0] + 2.0, at[1], at[2]]);
    let slot = w.combat_powers(pc);
    assert_eq!(slot[0].action, CombatAction::Power(FORCE_PUSH));
    assert!(slot[0].usable);
    assert_eq!(slot[0].icon, "ip_push");
    w.cast_spell(pc, FORCE_PUSH, foe, false).unwrap();
    assert_eq!(w.object(pc).unwrap().force, 2, "12 less 10");
    // Too dear now, and still on the slot to be greyed.
    let slot = w.combat_powers(pc);
    assert!(!slot[0].usable);
    assert!(w.cast_spell(pc, FORCE_PUSH, foe, false).is_err());
    // Nothing comes back during a fight.
    let o = w.object_mut(foe).unwrap();
    o.dead = true;
    o.hp = 0;
    for _ in 0..(6 * 60 + 30) {
        w.step(1.0 / 60.0);
    }
    assert_eq!(w.object(pc).unwrap().force, 5, "3 points in 6 s");
    assert!(!w.combat_powers(pc).is_empty());
}

#[test]
fn a_power_comes_at_every_second_level_from_the_normal_list() {
    let Some(mut w) = world(true, 10) else {
        return;
    };
    let pc = w.pc;
    assert_eq!(w.power_slots_for(pc, 2), 1);
    assert_eq!(w.power_slots_for(pc, 3), 0);
    assert_eq!(w.power_slots_for(pc, 4), 1);
    let choices: Vec<_> = w.power_choices(pc).into_iter().flatten().collect();
    assert!(choices.len() > 20, "{} powers", choices.len());
    let push = choices.iter().find(|c| c.id == FORCE_PUSH).unwrap();
    assert!(push.owned && !push.can_take);
    let next = w.recommended_power(pc).expect("something to take");
    assert_ne!(next, FORCE_PUSH);
    assert!(w.can_take_power(pc, next));
}

#[test]
fn becoming_a_jedi_carries_the_powers_over_and_pays_nothing_twice() {
    let Some(mut w) = world(true, 10) else {
        return;
    };
    let pc = w.pc;
    w.level_up(pc).unwrap();
    assert_eq!(w.object(pc).unwrap().max_force, 14);
    let p = script_add_class(&w, 4);
    w.run_program(&p, pc).unwrap();
    let o = w.object(pc).unwrap();
    assert_eq!(o.class, 4);
    assert!(o.spells.contains(&FORCE_PUSH));
    assert_eq!(
        o.max_force, 14,
        "more than the first level's 8, so unchanged"
    );
    assert!(!w.is_early(pc));
    // The Jedi class's own table from here: a Consular's die pays 5.
    assert_eq!(w.level_up(pc).unwrap().force, 5);
    assert_eq!(w.power_slots_for(pc, 3), w.power_slots_at(4, 3));
}

#[test]
fn with_the_rule_off_add_multi_class_pays_the_jedi_die() {
    let Some(mut w) = world(false, 10) else {
        return;
    };
    let pc = w.pc;
    let p = script_add_class(&w, 4);
    w.run_program(&p, pc).unwrap();
    let o = w.object(pc).unwrap();
    assert_eq!((o.class, o.max_force), (4, 8));
}

/// `AddMultiClass(class, OBJECT_SELF)`.
fn script_add_class(w: &World, class: i32) -> kotor_script::Program {
    let id = w.routine_id("AddMultiClass").unwrap();
    let mut b = vec![0x1E, 0x00, 0, 0, 0, 8, 0x20, 0x00];
    b.extend([0x04, 0x06]);
    b.extend(0i32.to_be_bytes());
    b.extend([0x04, 0x03]);
    b.extend(class.to_be_bytes());
    b.extend([0x05, 0x00]);
    b.extend(id.to_be_bytes());
    b.push(2);
    b.extend([0x20, 0x00]);
    let mut v = b"NCS V1.0B".to_vec();
    v.extend_from_slice(&((13 + b.len()) as u32).to_be_bytes());
    v.extend(b);
    kotor_script::disassemble(&v).unwrap()
}

#[test]
fn the_setting_stays_with_the_save() {
    let Some(on) = world(true, 10) else {
        return;
    };
    let saved = on.save_bytes("t").unwrap();
    let Some(mut other) = world(false, 10) else {
        return;
    };
    other.load_save(&saved).unwrap();
    assert!(other.early_force);
    let Some(off) = world(false, 10) else {
        return;
    };
    let saved = off.save_bytes("t").unwrap();
    let Some(mut other) = world(true, 10) else {
        return;
    };
    other.load_save(&saved).unwrap();
    assert!(!other.early_force, "a game begun Off stays Off");
}
