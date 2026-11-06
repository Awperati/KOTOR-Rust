//! What the properties in the game's own item files do.

use kotor_archive::Install;
use kotor_game::{CannotEquip, World};

const RIGHT_HAND: i32 = 4;
const BODY: i32 = 1;
const IMPLANT: i32 = 9;
const DEX: usize = 1;

fn world() -> Option<World> {
    let dir = kotor_core::install_dir()?;
    let mut w = World::new(Install::open(dir).unwrap()).unwrap();
    w.load_module("end_m01aa").unwrap();
    Some(w)
}

/// Puts something on without asking whether they may.
fn wear(w: &mut World, slot: i32, template: &str) -> u32 {
    let (pc, item) = (w.pc, w.spawn_item(template));
    w.object_mut(pc).unwrap().equipped.insert(slot, item);
    item
}

#[test]
fn a_weapons_enhancement_adds_to_attack_and_to_damage() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    // A plain pistol, and Bendak's, which has an enhancement of 1 and is the same kind.
    wear(&mut w, RIGHT_HAND, "g_w_blstrpstl001");
    let (attack, range) = (
        w.attack_bonus_of(pc, RIGHT_HAND),
        w.damage_range_of(pc, RIGHT_HAND).unwrap(),
    );
    wear(&mut w, RIGHT_HAND, "g_w_blstrpstl005");
    assert_eq!(w.attack_bonus_of(pc, RIGHT_HAND), attack + 1);
    let better = w.damage_range_of(pc, RIGHT_HAND).unwrap();
    assert_eq!(better, (range.0 + 1, range.1 + 1));
}

#[test]
fn an_upgrade_wakes_what_waits_for_it_and_taking_it_out_puts_it_back_to_sleep() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    // Carth's Blaster: a scope, row 16, adds 1 to attack, and an energy cell, row 17, 1 to damage.
    let gun = wear(&mut w, RIGHT_HAND, "g_w_blstrpstl010");
    let (attack, range) = (
        w.attack_bonus_of(pc, RIGHT_HAND),
        w.damage_range_of(pc, RIGHT_HAND).unwrap(),
    );
    let scope = w.spawn_item("g_i_upgrade001");
    w.carry(pc, scope);
    assert!(w.install_upgrade(pc, gun, 0, scope));
    assert_eq!(w.attack_bonus_of(pc, RIGHT_HAND), attack + 1);
    assert_eq!(w.damage_range_of(pc, RIGHT_HAND).unwrap(), range);
    let cell = w.spawn_item("g_i_upgrade002");
    w.carry(pc, cell);
    assert!(w.install_upgrade(pc, gun, 1, cell));
    assert_eq!(
        w.damage_range_of(pc, RIGHT_HAND).unwrap(),
        (range.0 + 1, range.1 + 1)
    );

    // It comes back with a save.
    let bytes = w.save_bytes("gear").unwrap();
    let mut again = World::new(Install::open(kotor_core::install_dir().unwrap()).unwrap()).unwrap();
    again.load_save(&bytes).unwrap();
    let them = again.pc;
    assert_eq!(again.attack_bonus_of(them, RIGHT_HAND), attack + 1);

    w.remove_upgrade(pc, gun, 0);
    assert_eq!(w.attack_bonus_of(pc, RIGHT_HAND), attack);
}

#[test]
fn armour_and_what_it_waits_for_add_to_defense() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    // Republic Mod Armor: reinforcement, row 20, adds 1.
    let armour = wear(&mut w, BODY, "g_a_class5010");
    let defense = w.defense_of(pc);
    let plate = w.spawn_item("g_i_upgrade005");
    w.carry(pc, plate);
    assert!(w.install_upgrade(pc, armour, 0, plate));
    assert_eq!(w.defense_of(pc), defense + 1);
}

#[test]
fn what_is_worn_adds_to_abilities_skills_and_saves() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    let dex = w.ability_score(pc, DEX);
    wear(&mut w, BODY, "g1_a_class5001");
    assert_eq!(w.ability_score(pc, DEX), dex + 1);
    let skill = w.skill_modifier(pc, 2);
    wear(&mut w, BODY, "g1_a_class5002");
    assert_eq!(w.skill_modifier(pc, 2), skill + 4);

    let will = w.save_bonus(pc, kotor_game::Save::Will);
    wear(&mut w, IMPLANT, "g1_i_mask01");
    assert_eq!(w.save_bonus(pc, kotor_game::Save::Will), will + 1);
}

#[test]
fn keen_doubles_the_threat_range_once_its_upgrade_is_in() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    // Zaalbar's Bowcaster: a hair trigger, row 19, makes it keen.
    let bow = wear(&mut w, RIGHT_HAND, "g_w_bowcstr003");
    assert_eq!(w.threat_factor(pc, RIGHT_HAND), 1);
    let trigger = w.spawn_item("g_i_upgrade004");
    w.carry(pc, trigger);
    assert!(w.install_upgrade(pc, bow, 3, trigger));
    assert_eq!(w.threat_factor(pc, RIGHT_HAND), 2);
}

#[test]
fn a_robe_of_one_side_is_only_for_that_side() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    // The Jedi robe with a light side limit, in the game's words at 1512.
    let robe = w.spawn_item("g_a_jedirobe06");
    w.object_mut(pc).unwrap().feats.extend(0..300);
    w.object_mut(pc).unwrap().alignment = 50;
    assert_eq!(w.can_wear(pc, robe), Err(CannotEquip::Alignment));
    w.object_mut(pc).unwrap().alignment = 90;
    assert_ne!(w.can_wear(pc, robe), Err(CannotEquip::Alignment));
}
