//! Facing, friendly fire and the attack cooldown (owner, 9 Nov 2026): a fighter turns to its target, nobody shoots a friend or
//! through one, and a new attack waits for the last one's clip.

use kotor_archive::Install;
use kotor_game::{Game, World, AIMED_SHOT_SECONDS};

const PISTOL: &str = "g_w_blstrpstl001";

fn world() -> Option<World> {
    let mut inst = Install::open(kotor_core::install_dir()?).ok()?;
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).ok()?;
    w.load_module("end_m01aa").ok()?;
    let spec = w.recommended_character("Player", 0, 1)?;
    w.create_character(&spec).ok()?;
    w.enter_area();
    Some(w)
}

/// A creature with only a pistol, standing at `at` and facing `facing` degrees.
fn gunman(w: &mut World, at: [f32; 3], facing: f32, faction: i32) -> u32 {
    let id = w.spawn_creature("end_trask", at);
    let gun = w.spawn_item(PISTOL);
    let o = w.object_mut(id).unwrap();
    o.equipped.clear();
    o.equipped.insert(4, gun);
    (o.facing, o.faction, o.party_member, o.hp, o.max_hp) = (facing, faction, false, 9000, 9000);
    id
}

fn off_by(a: f32, b: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    (a - b + tau / 2.0).rem_euclid(tau) - tau / 2.0
}

#[test]
fn a_fighter_turns_to_face_its_target_in_half_a_second() {
    let Some(w) = world() else { return };
    let mut g = Game::new(w);
    let pc = g.world.pc;
    let p = g.world.object(pc).unwrap().position;
    // Six metres off, with its back to the player.
    let foe = gunman(&mut g.world, [p[0] + 6.0, p[1], p[2]], 0.0, 1);
    let want = std::f32::consts::PI;
    g.face(foe, 0.0);
    g.world.attack(foe, pc);
    assert!(g.world.combat.target.contains_key(&foe));
    for _ in 0..40 {
        g.step_once();
    }
    let now = g.facing_of(foe).unwrap();
    assert!(off_by(now, want).abs() < 0.1, "faces {now}, wants {want}");
    assert!(off_by(g.world.object(foe).unwrap().facing.to_radians(), want).abs() < 0.1);
}

#[test]
fn a_turn_takes_time_and_is_not_a_snap() {
    let Some(w) = world() else { return };
    let mut g = Game::new(w);
    let pc = g.world.pc;
    let p = g.world.object(pc).unwrap().position;
    let foe = gunman(&mut g.world, [p[0] + 6.0, p[1], p[2]], 0.0, 1);
    g.face(foe, 0.0);
    g.world.attack(foe, pc);
    for _ in 0..5 {
        g.step_once();
    }
    let now = g.facing_of(foe).unwrap();
    assert!(
        off_by(now, std::f32::consts::PI).abs() > 1.0,
        "a few steps is not a half turn: {now}"
    );
}

#[test]
fn the_player_and_the_party_turn_to_their_targets_too() {
    let Some(w) = world() else { return };
    let mut g = Game::new(w);
    let pc = g.world.pc;
    let p = g.world.object(pc).unwrap().position;
    let foe = gunman(&mut g.world, [p[0], p[1] + 6.0, p[2]], 0.0, 1);
    let gun = g.world.spawn_item(PISTOL);
    g.world.object_mut(pc).unwrap().equipped.insert(4, gun);
    g.face(pc, 0.0);
    g.world.attack(pc, foe);
    for _ in 0..40 {
        g.step_once();
    }
    let want = std::f32::consts::FRAC_PI_2;
    assert!(
        off_by(g.world.pc_mover.heading, want).abs() < 0.1,
        "{}",
        g.world.pc_mover.heading
    );
}

#[test]
fn nobody_on_the_players_side_picks_a_friend_to_attack() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    let p = w.object(pc).unwrap().position;
    let trask = w.spawn_creature("end_trask", [p[0] + 2.0, p[1], p[2]]);
    w.party.push(trask);
    let t = w.object_mut(trask).unwrap();
    (t.party_member, t.faction) = (true, 2);
    // An ally who is not in the party: of the faction `repute.2da` makes the player's friends (Friendly_1, row 2).
    let ally = gunman(&mut w, [p[0] - 2.0, p[1], p[2]], 0.0, 2);
    assert!(w.reputation(ally, pc) >= 90, "{}", w.reputation(ally, pc));
    for (a, b) in [
        (trask, pc),
        (pc, trask),
        (ally, pc),
        (ally, trask),
        (trask, ally),
    ] {
        w.attack(a, b);
        assert!(!w.combat.target.contains_key(&a), "{a} must not pick {b}");
    }
    // A foe is still fair.
    let foe = gunman(&mut w, [p[0], p[1] + 5.0, p[2]], 0.0, 1);
    w.attack(trask, foe);
    assert_eq!(w.combat.target.get(&trask), Some(&foe));
}

#[test]
fn a_fight_never_sets_the_party_on_each_other() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    let p = w.object(pc).unwrap().position;
    let trask = w.spawn_creature("end_trask", [p[0] + 2.0, p[1], p[2]]);
    w.party.push(trask);
    w.object_mut(trask).unwrap().party_member = true;
    let foe = gunman(&mut w, [p[0], p[1] + 6.0, p[2]], 0.0, 1);
    w.attack(pc, foe);
    w.attack(foe, pc);
    for _ in 0..200 {
        w.step(0.05);
        let mine = |id: u32| id == pc || id == trask;
        assert!(
            !w.combat.target.iter().any(|(a, t)| mine(*a) && mine(*t)),
            "party on party"
        );
    }
}

#[test]
fn a_bolt_does_not_go_through_an_ally() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    let p = w.object(pc).unwrap().position;
    let (hp, max) = (9000, 9000);
    {
        let o = w.object_mut(pc).unwrap();
        (o.hp, o.max_hp) = (hp, max);
    }
    let shooter = gunman(&mut w, [p[0] + 8.0, p[1], p[2]], 180.0, 1);
    // A friend of the shooter's, standing on the line between them.
    let friend = gunman(&mut w, [p[0] + 4.0, p[1], p[2]], 0.0, 1);
    assert_eq!(w.friend_in_line(shooter, pc), Some(friend));
    assert!(!w.is_friend(shooter, pc));
    w.attack(shooter, pc);
    w.combat.log.clear();
    for _ in 0..140 {
        w.step(0.05);
    }
    assert!(
        !w.combat.log.iter().any(|l| l.attacker == shooter),
        "held fire with a friend in the way"
    );
    assert_eq!(w.object(pc).unwrap().hp, hp);
    // The friend steps aside, and the shot is taken.
    w.object_mut(friend).unwrap().position = [p[0] + 4.0, p[1] + 3.0, p[2]];
    assert_eq!(w.friend_in_line(shooter, pc), None);
    w.attack(shooter, pc);
    for _ in 0..140 {
        w.step(0.05);
    }
    assert!(
        w.combat.log.iter().any(|l| l.attacker == shooter),
        "shot once clear"
    );
}

#[test]
fn a_second_click_during_the_shot_is_ignored() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    let gun = w.spawn_item(PISTOL);
    w.object_mut(pc).unwrap().equipped.insert(4, gun);
    let eye = w.pc_gun();
    let dir = [1.0, 0.0, 0.0];
    assert!(
        w.fire_aimed(eye, dir, 10.0).is_some(),
        "the first click fires"
    );
    assert!(w.attack_busy(pc));
    assert!(
        w.fire_aimed(eye, dir, 10.0).is_none(),
        "the second, at once, does nothing"
    );
    w.step(AIMED_SHOT_SECONDS * 0.5);
    assert!(
        w.fire_aimed(eye, dir, 10.0).is_none(),
        "nor half way through the clip"
    );
    w.step(AIMED_SHOT_SECONDS * 0.6);
    assert!(!w.attack_busy(pc));
    assert!(
        w.fire_aimed(eye, dir, 10.0).is_some(),
        "the clip is over, a click fires again"
    );
}
