//! Detection: each enemy that has noticed the party has a number that fills while it perceives them and drains when it does
//! not, and attack mode goes off by itself only after every enemy has lost them and 10 more seconds have passed.

use kotor_archive::Install;
use kotor_game::{ObjId, World};

/// The player and a hostile soldier a few metres off, with no one else about.
fn scene() -> Option<(World, ObjId)> {
    let dir = kotor_core::install_dir()?;
    let mut base = Install::open(dir).ok()?;
    base.set_prefer_rim(true);
    let mut w = World::new(base).ok()?;
    w.load_module("end_m01aa").ok()?;
    let spec = w.recommended_character("Player", 0, 1)?;
    w.create_character(&spec).ok()?;
    let pc = w.pc;
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
    (
        w.object_mut(pc).unwrap().hp,
        w.object_mut(pc).unwrap().max_hp,
    ) = (20_000, 20_000);
    let p = w.object(pc).unwrap().position;
    let foe = w.spawn_creature("n_calonord", [p[0] + 4.0, p[1], p[2]]);
    let o = w.object_mut(foe).unwrap();
    o.faction = 1;
    (o.hp, o.max_hp) = (900, 900);
    Some((w, foe))
}

fn run(w: &mut World, seconds: f32) {
    for _ in 0..(seconds / 0.05) as usize {
        w.step(0.05);
    }
}

/// Take the foe far off, where it cannot perceive anyone, and end the fight.
fn lose_them(w: &mut World, foe: ObjId) {
    let o = w.object_mut(foe).unwrap();
    o.position = [o.position[0] + 500.0, o.position[1], o.position[2]];
    w.combat.target.clear();
}

#[test]
fn an_enemy_that_perceives_the_party_fills_its_bar_and_drains_it_when_it_loses_them() {
    let Some((mut w, foe)) = scene() else { return };
    assert_eq!(w.detection_of(foe), 0.0);
    assert!(!w.is_watched());
    // Not yet at its first look, then rising.
    run(&mut w, 0.6);
    let part = w.detection_of(foe);
    run(&mut w, 4.0);
    assert!(w.detection_of(foe) >= part, "it only rises");
    assert_eq!(w.detection_of(foe), 1.0, "and it is full: alerted");
    assert!(w.is_watched());
    let marks = w.detection_marks();
    assert!(marks.iter().any(|m| m.who == foe && m.fill == 1.0));
    // They get away.
    lose_them(&mut w, foe);
    run(&mut w, 1.0);
    let draining = w.detection_of(foe);
    assert!(draining < 1.0 && draining > 0.0, "it drains: {draining}");
    run(&mut w, 3.0);
    assert_eq!(w.detection_of(foe), 0.0, "and is empty");
    assert!(!w.is_watched());
}

#[test]
fn attack_mode_goes_off_ten_seconds_after_the_last_enemy_has_lost_them() {
    let Some((mut w, foe)) = scene() else { return };
    w.stance.live = true;
    w.stance.auto_enabled = true;
    run(&mut w, 5.0);
    assert!(w.attack_mode(), "the fight turned it on");
    lose_them(&mut w, foe);
    // The bar takes 3 s to drain, and only then do the 10 s begin.
    run(&mut w, 11.0);
    assert!(
        w.attack_mode(),
        "not yet: they were watched until the bar ran out"
    );
    run(&mut w, 4.0);
    assert!(!w.attack_mode(), "off");
    // And a new sighting while it counts down starts it over.
    let (mut w, foe) = scene().unwrap();
    w.stance.live = true;
    w.stance.auto_enabled = true;
    run(&mut w, 5.0);
    lose_them(&mut w, foe);
    run(&mut w, 8.0);
    let o = w.object_mut(foe).unwrap();
    o.position = [o.position[0] - 500.0, o.position[1], o.position[2]];
    run(&mut w, 6.0);
    assert!(w.attack_mode(), "seen again: the count stood still");
}

#[test]
fn the_party_is_louder_running_and_fighting_and_quieter_still_or_in_stealth() {
    let Some((mut w, foe)) = scene() else { return };
    w.combat.target.clear();
    let still = w.noise_rate();
    assert_eq!(still, kotor_game::NOISE_STILL, "standing still is quiet");
    w.pc_body.vel = [kotor_game::RUN_SPEED, 0.0];
    let run = w.noise_rate();
    assert_eq!(run, kotor_game::NOISE_RUN);
    w.pc_body.vel = [1.5, 0.0];
    assert_eq!(w.noise_rate(), kotor_game::NOISE_WALK);
    w.stealth_mode = true;
    assert_eq!(
        w.noise_rate(),
        kotor_game::NOISE_WALK * kotor_game::NOISE_STEALTH
    );
    w.stealth_mode = false;
    let pc = w.pc;
    w.combat.target.insert(foe, pc);
    assert_eq!(
        w.noise_rate(),
        kotor_game::NOISE_FIGHT,
        "a fight is loudest"
    );
    assert!(still < run && run < kotor_game::NOISE_FIGHT);
}
