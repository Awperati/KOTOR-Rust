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
    // Still down after 2 s; getting up at 3.5 s, and on its feet at 5 s.
    assert!(w.is_disabled(foe));
    for _ in 0..30 {
        w.step(0.05);
    }
    assert!(!w.is_knocked_down(foe));
    assert!(
        w.is_getting_up(foe),
        "the push is over and it is getting up"
    );
    assert!(w.is_disabled(foe), "it cannot act while it gets up");
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
    // The push lasts 3 s: after it the creature gets up (2 s), then stands.
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
        for _ in 0..100 {
            w.step(0.05);
        }
        let end = w.object(foe).unwrap().position;
        let moved = (end[0] - from[0]).hypot(end[1] - from[1]);
        let thuds: Vec<_> = std::mem::take(&mut w.thuds);
        assert_eq!(thuds.len(), 1, "one landing per throw");
        // Down for three seconds and then up.
        assert!(!w.is_disabled(foe), "up again after five seconds");
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

/// Force Push reaches by power and by the caster's level: Push 8 m and Wave 10 m at the first level, 0.1 m more a level after
/// that, 2 m more at the most.
#[test]
fn a_throw_reaches_further_with_a_stronger_power_and_a_higher_caster() {
    use kotor_game::push_reach;
    const WAVE: i32 = 26;
    assert_eq!(push_reach(kotor_game::FORCE_PUSH, 1), 8.0);
    assert_eq!(push_reach(WAVE, 1), 10.0);
    assert!((push_reach(kotor_game::FORCE_PUSH, 11) - 9.0).abs() < 1e-4);
    assert!((push_reach(kotor_game::FORCE_PUSH, 20) - 9.9).abs() < 1e-4);
    assert_eq!(push_reach(kotor_game::FORCE_PUSH, 40), 10.0);
    assert!(push_reach(WAVE, 20) > push_reach(kotor_game::FORCE_PUSH, 20));
}

fn shove(to: [f32; 3], wall: bool, back: [f32; 3]) -> kotor_game::Shove {
    let length = to[0];
    let hit = kotor_game::PUSH_SLIDE_SECONDS + kotor_game::PUSH_SECONDS_PER_METRE * length;
    kotor_game::Shove {
        who: 1,
        from: [0.0; 3],
        to,
        start: 0.0,
        seconds: hit
            + if wall {
                kotor_game::PUSH_REBOUND_SECONDS
            } else {
                0.0
            },
        wall,
        by: 2,
        fall: None,
        hit,
        back,
        lift: (kotor_game::PUSH_LIFT_PER_METRE * length).min(kotor_game::PUSH_LIFT_MOST),
        landed: false,
    }
}

/// In the open a thrown body flies, lands part way and skids to a stop.
#[test]
fn a_body_thrown_in_the_open_flies_lands_and_skids() {
    let s = shove([8.0, 0.0, 0.0], false, [8.0, 0.0, 0.0]);
    let top = s.at(s.seconds * kotor_game::PUSH_AIR_PART * 0.5);
    assert!(
        top[2] > 0.4,
        "off the floor at the top of its flight: {top:?}"
    );
    let skid = s.at(s.seconds * 0.85);
    assert_eq!(skid[2], 0.0, "on the floor for the end of it: {skid:?}");
    assert!(skid[0] < 8.0 && skid[0] > 6.0, "still sliding: {skid:?}");
    assert_eq!(s.at(s.seconds), [8.0, 0.0, 0.0]);
    // It slows to a stop.
    let late = s.at(s.seconds)[0] - s.at(s.seconds - 0.05)[0];
    let early = s.at(0.05)[0] - s.at(0.0)[0];
    assert!(late < early * 0.2, "early {early} late {late}");
}

/// Against a wall it hits at speed, still off the floor, then drops back from the wall onto the floor.
#[test]
fn a_body_thrown_at_a_wall_hits_it_at_speed_and_drops_back() {
    let s = shove([4.0, 0.0, 0.0], true, [3.65, 0.0, 0.0]);
    let mean = 4.0 / s.hit;
    let at_wall = (s.at(s.hit)[0] - s.at(s.hit - 0.02)[0]) / 0.02;
    assert!(
        at_wall > 0.6 * mean,
        "it hits at speed: {at_wall} m/s, {mean} on the way"
    );
    let struck = s.at(s.hit);
    assert!(
        (struck[0] - 4.0).abs() < 1e-4 && struck[2] > 0.1,
        "it strikes the wall in the air: {struck:?}"
    );
    let rest = s.at(s.seconds);
    assert!(
        (rest[0] - 3.65).abs() < 1e-4 && rest[2].abs() < 1e-4,
        "{rest:?}"
    );
}
