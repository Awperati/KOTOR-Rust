//! The Ebon Hawk's turret, through the game's own data and scripts.

use kotor_archive::Install;
use kotor_game::World;

fn world() -> Option<World> {
    let dir = kotor_core::install_dir()?;
    let mut w = World::new(Install::open(dir).unwrap()).unwrap();
    w.load_module("M12ab").unwrap();
    w.enter_area();
    Some(w)
}

const STEP: f32 = 0.05;

#[test]
fn the_area_says_what_the_game_is() {
    let Some(w) = world() else { return };
    let g = w.minigame.as_ref().expect("a minigame");
    assert_eq!(g.view_angle, 65.0);
    assert_eq!(g.player.hit_points, 3000);
    assert_eq!(g.player.models[0].0, "mgf_turret");
    assert_eq!(g.player.guns.len(), 2);
    assert_eq!(g.enemies.len(), 6);
    assert_eq!(g.enemies[0].hit_points, 100);
    assert_eq!(g.offset[0], 7.0);
    assert_eq!(
        w.global_numbers.get("EBO_NUM_FIGHTERS"),
        Some(&6),
        "their scripts count them"
    );
}

#[test]
fn a_module_with_no_game_has_none() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut w = World::new(Install::open(dir).unwrap()).unwrap();
    w.load_module("end_m01aa").unwrap();
    assert!(w.minigame.is_none());
}

#[test]
fn fighters_fly_their_tracks_and_the_turret_turns_inside_its_limits() {
    let Some(mut w) = world() else { return };
    w.step(STEP);
    let at = w.minigame.as_ref().unwrap().enemies[0].at;
    for _ in 0..40 {
        w.step(STEP);
    }
    let g = w.minigame.as_mut().unwrap();
    assert_ne!(g.enemies[0].at, at);
    g.turn_by(1000.0, 0.0);
    assert_eq!(g.offset[0], 45.0);
    g.turn_by(-1000.0, 20.0);
    assert_eq!(g.offset[0], 2.0);
    assert_eq!(g.offset[2], 20.0);
    assert_ne!(g.camera(), ([0.0; 3], [0.0; 3]));
}

#[test]
fn nobody_shooting_back_loses_hit_points_and_the_health_shows() {
    let Some(mut w) = world() else { return };
    for _ in 0..(60.0 / STEP) as usize {
        w.step(STEP);
    }
    let g = w.minigame.as_ref().unwrap();
    assert!(g.player.hit_points < 3000, "{}", g.player.hit_points);
    assert!(w.errors.is_empty(), "{:?}", w.errors);
}

/// Fire at each in turn until none is left.
fn play_to_the_end(w: &mut World) {
    for _ in 0..(300.0 / STEP) as usize {
        let Some(g) = w.minigame.as_mut() else { break };
        let target = g
            .enemies
            .iter()
            .find(|e| e.dying.is_none() && !e.gone)
            .map(|e| e.at);
        let Some(target) = target else { break };
        g.point_at(target);
        g.firing = true;
        w.step(STEP);
    }
}

#[test]
fn shooting_every_fighter_ends_it_and_the_story_goes_on() {
    let Some(mut w) = world() else { return };
    w.global_numbers.insert("K_KOTOR_MASTER".to_string(), 15);
    w.global_booleans
        .insert("K_RANDOM_MINI_GAME".to_string(), true);
    play_to_the_end(&mut w);
    let g = w.minigame.as_ref().unwrap();
    assert_eq!(g.alive(), 0, "{} hit points left", g.player.hit_points);
    assert_eq!(w.global_numbers.get("EBO_NUM_FIGHTERS"), Some(&0));
    assert_eq!(w.global_booleans.get("EBO_TURRET_DONE"), Some(&true));
    for _ in 0..(10.0 / STEP) as usize {
        w.step(STEP);
        if w.transition_request.is_some() {
            break;
        }
    }
    assert!(
        w.transition_request.is_some() || w.module != "M12ab",
        "{:?}",
        w.errors
    );
}

#[test]
fn losing_every_hit_point_ends_it() {
    let Some(mut w) = world() else { return };
    w.minigame.as_mut().unwrap().player.hit_points = 10;
    for _ in 0..(90.0 / STEP) as usize {
        w.step(STEP);
        if w.global_booleans.get("M12AB_END_SYNC") == Some(&true) {
            break;
        }
    }
    assert_eq!(w.global_booleans.get("M12AB_END_SYNC"), Some(&true));
}
