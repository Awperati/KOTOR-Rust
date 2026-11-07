//! What is queued: several actions per member, played in order, one a round; a failed one falls through to the next; each
//! member has their own; undo and clear work.

use kotor_archive::Install;
use kotor_game::{CombatAction, ObjId, World};

fn scene() -> Option<(World, ObjId, ObjId)> {
    let dir = kotor_core::install_dir()?;
    let mut base = Install::open(dir).ok()?;
    base.set_prefer_rim(true);
    let mut w = World::new(base).ok()?;
    w.load_module("end_m01aa").ok()?;
    let spec = w.recommended_character("Player", 0, 1)?;
    w.create_character(&spec).ok()?;
    let pc = w.pc;
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

/// Run `n` whole combat rounds.
fn rounds(w: &mut World, n: u32) {
    let target = w.combat.round + n;
    for _ in 0..3000 {
        if w.combat.round >= target {
            return;
        }
        w.step(0.05);
    }
    panic!("no round");
}

#[test]
fn a_member_queues_up_to_three_and_the_fourth_is_refused() {
    let Some((mut w, _, _)) = scene() else {
        return;
    };
    let pc = w.pc;
    for _ in 0..3 {
        assert!(w.queue_action(pc, CombatAction::Attack));
    }
    assert!(!w.queue_action(pc, CombatAction::Attack), "full");
    assert_eq!(w.queue_of(pc).len(), 3);
}

#[test]
fn undo_takes_back_the_last_and_clear_takes_all() {
    let Some((mut w, _, _)) = scene() else {
        return;
    };
    let pc = w.pc;
    w.queue_action(pc, CombatAction::Feat(28));
    w.queue_action(pc, CombatAction::Power(23));
    w.queue_action(pc, CombatAction::Attack);
    assert_eq!(w.undo_queued(pc), Some(CombatAction::Attack));
    assert_eq!(
        w.queue_of(pc),
        vec![CombatAction::Feat(28), CombatAction::Power(23)]
    );
    w.clear_queue(pc);
    assert!(w.queue_of(pc).is_empty());
    assert_eq!(w.undo_queued(pc), None);
}

#[test]
fn queued_actions_play_in_order_one_a_round() {
    let Some((mut w, _, foe)) = scene() else {
        return;
    };
    let pc = w.pc;
    {
        let o = w.object_mut(pc).unwrap();
        o.feats.insert(28);
        o.spells.insert(23);
        (o.force, o.max_force) = (80, 80);
    }
    // A power first, then a boosted attack, then a plain one.
    w.queue_action(pc, CombatAction::Power(23));
    w.queue_action(pc, CombatAction::Feat(28));
    w.queue_action(pc, CombatAction::Attack);
    w.attack(pc, foe);
    let force_before = w.object(pc).unwrap().force;
    rounds(&mut w, 1);
    assert!(
        w.object(pc).unwrap().force < force_before,
        "round 1: the power"
    );
    assert_eq!(
        w.queue_of(pc),
        vec![CombatAction::Feat(28), CombatAction::Attack]
    );
    rounds(&mut w, 1);
    assert_eq!(
        w.queue_of(pc),
        vec![CombatAction::Attack],
        "round 2: the feat"
    );
    rounds(&mut w, 1);
    assert!(w.queue_of(pc).is_empty(), "round 3: the plain attack");
}

#[test]
fn an_action_that_cannot_be_done_falls_through_to_the_next() {
    let Some((mut w, _, foe)) = scene() else {
        return;
    };
    let pc = w.pc;
    {
        let o = w.object_mut(pc).unwrap();
        o.feats.insert(28);
        o.spells.insert(23);
        // Not a drop of Force: the power cannot be cast.
        (o.force, o.max_force) = (0, 0);
    }
    w.queue_action(pc, CombatAction::Power(23));
    // An item that is not in the bag.
    let stranger = w.spawn_item("g_w_fraggren01");
    w.queue_action(pc, CombatAction::Item(stranger));
    w.queue_action(pc, CombatAction::Feat(28));
    w.combat.log.clear();
    w.attack(pc, foe);
    rounds(&mut w, 1);
    // The first round did not stand idle: the two that failed were dropped, and the feat was used in the same round.
    assert!(w.queue_of(pc).is_empty(), "{:?}", w.queue_of(pc));
    assert!(
        w.combat.log.iter().any(|l| l.attacker == pc),
        "an attack was made"
    );
}

#[test]
fn a_queued_item_is_used_on_the_target() {
    let Some((mut w, _, foe)) = scene() else {
        return;
    };
    let pc = w.pc;
    let frag = w.spawn_item("g_w_fraggren01");
    w.carry(pc, frag);
    w.queue_action(pc, CombatAction::Item(frag));
    w.attack(pc, foe);
    rounds(&mut w, 1);
    assert!(w.queue_of(pc).is_empty());
    assert!(!w.carried(pc).contains(&frag), "it was thrown");
}

#[test]
fn a_companions_queue_plays_in_their_own_rounds() {
    let Some((mut w, mate, foe)) = scene() else {
        return;
    };
    let pc = w.pc;
    w.object_mut(mate).unwrap().feats.insert(28);
    // The companion queued it while they led.
    assert!(w.switch_leader(mate));
    assert!(w.queue_action(mate, CombatAction::Feat(28)));
    assert!(w.switch_leader(pc));
    assert_eq!(w.queue_of(mate), vec![CombatAction::Feat(28)]);
    assert!(w.queue_of(pc).is_empty());
    w.attack(pc, foe);
    rounds(&mut w, 2);
    assert!(w.queue_of(mate).is_empty(), "they used it");
}

#[test]
fn the_queue_is_kept_while_the_target_is_out_of_reach_and_dropped_with_attack_mode() {
    let Some((mut w, _, foe)) = scene() else {
        return;
    };
    let pc = w.pc;
    w.queue_action(pc, CombatAction::Attack);
    w.attack(pc, foe);
    // The target dies before the round: nothing is lost.
    w.object_mut(foe).unwrap().dead = true;
    for _ in 0..100 {
        w.step(0.05);
    }
    assert_eq!(w.queue_of(pc), vec![CombatAction::Attack]);
    w.set_attack_mode(false);
    assert!(w.queue_of(pc).is_empty());
}
