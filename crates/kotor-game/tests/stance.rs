//! Attack mode for each member of the party: it shows the truth, goes on with a fight by itself when the player's switch is on,
//! and a click on it when lit ends the fight (the old Disengage button).

use kotor_archive::Install;
use kotor_game::{CombatAction, ObjId, World};

/// The player, a companion at their side and a hostile soldier a few steps off.
fn scene() -> Option<(World, ObjId, ObjId)> {
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
    let p = w.object(pc).unwrap().position;
    let faction = w.object(pc).unwrap().faction;
    let mate = w.spawn_creature("n_calonord", [p[0] - 0.5, p[1], p[2]]);
    let o = w.object_mut(mate).unwrap();
    o.faction = faction;
    o.party_member = true;
    (o.hp, o.max_hp) = (900, 900);
    w.party.push(mate);
    let foe = w.spawn_creature("n_calonord", [p[0] + 0.5, p[1], p[2]]);
    let o = w.object_mut(foe).unwrap();
    o.faction = 1;
    (o.hp, o.max_hp) = (900, 900);
    Some((w, mate, foe))
}

fn run(w: &mut World, seconds: f32) {
    for _ in 0..(seconds / 0.05) as usize {
        w.step(0.05);
    }
}

#[test]
fn the_button_is_lit_for_a_member_in_attack_mode_or_in_a_fight_and_a_click_toggles_it() {
    let Some((mut w, mate, foe)) = scene() else {
        return;
    };
    let pc = w.pc;
    assert!(!w.in_attack_mode(pc) && !w.in_attack_mode(mate));
    // A click turns it on, and again turns it off.
    assert!(w.toggle_mode_of(pc));
    assert!(w.in_attack_mode(pc) && w.attack_mode());
    assert!(!w.toggle_mode_of(pc));
    assert!(!w.in_attack_mode(pc) && !w.attack_mode());
    // Fighting without the mode on (the switch is off) still shows lit.
    w.attack(pc, foe);
    assert!(w.in_attack_mode(pc));
    assert!(!w.attack_mode(), "the law's mode is not on");
    // A click on a lit button ends the fight, as Disengage did.
    assert!(!w.toggle_mode_of(pc));
    assert!(!w.combat.target.contains_key(&pc));
    assert!(!w.in_attack_mode(pc));
    // Each member has their own.
    w.set_mode_of(mate, true);
    assert!(w.in_attack_mode(mate) && !w.in_attack_mode(pc));
}

#[test]
fn turning_the_mode_off_ends_the_attack_and_the_chase_and_what_was_queued() {
    let Some((mut w, _, foe)) = scene() else {
        return;
    };
    let pc = w.pc;
    w.set_attack_mode(true);
    w.attack(pc, foe);
    assert!(w.queue_action(pc, CombatAction::Attack));
    assert!(w.combat.target.contains_key(&pc));
    w.set_attack_mode(false);
    assert!(!w.combat.target.contains_key(&pc));
    assert!(w.queue_of(pc).is_empty());
    run(&mut w, 3.0);
    assert!(!w.combat.target.contains_key(&pc), "and it stays ended");
}

#[test]
fn with_auto_attack_mode_a_fight_turns_it_on_for_the_leader_and_the_companion() {
    let Some((mut w, mate, foe)) = scene() else {
        return;
    };
    let pc = w.pc;
    w.stance.auto_enabled = true;
    assert!(!w.attack_mode() && !w.in_attack_mode(mate), "no fight yet");
    w.attack(pc, foe);
    run(&mut w, 1.0);
    assert!(w.attack_mode(), "the leader's");
    assert!(w.in_attack_mode(mate), "the companion joined the fight");
    assert!(
        w.stance.members.get(&mate).is_some_and(|s| s.on && s.auto),
        "and its own mode came on by itself"
    );
}

#[test]
fn a_mode_goes_off_ten_seconds_after_the_fight_and_one_set_by_hand_and_never_fought_in_stays() {
    let Some((mut w, mate, foe)) = scene() else {
        return;
    };
    let pc = w.pc;
    w.stance.auto_enabled = true;
    w.set_mode_of(mate, true);
    w.attack(pc, foe);
    run(&mut w, 1.0);
    assert!(w.attack_mode());
    // The fight is over.
    w.object_mut(foe).unwrap().dead = true;
    w.combat.target.clear();
    run(&mut w, 9.0);
    assert!(w.attack_mode(), "not yet: ten seconds");
    assert!(w.in_attack_mode(mate), "nor the companion's");
    // A new fight starts the count again.
    let foe2 = w.spawn_creature("n_calonord", w.object(pc).unwrap().position);
    w.object_mut(foe2).unwrap().faction = 1;
    w.attack(pc, foe2);
    run(&mut w, 1.0);
    w.object_mut(foe2).unwrap().dead = true;
    w.combat.target.clear();
    run(&mut w, 9.0);
    assert!(w.attack_mode(), "the timer began again");
    run(&mut w, 2.0);
    assert!(!w.attack_mode(), "the leader's mode went off");
    assert!(!w.in_attack_mode(mate), "and the companion's, though it was set by hand");
    // Set by hand and never fought in: it stays.
    w.set_mode_of(pc, true);
    run(&mut w, 30.0);
    assert!(w.attack_mode());
    w.set_mode_of(pc, false);
    assert!(!w.attack_mode());
}

#[test]
fn without_the_switch_nothing_goes_on_by_itself() {
    let Some((mut w, mate, foe)) = scene() else {
        return;
    };
    let pc = w.pc;
    assert!(!w.stance.auto_enabled, "off in a bare world");
    w.attack(pc, foe);
    run(&mut w, 2.0);
    assert!(!w.attack_mode());
    assert!(!w.stance.members.get(&mate).is_some_and(|s| s.on));
}

#[test]
fn a_companion_told_to_stop_holds_fire_until_the_fight_is_over() {
    let Some((mut w, mate, foe)) = scene() else {
        return;
    };
    let pc = w.pc;
    w.attack(pc, foe);
    run(&mut w, 1.0);
    assert!(w.combat.target.contains_key(&mate), "it joined at first");
    w.set_mode_of(mate, false);
    assert!(!w.combat.target.contains_key(&mate), "and it stopped");
    run(&mut w, 2.0);
    assert!(!w.combat.target.contains_key(&mate), "it holds fire");
    // Once the fight is over, it is free again.
    w.combat.target.clear();
    w.stop_attacking(pc);
    run(&mut w, 1.0);
    assert!(w.holds_fire(mate), "for a few seconds more");
    run(&mut w, 5.0);
    assert!(!w.holds_fire(mate));
}

#[test]
fn a_change_of_leader_keeps_each_ones_mode_and_queue() {
    let Some((mut w, mate, _)) = scene() else {
        return;
    };
    let pc = w.pc;
    w.set_attack_mode(true);
    assert!(w.queue_action(pc, CombatAction::Feat(28)));
    assert!(w.queue_action(pc, CombatAction::Attack));
    assert!(w.switch_leader(mate));
    assert!(!w.attack_mode(), "the new leader's own mode");
    assert!(w.queue_of(w.pc).is_empty(), "and their own queue");
    assert!(w.queue_action(mate, CombatAction::Power(23)));
    assert!(w.in_attack_mode(pc), "the old leader's is kept");
    assert_eq!(
        w.queue_of(pc),
        vec![CombatAction::Feat(28), CombatAction::Attack]
    );
    assert!(w.switch_leader(pc));
    assert!(w.attack_mode());
    assert_eq!(w.queue_of(mate), vec![CombatAction::Power(23)]);
    assert_eq!(
        w.queue_of(pc),
        vec![CombatAction::Feat(28), CombatAction::Attack]
    );
}

#[test]
fn a_save_keeps_the_leaders_mode() {
    let Some((mut w, _, _)) = scene() else {
        return;
    };
    w.set_attack_mode(true);
    let bytes = w.save_bytes("stance").expect("a save");
    w.set_attack_mode(false);
    w.load_save(&bytes).unwrap();
    assert!(w.attack_mode());
    assert!(w.in_attack_mode(w.pc));
}
