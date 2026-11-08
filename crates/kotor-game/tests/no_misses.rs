//! "No misses": what used to miss lands as a glancing blow for the least damage it can do, scaled by the attacker.

use kotor_archive::Install;
use kotor_game::{AttackLog, Outcome, World};

fn world(no_misses: bool) -> Option<(World, u32, u32)> {
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
    // His own weapon stuns on a hit now, and this is about the player's blows.
    f.equipped.remove(&4);
    f.equipped.remove(&5);
    // So armoured that only a natural 20 hits: nearly every attack would have missed.
    (
        f.hp,
        f.max_hp,
        f.plot,
        f.min_one_hp,
        f.faction,
        f.natural_ac,
    ) = (90000, 90000, false, false, 1, 100);
    w.object_mut(pc)?.min_one_hp = true;
    w.no_misses = no_misses;
    Some((w, pc, foe))
}

fn wield(w: &mut World, pc: u32, right: Option<&str>) {
    let o = w.object_mut(pc).unwrap();
    o.equipped.remove(&4);
    o.equipped.remove(&5);
    if let Some(r) = right {
        let r = w.spawn_item(r);
        w.object_mut(pc).unwrap().equipped.insert(4, r);
    }
}

/// The player's attacks over `rounds` rounds.
fn fight(w: &mut World, pc: u32, foe: u32, rounds: usize) -> Vec<AttackLog> {
    w.combat.log.clear();
    w.attack(pc, foe);
    for _ in 0..(rounds as f32 * 3.0 / 0.05) as usize {
        w.step(0.05);
        if w.combat.log.iter().filter(|l| l.attacker == pc).count() >= rounds {
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

const WEAPONS: [(&str, Option<&str>); 4] = [
    ("blaster pistol", Some("g_w_blstrpstl001")),
    ("blaster rifle", Some("g_w_blstrrfl001")),
    ("vibroblade", Some("g_w_lngswrd01")),
    ("lightsaber", Some("g_w_lghtsbr01")),
];

#[test]
fn with_no_misses_off_the_original_rules_miss() {
    for (name, item) in WEAPONS.into_iter().chain([("fists", None)]) {
        let Some((mut w, pc, foe)) = world(false) else {
            return;
        };
        wield(&mut w, pc, item);
        let log = fight(&mut w, pc, foe, 12);
        assert!(log.iter().any(|l| l.outcome == Outcome::Miss), "{name}");
        assert!(!log.iter().any(|l| l.outcome == Outcome::Glance), "{name}");
    }
}

#[test]
fn with_no_misses_on_every_weapon_lands_for_at_least_one_and_a_real_hit_is_unchanged() {
    for (name, item) in WEAPONS.into_iter().chain([("fists", None)]) {
        let Some((mut w, pc, foe)) = world(true) else {
            return;
        };
        wield(&mut w, pc, item);
        let log = fight(&mut w, pc, foe, 12);
        assert!(log.len() >= 8, "{name}: {}", log.len());
        assert!(!log.iter().any(|l| l.outcome == Outcome::Miss), "{name}");
        let glances: Vec<_> = log
            .iter()
            .filter(|l| l.outcome == Outcome::Glance)
            .collect();
        assert!(glances.len() >= 6, "{name}: nearly every attack glances");
        assert!(glances.iter().all(|l| l.damage >= 1), "{name}");
        // A glance is the least a blow can do: never more than a plain hit's damage with its dice at their least + a bit.
        for l in log.iter().filter(|l| l.outcome == Outcome::Hit) {
            assert!(l.damage >= 1, "{name}");
        }
        // The foe lost exactly what was dealt.
        let dealt: i32 = log.iter().map(|l| l.damage).sum();
        assert_eq!(w.object(foe).unwrap().hp, 90000 - dealt, "{name}");
    }
}

#[test]
fn a_stronger_character_glances_harder() {
    let Some((mut w, pc, foe)) = world(true) else {
        return;
    };
    wield(&mut w, pc, Some("g_w_lngswrd01"));
    w.object_mut(pc).unwrap().abilities[0] = 10;
    w.object_mut(pc).unwrap().level = 1;
    let least = |w: &mut World| {
        fight(w, pc, foe, 8)
            .iter()
            .filter(|l| l.outcome == Outcome::Glance)
            .map(|l| l.damage)
            .max()
            .unwrap()
    };
    let weak = least(&mut w);
    w.object_mut(pc).unwrap().abilities[0] = 20;
    let strong = least(&mut w);
    assert!(
        strong >= weak + 2,
        "Strength 20 against 10: {weak} then {strong}"
    );
    w.object_mut(pc).unwrap().level = 12;
    let veteran = least(&mut w);
    assert!(
        veteran > strong,
        "level 12 against 1: {strong} then {veteran}"
    );
}

#[test]
fn an_enemys_miss_on_the_player_or_a_companion_is_a_narrow_miss_with_no_damage() {
    for target_is_companion in [false, true] {
        let Some((mut w, pc, foe)) = world(true) else {
            return;
        };
        let victim = if target_is_companion {
            let mate = w.spawn_creature("n_calonord", [0.0, 0.0, -300.0]);
            let at = w.object(pc).unwrap().position;
            w.teleport(mate, [at[0] + 0.5, at[1], at[2]]);
            let o = w.object_mut(mate).unwrap();
            (o.party_member, o.faction, o.hp, o.max_hp) = (true, 0, 9000, 9000);
            w.party.push(mate);
            mate
        } else {
            pc
        };
        // The foe has the sword and a hand that cannot find its mark.
        let mut dull = kotor_game::EffectRec::new("EffectAttackDecrease");
        dull.nums = vec![60.0];
        w.effects.push(dull);
        let handle = w.effects.len() as u32;
        w.apply_effect(foe, foe, handle, kotor_game::DURATION_PERMANENT, 0.0);
        let sword = w.spawn_item("g_w_lngswrd01");
        w.object_mut(foe).unwrap().equipped.insert(4, sword);
        let hp = w.object(victim).unwrap().hp;
        w.combat.log.clear();
        w.attack(foe, victim);
        for _ in 0..(40.0 / 0.05) as usize {
            w.step(0.05);
        }
        w.stop_attacking(foe);
        let theirs: Vec<_> = w.combat.log.iter().filter(|l| l.attacker == foe).collect();
        assert!(theirs.len() >= 5);
        assert!(
            !theirs.iter().any(|l| l.outcome == Outcome::Glance),
            "no glance on the player's side"
        );
        let misses: Vec<_> = theirs
            .iter()
            .filter(|l| l.outcome == Outcome::Miss)
            .collect();
        assert!(
            misses.len() >= 4,
            "nearly all of them narrowly miss: {theirs:?}"
        );
        assert!(misses.iter().all(|l| l.damage == 0));
        // Only the rare real hit or critical took anything.
        let taken: i32 = theirs.iter().map(|l| l.damage).sum();
        assert!(hp - w.object(victim).unwrap().hp <= taken);
    }
}

#[test]
fn a_companions_miss_on_an_enemy_is_a_glance() {
    let Some((mut w, pc, foe)) = world(true) else {
        return;
    };
    let mate = w.spawn_creature("n_calonord", [0.0, 0.0, -300.0]);
    let at = w.object(pc).unwrap().position;
    w.teleport(mate, [at[0] + 0.5, at[1], at[2]]);
    let o = w.object_mut(mate).unwrap();
    (o.party_member, o.faction, o.hp, o.max_hp) = (true, 0, 9000, 9000);
    w.party.push(mate);
    let sword = w.spawn_item("g_w_lngswrd01");
    w.object_mut(mate).unwrap().equipped.insert(4, sword);
    w.combat.log.clear();
    w.attack(mate, foe);
    for _ in 0..(30.0 / 0.05) as usize {
        w.step(0.05);
    }
    w.stop_attacking(mate);
    let mine: Vec<_> = w.combat.log.iter().filter(|l| l.attacker == mate).collect();
    assert!(mine.len() >= 4);
    assert!(!mine.iter().any(|l| l.outcome == Outcome::Miss));
    assert!(mine
        .iter()
        .any(|l| l.outcome == Outcome::Glance && l.damage >= 1));
}

#[test]
fn an_aimed_shot_never_misses_and_its_graze_is_half_a_hit_whatever_the_switch_says() {
    let Some((mut w, pc, foe)) = world(false) else {
        return;
    };
    wield(&mut w, pc, Some("g_w_blstrpstl001"));
    w.combat.log.clear();
    for _ in 0..30 {
        w.resolve_aimed(pc, foe, 4);
    }
    let mine: Vec<_> = w.combat.log.iter().filter(|l| l.attacker == pc).collect();
    assert_eq!(mine.len(), 30);
    assert!(!mine.iter().any(|l| l.outcome == Outcome::Miss));
    assert!(mine.iter().filter(|l| l.outcome == Outcome::Glance).count() >= 20);
    assert!(mine.iter().all(|l| l.damage >= 1));
}
