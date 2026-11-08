//! What worn items do beyond numbers: resist and ignore damage, be immune, deflect bolts, regenerate, give feats and force
//! resistance, do something on a hit, and cap the Dexterity bonus (`docs/GEAR_CHECK.md`).

use kotor_archive::Install;
use kotor_game::{World, FORCE_PUSH};

const SONIC: i32 = 1 << 10;
const COLD: i32 = 1 << 5;
const SLOT_BODY: i32 = 1;
const SLOT_IMPLANT: i32 = 9;
const IMMUNITY_MIND: i32 = 1;
const IMMUNITY_CRITICAL: i32 = 31;
const DEX: usize = 1;

fn world() -> Option<World> {
    let dir = kotor_core::install_dir()?;
    let mut w = World::new(Install::open(dir).unwrap()).unwrap();
    w.load_module("end_m01aa").unwrap();
    Some(w)
}

fn wear(w: &mut World, slot: i32, template: &str) -> u32 {
    let (pc, item) = (w.pc, w.spawn_item(template));
    w.object_mut(pc).unwrap().equipped.insert(slot, item);
    item
}

#[test]
fn damage_resistance_takes_a_flat_amount_off_its_type_only() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    assert_eq!(w.adjust_damage(pc, 40, SONIC), 40);
    // Cinnagar Plate Armor: sonic resistance.
    wear(&mut w, SLOT_BODY, "g_a_class8003");
    assert!(w.gear_damage_resist(pc, SONIC) > 0);
    assert!(w.adjust_damage(pc, 40, SONIC) < 40, "sonic is resisted");
    assert_eq!(w.adjust_damage(pc, 40, COLD), 40, "cold is not");
    // A chain's small blow loses a share of the amount, not all of it.
    let whole = w.adjust_damage(pc, 40, SONIC);
    let blow = w.adjust_damage_share(pc, 10, SONIC, 0.25);
    assert!(blow > 0, "a quarter blow of 10 is not swallowed: {blow}");
    assert!((40 - whole - (10 - blow) * 4).abs() <= 4, "{whole} {blow}");
}

#[test]
fn damage_immunity_ignores_a_percentage_of_its_type() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    // Environmental Bastion Armor: immune to cold, fire and sonic damage.
    wear(&mut w, SLOT_BODY, "g1_a_class6001");
    assert!(w.gear_damage_immunity(pc, SONIC) > 0);
    assert!(w.adjust_damage(pc, 40, SONIC) < 40);
    assert_eq!(w.adjust_damage(pc, 40, 1 << 7), 40, "electrical is not");
}

#[test]
fn immunity_stops_the_effect_it_names() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    assert!(!w.is_immune(pc, IMMUNITY_CRITICAL));
    // Jamoh Hogra's Battle Armor: critical hits. The Sith Mask: mind spells.
    wear(&mut w, SLOT_BODY, "g_a_class7006");
    assert!(w.is_immune(pc, IMMUNITY_CRITICAL));
    assert!(!w.is_immune(pc, IMMUNITY_MIND));
    wear(&mut w, 0, "g_i_mask14");
    assert!(w.is_immune(pc, IMMUNITY_MIND));
}

#[test]
fn stabilizer_gloves_turn_aside_some_bolts() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    assert_eq!(w.gear_deflect_percent(pc), 0);
    wear(&mut w, 3, "g1_i_gauntlet01");
    let turned = w.gear_deflect_percent(pc);
    assert!((1..=75).contains(&turned), "{turned}");
}

#[test]
fn a_regenerator_gives_back_hit_points_and_a_mask_force_points() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    {
        let o = w.object_mut(pc).unwrap();
        (o.max_hp, o.hp, o.max_force, o.force) = (100, 50, 50, 10);
    }
    wear(&mut w, SLOT_IMPLANT, "g_i_implant308");
    wear(&mut w, 0, "g_i_mask14");
    let (hp, fp) = w.gear_regeneration(pc);
    assert!(hp > 0 && fp > 0, "{hp} {fp}");
    for _ in 0..70 {
        w.step(0.1);
    }
    let o = w.object(pc).unwrap();
    assert!(o.hp > 50 && o.force > 10, "{} {}", o.hp, o.force);
}

#[test]
fn bonus_feats_are_known_while_the_item_is_worn_and_not_after() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    // The Sith Mask gives feat 36.
    assert!(!w.object(pc).unwrap().feats.contains(&36));
    wear(&mut w, 0, "g_i_mask14");
    w.step(0.1);
    assert!(w.object(pc).unwrap().feats.contains(&36), "worn");
    w.object_mut(pc).unwrap().equipped.remove(&0);
    w.step(0.1);
    assert!(!w.object(pc).unwrap().feats.contains(&36), "taken off");
}

#[test]
fn hide_adds_force_resistance() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    assert_eq!(w.gear_magic_resist(pc), 0);
    // A creature hide item (Tuk'ata Hide) worn in the body slot.
    wear(&mut w, 11, "g_i_crhide004");
    assert!(w.gear_magic_resist(pc) > 0);
    let _ = FORCE_PUSH;
}

#[test]
fn a_weapon_with_a_rider_can_stun_whoever_it_hits() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    // The Sith Assassin Pistol stuns.
    wear(&mut w, 4, "g_w_hldoblstr03");
    let foe = w.spawn_creature("end_trask", [0.0, 0.0, 0.0]);
    w.object_mut(foe).unwrap().faction = 1;
    let mut stunned = false;
    for _ in 0..60 {
        w.gear_on_hit(pc, foe, 4);
        if w.is_disabled(foe) {
            stunned = true;
            break;
        }
    }
    assert!(stunned, "some hit stuns");
}

#[test]
fn heavy_armour_caps_the_dexterity_bonus() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    w.object_mut(pc).unwrap().abilities[DEX] = 20;
    let bare = w.defense_of(pc);
    // Heavy Battle Armor, class 9, base armour 9, no Dexterity bonus allowed.
    wear(&mut w, SLOT_BODY, "g_a_class9001");
    assert_eq!(
        w.defense_of(pc),
        19 + (bare - 10 - 5),
        "Dex 20 gives nothing"
    );
    // Light armour allows +5 of it.
    wear(&mut w, SLOT_BODY, "g1_a_class5001");
    assert!(w.defense_of(pc) >= 10 + 5 + 4, "{}", w.defense_of(pc));
}
