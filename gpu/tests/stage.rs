//! A headless run of the loop a window drives: conversation, click-to-walk, doors, and a frame on the GPU.
use kotor_game::{DoorState, Kind};
use kotor_gpu::stage::{Cue, Stage};
use kotor_gpu::{composite, GpuRenderer};
use kotor_render::Scene;

const SIZE: (u32, u32) = (640, 360);
const DT: f32 = 1.0 / 30.0;

fn stage() -> Option<Stage> {
    kotor_core::install_dir()?;
    Some(Stage::open("end_m01aa", true).unwrap())
}

/// Finish whatever conversation is on, taking the first reply.
fn play_out(s: &mut Stage, cues: &mut Vec<Cue>) {
    for _ in 0..6000 {
        cues.extend(s.update(DT));
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        } else if s
            .game
            .world
            .events
            .iter()
            .any(|e| e.contains(" ends after"))
        {
            return;
        }
    }
    panic!("the conversation did not end");
}

fn bytes_of(scene: &Scene) -> Vec<[f32; 3]> {
    scene
        .meshes
        .iter()
        .flat_map(|m| m.positions.iter().copied())
        .collect()
}

#[test]
fn the_loop_plays_a_conversation_walks_opens_a_door_and_draws() {
    let Some(mut s) = stage() else { return };
    assert!(s.game.world.interactive_dialogue);
    let mut cues = s.update(DT);
    assert!(
        cues.iter()
            .any(|c| matches!(c, Cue::Music(_) | Cue::StopMusic)),
        "{cues:?}"
    );
    assert!(s.overlay().is_some(), "HUD or dialogue box");

    // The opening conversation waits for the player, line by line.
    let mut waited = false;
    for _ in 0..300 {
        cues.extend(s.update(DT));
        if s.game.world.in_dialogue() {
            waited = true;
            break;
        }
    }
    assert!(waited, "the opening conversation should hold");
    // Its first beat looks through the camera model: the file's narrow lens, a wide shot of the bunks.
    let cut = s.camera();
    assert_eq!(cut.fov, 34.5, "{cut:?}");
    let here = s.game.world.object(s.game.world.pc).unwrap().position;
    let away = ((cut.eye[0] - here[0]).powi(2) + (cut.eye[1] - here[1]).powi(2)).sqrt();
    assert!(away < 15.0, "the camera model is in world space: {cut:?}");
    assert!(s.label().contains("from the cutscene"), "{}", s.label());
    let v0 = s.overlay_version;
    let first = s.game.world.dialogue_view().unwrap();
    assert!(!first.1.is_empty(), "{first:?}");
    play_out(&mut s, &mut cues);
    assert!(s.lines_shown >= 2, "{} lines", s.lines_shown);

    // Trask's talk offers replies; a number key takes one.
    s.game.world.events.clear();
    let trask = s.game.world.by_tag("end_trask")[0];
    s.game.world.talk_to(trask);
    let mut offered = 0;
    for _ in 0..3000 {
        cues.extend(s.update(DT));
        match s.game.world.dialogue_view() {
            Some((_, _, _, replies)) if !replies.is_empty() => {
                offered = replies.len();
                let said = s.game.world.transcript.len();
                s.number_key(offered + 1);
                assert_eq!(s.game.world.transcript.len(), said, "no such reply");
                s.number_key(offered);
                assert!(s.game.world.transcript.len() > said, "the reply is said");
                break;
            }
            Some(_) => s.continue_line(),
            None => break,
        }
    }
    assert!(offered > 0, "a reply list was offered");
    play_out(&mut s, &mut cues);
    assert!(
        s.overlay_version > v0,
        "the overlay changed back to the HUD"
    );
    assert!(!s.game.world.in_dialogue());
    assert!(
        cues.iter().any(|c| matches!(c, Cue::Voice(_))) || cues.contains(&Cue::StopVoice),
        "{cues:?}"
    );

    // A click on the floor walks there.
    s.game.world.events.clear();
    let before = s.game.world.object(s.game.world.pc).unwrap().position;
    let cam = s.camera();
    let mut hit = None;
    'find: for y in (SIZE.1 / 2..SIZE.1).rev().step_by(20) {
        for x in (40..SIZE.0 - 40).step_by(40) {
            let (o, d) =
                kotor_game::pixel_ray(cam.eye, cam.target, cam.fov, SIZE, (x as f32, y as f32));
            if s.game.world.pick(o, d).is_some() {
                continue;
            }
            if let Some(p) = s.floor_under_ray(o, d) {
                let (dx, dy) = (p[0] - before[0], p[1] - before[1]);
                if (dx * dx + dy * dy).sqrt() > 1.0 {
                    hit = Some((x as f32, y as f32));
                    break 'find;
                }
            }
        }
    }
    let px = hit.expect("floor somewhere on screen, a metre away");
    s.click(px, SIZE);
    assert!(s.game.world.pc_walking(), "the click started a walk");
    let still = bytes_of(&s.dynamic_scene());
    for _ in 0..30 {
        s.update(DT);
    }
    let after = s.game.world.object(s.game.world.pc).unwrap().position;
    let moved = ((after[0] - before[0]).powi(2) + (after[1] - before[1]).powi(2)).sqrt();
    assert!(moved > 0.2, "walked {moved} m");
    assert_eq!(
        s.game.clip(s.game.world.pc).map(|c| c.name),
        Some("walk"),
        "the player plays its walk clip"
    );
    assert_ne!(bytes_of(&s.dynamic_scene()), still, "bodies move");

    // A door swings: its posed vertices change as the clip runs.
    let door = s
        .game
        .world
        .objects
        .values()
        .find(|o| o.kind == Kind::Door && !o.locked && !o.open)
        .map(|o| o.id)
        .expect("an unlocked closed door");
    let closed = bytes_of(&s.dynamic_scene());
    assert!(s.game.world.open_door(door));
    for _ in 0..12 {
        s.update(DT);
    }
    assert!(matches!(
        s.game.world.object(door).unwrap().door,
        DoorState::Opening(_) | DoorState::Open
    ));
    assert_ne!(bytes_of(&s.dynamic_scene()), closed);

    // The frame: scene on the GPU, overlay on top. A machine with no adapter skips this part.
    let Ok(mut gpu) = GpuRenderer::new(SIZE.0, SIZE.1) else {
        return;
    };
    let scene = s.static_scene().clone();
    gpu.set_static_scene(&scene, &mut |n| s.texture(n));
    let moving = s.dynamic_scene();
    let cam = s.camera();
    let mut img = gpu.render(&moving, &mut |n| s.texture(n), &cam).unwrap();
    let plain = img.rgba.clone();
    assert!(plain.chunks(4).any(|p| p[0] > 60), "the frame has light");
    composite(&mut img, s.overlay().unwrap());
    assert_ne!(img.rgba, plain, "the overlay shows");
    assert!(
        s.game.world.errors.is_empty(),
        "{:?}",
        &s.game.world.errors[..s.game.world.errors.len().min(5)]
    );
}

#[test]
fn a_fight_plays_clips_the_bodies_have() {
    let Some(mut s) = stage() else { return };
    let pc = s.game.world.pc;
    let foe = s
        .game
        .world
        .objects
        .values()
        .find(|o| o.template == "n_sithsoldier003")
        .map(|o| o.id)
        .expect("a Sith soldier");
    let at = s.game.world.object(foe).unwrap().position;
    s.game.world.teleport(pc, [at[0] - 1.5, at[1], at[2]]);
    s.game.world.attack(pc, foe);
    let mut seen = Vec::new();
    for _ in 0..600 {
        s.update(DT);
        if s.game.world.in_dialogue() {
            s.continue_line();
            s.number_key(1);
        }
        if let Some(c) = s.game.clip(pc) {
            if !seen.contains(&c.name) {
                seen.push(c.name);
            }
        }
    }
    assert!(!s.game.world.combat.log.is_empty(), "blows were struck");
    assert!(
        seen.iter().any(|n| n.len() == 4 && n.ends_with("a1")),
        "the attacker swung: {seen:?}"
    );
    let appearance = s.game.world.object(pc).unwrap().appearance as usize;
    let dir = kotor_core::install_dir().unwrap();
    let mut inst = kotor_archive::Install::open(dir).unwrap();
    let rig = kotor_render::rig_for_appearance(&mut inst, appearance, 1).unwrap();
    for name in &seen {
        assert!(rig.animation_length(name).is_some(), "no clip {name}");
    }
}

/// Run until the wordless opening has played.
fn past_the_opening(s: &mut Stage) {
    for _ in 0..3000 {
        s.update(DT);
        if s.game
            .world
            .events
            .iter()
            .any(|e| e.contains("m01aa_c01 ends"))
        {
            return;
        }
        s.continue_line();
    }
    panic!("the opening did not end");
}

/// Run on past Trask's first talk, which the opening's end script starts a moment later.
fn past_the_talk(s: &mut Stage) {
    past_the_opening(s);
    let mut talked = false;
    for _ in 0..3000 {
        s.update(DT);
        if s.game.world.in_dialogue() {
            talked = true;
            s.number_key(1);
            s.continue_line();
        } else if talked {
            return;
        }
    }
    panic!("Trask's talk did not come and go");
}

#[test]
fn the_camera_finds_trask_when_he_talks_and_scripts_can_walk_him() {
    let Some(mut s) = stage() else { return };
    past_the_opening(&mut s);
    let trask = s.game.world.by_tag("end_trask")[0];
    let mut talking = false;
    for _ in 0..30 {
        s.update(DT);
        if s.game.world.in_dialogue() && s.game.world.dialogue_owner() == Some(trask) {
            talking = true;
            break;
        }
    }
    assert!(talking, "the opening's end script starts Trask's talk");
    let at = s.game.world.object(trask).unwrap().position;
    let cam = s.camera();
    // Either the node's own camera or the close shot: he is inside the frame.
    let look = (cam.target[1] - cam.eye[1]).atan2(cam.target[0] - cam.eye[0]);
    let to_him = (at[1] - cam.eye[1]).atan2(at[0] - cam.eye[0]);
    let off = (to_him - look + 9.0 * std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;
    assert!(off.abs().to_degrees() < cam.fov, "{cam:?} {at:?}");
    while s.game.world.in_dialogue() {
        s.number_key(1);
        s.continue_line();
        s.update(DT);
    }
    // A script's move action walks him over time. (Not to the player: a locked door lies between them.)
    let start = s.game.world.object(trask).unwrap().position;
    let nav = s.game.world.nav.as_ref().unwrap();
    let goal = [[3.0, 0.0], [-3.0, 0.0], [0.0, 3.0], [0.0, -3.0]]
        .iter()
        .find_map(|d: &[f32; 2]| {
            let (x, y) = (start[0] + d[0], start[1] + d[1]);
            Some([x, y, nav.walkable_at(x, y, start[2] + 0.5)?])
        })
        .expect("floor 3 m from Trask");
    s.game.world.queue(
        trask,
        kotor_game::Action::MoveTo {
            target: None,
            to: goal,
            range: 0.2,
            run: false,
            walker: None,
        },
    );
    s.update(DT);
    let step = s.game.world.object(trask).unwrap().position;
    assert!(
        (step[0] - start[0]).hypot(step[1] - start[1]) < 0.5,
        "he walks, he does not jump"
    );
    for _ in 0..300 {
        s.update(DT);
    }
    let end = s.game.world.object(trask).unwrap().position;
    assert!(
        (end[0] - goal[0]).hypot(end[1] - goal[1]) < 0.5,
        "{start:?} {goal:?} {end:?}"
    );
}

#[test]
fn wasd_runs_relative_to_the_camera_and_stops_at_walls() {
    let Some(mut s) = stage() else { return };
    past_the_talk(&mut s);
    let pc = s.game.world.pc;
    s.yaw = 0.0;
    let start = s.game.world.object(pc).unwrap().position;
    // Right of a camera looking along +x is -y.
    s.steer = (0.0, 1.0, false);
    for _ in 0..15 {
        s.update(DT);
    }
    assert!(!s.game.world.in_dialogue());
    let moved = s.game.world.object(pc).unwrap().position;
    assert!(moved[1] < start[1] - 0.5, "{start:?} to {moved:?}");
    assert!((moved[0] - start[0]).abs() < 0.3, "{start:?} to {moved:?}");
    assert_eq!(s.game.clip(pc).map(|c| c.name), Some("run"));
    // Holding on, the player ends up against something and stays on the floor.
    for _ in 0..600 {
        s.update(DT);
        s.continue_line();
        s.number_key(1);
    }
    let end = s.game.world.object(pc).unwrap().position;
    assert!(
        s.game
            .world
            .nav
            .as_ref()
            .unwrap()
            .walkable_at(end[0], end[1], end[2] + 0.5)
            .is_some(),
        "{end:?}"
    );
}

#[test]
fn a_footlocker_opens_and_gives_up_its_items() {
    let Some(mut s) = stage() else { return };
    past_the_talk(&mut s);
    let w = &mut s.game.world;
    let locker = w.by_tag("end_locker01")[0];
    let at = w.object(locker).unwrap().position;
    let pc = w.pc;
    w.teleport(pc, [at[0] + 0.5, at[1], at[2]]);
    w.click(locker);
    assert_eq!(s.game.world.opened, Some(locker), "the locker opened");
    let items = s.game.world.container_items();
    assert!(!items.is_empty(), "the opening script fills this locker");
    let before = s.game.world.object(pc).unwrap().inventory.len();
    s.update(DT);
    // The row list is on screen, and a click on the first row takes the first item.
    let rows = kotor_ui_row(&mut s);
    s.click(rows, (853, 480));
    assert_eq!(
        s.game.world.object(pc).unwrap().inventory.len(),
        before + 1,
        "a click took one item"
    );
    s.number_key(s.game.world.container_items().len() + 1);
    assert!(s.game.world.container_items().is_empty());
    assert_eq!(
        s.game.world.object(pc).unwrap().inventory.len(),
        before + items.len()
    );
    s.continue_line();
    assert_eq!(s.game.world.opened, None);
}

/// A point on the first row of the choice list, on a 853x480 window.
fn kotor_ui_row(s: &mut Stage) -> (f32, f32) {
    s.set_view((853, 480));
    (120.0, 385.0)
}

#[test]
fn soldiers_hold_their_weapons() {
    let Some(mut s) = stage() else { return };
    let pc = s.game.world.pc;
    let foe = s
        .game
        .world
        .objects
        .values()
        .find(|o| o.template == "n_sithsoldier003")
        .map(|o| o.id)
        .expect("a Sith soldier");
    assert!(
        s.game.world.object(foe).unwrap().equipped.contains_key(&4),
        "it has a weapon in hand"
    );
    let at = s.game.world.object(foe).unwrap().position;
    s.game.world.teleport(pc, [at[0] - 1.5, at[1], at[2]]);
    s.update(DT);
    let scene = s.dynamic_scene();
    assert!(
        scene.meshes.iter().any(|m| m
            .texture
            .as_ref()
            .is_some_and(|t| t.to_ascii_lowercase().starts_with("w_"))),
        "a weapon's mesh is drawn"
    );
}

#[test]
fn worn_armour_changes_the_body() {
    let Some(mut s) = stage() else { return };
    let pc = s.game.world.pc;
    let body = |s: &mut Stage| -> Vec<String> {
        let at = s.game.world.object(s.game.world.pc).unwrap().position;
        let scene = s.dynamic_scene();
        scene
            .meshes
            .iter()
            .filter(|m| {
                m.positions
                    .iter()
                    .any(|p| (p[0] - at[0]).hypot(p[1] - at[1]) < 0.6)
            })
            .filter_map(|m| m.texture.clone())
            .collect()
    };
    s.update(DT);
    let plain = body(&mut s);
    assert!(!plain.is_empty(), "the player has a body");
    let armour = s.game.world.spawn_item("g_a_class4001");
    s.game
        .world
        .object_mut(pc)
        .unwrap()
        .equipped
        .insert(1, armour);
    let worn = body(&mut s);
    assert_ne!(plain, worn, "another column of appearance.2da");
    assert!(
        worn.iter()
            .any(|t| t.to_ascii_uppercase().starts_with("PFBC")),
        "class 4 armour is column c: {worn:?}"
    );
}

#[test]
fn the_hud_shows_the_target_and_its_action() {
    let Some(mut s) = stage() else { return };
    past_the_talk(&mut s);
    s.game.world.events.clear();
    assert!(s.stand_near("end_locker01"));
    s.update(DT);
    let with = s.overlay().unwrap().clone();
    assert!(s.state_line().contains("target="), "{}", s.state_line());
    assert!(!s.state_line().ends_with("target=-"), "{}", s.state_line());
    // The action slot at (40, 384) to (104, 448) of the layout has something in it.
    let w = with.width as usize;
    let inked = (390..440)
        .flat_map(|y| (50..100).map(move |x| (x, y)))
        .filter(|(x, y)| with.rgba[(y * w + x) * 4 + 3] > 100)
        .count();
    assert!(inked > 500, "{inked} inked pixels in the slot");
    // A click on the slot uses the target: the locker opens.
    s.click((70.0, 410.0), (with.width, with.height));
    s.update(DT);
    assert!(s.game.world.opened.is_some(), "{}", s.state_line());
}
