use kotor_archive::Install;
use kotor_game::{Autopilot, Game, Kind, World};

fn world() -> Option<World> {
    let mut w = world_before_entering("end_m01aa")?;
    w.enter_area();
    Some(w)
}

fn world_before_entering(module: &str) -> Option<World> {
    let mut inst = Install::open(kotor_core::install_dir()?).ok()?;
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).ok()?;
    w.interactive_dialogue = true;
    w.load_module(module).ok()?;
    let spec = w.recommended_character("Player", 0, 1)?;
    w.create_character(&spec).ok()?;
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
    while p.time < 1500.0 && !p.finished && g.world.module != "tar_m02af" {
        p.run_steps(&mut g, 60);
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
    // Talks move quests on: this one has no script that does, only `QuestEntry` on its nodes.
    assert_eq!(g.world.journal.get("end_attack"), Some(&99));
    let events = g.world.events.join("\n");
    assert!(events.contains("Sith Apprentice (end_darkjedi01) dies"));
    assert!(events.contains("Trask (end_trask) leads the party"));
    assert!(g.world.stubbed.is_empty(), "{:?}", g.world.stubbed);
    // The way on from the second module's start is a locked door. Its answer lies in a footlocker behind another door.
    assert!(events.contains("used Footlocker (FootLker)"));
    // `end_trask01` names `k_pend_carth11`, which only `end_m01ab` has. The talk has to carry on past that reply.
    assert!(g
        .world
        .errors
        .iter()
        .any(|e| e == "script k_pend_carth11 not found"));
    assert!(g
        .world
        .transcript
        .iter()
        .any(|l| l.dialogue == "end_trask01" && l.node == 35));
}

#[test]
fn the_pilot_goes_where_it_has_been_least_and_finds_the_lower_citys_rooms() {
    let Some(w) = world() else {
        return;
    };
    let mut g = Game::new(w);
    let mut p = Autopilot::new();
    while p.time < 6000.0 && !p.finished && g.world.module != "tar_m11aa" {
        p.run_steps(&mut g, 60);
    }
    assert_eq!(g.world.module, "tar_m11aa", "{}", p.summary(&g.world));
    // On the way: the lower city's rooms, and the first door of each only once.
    for m in ["tar_m03aa", "tar_m03ab", "tar_m03ae"] {
        assert!(p.modules.iter().any(|v| v == m), "{m}: {:?}", p.modules);
    }
}

#[test]
fn a_ladder_that_took_the_pilot_away_waits_until_the_module_is_done() {
    let Some(mut w) = world() else {
        return;
    };
    w.load_module("tar_m05aa").unwrap();
    w.enter_area();
    let mut g = Game::new(w);
    let mut p = Autopilot::new();
    let run_until = |g: &mut Game, p: &mut Autopilot, module: &str, leave: bool, most: f32| {
        let end = p.time + most;
        while p.time < end && !p.finished && (g.world.module == module) == leave {
            p.run_steps(g, 60);
        }
    };
    // The ladder by the door takes it up into `tar_m04aa`, and the door there brings it back.
    run_until(&mut g, &mut p, "tar_m05aa", true, 900.0);
    run_until(&mut g, &mut p, "tar_m05aa", false, 1500.0);
    assert_eq!(g.world.module, "tar_m05aa", "{:?}", p.modules);
    let back = p.time;
    run_until(&mut g, &mut p, "tar_m05aa", true, 100.0);
    assert!(p.time >= back + 100.0, "left after {} s", p.time - back);
}

#[test]
fn the_pilot_stops_when_nothing_new_has_come_for_a_long_time() {
    let Some(w) = world() else {
        return;
    };
    let mut g = Game::new(w);
    let mut p = Autopilot::new();
    p.run_steps(&mut g, 60);
    p.time += 9000.0;
    p.run_steps(&mut g, 1);
    assert!(p.finished);
    assert_eq!(p.stalls.last().unwrap().kind, "softlock");
}

#[test]
fn the_reply_that_sets_an_unhurtable_guard_on_the_player_is_known() {
    let Some(mut w) = world() else {
        return;
    };
    w.load_module("tar_m03aa").unwrap();
    w.enter_area();
    let guard = id_of(&w, "SithPatrol031");
    let at = w.object(guard).unwrap().position;
    let pc = w.pc;
    w.teleport(pc, [at[0] + 1.0, at[1], at[2]]);
    let mut fatal = Vec::new();
    // The first talk only warns. The second offers what gets a player shot.
    for _ in 0..2 {
        assert!(w.click(guard));
        for _ in 0..30 {
            let Some((_, _, _, replies)) = w.dialogue_view() else {
                break;
            };
            let safe = (0..replies.len()).rfind(|k| !w.reply_is_fatal(*k));
            fatal.extend(
                (0..replies.len())
                    .filter(|k| w.reply_is_fatal(*k))
                    .map(|k| replies[k].clone()),
            );
            w.dialogue_choose(safe.unwrap_or(0));
        }
    }
    assert!(!w.in_dialogue());
    fatal.dedup();
    assert_eq!(fatal.len(), 1, "{fatal:?}");
    assert!(w.reputation(pc, guard) > 10, "it stays calm");
}

#[test]
fn the_pilot_takes_along_who_is_on_offer_and_leads_itself() {
    let Some(mut w) = world() else {
        return;
    };
    w.load_module("tar_m02aa").unwrap();
    w.enter_area();
    w.available.insert(2, "p_carth".to_string());
    let mut g = Game::new(w);
    let mut p = Autopilot::new();
    p.run_steps(&mut g, 60);
    assert_eq!(g.world.party_slots(), [2]);
    let carth = g.world.npc_slots[&2];
    assert!(g.world.switch_leader(carth));
    for _ in 0..30 {
        p.run_steps(&mut g, 60);
        if g.world.pc == g.world.hero {
            break;
        }
    }
    assert_eq!(g.world.pc, g.world.hero);
    assert_eq!(g.world.party, [carth], "one Carth, not a second");
}

#[test]
fn whoever_is_best_at_it_goes_up_to_a_lock() {
    let Some(mut w) = world() else {
        return;
    };
    w.load_module("tar_m02aa").unwrap();
    w.enter_area();
    w.available.insert(6, "p_mission".to_string());
    w.choose_party(&[6], "");
    let mission = w.npc_slots[&6];
    w.object_mut(mission).unwrap().skills = vec![0, 0, 0, 0, 0, 0, 6, 0];
    w.object_mut(w.pc).unwrap().skills = vec![0; 8];
    kotor_game::lead_with_the_best_at(&mut w, 6);
    assert_eq!(w.pc, mission);
    kotor_game::lead_with_the_best_at(&mut w, 6);
    assert_eq!(w.pc, mission, "nobody better now");
}

#[test]
fn a_foe_with_something_to_say_is_still_fought() {
    let Some(mut w) = world() else {
        return;
    };
    w.load_module("tar_m02ae").unwrap();
    w.enter_area();
    let mut g = Game::new(w);
    let mut p = Autopilot::new();
    p.run_steps(&mut g, 60);
    let w = &mut g.world;
    let pc = w.pc;
    let here = w.object(pc).unwrap().position;
    let foe = w
        .objects
        .values()
        .find(|o| {
            o.kind == Kind::Creature
                && o.id != pc
                && !o.plot
                && !o.conversation.is_empty()
                && (o.position[0] - here[0]).hypot(o.position[1] - here[1]) > 30.0
        })
        .unwrap()
        .id;
    w.object_mut(foe).unwrap().faction = 1;
    // It looks over who there is to talk to with this one hostile and far away.
    p.run_steps(&mut g, 60);
    let w = &mut g.world;
    let pc = w.pc;
    let here = w.object(pc).unwrap().position;
    w.teleport(foe, [here[0] + 1.0, here[1], here[2]]);
    let mut fought = false;
    for _ in 0..60 {
        p.run_steps(&mut g, 60);
        fought |= g.world.combat.target.get(&g.world.pc) == Some(&foe);
    }
    assert!(fought, "{}", p.summary(&g.world));
}

#[test]
fn a_mortal_pilot_spends_its_points_wears_its_finds_and_takes_medpacks() {
    let Some(w) = world() else {
        return;
    };
    let mut g = Game::new(w);
    let mut p = Autopilot::new();
    p.immortal = false;
    let pc = g.world.pc;
    p.run_steps(&mut g, 60);
    let o = g.world.object(pc).unwrap();
    assert!(o.skills.iter().sum::<i32>() > 0, "skills bought");
    assert_eq!(o.skill_points, 0);
    // A level waits to be taken, and is.
    g.world.grant_xp(pc, 1000);
    assert_eq!(g.world.object(pc).unwrap().level, 1);
    p.run_steps(&mut g, 60);
    assert_eq!(g.world.object(pc).unwrap().level, 2);
    // Better armour goes on, and a medpack when hurt.
    let suit = g.world.spawn_item("g_a_class4001");
    g.world.carry(pc, suit);
    let pack = g.world.spawn_item("g_i_medeqpmnt01");
    g.world.carry(pc, pack);
    // The pilot looks after itself between talks.
    for _ in 0..600 {
        if !g.world.in_dialogue() {
            break;
        }
        p.run_steps(&mut g, 60);
    }
    assert!(!g.world.in_dialogue());
    g.world.object_mut(pc).unwrap().hp = 1;
    p.run_steps(&mut g, 120);
    let o = g.world.object(pc).unwrap();
    assert_eq!(o.equipped.get(&1), Some(&suit), "the better armour");
    assert!(o.hp > 1, "the medpack worked");
}

#[test]
fn the_party_down_ends_a_mortal_run() {
    let Some(w) = world() else {
        return;
    };
    let mut g = Game::new(w);
    let mut p = Autopilot::new();
    p.immortal = false;
    g.world.game_over = true;
    p.run_steps(&mut g, 1);
    assert!(p.finished);
    assert_eq!(p.stalls.last().map(|s| s.kind), Some("death"));
}

/// What must come out the same whatever the speed: the digests on the way, the last one, and a save file.
type Played = (Vec<(u64, u64)>, u64, Vec<u8>);

fn play_in_chunks(chunk: usize) -> Option<Played> {
    let mut g = Game::new(world()?);
    let mut p = Autopilot::new();
    // About 150 game seconds, and a whole number of the largest chunk.
    let total = 1024 * 9;
    for _ in 0..total / chunk {
        p.run_steps(&mut g, chunk);
    }
    let digest = kotor_game::state_digest(&g.world);
    Some((p.digests, digest, g.world.save_bytes("speed").ok()?))
}

#[test]
fn how_steps_are_grouped_into_frames_changes_nothing() {
    let Some(one) = play_in_chunks(1) else {
        return;
    };
    assert!(one.0.len() >= 15, "{} digests", one.0.len());
    assert_eq!(Some(&one), play_in_chunks(16).as_ref());
    assert_eq!(Some(&one), play_in_chunks(1024).as_ref());
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
    // `end_door02` asks for Security at DC 1, and the hero has no ranks in it. With none they cannot try: the talk table's
    // row 256.
    let door = id_of(&w, "end_door02");
    w.object_mut(door).unwrap().locked = true;
    w.use_door(door);
    let events = w.events.join("\n");
    assert!(!events.contains("security"), "{events}");
    assert!(events.contains("is locked"), "{events}");
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

/// Aboard the Ebon Hawk, with Dantooine reached: `K_CURRENT_PLANET` is what `k_pebn_exithawk` goes by.
fn aboard_the_hawk() -> Option<World> {
    let mut w = world_before_entering("ebo_m12aa")?;
    w.global_numbers.insert("K_CURRENT_PLANET".to_string(), 15);
    w.global_numbers.insert("K_KOTOR_MASTER".to_string(), 10);
    w.enter_area();
    Some(w)
}

#[test]
fn the_pilot_walks_down_the_ramp_of_the_ebon_hawk() {
    let Some(w) = aboard_the_hawk() else {
        return;
    };
    let mut g = Game::new(w);
    let mut p = Autopilot::new();
    while p.time < 900.0 && !p.finished && g.world.module == "ebo_m12aa" {
        p.run_steps(&mut g, 60);
    }
    assert_eq!(g.world.module, "danm13", "{:?}", p.stalls.last());
}

#[test]
fn the_pilot_goes_to_each_planet_on_the_map_once() {
    let Some(mut w) = aboard_the_hawk() else {
        return;
    };
    let pc = w.pc;
    w.global_numbers.insert("K_KOTOR_MASTER".to_string(), 15);
    w.run_script("k_pebn_galaxy", pc).unwrap();
    assert!(w.galaxy.is_some());
    let mut g = Game::new(w);
    let mut p = Autopilot::new();
    p.step(&mut g, 0.05);
    assert!(g.world.galaxy.is_none(), "it chose");
    assert_eq!(
        g.world.global_numbers.get("K_FUTURE_PLANET"),
        Some(&15),
        "Dantooine first, as the table has it"
    );
    g.world.galaxy = Some(kotor_game::GalaxyOpen { current: 3 });
    p.step(&mut g, 0.05);
    assert_eq!(
        g.world.global_numbers.get("K_FUTURE_PLANET"),
        Some(&35),
        "then Tatooine"
    );
    for _ in 0..3 {
        g.world.galaxy = Some(kotor_game::GalaxyOpen { current: 3 });
        p.step(&mut g, 0.05);
    }
    let last = g.world.global_numbers.get("K_FUTURE_PLANET").copied();
    g.world.galaxy = Some(kotor_game::GalaxyOpen { current: 3 });
    p.step(&mut g, 0.05);
    assert!(g.world.galaxy.is_none(), "nowhere left: it closes the map");
    assert_eq!(g.world.global_numbers.get("K_FUTURE_PLANET").copied(), last);
}

/// The whole party down holds the world still. An immortal pilot stood through the rest of the run in `tar_m09aa`.
#[test]
fn an_immortal_pilot_gets_up_when_the_whole_party_is_down() {
    let Some(mut w) = world() else {
        return;
    };
    let pc = w.pc;
    w.object_mut(pc).unwrap().down = true;
    w.game_over = true;
    let mut g = Game::new(w);
    let mut p = Autopilot::new();
    p.run_steps(&mut g, 2);
    assert!(!g.world.game_over);
    assert!(!g.world.object(pc).unwrap().down);
}

#[test]
fn the_pilot_shoots_down_a_fighter_from_the_turret() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut w = World::new(Install::open(dir).unwrap()).unwrap();
    w.load_module("M12ab").unwrap();
    w.enter_area();
    let mut g = Game::new(w);
    let mut p = Autopilot::new();
    let hit = |g: &Game| {
        g.world
            .minigame
            .as_ref()
            .is_some_and(|m| m.enemies.iter().any(|e| e.dying.is_some() || e.gone))
    };
    while p.time < 120.0 && !hit(&g) {
        p.run_steps(&mut g, 60);
    }
    assert!(hit(&g), "none down after {} s", p.time);
}
