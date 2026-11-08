//! Damage types meet resistance, reduction and immunity (GAPS row 9, my share).

use kotor_archive::Install;
use kotor_game::{EffectRec, World, DURATION_INSTANT, DURATION_PERMANENT};
use kotor_script::disassemble;

const ELECTRICAL: f32 = 128.0;
const FIRE: f32 = 256.0;
const UNIVERSAL: f32 = 8.0;

fn world() -> Option<(World, u32, u32)> {
    let mut base = Install::open(kotor_core::install_dir()?).ok()?;
    base.set_prefer_rim(true);
    let mut w = World::new(base).ok()?;
    w.load_module("end_m01aa").ok()?;
    let spec = w.recommended_character("Player", 0, 1)?;
    w.create_character(&spec).ok()?;
    let p = w.object(w.pc)?.position;
    let foe = w.spawn_creature("n_calonord", [p[0] + 2.0, p[1], p[2]]);
    let o = w.object_mut(foe)?;
    (o.hp, o.max_hp, o.plot, o.min_one_hp) = (900, 900, false, false);
    // Calo Nord's own armour resists damage now: these tests are about the effects, so he wears nothing.
    o.equipped.clear();
    let pc = w.pc;
    Some((w, pc, foe))
}

fn put(w: &mut World, who: u32, name: &str, nums: &[f32]) {
    let mut e = EffectRec::new(name);
    e.nums = nums.to_vec();
    w.effects.push(e);
    let h = w.effects.len() as u32;
    w.apply_effect(who, who, h, DURATION_PERMANENT, 0.0);
}

fn hit(w: &mut World, from: u32, to: u32, amount: f32, kind: f32) -> i32 {
    let before = w.object(to).unwrap().hp;
    let mut e = EffectRec::new("EffectDamage");
    e.nums = vec![amount, kind, 0.0];
    w.effects.push(e);
    let h = w.effects.len() as u32;
    w.apply_effect(from, to, h, DURATION_INSTANT, 0.0);
    before - w.object(to).unwrap().hp
}

#[test]
fn resistance_takes_a_flat_amount_off_the_matching_type_only() {
    let Some((mut w, pc, foe)) = world() else {
        return;
    };
    assert_eq!(hit(&mut w, pc, foe, 20.0, ELECTRICAL), 20);
    put(
        &mut w,
        foe,
        "EffectDamageResistance",
        &[ELECTRICAL, 8.0, 0.0],
    );
    assert_eq!(hit(&mut w, pc, foe, 20.0, ELECTRICAL), 12);
    assert_eq!(hit(&mut w, pc, foe, 20.0, FIRE), 20, "another type");
    assert_eq!(
        hit(&mut w, pc, foe, 20.0, UNIVERSAL),
        20,
        "universal passes"
    );
    assert_eq!(
        hit(&mut w, pc, foe, 5.0, ELECTRICAL),
        0,
        "never below nothing"
    );
}

#[test]
fn immunity_is_a_percentage_that_can_be_lowered_again() {
    let Some((mut w, pc, foe)) = world() else {
        return;
    };
    put(&mut w, foe, "EffectDamageImmunityIncrease", &[FIRE, 50.0]);
    assert_eq!(hit(&mut w, pc, foe, 20.0, FIRE), 10);
    put(&mut w, foe, "EffectDamageImmunityDecrease", &[FIRE, 25.0]);
    assert_eq!(hit(&mut w, pc, foe, 20.0, FIRE), 15);
    put(&mut w, foe, "EffectDamageImmunityIncrease", &[FIRE, 100.0]);
    assert_eq!(hit(&mut w, pc, foe, 20.0, FIRE), 0, "fully immune");
}

#[test]
fn reduction_takes_something_off_every_blow() {
    let Some((mut w, pc, foe)) = world() else {
        return;
    };
    put(&mut w, foe, "EffectDamageReduction", &[3.0, 0.0, 0.0]);
    assert_eq!(hit(&mut w, pc, foe, 10.0, FIRE), 7);
}

#[test]
fn an_immune_creature_shrugs_off_the_effect_it_is_immune_to() {
    let Some((mut w, pc, foe)) = world() else {
        return;
    };
    const STUN: f32 = 12.0;
    let stun = |w: &mut World| {
        let mut e = EffectRec::new("EffectStunned");
        e.nums = vec![];
        w.effects.push(e);
        let h = w.effects.len() as u32;
        w.apply_effect(pc, foe, h, DURATION_PERMANENT, 0.0);
        h
    };
    let h = {
        let mut e = EffectRec::new("EffectStunned");
        w.effects.push(e.clone());
        e.nums.clear();
        w.effects.len() as u32
    };
    assert!(!w.is_link_immune(foe, h));
    stun(&mut w);
    assert!(w.is_disabled(foe), "an unprotected creature is stunned");
    w.object_mut(foe).unwrap().effects.clear();
    put(&mut w, foe, "EffectImmunity", &[STUN]);
    assert!(w.is_link_immune(foe, h));
    stun(&mut w);
    assert!(!w.is_disabled(foe), "stun immunity stops the stun");
    let mut e = EffectRec::new("EffectParalyze");
    e.nums = vec![];
    w.effects.push(e);
    let p = w.effects.len() as u32;
    w.apply_effect(pc, foe, p, DURATION_PERMANENT, 0.0);
    assert!(w.is_disabled(foe), "stun immunity does not stop paralysis");
}

fn const_obj(v: u32) -> Vec<u8> {
    let mut b = vec![0x04, 0x06];
    b.extend_from_slice(&v.to_be_bytes());
    b
}

/// `SetGlobalNumber("RESISTED", ResistForce(caster, target))`, run as a script.
fn resist_force(w: &mut World, caster: u32, target: u32) -> i32 {
    let call = |name: &str, argc: u8, w: &World| {
        let mut b = vec![0x05, 0x00];
        b.extend_from_slice(&w.routine_id(name).unwrap().to_be_bytes());
        b.push(argc);
        b
    };
    let mut body = Vec::new();
    body.extend(const_obj(target));
    body.extend(const_obj(caster));
    body.extend(call("ResistForce", 2, w));
    body.extend([0x04, 0x05, 0, 8]);
    body.extend(b"RESISTED");
    body.extend(call("SetGlobalNumber", 2, w));
    let mut b = vec![0x1E, 0x00, 0, 0, 0, 8, 0x20, 0x00];
    b.extend(body);
    b.extend([0x20, 0x00]);
    let mut v = b"NCS V1.0B".to_vec();
    v.extend_from_slice(&((13 + b.len()) as u32).to_be_bytes());
    v.extend(b);
    let prog = disassemble(&v).unwrap();
    w.run_program(&prog, caster).unwrap();
    w.global_numbers["RESISTED"]
}

#[test]
fn force_resistance_turns_a_power_aside_by_its_rating_against_the_casters_roll() {
    let Some((mut w, pc, foe)) = world() else {
        return;
    };
    assert_eq!(
        resist_force(&mut w, pc, foe),
        0,
        "no resistance, no resisting"
    );
    // 100 beats a d20 plus a first-level caster every time, 0 beats none.
    put(&mut w, foe, "EffectForceResistanceIncrease", &[100.0]);
    for _ in 0..20 {
        assert_eq!(resist_force(&mut w, pc, foe), 1);
    }
    // A decrease of the same size takes it away again.
    put(&mut w, foe, "EffectForceResistanceDecrease", &[100.0]);
    for _ in 0..20 {
        assert_eq!(resist_force(&mut w, pc, foe), 0);
    }
    // Between: 15 against d20 + 1 turns aside about 13 in 20.
    put(&mut w, foe, "EffectForceResistanceIncrease", &[15.0]);
    let turned = (0..400)
        .filter(|_| resist_force(&mut w, pc, foe) == 1)
        .count();
    assert!((190..330).contains(&turned), "{turned} of 400");
}
