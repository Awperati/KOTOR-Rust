//! Teammates fight what the leader fights and never each other; the knocked out are left alone (bug 21, GAPS row 5).

use kotor_archive::Install;
use kotor_game::{ObjId, World};

fn world() -> Option<(World, ObjId, ObjId)> {
    let mut base = Install::open(kotor_core::install_dir()?).ok()?;
    base.set_prefer_rim(true);
    let mut w = World::new(base).ok()?;
    w.load_module("end_m01aa").ok()?;
    let spec = w.recommended_character("Player", 0, 1)?;
    w.create_character(&spec).ok()?;
    w.enter_area();
    let p = w.object(w.pc)?.position;
    let mate = w.spawn_creature("n_calonord", [p[0] + 1.0, p[1], p[2]]);
    let pc = w.pc;
    w.object_mut(pc)?.min_one_hp = false;
    let pcf = w.object(pc)?.faction;
    w.object_mut(mate)?.faction = pcf;
    w.object_mut(mate)?.party_member = true;
    w.party.push(mate);
    let template = w
        .objects
        .values()
        .find(|o| o.kind == kotor_game::Kind::Creature && w.reputation(w.pc, o.id) <= 10)?
        .template
        .clone();
    let foe = w.spawn_creature(&template, [p[0] + 2.0, p[1], p[2]]);
    let o = w.object_mut(foe)?;
    (o.hp, o.max_hp) = (900, 900);
    Some((w, mate, foe))
}

#[test]
fn teammates_do_not_attack_one_another_or_the_player() {
    let Some((mut w, mate, _)) = world() else {
        return;
    };
    let pc = w.pc;
    w.attack(mate, pc);
    w.attack(pc, mate);
    assert!(w.combat.target.is_empty());
}

#[test]
fn a_teammate_with_no_target_joins_the_leaders() {
    let Some((mut w, mate, foe)) = world() else {
        return;
    };
    let pc = w.pc;
    w.attack(pc, foe);
    for _ in 0..80 {
        w.step(0.05);
    }
    assert_eq!(w.combat.target.get(&mate), Some(&foe));
}

#[test]
fn nobody_keeps_hitting_someone_who_is_down() {
    let Some((mut w, mate, foe)) = world() else {
        return;
    };
    let pc = w.pc;
    w.attack(foe, pc);
    // Let the foe notice them first.
    for _ in 0..40 {
        w.step(0.05);
    }
    w.attack(foe, pc);
    // The hero is knocked out.
    let hp = w.object(pc).unwrap().hp;
    w.hurt(foe, pc, hp + 5);
    assert!(w.object(pc).unwrap().down);
    w.combat.log.clear();
    for _ in 0..(20.0 / 0.05) as usize {
        w.step(0.05);
    }
    assert!(
        !w.combat.log.iter().any(|l| l.target == pc),
        "hit while down"
    );
    let _ = mate;
}

#[test]
fn the_player_does_not_walk_through_a_teammate_at_once() {
    let Some((mut w, mate, foe)) = world() else {
        return;
    };
    let pc = w.pc;
    // No fight: the foe is gone, and the teammate stands 4 m east of the player.
    w.object_mut(foe).unwrap().dead = true;
    let p = w.object(pc).unwrap().position;
    w.object_mut(mate).unwrap().position = [p[0] + 4.0, p[1], p[2]];
    let mut contact = None;
    let mut closest_early = f32::MAX;
    for i in 0..120 {
        w.steer = Some(([1.0, 0.0], false));
        w.step(0.05);
        let (a, b) = (
            w.object(pc).unwrap().position,
            w.object(mate).unwrap().position,
        );
        let d = (a[0] - b[0]).hypot(a[1] - b[1]);
        if d < 0.9 && contact.is_none() {
            contact = Some(i);
        }
        if contact.is_some_and(|c| i <= c + 6) {
            closest_early = closest_early.min(d);
        }
    }
    assert!(contact.is_some(), "never reached the teammate");
    assert!(
        closest_early >= 0.7,
        "walked into the teammate: {closest_early} m"
    );
}

#[test]
fn the_party_ai_style_decides_who_joins_the_leaders_fight() {
    let Some((mut w, mate, foe)) = world() else {
        return;
    };
    let pc = w.pc;
    // Aggressive, the default: the teammate joins.
    assert_eq!(w.party_ai_style, 0);
    w.attack(pc, foe);
    for _ in 0..80 {
        w.step(0.05);
    }
    assert_eq!(w.combat.target.get(&mate), Some(&foe));
    // Passive: it lets go and stays out.
    w.party_ai_style = 2;
    for _ in 0..80 {
        w.step(0.05);
    }
    assert!(!w.combat.target.contains_key(&mate));
    // Defensive: it stays out while the foe has not turned on the party, and the style answers back to scripts.
    w.party_ai_style = 1;
    w.combat.target.remove(&foe);
    w.object_mut(foe).unwrap().faction = 0;
    for _ in 0..80 {
        w.step(0.05);
    }
    assert!(!w.combat.target.contains_key(&mate));
}

#[test]
fn the_party_ai_style_survives_a_save() {
    let Some((mut w, _, _)) = world() else { return };
    w.party_ai_style = 1;
    let bytes = w.save_bytes("Style").unwrap();
    let Some((mut b, _, _)) = world() else { return };
    assert_eq!(b.party_ai_style, 0);
    b.load_save(&bytes).unwrap();
    assert_eq!(b.party_ai_style, 1);
}
