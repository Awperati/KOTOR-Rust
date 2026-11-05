use kotor_archive::Install;
use kotor_game::{pixel_ray, DoorState, Kind, ObjId, World};
use kotor_script::disassemble;

fn world() -> Option<World> {
    let dir = kotor_core::install_dir()?;
    let mut w = World::new(Install::open(dir).unwrap()).unwrap();
    w.load_module("end_m01aa").unwrap();
    Some(w)
}

fn door(w: &World, tag: &str, near_x: f32) -> ObjId {
    *w.by_tag(tag)
        .iter()
        .find(|id| (w.object(**id).unwrap().position[0] - near_x).abs() < 1.0)
        .unwrap_or_else(|| panic!("no door {tag} near x={near_x}"))
}

fn route_exists(w: &World, to: [f32; 3]) -> bool {
    let from = w.object(w.pc).unwrap().position;
    w.nav.as_ref().unwrap().find_path(from, to).is_some()
}

/// Walk the game forward until `done` or `limit` seconds, in 50 ms steps.
fn run(w: &mut World, limit: f32, mut done: impl FnMut(&World) -> bool) -> f32 {
    let mut t = 0.0;
    while t < limit && !done(w) {
        w.step(0.05);
        t += 0.05;
    }
    t
}

#[test]
fn a_closed_door_cuts_the_route_and_opening_it_restores_it() {
    let Some(mut w) = world() else { return };
    // Room m01aa_02a lies beyond the first door (end_door01 at x = 21.1).
    let beyond = [30.8, 20.7, -1.275];
    assert!(
        !route_exists(&w, beyond),
        "the closed door should block the corridor"
    );
    let d = door(&w, "end_door01", 21.1);
    assert!(w.open_door(d));
    assert!(
        !route_exists(&w, beyond),
        "the doorway stays blocked while the door swings"
    );
    assert!(matches!(w.object(d).unwrap().door, DoorState::Opening(_)));
    for _ in 0..22 {
        w.step(0.05);
    }
    assert_eq!(w.object(d).unwrap().door, DoorState::Open);
    assert!(
        route_exists(&w, beyond),
        "opening the door should unblock it"
    );
    w.close_door(d);
    assert!(!route_exists(&w, beyond), "closing it blocks again");
}

#[test]
fn a_locked_door_does_not_open() {
    let Some(mut w) = world() else { return };
    let d = door(&w, "end_door02", 40.8);
    assert!(w.object(d).unwrap().locked);
    assert!(!w.open_door(d));
    assert!(!w.object(d).unwrap().open);
    assert!(
        w.events.iter().any(|e| e.contains("is locked")),
        "{:?}",
        w.events
    );
}

#[test]
fn clicking_a_door_walks_up_to_it_and_opens_it() {
    let Some(mut w) = world() else { return };
    let d = door(&w, "end_door01", 21.1);
    assert!(w.click(d));
    assert!(w.pc_walking(), "the player should start walking");
    let t = run(&mut w, 30.0, |w| w.object(d).unwrap().open);
    assert!(w.object(d).unwrap().open, "door still closed after {t}s");
    let (p, dp) = (
        w.object(w.pc).unwrap().position,
        w.object(d).unwrap().position,
    );
    let gap = ((p[0] - dp[0]).powi(2) + (p[1] - dp[1]).powi(2)).sqrt();
    assert!(
        gap <= kotor_game::DOOR_RANGE + 0.3,
        "stopped {gap} m from the door"
    );
    // About 5.7 m at 1.6 m/s, minus the range: a few seconds, not instant and not forever.
    assert!(t > 1.0 && t < 12.0, "{t}s");
    assert!(
        w.events.iter().any(|e| e.ends_with("opens")),
        "{:?}",
        w.events
    );
}

#[test]
fn clicking_trask_walks_through_the_opened_door_and_talks() {
    let Some(mut w) = world() else { return };
    let trask = w.by_tag("end_trask")[0];
    // With the first door closed Trask cannot be reached.
    assert!(!w.click(trask));
    assert!(
        w.events.iter().any(|e| e.starts_with("cannot reach")),
        "{:?}",
        w.events
    );
    let d = door(&w, "end_door01", 21.1);
    w.click(d);
    run(&mut w, 30.0, |w| {
        w.object(d).unwrap().door == DoorState::Open
    });
    assert!(w.click(trask), "Trask is reachable once the door is open");
    run(&mut w, 40.0, |w| w.transcript.len() >= 10);
    assert!(
        w.transcript.len() >= 10,
        "only {} lines",
        w.transcript.len()
    );
    assert_eq!(w.transcript[0].speaker, "Trask");
    assert!(w.errors.is_empty(), "{:?}", w.errors);
}

#[test]
fn a_click_ray_picks_the_object_under_the_cursor() {
    let Some(w) = world() else { return };
    let trask = w.by_tag("end_trask")[0];
    let p = w.object(trask).unwrap().position;
    // From the north, so the door west of him is not in the way.
    let eye = [p[0], p[1] + 5.0, p[2] + 1.7];
    let target = [p[0], p[1], p[2] + 0.9];
    let (o, d) = pixel_ray(eye, target, 60.0, (800, 600), (400.0, 300.0));
    assert_eq!(
        w.pick(o, d),
        Some(trask),
        "the centre of the screen looks at Trask"
    );
    // Looking well away from him picks nobody.
    let (o, d) = pixel_ray(eye, target, 60.0, (800, 600), (20.0, 20.0));
    assert_eq!(w.pick(o, d), None);
    // A door is picked the same way.
    let dr = door(&w, "end_door01", 21.1);
    let dp = w.object(dr).unwrap().position;
    let (o, d) = pixel_ray(
        [dp[0] - 4.0, dp[1], dp[2] + 1.5],
        [dp[0], dp[1], dp[2] + 1.2],
        60.0,
        (800, 600),
        (400.0, 300.0),
    );
    assert_eq!(w.pick(o, d), Some(dr));
}

#[test]
fn walking_into_a_trigger_fires_its_enter_script_once() {
    let Some(mut w) = world() else { return };
    let t = w.by_tag("end_trig02")[0];
    let poly = w.object(t).unwrap().polygon.clone();
    let c = [
        poly.iter().map(|p| p[0]).sum::<f32>() / poly.len() as f32,
        poly.iter().map(|p| p[1]).sum::<f32>() / poly.len() as f32,
    ];
    w.step(0.05);
    assert!(!w.events.iter().any(|e| e.contains("enters end_trig02")));
    // Put the player inside the outline (as if walked there) and step.
    let pc = w.pc;
    let z = w.object(pc).unwrap().position[2];
    w.object_mut(pc).unwrap().position = [c[0], c[1], z];
    w.pc_mover = kotor_walkmesh::Mover::new([c[0], c[1], z], 0.0);
    w.step(0.05);
    w.step(0.05);
    let enters = w
        .events
        .iter()
        .filter(|e| e.contains("enters") && e.contains("end_trig02"))
        .count();
    assert_eq!(enters, 1, "{:?}", w.events);
    assert!(w.errors.is_empty(), "{:?}", w.errors);
    // Leaving fires nothing more for this trigger (it has no exit script) but is recorded.
    w.object_mut(pc).unwrap().position = [c[0] + 50.0, c[1] + 50.0, z];
    w.pc_mover = kotor_walkmesh::Mover::new([c[0] + 50.0, c[1] + 50.0, z], 0.0);
    w.step(0.05);
    assert!(w
        .events
        .iter()
        .any(|e| e.contains("leaves") && e.contains("end_trig02")));
}

#[test]
fn the_transition_door_moves_the_player_to_the_next_module() {
    let Some(mut w) = world() else { return };
    w.global_numbers.insert("K_TEST".into(), 7);
    w.journal.insert("test_quest".into(), 3);
    let d = w
        .by_tag("end_door10")
        .into_iter()
        .find(|id| !w.object(*id).unwrap().link_module.is_empty())
        .unwrap();
    assert_eq!(w.object(d).unwrap().link_module, "end_m01ab");
    let old_area = w.area;
    w.use_door(d);
    w.step(0.05);
    assert!(w.errors.is_empty(), "{:?}", w.errors);
    assert_eq!(w.module, "end_m01ab");
    assert_ne!(w.area, old_area);
    assert!(w.by_tag("end_trask").is_empty() || w.by_tag("end_trask").iter().all(|id| *id != 259));
    // The player arrives at the waypoint the door named, and the state carried over.
    let wp = w.by_tag("from_01a");
    assert!(!wp.is_empty(), "no waypoint from_01a in end_m01ab");
    let (pp, wpp) = (
        w.object(w.pc).unwrap().position,
        w.object(wp[0]).unwrap().position,
    );
    assert!(
        ((pp[0] - wpp[0]).powi(2) + (pp[1] - wpp[1]).powi(2)).sqrt() < 0.5,
        "{pp:?} vs {wpp:?}"
    );
    assert_eq!(w.global_numbers.get("K_TEST"), Some(&7));
    assert_eq!(w.journal.get("test_quest"), Some(&3));
    assert!(
        w.events
            .iter()
            .any(|e| e.starts_with("transition to end_m01ab")),
        "{:?}",
        w.events
    );
    // The new area has its own navigation mesh and the player stands on it.
    let nav = w.nav.as_ref().unwrap();
    assert!(
        nav.height_at(pp[0], pp[1], pp[2] + 1.0).is_some(),
        "player not on the new floor at {pp:?}"
    );
}

/// Assemble `void main() { <calls> }`: each call is (routine name, pushed arguments last-to-first, arg count).
fn call_program(w: &World, calls: &[(&str, Vec<Vec<u8>>)]) -> kotor_script::Program {
    let mut body = Vec::new();
    for (name, pushes) in calls {
        for p in pushes {
            body.extend_from_slice(p);
        }
        // An empty name only pushes; "Name/N" calls with N arguments instead of the number just pushed.
        if name.is_empty() {
            continue;
        }
        let (name, argc) = match name.split_once('/') {
            Some((n, c)) => (n, c.parse::<u8>().unwrap()),
            None => (*name, pushes.len() as u8),
        };
        let id = w
            .routine_id(name)
            .unwrap_or_else(|| panic!("no routine {name}"));
        body.extend_from_slice(&[0x05, 0x00]);
        body.extend_from_slice(&id.to_be_bytes());
        body.push(argc);
    }
    // RSADD (return slot is not needed for void), JSR main, RETN, then main.
    let mut b = vec![0x1E, 0x00, 0, 0, 0, 8, 0x20, 0x00];
    b.extend(body);
    b.extend([0x20, 0x00]);
    let mut v = b"NCS V1.0B".to_vec();
    v.extend_from_slice(&((13 + b.len()) as u32).to_be_bytes());
    v.extend(b);
    disassemble(&v).unwrap()
}

fn ci(v: i32) -> Vec<u8> {
    let mut b = vec![0x04, 0x03];
    b.extend_from_slice(&v.to_be_bytes());
    b
}
fn cs(s: &str) -> Vec<u8> {
    let mut b = vec![0x04, 0x05];
    b.extend_from_slice(&(s.len() as u16).to_be_bytes());
    b.extend_from_slice(s.as_bytes());
    b
}
fn co(v: i32) -> Vec<u8> {
    let mut b = vec![0x04, 0x06];
    b.extend_from_slice(&v.to_be_bytes());
    b
}

#[test]
fn routines_change_journal_xp_party_and_inventory() {
    let Some(mut w) = world() else { return };
    let trask = w.by_tag("end_trask")[0];
    let pc = w.pc as i32;
    // Arguments are pushed last-to-first. AddJournalQuestEntry(tag, state), GiveXPToCreature(creature, xp),
    // AddPartyMember(slot, creature), CreateItemOnObject(template, target).
    let prog = call_program(
        &w,
        &[
            ("AddJournalQuestEntry", vec![ci(2), cs("end_quest")]),
            ("GiveXPToCreature", vec![ci(250), co(pc)]),
            ("GiveXPToCreature", vec![ci(50), co(pc)]),
            ("AddPartyMember", vec![co(trask as i32), ci(0)]),
            (
                "CreateItemOnObject",
                vec![ci(1), co(trask as i32), cs("g_w_blstrpstl001")],
            ),
        ],
    );
    w.run_program(&prog, pc as u32).unwrap();
    assert_eq!(w.journal.get("end_quest"), Some(&2));
    assert_eq!(w.object(w.pc).unwrap().xp, 300);
    assert_eq!(w.party, vec![trask]);
    assert!(w.object(trask).unwrap().party_member);
    let inv = &w.object(trask).unwrap().inventory;
    assert_eq!(inv.len(), 1);
    let item = w.object(inv[0]).unwrap();
    assert_eq!(
        (item.kind, item.template.as_str()),
        (Kind::Item, "g_w_blstrpstl001")
    );
    assert!(
        item.base_item > 0,
        "base item type should come from the UTI"
    );
    // A party member leaves the module with the player, and so does the item it carries.
    let before = inv[0];
    w.use_door(
        w.by_tag("end_door10")
            .into_iter()
            .find(|id| !w.object(*id).unwrap().link_module.is_empty())
            .unwrap(),
    );
    w.step(0.05);
    assert_eq!(w.module, "end_m01ab");
    assert!(
        w.object(trask).is_some(),
        "the party member must come along"
    );
    assert!(w.object(before).is_some(), "carried items must come along");
    assert_eq!(w.object(w.pc).unwrap().xp, 300);
    assert!(w.errors.is_empty(), "{:?}", w.errors);
}

fn cf(v: f32) -> Vec<u8> {
    let mut b = vec![0x04, 0x04];
    b.extend_from_slice(&v.to_be_bytes());
    b
}

#[test]
fn effects_jumps_factions_and_experience_levels_work() {
    let Some(mut w) = world() else { return };
    let trask = w.by_tag("end_trask")[0];
    let pc = w.pc;
    // ApplyEffectToObject(nDurationType, eEffect, oTarget, fDuration): EffectDeath() is made between the pushes.
    let prog = call_program(
        &w,
        &[
            ("", vec![cf(0.0), co(trask as i32)]),
            ("EffectDeath/0", vec![]),
            ("", vec![ci(0)]),
            ("ApplyEffectToObject/4", vec![]),
            ("ActionJumpToObject/1", vec![co(trask as i32)]),
        ],
    );
    w.run_program(&prog, pc).unwrap();
    assert!(w.object(trask).unwrap().dead, "EffectDeath should kill");
    // Death acts at once and is not kept as an active effect.
    assert_eq!(w.object(trask).unwrap().effects.len(), 0);
    w.step(0.05);
    assert_eq!(
        w.object(pc).unwrap().position,
        w.object(trask).unwrap().position,
        "the queued jump should land on the target"
    );
    assert!(w.level_for_xp(0) == 1 && w.level_for_xp(1_000_000) > 1);
    let rep = w.reputation(pc, trask);
    assert!((0..=100).contains(&rep), "{rep}");
    assert!(w.strref_text(-1).is_empty());
    assert!(w.errors.is_empty(), "{:?}", w.errors);
}

fn flat_dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Open end_door01 and let it finish swinging.
fn open_first_door(w: &mut World) {
    let d = door(w, "end_door01", 21.1);
    assert!(w.open_door(d));
    for _ in 0..22 {
        w.step(0.05);
    }
}

#[test]
fn a_party_member_follows_the_walking_player() {
    let Some(mut w) = world() else { return };
    open_first_door(&mut w);
    let trask = w.by_tag("end_trask")[0];
    // Trask joins the party and starts beside the player.
    w.party.push(trask);
    w.object_mut(trask).unwrap().party_member = true;
    let start = w.object(w.pc).unwrap().position;
    w.teleport(trask, [start[0] - 1.0, start[1], start[2]]);
    assert!(w.walk_to([30.8, 20.7, -1.275]), "the far room is reachable");
    let mut worst: f32 = 0.0;
    let t = run(&mut w, 40.0, |w| {
        worst = worst.max(flat_dist(
            w.object(w.pc).unwrap().position,
            w.object(trask).unwrap().position,
        ));
        !w.pc_walking()
    });
    assert!(!w.pc_walking(), "the player never arrived ({t}s)");
    assert!(
        worst <= kotor_game::FOLLOW_START + 0.5,
        "follower fell {worst} m behind"
    );
    for _ in 0..200 {
        w.step(0.05);
    }
    let gap = flat_dist(
        w.object(w.pc).unwrap().position,
        w.object(trask).unwrap().position,
    );
    assert!(
        gap <= kotor_game::FOLLOW_START,
        "follower settled {gap} m away"
    );
    assert!(w.errors.is_empty(), "{:?}", w.errors);
}

#[test]
fn the_player_goes_around_a_creature_in_its_way() {
    let Some(mut w) = world() else { return };
    open_first_door(&mut w);
    let trask = w.by_tag("end_trask")[0];
    // Put Trask in the middle of the walk from the spawn to the far room, on the straight line.
    let a = w.object(w.pc).unwrap().position;
    let goal = [30.8, 20.7, -1.275];
    let mid = [(a[0] + goal[0]) / 2.0, (a[1] + goal[1]) / 2.0, a[2]];
    w.teleport(trask, mid);
    assert!(w.walk_to(goal));
    let mut closest = f32::MAX;
    run(&mut w, 40.0, |w| {
        closest = closest.min(flat_dist(
            w.object(w.pc).unwrap().position,
            w.object(trask).unwrap().position,
        ));
        !w.pc_walking()
    });
    assert!(!w.pc_walking(), "the player got stuck behind the creature");
    assert!(
        closest >= 2.0 * kotor_game::CREATURE_RADIUS - 0.01,
        "walked into the creature: {closest} m"
    );
}

#[test]
fn doors_pose_through_their_clips() {
    use kotor_game::Game;
    let Some(w) = world() else { return };
    let mut g = Game::new(w);
    let d = door(&g.world, "end_door01", 21.1);
    let clip = |g: &Game| {
        g.poses()
            .into_iter()
            .find(|p| p.id == d)
            .and_then(|p| p.clip)
            .unwrap()
    };
    assert_eq!(clip(&g).name, "closed");
    assert!(g.world.open_door(d));
    g.advance(0.25);
    let c = clip(&g);
    assert_eq!(c.name, "opening1");
    assert!(c.time > 0.1 && c.time < 0.4, "{c:?}");
    g.advance(0.25);
    g.advance(0.25);
    g.advance(0.25);
    g.advance(0.25);
    assert_eq!(clip(&g).name, "opened1");
    g.world.close_door(d);
    g.advance(0.1);
    assert_eq!(clip(&g).name, "closing1");
}

#[test]
fn the_fixed_step_loop_joins_movement_clips_and_poses() {
    use kotor_game::Game;
    let Some(w) = world() else { return };
    let mut g = Game::new(w);
    let d = door(&g.world, "end_door01", 21.1);
    let pc = g.world.pc;
    // Real frames of 1/30 s feed two fixed steps of 1/60 s each.
    assert_eq!(g.advance(1.0 / 30.0 + 1e-4), 2);
    assert_eq!(
        g.clip(pc).map(|c| c.name),
        Some("pause1"),
        "idle before any order"
    );
    assert!(g.world.click(d));
    g.advance(0.1);
    assert_eq!(g.clip(pc).map(|c| c.name), Some("walk"));
    let start = g.world.time;
    let mut frames = 0;
    while !g.world.object(d).unwrap().open && frames < 1000 {
        g.advance(1.0 / 30.0 + 1e-4);
        frames += 1;
    }
    assert!(g.world.object(d).unwrap().open, "the door never opened");
    // Simulated time equals steps x dt, and about equals the frames fed.
    assert!(
        (g.world.time - g.steps as f32 / 60.0).abs() < 0.01,
        "{} vs {}",
        g.world.time,
        g.steps
    );
    assert!((g.world.time - start - frames as f32 / 30.0).abs() < 0.2);
    // After arrival the player idles again, and the clip restarts from zero.
    g.advance(0.1);
    assert_eq!(g.clip(pc).map(|c| c.name), Some("pause1"));
    assert!(g.clip(pc).unwrap().time < 0.2);
    let pose = g.poses().into_iter().find(|p| p.id == pc).unwrap();
    assert!(
        pose.heading.abs() < 0.5,
        "the player walked east: heading {}",
        pose.heading
    );
    assert!(g.poses().iter().any(|p| p.kind == Kind::Door));
    // A stall does not make the loop run away: a long pause is capped at a quarter of a second (15 steps).
    assert!(g.advance(10.0) <= 16);
    assert!(g.world.errors.is_empty(), "{:?}", g.world.errors);
}
