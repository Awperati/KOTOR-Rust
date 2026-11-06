//! The player's death: a fall with the world held, a fade, then the game-over screen. A party member down alone, or
//! the player down with others standing, leaves the fight to go on.

use kotor_archive::Install;
use kotor_game::{
    DeathPhase, EffectRec, World, DEATH_FADE_SECONDS, DEATH_FALL_SECONDS, DURATION_INSTANT,
};

fn world() -> Option<(World, u32, u32)> {
    let mut base = Install::open(kotor_core::install_dir()?).ok()?;
    base.set_prefer_rim(true);
    let mut w = World::new(base).ok()?;
    w.load_module("end_m01aa").ok()?;
    let spec = w.recommended_character("Player", 0, 1)?;
    w.create_character(&spec).ok()?;
    let pc = w.pc;
    let trask = w.by_tag("end_trask")[0];
    w.party.push(trask);
    for id in [pc, trask] {
        let o = w.object_mut(id)?;
        o.party_member = true;
        o.min_one_hp = false;
        o.plot = false;
    }
    Some((w, pc, trask))
}

fn kill(w: &mut World, who: u32) {
    let mut e = EffectRec::new("EffectDamage");
    e.nums = vec![1000.0, 0.0, 0.0];
    w.effects.push(e);
    let h = w.effects.len() as u32;
    w.apply_effect(who, who, h, DURATION_INSTANT, 0.0);
}

fn steps(w: &mut World, seconds: f32) {
    for _ in 0..(seconds * 60.0) as usize {
        w.step(1.0 / 60.0);
    }
}

#[test]
fn the_whole_party_down_ends_in_a_fall_a_fade_and_a_game_over() {
    let Some((mut w, pc, trask)) = world() else {
        return;
    };
    assert_eq!(w.death_phase(), None);
    kill(&mut w, trask);
    kill(&mut w, pc);
    w.step(1.0 / 60.0);
    assert_eq!(w.death_phase(), Some(DeathPhase::Falling));
    // Input is dead: pressing forward, clicking and a hostile who is still aiming change nothing.
    let at = w.object(pc).unwrap().position;
    w.steer = Some(([1.0, 0.0], true));
    let foe = w.spawn_creature("n_calonord", [at[0] + 3.0, at[1], at[2]]);
    w.attack(foe, pc);
    steps(&mut w, 1.0);
    assert_eq!(w.object(pc).unwrap().position, at, "no free movement");
    assert!(w.combat.target.is_empty(), "the fight is called off");
    assert_eq!(w.death_phase(), Some(DeathPhase::Falling));
    steps(&mut w, DEATH_FALL_SECONDS);
    match w.death_phase() {
        Some(DeathPhase::Fading(f)) => assert!(f > 0.0 && f < 1.0, "{f}"),
        other => panic!("{other:?}"),
    }
    steps(&mut w, DEATH_FADE_SECONDS);
    assert_eq!(w.death_phase(), Some(DeathPhase::Over));
}

#[test]
fn a_party_member_down_alone_leaves_the_rest_playing() {
    let Some((mut w, pc, trask)) = world() else {
        return;
    };
    kill(&mut w, trask);
    assert!(w.object(trask).unwrap().down);
    steps(&mut w, 0.1);
    assert_eq!(w.death_phase(), None, "the player still stands");
    // With nothing fighting, Trask gets back up.
    assert!(!w.object(trask).unwrap().down);
    // The player still walks.
    let at = w.object(pc).unwrap().position;
    w.steer = Some(([1.0, 0.0], true));
    steps(&mut w, 0.5);
    assert_ne!(w.object(pc).unwrap().position, at);
}

#[test]
fn a_player_down_with_others_standing_cannot_walk() {
    let Some((mut w, pc, trask)) = world() else {
        return;
    };
    // A soldier keeps the fight going, aimed at Trask.
    let p = w.object(pc).unwrap().position;
    let foe = w.spawn_creature("n_calonord", [p[0] + 6.0, p[1], p[2]]);
    w.object_mut(foe).unwrap().hp = 900;
    w.attack(foe, trask);
    kill(&mut w, pc);
    steps(&mut w, 0.1);
    assert_eq!(w.death_phase(), None, "Trask still stands");
    assert!(
        w.object(pc).unwrap().down,
        "the fight holds the player down"
    );
    let at = w.object(pc).unwrap().position;
    w.steer = Some(([1.0, 0.0], true));
    steps(&mut w, 1.0);
    assert_eq!(
        w.object(pc).unwrap().position,
        at,
        "a downed player lies where it fell"
    );
}
