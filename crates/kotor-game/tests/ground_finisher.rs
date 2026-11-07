//! Ground finishers: a melee blow on someone knocked down is an automatic critical; a blaster aims at the body and rolls as ever.

use kotor_archive::Install;
use kotor_game::{AttackLog, EffectRec, Outcome, World, DURATION_TEMPORARY, FORCE_PUSH};

fn world(finishers: bool) -> Option<(World, u32, u32)> {
    let mut base = Install::open(kotor_core::install_dir()?).ok()?;
    base.set_prefer_rim(true);
    let mut w = World::new(base).ok()?;
    w.load_module("end_m01aa").ok()?;
    let spec = w.recommended_character("Player", 0, 1)?;
    w.create_character(&spec).ok()?;
    let pc = w.pc;
    let p = w.object(pc)?.position;
    let foe = w.spawn_creature("n_calonord", [p[0] + 1.5, p[1], p[2]]);
    let f = w.object_mut(foe)?;
    // So armoured that only a natural 20 hits: any critical here is the finisher's.
    (
        f.hp,
        f.max_hp,
        f.plot,
        f.min_one_hp,
        f.faction,
        f.natural_ac,
    ) = (90000, 90000, false, false, 1, 100);
    let o = w.object_mut(pc)?;
    o.min_one_hp = true;
    o.force = 100;
    o.spells.insert(FORCE_PUSH);
    w.ground_finishers = finishers;
    Some((w, pc, foe))
}

fn wield(w: &mut World, pc: u32, item: &str) {
    w.object_mut(pc).unwrap().equipped.remove(&5);
    let r = w.spawn_item(item);
    w.object_mut(pc).unwrap().equipped.insert(4, r);
}

/// Knock `foe` down for ten seconds, as a push does, without the Force power that uses up the round.
fn knock_down(w: &mut World, pc: u32, foe: u32) {
    w.effects.push(EffectRec::new("EffectKnockdown"));
    let handle = w.effects.len() as u32;
    w.apply_effect(pc, foe, handle, DURATION_TEMPORARY, 10.0);
    assert!(w.is_knocked_down(foe), "knocked down");
}

/// The player's attacks while the foe lies on the floor.
fn strikes(w: &mut World, pc: u32, foe: u32) -> Vec<AttackLog> {
    knock_down(w, pc, foe);
    w.combat.log.clear();
    w.attack(pc, foe);
    for _ in 0..(6.0 / 0.05) as usize {
        w.step(0.05);
        if w.combat.log.iter().any(|l| l.attacker == pc) {
            break;
        }
    }
    w.stop_attacking(pc);
    w.combat
        .log
        .iter()
        .filter(|l| l.attacker == pc)
        .cloned()
        .collect()
}

#[test]
fn a_blade_on_someone_knocked_down_is_a_critical_and_is_listed_for_the_clip() {
    for item in ["g_w_lngswrd01", "g_w_lghtsbr01"] {
        let Some((mut w, pc, foe)) = world(true) else {
            return;
        };
        wield(&mut w, pc, item);
        let log = strikes(&mut w, pc, foe);
        assert!(!log.is_empty(), "{item}: it struck");
        assert!(
            log.iter().all(|l| l.outcome == Outcome::Critical),
            "{item}: {log:?}"
        );
        assert!(w.finishers.contains(&(pc, foe)), "{item}");
        // The critical doubled (or more) what the dice and bonuses gave.
        assert!(log.iter().all(|l| l.damage >= 2), "{item}");
    }
}

#[test]
fn fists_finish_too_and_with_the_switch_off_nothing_changes() {
    let Some((mut w, pc, foe)) = world(true) else {
        return;
    };
    w.object_mut(pc).unwrap().equipped.remove(&4);
    let log = strikes(&mut w, pc, foe);
    assert!(
        log.iter().all(|l| l.outcome == Outcome::Critical),
        "{log:?}"
    );
    let Some((mut w, pc, foe)) = world(false) else {
        return;
    };
    wield(&mut w, pc, "g_w_lngswrd01");
    let log = strikes(&mut w, pc, foe);
    assert!(
        !log.iter().any(|l| l.outcome == Outcome::Critical),
        "{log:?}"
    );
    assert!(w.finishers.is_empty());
}

#[test]
fn a_blaster_is_no_finisher_and_a_standing_foe_is_struck_as_ever() {
    let Some((mut w, pc, foe)) = world(true) else {
        return;
    };
    wield(&mut w, pc, "g_w_blstrpstl001");
    let log = strikes(&mut w, pc, foe);
    assert!(
        !log.iter().any(|l| l.outcome == Outcome::Critical),
        "{log:?}"
    );
    assert!(w.finishers.is_empty());
    // Standing: a blade is rolled for, and with this armour it misses.
    let Some((mut w, pc, foe)) = world(true) else {
        return;
    };
    wield(&mut w, pc, "g_w_lngswrd01");
    w.combat.log.clear();
    w.attack(pc, foe);
    for _ in 0..(12.0 / 0.05) as usize {
        w.step(0.05);
    }
    let log: Vec<_> = w.combat.log.iter().filter(|l| l.attacker == pc).collect();
    assert!(log.len() >= 2);
    assert!(
        log.iter()
            .filter(|l| l.outcome == Outcome::Critical)
            .count()
            <= 1
    );
    assert!(w.finishers.is_empty());
}

#[test]
fn someone_on_the_floor_counts_as_prone_for_aim() {
    let Some((mut w, pc, foe)) = world(true) else {
        return;
    };
    assert!(!w.is_prone(foe));
    w.cast_spell(pc, FORCE_PUSH, foe, false).unwrap();
    assert!(w.is_prone(foe), "pushed down");
    let other = w.spawn_creature("n_calonord", [0.0, 0.0, -300.0]);
    w.object_mut(other).unwrap().dead = true;
    assert!(w.is_prone(other), "a body");
}
