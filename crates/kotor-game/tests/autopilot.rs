use kotor_archive::Install;
use kotor_game::{Autopilot, Game, Kind, World};

fn world() -> Option<World> {
    let mut inst = Install::open(kotor_core::install_dir()?).ok()?;
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).ok()?;
    w.interactive_dialogue = true;
    w.load_module("end_m01aa").ok()?;
    let spec = w.recommended_character("Player", 0, 1)?;
    w.create_character(&spec).ok()?;
    w.enter_area();
    Some(w)
}

fn id_of(w: &World, tag: &str) -> u32 {
    w.by_tag(tag)[0]
}

#[test]
fn the_pilot_plays_off_the_endar_spire() {
    let Some(w) = world() else {
        return;
    };
    let mut g = Game::new(w);
    let mut p = Autopilot::new();
    let dt = 1.0 / 30.0;
    while p.time < 1500.0 && !p.finished && g.world.module != "tar_m02af" {
        p.step(&mut g, dt);
        g.advance(dt);
    }
    assert_eq!(
        g.world.module,
        "tar_m02af",
        "{}\n{:#?}",
        p.summary(&g.world),
        p.stalls
    );
    assert!(p.modules.iter().any(|m| m == "end_m01ab"));
    // On the way: Trask talked and led, the fight in the first room played out and the Jedi duel ended.
    let said = |dialogue: &str| g.world.transcript.iter().any(|l| l.dialogue == dialogue);
    assert!(said("end_trask01") && said("end_cut04") && said("end_room3"));
    let events = g.world.events.join("\n");
    assert!(events.contains("Sith Apprentice (end_darkjedi01) dies"));
    assert!(events.contains("Trask (end_trask) leads the party"));
    assert!(g.world.stubbed.is_empty(), "{:?}", g.world.stubbed);
}

#[test]
fn spawn_scripts_run_and_area_tags_win() {
    let Some(w) = world() else {
        return;
    };
    // `k_pend_sith_9` keeps these two from fighting until their cutscene turns the flag off again.
    let sith = w.object(id_of(&w, "end_sith2")).unwrap();
    assert_eq!(sith.local_booleans.get(&62), Some(&true));
    assert!(sith.spawned);
    // Every waypoint shares the template `sw_waypoint001`, and scripts find them by the tag in the area.
    let wp = w.object(id_of(&w, "end_jump_pc")).unwrap();
    assert_eq!(wp.kind, Kind::Waypoint);
    assert_eq!(wp.template, "sw_waypoint001");
}

#[test]
fn taking_from_the_footlocker_moves_the_tutorial_on() {
    let Some(mut w) = world() else {
        return;
    };
    assert_eq!(
        w.global_numbers.get("END_TRASK_DLG").copied().unwrap_or(0),
        0
    );
    let locker = id_of(&w, "end_locker01");
    let item = w.spawn_item("g_a_clothes01");
    w.object_mut(locker).unwrap().inventory.push(item);
    w.opened = Some(locker);
    w.take_item(item);
    assert_eq!(w.global_numbers.get("END_TRASK_DLG"), Some(&2));
    assert!(w.object(w.pc).unwrap().inventory.contains(&item));
}

#[test]
fn a_locked_door_checks_security_and_keeps_shut_on_a_failure() {
    let Some(mut w) = world() else {
        return;
    };
    // `end_door02` asks for Security at DC 1, and the hero has no ranks in it.
    let door = id_of(&w, "end_door02");
    w.object_mut(door).unwrap().locked = true;
    w.use_door(door);
    let events = w.events.join("\n");
    assert!(events.contains("security"), "{events}");
    let d = w.object(door).unwrap();
    assert!(d.locked && !d.open);
}

#[test]
fn a_creature_that_idles_on_its_own_queue_takes_its_time() {
    let Some(mut w) = world() else {
        return;
    };
    // The utility droid repeats an animation for ever from its own queue. A tick must end, and its events must not pile up.
    for _ in 0..300 {
        w.step(1.0 / 30.0);
    }
    let n = w
        .events
        .iter()
        .filter(|e| e.contains("ActionPlayAnimation"))
        .count();
    assert!(n < 60, "{n} animations in ten seconds");
}
