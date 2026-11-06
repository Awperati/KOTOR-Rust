//! Which clip a creature plays through a cast, a throw, a flinch and a fight. Items 18, 24, 27 and 28 of the owner's list
//! of 6 Nov 2026. Clip lengths are those of the humanoid supermodel (`kotor-tool anims S_Male02`).
use kotor_archive::Install;
use kotor_game::{Game, World};

fn game() -> Option<Game> {
    let mut inst = Install::open(kotor_core::install_dir()?).ok()?;
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).ok()?;
    w.interactive_dialogue = true;
    w.load_module("end_m01aa").ok()?;
    let spec = w.recommended_character("Player", 0, 1)?;
    w.create_character(&spec).ok()?;
    w.enter_area();
    Some(Game::new(w))
}

/// The names of the clips the player plays over `seconds`, each once in a row, with how long each lasted.
fn clips(g: &mut Game, seconds: f32) -> Vec<(&'static str, f32)> {
    let pc = g.world.pc;
    let mut out: Vec<(&'static str, f32)> = Vec::new();
    for _ in 0..(seconds / g.dt) as usize {
        g.step_once();
        let name = g.clip(pc).map_or("-", |c| c.name);
        match out.last_mut() {
            Some((n, t)) if *n == name => *t += g.dt,
            _ => out.push((name, g.dt)),
        }
    }
    out
}

fn gives_force(g: &mut Game, spell: i32) {
    let pc = g.world.pc;
    let o = g.world.object_mut(pc).unwrap();
    o.force = 200;
    o.max_force = 200;
    o.spells.insert(spell);
}

#[test]
fn a_power_plays_its_cast_once_and_a_grenade_its_throw() {
    let Some(mut g) = game() else { return };
    let pc = g.world.pc;
    let foe = g
        .world
        .objects
        .values()
        .find(|o| o.template == "n_sithsoldier003")
        .map(|o| o.id)
        .unwrap();
    let at = g.world.object(foe).unwrap().position;
    g.world.teleport(pc, [at[0] - 4.0, at[1], at[2]]);
    g.step_once();
    // Force Push, spell 23: the start of the cast, then its loop, once each, and then standing again.
    gives_force(&mut g, 23);
    g.world.cast_spell(pc, 23, foe, false).unwrap();
    let seen = clips(&mut g, 2.5);
    let names: Vec<_> = seen.iter().map(|c| c.0).collect();
    assert_eq!(&names[..2], ["castout1", "castoutlp1"], "{seen:?}");
    assert!((seen[0].1 - 0.333).abs() < 0.05, "{seen:?}");
    assert!((seen[1].1 - 0.933).abs() < 0.05, "{seen:?}");
    assert!(!names[2..].contains(&"castout1"), "no looping: {seen:?}");
    // A grenade, spell 87: its throw, 1.5 s, once.
    g.world.cast_spell(pc, 87, foe, true).unwrap();
    let seen = clips(&mut g, 2.5);
    assert_eq!(seen[0].0, "throwgren", "{seen:?}");
    assert!((seen[0].1 - 1.5).abs() < 0.05, "{seen:?}");
    assert!(seen[1..].iter().all(|c| c.0 != "throwgren"), "{seen:?}");
}

#[test]
fn a_fighter_keeps_its_stance_between_rounds_and_a_flinch_is_not_cut() {
    let Some(mut g) = game() else { return };
    let pc = g.world.pc;
    let foe = g
        .world
        .objects
        .values()
        .find(|o| o.template == "n_sithsoldier003")
        .map(|o| o.id)
        .unwrap();
    let at = g.world.object(foe).unwrap().position;
    g.world.teleport(pc, [at[0] - 1.5, at[1], at[2]]);
    g.world.attack(foe, pc);
    let mut flinched = 0.0f32;
    let mut stood = 0.0f32;
    let mut fought = false;
    for _ in 0..(20.0 / g.dt) as usize {
        g.step_once();
        let name = g.clip(foe).map_or("-", |c| c.name);
        fought |= !g.world.combat.log.is_empty();
        if fought && g.world.object(foe).is_some_and(|o| !o.dead && !o.down) {
            if name == "pause1" {
                stood += g.dt;
            }
            if name.ends_with("g1") {
                flinched += g.dt;
            }
        }
    }
    assert!(fought, "a fight happened");
    assert!(stood < 0.5, "idle in a fight for {stood} s");
    // The flinches that began were shown to their end: whole multiples of 1.47 s, not cut.
    let whole = (flinched / 1.467).round() * 1.467;
    assert!((flinched - whole).abs() < 0.2, "{flinched}");
}

#[test]
fn a_change_of_clip_fades_over_a_fifth_of_a_second() {
    let Some(mut g) = game() else { return };
    let pc = g.world.pc;
    for _ in 0..30 {
        g.step_once();
    }
    assert!(g.clip(pc).unwrap().fade.is_none(), "standing needs no fade");
    // Ordered to walk: the first step of walking carries the idle clip with weight 1, and it is gone by 0.2 s.
    let at = g.world.object(pc).unwrap().position;
    g.world.walk_to([at[0] + 3.0, at[1], at[2]]);
    let mut first = None;
    let mut gone = None;
    for i in 0..60 {
        g.step_once();
        let c = g.clip(pc).unwrap();
        if first.is_none() && c.name != "pause1" {
            first = c.fade;
        }
        if first.is_some() && gone.is_none() && c.fade.is_none() {
            gone = Some(i);
        }
    }
    let f = first.expect("the idle clip fades out");
    assert_eq!(f.name, "pause1");
    assert!(f.weight > 0.9, "{f:?}");
    assert!(gone.is_some(), "the fade ends");
}
