//! Creatures that come back: after the timer, once the player is far and out of sight; never the story's own.

use kotor_archive::Install;
use kotor_game::{ObjId, RespawnTable, World};

/// The Taris undercity (rakghouls: hostile, ambient, with the game's plain death script) and the player, with respawning on.
fn undercity() -> Option<(World, ObjId)> {
    let mut inst = Install::open(kotor_core::install_dir()?).ok()?;
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).ok()?;
    w.respawn.enabled = true;
    w.load_module("tar_m04aa").ok()?;
    let spec = w.recommended_character("Player", 0, 1)?;
    w.create_character(&spec).ok()?;
    w.enter_area();
    let pc = w.pc;
    w.object_mut(pc)?.min_one_hp = true;
    Some((w, pc))
}

/// A rakghoul of the area, not the alpha: hostile, ordinary.
fn rakghoul(w: &World) -> Option<ObjId> {
    w.objects
        .values()
        .find(|o| o.tag == "tar04_rakghoulr" && !o.dead)
        .map(|o| o.id)
}

/// Kill it as the player would, from a long way off, and clear the fight.
fn kill(w: &mut World, pc: ObjId, id: ObjId) {
    let o = w.object_mut(id).unwrap();
    o.hp = 1;
    assert!(w.hurt(pc, id, 50), "it fell");
    w.combat.target.clear();
}

/// Put the player far from `at`, with the line between them not clear, for a while.
fn go_far(w: &mut World, pc: ObjId, at: [f32; 3]) {
    w.teleport(pc, [at[0] + 300.0, at[1], at[2]]);
}

/// Everyone else of the area out of the way, so that no fight is on.
fn alone(w: &mut World, pc: ObjId) {
    let ids: Vec<ObjId> = w
        .objects
        .values()
        .filter(|o| o.kind == kotor_game::Kind::Creature && o.id != pc && !o.dead)
        .map(|o| o.id)
        .collect();
    for id in ids {
        w.teleport(id, [0.0, 0.0, -300.0]);
    }
    w.combat.target.clear();
}

fn run(w: &mut World, seconds: f32) {
    for _ in 0..(seconds / 0.5) as usize {
        w.step(0.5);
    }
}

#[test]
fn the_table_is_read_and_names_the_story_out() {
    let t = RespawnTable::get();
    assert_eq!(t.setting("range_metres", 0.0), 60.0);
    assert_eq!(t.setting("timer_seconds", 0.0), 300.0);
    assert_eq!(t.setting("require_no_line_of_sight", 0.0), 1.0);
    assert!(
        t.excluded("end_m01aa", "SithSoldier"),
        "the opening is scripted"
    );
    assert!(t.excluded("tar_m04aa", "Bastila"));
    assert!(!t.excluded("tar_m04aa", "tar04_rakghoulr"));
}

#[test]
fn an_enemy_killed_in_a_fight_comes_back_after_the_timer_when_the_player_is_far() {
    let Some((mut w, pc)) = undercity() else {
        return;
    };
    let id = rakghoul(&w).expect("a rakghoul");
    let at = w.object(id).unwrap().position;
    let xp = w.kill_xp(id);
    kill(&mut w, pc, id);
    assert!(w.object(id).unwrap().dead);
    assert_eq!(w.respawn.pending.len(), 1, "its timer is set");
    // Not before the timer, though the player is far.
    go_far(&mut w, pc, at);
    run(&mut w, 100.0);
    assert!(!w.objects.values().any(|o| o.respawned), "not yet");
    // After it: back, whole, where it began, and a new creature of the same template.
    run(&mut w, 260.0);
    assert!(w.respawn.pending.is_empty(), "it came back");
    let back = w
        .objects
        .values()
        .find(|o| o.respawned)
        .expect("a creature that came back");
    assert_eq!(back.template, "tar04_rakghoulr");
    assert_eq!(back.tag, "tar04_rakghoulr");
    assert!(!back.dead && back.hp == back.max_hp && back.hp > 1);
    assert!((back.position[0] - at[0]).abs() < 1.0 && (back.position[1] - at[1]).abs() < 1.0);
    // It pays half the experience when it falls again.
    let again = back.id;
    assert_eq!(w.kill_xp(again), xp, "the same challenge");
    let start = w.object(pc).unwrap().xp;
    let o = w.object_mut(again).unwrap();
    o.hp = 1;
    w.teleport(pc, at);
    assert!(w.hurt(pc, again, 50));
    let gained = w.object(pc).unwrap().xp - start;
    assert_eq!(gained, (xp as f32 * 0.5).round() as i32, "half of {xp}");
}

#[test]
fn nothing_comes_back_while_the_player_is_near_or_can_see_the_place() {
    let Some((mut w, pc)) = undercity() else {
        return;
    };
    let id = rakghoul(&w).unwrap();
    let at = w.object(id).unwrap().position;
    kill(&mut w, pc, id);
    alone(&mut w, pc);
    // The timer is up, but the player stands 5 m away: in range and in view.
    w.respawn.pending[0].due = 0.0;
    w.teleport(pc, [at[0] + 5.0, at[1], at[2]]);
    run(&mut w, 30.0);
    assert_eq!(w.respawn.pending.len(), 1, "near: it waits");
    // Far by distance but with the line clear: it waits while the line is needed to be broken.
    w.respawn.range_metres = 3.0;
    w.teleport(pc, [at[0] + 5.0, at[1], at[2]]);
    run(&mut w, 5.0);
    let in_view = w.can_see_along(
        [at[0] + 5.0, at[1], at[2] + 1.5],
        [at[0], at[1], at[2] + 1.5],
    );
    if in_view {
        assert_eq!(w.respawn.pending.len(), 1, "in view: it waits");
        w.respawn.require_no_line_of_sight = false;
        run(&mut w, 5.0);
        assert!(
            w.respawn.pending.is_empty(),
            "distance alone, once the owner's line row is off"
        );
    }
}

#[test]
fn nothing_comes_back_in_a_fight_or_a_conversation_or_with_the_switch_off() {
    let Some((mut w, pc)) = undercity() else {
        return;
    };
    let id = rakghoul(&w).unwrap();
    let at = w.object(id).unwrap().position;
    kill(&mut w, pc, id);
    w.respawn.pending[0].due = 0.0;
    go_far(&mut w, pc, at);
    w.respawn.enabled = false;
    run(&mut w, 20.0);
    assert_eq!(w.respawn.pending.len(), 1, "the switch is off");
    w.respawn.enabled = true;
    // A fight somewhere keeps everything waiting.
    let other = w
        .objects
        .values()
        .find(|o| o.tag == "tar04_rakghoulr" && !o.dead)
        .map(|o| o.id)
        .unwrap();
    w.combat.target.insert(other, pc);
    w.combat.target.insert(pc, other);
    run(&mut w, 5.0);
    assert_eq!(w.respawn.pending.len(), 1, "a fight is on");
}

#[test]
fn the_storys_own_are_never_queued() {
    let Some((mut w, pc)) = undercity() else {
        return;
    };
    // A plot creature cannot even be hurt. One with a conversation, a party member, one a script killed, one named in the table: none.
    let plain = rakghoul(&w).unwrap();
    let make = |w: &mut World, how: &dyn Fn(&mut kotor_game::Object)| {
        let id = w.spawn_creature("tar04_rakghoulr", [0.0, 0.0, -300.0]);
        w.respawn
            .spawn_points
            .insert(id, w.respawn.spawn_points.get(&plain).cloned().unwrap());
        let faction = w.object(plain).unwrap().faction;
        w.object_mut(id).unwrap().faction = faction;
        how(w.object_mut(id).unwrap());
        id
    };
    let talker = make(&mut w, &|o| o.conversation = "k_someone".to_string());
    let scripted = make(&mut w, &|o| {
        o.scripts
            .insert("ScriptDeath".to_string(), "k_pend_trask_7".to_string());
    });
    let mate = make(&mut w, &|o| o.party_member = true);
    for id in [talker, scripted, mate] {
        kill(&mut w, pc, id);
        assert!(w.object(id).unwrap().dead);
    }
    assert!(w.respawn.pending.is_empty(), "{:?}", w.respawn.pending);
    // A death that did not come through a blow (a script's) is not queued either.
    let silent = make(&mut w, &|_| {});
    w.object_mut(silent).unwrap().dead = true;
    assert!(w.respawn.pending.is_empty());
}

#[test]
fn the_timers_survive_a_save_and_a_load() {
    let Some((mut w, pc)) = undercity() else {
        return;
    };
    let id = rakghoul(&w).unwrap();
    let at = w.object(id).unwrap().position;
    kill(&mut w, pc, id);
    run(&mut w, 40.0);
    let left = w.respawn.pending[0].due - w.time;
    assert!(left > 200.0 && left < 270.0, "{left}");
    let bytes = w.save_bytes("respawn").expect("a save");
    let Some((mut again, _)) = undercity() else {
        return;
    };
    again.load_save(&bytes).unwrap();
    assert_eq!(again.respawn.pending.len(), 1);
    let p = &again.respawn.pending[0];
    assert_eq!(p.at.template, "tar04_rakghoulr");
    assert!((p.at.position[0] - at[0]).abs() < 0.01);
    let left_again = p.due - again.time;
    assert!((left_again - left).abs() < 1.5, "{left} then {left_again}");
}
