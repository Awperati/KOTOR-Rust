//! The turret on the screen, without a window.

use kotor_gpu::stage::Stage;

const DT: f32 = 1.0 / 30.0;

fn at_turret() -> Option<Stage> {
    kotor_core::install_dir()?;
    let mut s = Stage::open("M12ab", true).ok()?;
    s.update(DT);
    Some(s)
}

#[test]
fn the_mouse_and_keys_turn_it_the_button_fires_and_there_is_no_hud() {
    let Some(mut s) = at_turret() else { return };
    assert!(s.turret_on());
    assert!(s.overlay().is_none(), "the cockpit has its own dials");
    assert_eq!(s.camera().fov, 65.0);
    let looks = s.camera().target;

    s.turret_look(50.0, 0.0);
    let offset = s.game.world.minigame.as_ref().unwrap().offset;
    assert!(offset[2] < 0.0, "right is round to the right: {offset:?}");
    assert_ne!(s.camera().target, looks);

    s.turret_look(0.0, -50.0);
    let up = s.game.world.minigame.as_ref().unwrap().offset[0];
    assert!(up > offset[0], "up is up");

    s.steer = (1.0, 0.0, false);
    s.update(DT);
    s.steer = (0.0, 0.0, false);
    assert!(s.game.world.minigame.as_ref().unwrap().offset[0] > up);
    assert!(s.game.world.steer.is_none(), "nobody walks");

    // A tap fires once, and the two guns have one shot each.
    let shots = |s: &Stage| {
        let g = s.game.world.minigame.as_ref().unwrap();
        g.bullets
            .iter()
            .filter(|b| b.model == "mgb_ebonleft")
            .count()
    };
    assert_eq!(shots(&s), 0);
    s.turret_fire(true);
    s.turret_fire(false);
    s.update(DT);
    assert_eq!(shots(&s), 2);
}

#[test]
fn a_screen_over_it_stops_it_and_takes_the_mouse() {
    let Some(mut s) = at_turret() else { return };
    let turn = s.game.world.minigame.as_ref().unwrap().offset;
    s.escape();
    assert!(s.menu_open());
    assert!(!s.turret_on());
    let time = s.game.world.time;
    s.update(DT);
    assert_eq!(s.game.world.time, time);
    s.escape();
    assert!(s.turret_on());
    assert_eq!(s.game.world.minigame.as_ref().unwrap().offset, turn);
}

#[test]
fn the_scene_has_the_cockpit_the_sky_and_each_fighter_until_it_goes() {
    let Some(mut s) = at_turret() else { return };
    let count = |s: &mut Stage, name: &str| {
        s.dynamic_scene()
            .meshes
            .iter()
            .filter(|m| m.name.eq_ignore_ascii_case(name))
            .count()
    };
    assert_eq!(count(&mut s, "sith_wing"), 6);
    assert_eq!(count(&mut s, "Sphere01"), 1, "the stars");
    assert_eq!(count(&mut s, "target"), 1, "the crosshair");

    // A shot at one, and it comes apart, and then it is gone.
    let g = s.game.world.minigame.as_mut().unwrap();
    g.enemies[0].hit_points = 1;
    let target = g.enemies[0].at;
    g.point_at(target);
    s.turret_fire(true);
    for _ in 0..(40.0 / DT) as usize {
        let g = s.game.world.minigame.as_mut().unwrap();
        let first = g.enemies[0].at;
        g.point_at(first);
        s.update(DT);
        if s.game.world.minigame.as_ref().unwrap().enemies[0].gone {
            break;
        }
    }
    assert!(s.game.world.minigame.as_ref().unwrap().enemies[0].gone);
    // Whoever else was in the way has gone too, once the last shots have landed and the last burst is over: a fighter
    // that is dying may or may not show.
    s.turret_fire(false);
    let settled = |s: &Stage| {
        let g = s.game.world.minigame.as_ref().unwrap();
        g.bullets.iter().all(|b| b.model != "mgb_ebonleft")
            && g.enemies.iter().all(|e| e.gone || e.dying.is_none())
    };
    for _ in 0..(20.0 / DT) as usize {
        if settled(&s) {
            break;
        }
        s.update(DT);
    }
    assert!(settled(&s));
    let left = s
        .game
        .world
        .minigame
        .as_ref()
        .unwrap()
        .enemies
        .iter()
        .filter(|e| !e.gone)
        .count();
    assert_eq!(count(&mut s, "sith_wing"), left);
    assert!(left < 6);
}
