//! Medication Sickness: a medical item used inside the cooldown of the last one heals less, and makes the user's aim and
//! damage worse for as long as the cooldown runs. The numbers are in docs/medication.tsv.

use kotor_archive::Install;
use kotor_game::{ObjId, Sickness, UseRequest, World};

fn world() -> Option<World> {
    let dir = kotor_core::install_dir()?;
    let mut base = Install::open(dir).ok()?;
    base.set_prefer_rim(true);
    let mut w = World::new(base).ok()?;
    w.load_module("end_m01aa").ok()?;
    let spec = w.recommended_character("Player", 0, 1)?;
    w.create_character(&spec).ok()?;
    let pc = w.pc;
    // The ship's own fights are not wanted here.
    let others: Vec<ObjId> = w
        .objects
        .values()
        .filter(|o| o.kind == kotor_game::Kind::Creature && o.id != pc)
        .map(|o| o.id)
        .collect();
    for id in others {
        w.object_mut(id).unwrap().dead = true;
    }
    w.combat.target.clear();
    Some(w)
}

fn pack(w: &mut World) -> ObjId {
    let (pc, item) = (w.pc, w.spawn_item("g_i_medeqpmnt01"));
    w.carry(pc, item);
    item
}

/// Hit points a use gives from 1 hit point.
fn heal_of_one_use(w: &mut World, item: ObjId) -> i32 {
    let pc = w.pc;
    {
        let o = w.object_mut(pc).unwrap();
        o.max_hp = 900;
        o.hp = 1;
    }
    w.use_item(pc, item).unwrap();
    w.object(pc).unwrap().hp - 1
}

fn run(w: &mut World, seconds: f32) {
    for _ in 0..(seconds / 0.05) as usize {
        w.step(0.05);
    }
}

#[test]
fn a_medpack_used_inside_the_cooldown_heals_half_and_then_a_quarter() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    let item = pack(&mut w);
    for _ in 0..4 {
        pack(&mut w);
    }
    assert_eq!(w.medication_cooldown(pc), 0.0);
    assert_eq!(w.use_consumable_request(pc, item), UseRequest::Ready);
    let full = heal_of_one_use(&mut w, item);
    assert!(full > 8, "a medpack heals: {full}");
    assert_eq!(w.medication_cooldown(pc), 1.0, "the cooldown began");
    assert_eq!(w.medication_stack(pc), 0, "the first use costs nothing");
    // Inside the cooldown: the menus must ask first.
    assert_eq!(w.use_consumable_request(pc, item), UseRequest::NeedsConfirm);
    w.accept_medication_warning(pc);
    assert_eq!(
        w.use_consumable_request(pc, item),
        UseRequest::Ready,
        "once for a window"
    );
    let half = heal_of_one_use(&mut w, item);
    assert_eq!(w.medication_stack(pc), 1);
    assert!((half - full / 2).abs() <= 1, "half: {half} of {full}");
    let quarter = heal_of_one_use(&mut w, item);
    assert_eq!(w.medication_stack(pc), 2);
    assert!(
        (quarter - full / 4).abs() <= 1,
        "a quarter: {quarter} of {full}"
    );
    let again = heal_of_one_use(&mut w, item);
    assert_eq!(w.medication_stack(pc), 2, "no worse than that");
    assert!((again - full / 4).abs() <= 1);
    assert!(w.errors.is_empty(), "{:?}", w.errors);
}

#[test]
fn the_cooldown_runs_down_on_the_clock_and_each_use_restarts_it() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    let item = pack(&mut w);
    for _ in 0..3 {
        pack(&mut w);
    }
    let full = heal_of_one_use(&mut w, item);
    run(&mut w, 10.0);
    let left = w.medication_cooldown(pc);
    assert!(
        (left - 0.5).abs() < 0.05,
        "half of the 20 seconds left: {left}"
    );
    // A use now restarts the 20 seconds and is the first extra.
    w.accept_medication_warning(pc);
    heal_of_one_use(&mut w, item);
    assert_eq!(w.medication_cooldown(pc), 1.0);
    assert_eq!(w.medication_stack(pc), 1);
    run(&mut w, 19.0);
    assert_eq!(w.medication_stack(pc), 1, "still sick");
    run(&mut w, 2.0);
    assert_eq!(w.medication_cooldown(pc), 0.0, "over");
    assert_eq!(w.medication_stack(pc), 0, "and the stack is gone");
    assert_eq!(w.use_consumable_request(pc, item), UseRequest::Ready);
    let healed = heal_of_one_use(&mut w, item);
    assert!(
        (healed - full).abs() <= 1,
        "in full again: {healed} of {full}"
    );
    // A new window asks again.
    assert_eq!(w.use_consumable_request(pc, item), UseRequest::NeedsConfirm);
}

#[test]
fn what_is_not_medical_has_no_cooldown() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    let armour = w.spawn_item("g_a_class4001");
    w.carry(pc, armour);
    assert_eq!(w.use_consumable_request(pc, armour), UseRequest::Ready);
    let item = pack(&mut w);
    heal_of_one_use(&mut w, item);
    assert_eq!(w.use_consumable_request(pc, armour), UseRequest::Ready);
}

#[test]
fn the_sickness_is_saved() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    let item = pack(&mut w);
    pack(&mut w);
    heal_of_one_use(&mut w, item);
    w.accept_medication_warning(pc);
    heal_of_one_use(&mut w, item);
    run(&mut w, 5.0);
    let (stack, left) = (w.medication_stack(pc), w.medication_cooldown(pc));
    let bytes = w.save_bytes("Sick").unwrap();
    let mut b = world().unwrap();
    b.load_save(&bytes).unwrap();
    assert_eq!(b.medication_stack(pc), stack);
    assert!((b.medication_cooldown(pc) - left).abs() < 0.02);
    assert_eq!(
        b.medication.get(&pc).map(|s| s.warned),
        Some(true),
        "and the warning was answered"
    );
}

/// What the player takes off a tough foe in `seconds`, with the sickness at `stack`.
fn damage_dealt(stack: u8, seconds: f32) -> i32 {
    let mut w = world().unwrap();
    w.seed_dice(11);
    let pc = w.pc;
    let p = w.object(pc).unwrap().position;
    // One of the ship's own soldiers: Calo Nord is too hard for a first level player to hit.
    let foe = w.spawn_creature("end_trask", [p[0] + 0.5, p[1], p[2]]);
    {
        let o = w.object_mut(foe).unwrap();
        o.faction = 1;
        (o.hp, o.max_hp) = (20_000, 20_000);
    }
    {
        let o = w.object_mut(pc).unwrap();
        (o.hp, o.max_hp) = (20_000, 20_000);
    }
    if stack > 0 {
        w.medication.insert(
            pc,
            Sickness {
                until: 1.0e6,
                stack,
                warned: true,
            },
        );
    }
    w.attack(pc, foe);
    run(&mut w, seconds);
    20_000 - w.object(foe).unwrap().hp
}

#[test]
fn a_sick_character_hits_less_often_and_less_hard() {
    if world().is_none() {
        return;
    }
    let healthy = damage_dealt(0, 120.0);
    let once = damage_dealt(1, 120.0);
    let twice = damage_dealt(2, 120.0);
    assert!(healthy > 100, "the control does damage: {healthy}");
    // One extra use: half the hits and three quarters of the damage, about 0.375 of the healthy total.
    assert!(once < healthy * 6 / 10, "{once} against {healthy}");
    // Two: a fifth of the hits at half the damage, about a tenth.
    assert!(twice < once * 6 / 10, "{twice} against {once}");
}
