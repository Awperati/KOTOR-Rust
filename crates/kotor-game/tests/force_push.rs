//! Force Push throws its target back and leaves it down for a few seconds.

use kotor_archive::Install;
use kotor_game::{EffectRec, Game, World, DURATION_TEMPORARY};

#[test]
fn a_pushed_creature_slides_away_and_cannot_act_for_a_while() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut base = Install::open(dir).unwrap();
    base.set_prefer_rim(true);
    let mut w = World::new(base).unwrap();
    w.load_module("end_m01aa").unwrap();
    let spec = w.recommended_character("Player", 0, 1).unwrap();
    w.create_character(&spec).unwrap();
    let pc = w.pc;
    let p = w.object(pc).unwrap().position;
    let foe = w.spawn_creature("n_calonord", [p[0] + 0.8, p[1], p[2]]);
    let start = w.object(foe).unwrap().position;
    w.effects.push(EffectRec::new("EffectForcePushed"));
    let handle = w.effects.len() as u32;
    w.apply_effect(pc, foe, handle, DURATION_TEMPORARY, 0.1);
    assert!(w.is_disabled(foe));
    for _ in 0..40 {
        w.step(0.05);
    }
    let end = w.object(foe).unwrap().position;
    let moved = (end[0] - start[0]).hypot(end[1] - start[1]);
    assert!(moved > 1.0, "moved {moved} m");
    // Still down after 2 s, up after 3.5 s.
    assert!(w.is_disabled(foe));
    for _ in 0..30 {
        w.step(0.05);
    }
    assert!(!w.is_disabled(foe));
}

/// What the creature is shown doing: a pushed one lies down as a body does and gets up when the push is over.
#[test]
fn a_pushed_creature_is_drawn_lying_down_and_then_getting_up() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut base = Install::open(dir).unwrap();
    base.set_prefer_rim(true);
    let mut w = World::new(base).unwrap();
    w.load_module("end_m01aa").unwrap();
    let spec = w.recommended_character("Player", 0, 1).unwrap();
    w.create_character(&spec).unwrap();
    let pc = w.pc;
    let p = w.object(pc).unwrap().position;
    let foe = w.spawn_creature("n_calonord", [p[0] + 0.8, p[1], p[2]]);
    let mut g = Game::new(w);
    let clip = |g: &Game| {
        g.poses()
            .iter()
            .find(|q| q.id == foe)
            .and_then(|q| q.clip.as_ref().map(|c| c.name))
    };
    g.step_once();
    assert_ne!(clip(&g), Some("die"));
    g.world.effects.push(EffectRec::new("EffectForcePushed"));
    let handle = g.world.effects.len() as u32;
    g.world
        .apply_effect(pc, foe, handle, DURATION_TEMPORARY, 0.1);
    for _ in 0..60 {
        g.step_once();
    }
    assert_eq!(clip(&g), Some("die"), "down for the push's three seconds");
    // The push lasts 3 s: after it the creature gets up, then stands.
    for _ in 0..(3.2 * 60.0) as usize {
        g.step_once();
    }
    assert_eq!(clip(&g), Some("getupdead"));
    for _ in 0..(3.2 * 60.0) as usize {
        g.step_once();
    }
    assert_ne!(clip(&g), Some("getupdead"));
    assert_ne!(clip(&g), Some("die"));
}

/// Owner, 7 Nov: Force Push throws much further (8 m, not 3); against a wall behind them they hit it and fall; in the open they
/// land on their back; either way they are down three seconds and then get up. Each throw is tried round the player, in every
/// direction the floor allows: some end in open air, some on a wall.
#[test]
fn force_push_throws_eight_metres_in_the_open_and_stops_at_a_wall() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut base = Install::open(dir).unwrap();
    base.set_prefer_rim(true);
    let mut w = World::new(base).unwrap();
    w.load_module("end_m01aa").unwrap();
    let spec = w.recommended_character("Player", 0, 1).unwrap();
    w.create_character(&spec).unwrap();
    w.enter_area();
    let pc = w.pc;
    let home = w.object(pc).unwrap().position;
    let foe = w.spawn_creature("n_calonord", [home[0], home[1], home[2]]);
    w.object_mut(foe).unwrap().max_hp = 900;
    w.object_mut(foe).unwrap().hp = 900;
    w.object_mut(pc).unwrap().force = 500;
    w.object_mut(pc).unwrap().max_force = 500;
    let mut open: Vec<(f32, bool)> = Vec::new();
    let mut walled: Vec<(f32, bool)> = Vec::new();
    for k in 0..24 {
        let a = k as f32 * std::f32::consts::TAU / 24.0;
        let from = [home[0] + a.cos() * 1.0, home[1] + a.sin() * 1.0];
        let Some(z) = w
            .nav
            .as_ref()
            .and_then(|n| n.height_at(from[0], from[1], home[2] + 0.4))
        else {
            continue;
        };
        w.teleport(foe, [from[0], from[1], z]);
        w.cast_spell(pc, kotor_game::FORCE_PUSH, foe, true).unwrap();
        for _ in 0..80 {
            w.step(0.05);
        }
        let end = w.object(foe).unwrap().position;
        let moved = (end[0] - from[0]).hypot(end[1] - from[1]);
        let thuds: Vec<_> = std::mem::take(&mut w.thuds);
        assert_eq!(thuds.len(), 1, "one landing per throw");
        // Down for three seconds and then up.
        assert!(!w.is_disabled(foe), "up again after four seconds");
        if thuds[0].wall {
            walled.push((moved, true));
        } else {
            open.push((moved, false));
        }
        w.teleport(foe, [home[0], home[1], home[2]]);
        for _ in 0..40 {
            w.step(0.05);
        }
        w.thuds.clear();
    }
    assert!(
        open.iter().any(|(m, _)| *m > 7.0),
        "in the open it goes 8 m: {open:?} walled {walled:?}"
    );
    assert!(
        open.iter()
            .all(|(m, _)| *m <= kotor_game::push_metres(kotor_game::FORCE_PUSH) + 0.3),
        "{open:?}"
    );
    assert!(
        !walled.is_empty(),
        "some direction ends on a wall: {open:?}"
    );
    assert!(
        walled.iter().all(|(m, _)| *m < 8.0),
        "a wall stops it short of the 8 m: {walled:?}"
    );
}
