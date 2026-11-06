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

#[test]
fn a_race_draws_from_its_camera_and_the_space_key_shifts() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let mut s = Stage::open("tar_m03mg", true).unwrap();
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    // The lights take 5 s, with the clock and the meter of the camera's model in a frame's scene.
    for _ in 0..(5.2 / DT) as usize {
        s.update(DT);
    }
    assert_eq!(s.game.world.global_numbers["MIN_RACE_GEAR"], 0);
    let (scene, camera, figure) = s.frame_scene();
    assert!(!figure, "the rooms are there too");
    assert!(scene.meshes.iter().any(|m| m
        .texture
        .as_deref()
        .is_some_and(|t| t.eq_ignore_ascii_case("LMG_Numbers"))));
    // Behind the bike, and looking along the track.
    assert!(
        camera.eye[1] < 0.0 && camera.target[1] > camera.eye[1],
        "{camera:?}"
    );
    assert!(s.overlay().is_none(), "no HUD over the race's own");
    s.space_key();
    for _ in 0..30 {
        s.update(DT);
    }
    assert_eq!(s.game.world.global_numbers["MIN_RACE_GEAR"], 1);
    let game = s.game.world.race.as_ref().unwrap();
    assert!(game.speed > 10.0, "{}", game.speed);
    // Rightward keys take it right.
    s.steer = (0.0, 1.0, false);
    for _ in 0..30 {
        s.update(DT);
    }
    let x = s.game.world.race.as_ref().unwrap().position()[0];
    assert!(x > 100.5, "{x}");
}

#[test]
fn a_duel_in_another_room_cannot_be_heard_from_where_a_new_game_starts() {
    let Some(mut s) = stage() else {
        return;
    };
    s.enable_tutorials(false);
    let mut heard = 0;
    for _ in 0..(16.0 / DT) as usize {
        s.number_key(1);
        s.continue_line();
        for c in s.update(DT) {
            if s.game.world.combat.log.len() > 1
                && matches!(c, Cue::Effect(_) | Cue::FaintEffect(..))
            {
                heard += 1;
            }
        }
    }
    assert!(s.game.world.combat.log.len() >= 3, "the duel goes on");
    assert_eq!(heard, 0);
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
                // The camera holds the speaker while the line is said, then cuts to the player for the replies.
                let looks_at_player = |s: &Stage| {
                    let aim = s.speaker_camera().unwrap().target;
                    let from = |id| {
                        let p = s.game.world.object(id).unwrap().position;
                        (p[0] - aim[0]).hypot(p[1] - aim[1])
                    };
                    from(s.game.world.pc) < from(trask)
                };
                if !s.answering() {
                    assert!(!looks_at_player(&s), "the speaker has the shot");
                }
                for _ in 0..3000 {
                    if s.answering() {
                        break;
                    }
                    cues.extend(s.update(DT));
                }
                assert!(s.answering(), "the line ends");
                assert!(looks_at_player(&s), "the player has the shot");
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
    for _ in 0..4 {
        s.update(DT);
    }
    let after = s.game.world.object(s.game.world.pc).unwrap().position;
    let moved = ((after[0] - before[0]).powi(2) + (after[1] - before[1]).powi(2)).sqrt();
    assert!(moved > 0.2, "ran {moved} m");
    assert_eq!(
        s.game.clip(s.game.world.pc).map(|c| c.name),
        Some("run"),
        "the player plays its run clip"
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
    for _ in 0..8 {
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
    s.set_view((853, 480));
    assert_eq!(s.menu_name(), "container");
    let row = s.menu_pixel("LB_ITEMS", Some(0), (853, 480)).unwrap();
    s.click(row, (853, 480));
    assert_eq!(
        s.game.world.object(pc).unwrap().inventory.len(),
        before + 1,
        "a click took one item"
    );
    s.number_key(s.game.world.container_items().len() + 1);
    assert!(s.game.world.container_items().is_empty());
    // Identical items share a stack.
    let carried: i32 = s
        .game
        .world
        .carried(pc)
        .iter()
        .map(|i| s.game.world.object(*i).unwrap().stack)
        .sum();
    assert_eq!(carried as usize, before + items.len());
    s.continue_line();
    assert_eq!(s.game.world.opened, None);
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
    // The first action slot, at the bottom right, has something in it.
    let w = with.width as usize;
    let k = with.height as f32 / 480.0;
    let size = (with.width, with.height);
    let at = s.hud_pixel("LBL_ACTION0", size).unwrap();
    let side = (24.0 * k) as usize;
    let (x0, y0) = (at.0 as usize - side / 2, at.1 as usize - side / 2);
    let inked = (y0..y0 + side)
        .flat_map(|y| (x0..x0 + side).map(move |x| (x, y)))
        .filter(|(x, y)| with.rgba[(y * w + x) * 4 + 3] > 100)
        .count();
    assert!(inked > 200, "{inked} inked pixels in the slot");
    // A click on the slot uses the target: the locker opens.
    s.click(at, size);
    s.update(DT);
    assert!(s.game.world.opened.is_some(), "{}", s.state_line());
}

#[test]
fn every_resolution_gets_the_interface_pixel_for_pixel() {
    let Some(mut s) = stage() else { return };
    past_the_talk(&mut s);
    assert!(s.stand_near("end_locker01"));
    s.game.world.events.clear();
    let id = s.game.world.by_tag("end_locker01")[0];
    s.game.world.click(id);
    assert!(s.game.world.opened.is_some());
    for size in [(1280, 720), (1920, 1080), (2560, 1440), (3840, 2160)] {
        s.set_view(size);
        s.update(DT);
        let o = s.overlay().unwrap();
        assert_eq!(
            (o.width, o.height),
            size,
            "one overlay pixel per screen pixel"
        );
        // The panel is in the middle, the scene shows around it, and the first row works at every size.
        let alpha = |x: u32, y: u32| o.rgba[((y * o.width + x) * 4 + 3) as usize];
        assert_eq!(alpha(0, size.1 - 2), 0, "{size:?}");
        assert!(alpha(size.0 / 2, size.1 / 3) > 0, "{size:?}");
        let before = s.game.world.container_items().len();
        let row = s.menu_pixel("LB_ITEMS", Some(0), size).unwrap();
        s.click(row, size);
        assert_eq!(s.game.world.container_items().len(), before - 1, "{size:?}");
    }
}

#[test]
fn the_gpu_draws_at_any_size_after_a_resize() {
    let Some(mut s) = stage() else { return };
    let Ok(mut gpu) = GpuRenderer::new(320, 180) else {
        return;
    };
    s.update(DT);
    let scene = s.static_scene().clone();
    gpu.set_static_scene(&scene, &mut |n| s.texture(n));
    let moving = s.dynamic_scene();
    let cam = s.camera();
    for (w, h) in [(1280, 720), (3840, 2160), (640, 360)] {
        gpu.resize(w, h);
        let img = gpu.render(&moving, &mut |n| s.texture(n), &cam).unwrap();
        assert_eq!((img.width, img.height), (w, h));
        assert_eq!(img.rgba.len(), (w * h * 4) as usize);
    }
}

#[test]
fn the_area_smokes_and_fights_make_sounds_and_bolts() {
    let Some(mut s) = stage() else { return };
    assert!(
        s.effects.source_count() >= 10,
        "{}",
        s.effects.source_count()
    );
    // Stand near a smoking room: particles appear, each with a texture that exists.
    let mut cues = Vec::new();
    let foe = s
        .game
        .world
        .objects
        .values()
        .find(|o| o.template == "n_sithsoldier003")
        .map(|o| o.id)
        .expect("a Sith soldier");
    let at = s.game.world.object(foe).unwrap().position;
    let pc = s.game.world.pc;
    s.game.world.teleport(pc, [at[0] - 5.0, at[1], at[2]]);
    // The player shoots: the AI of a soldier this close puts its rifle away for a blade.
    let rifle = s.game.world.spawn_item("g_w_blstrrfl001");
    s.game
        .world
        .object_mut(pc)
        .unwrap()
        .equipped
        .insert(4, rifle);
    s.game.world.attack(pc, foe);
    let mut saw_bolt = false;
    // Short frames: the soldier comes close, and at normal length a bolt lands within the frame it is fired in.
    for _ in 0..3600 {
        cues.extend(s.update(DT / 4.0));
        s.continue_line();
        s.number_key(1);
        saw_bolt |= s.effects.bolt_count() > 0;
    }
    assert!(saw_bolt, "the rifle fires bolts");
    let sounds: Vec<&Vec<u8>> = cues
        .iter()
        .filter_map(|c| match c {
            Cue::Effect(b) => Some(b),
            _ => None,
        })
        .collect();
    assert!(!sounds.is_empty(), "shots make sounds");
    for b in sounds {
        kotor_audio_out::decode_any(b).expect("every effect decodes");
    }
    // Walking makes steps; a door makes its sound.
    let mut s = stage().unwrap();
    past_the_opening(&mut s);
    let doors: Vec<_> = s
        .game
        .world
        .objects
        .values()
        .filter(|o| o.kind == Kind::Door && !o.locked && !o.open)
        .map(|o| o.id)
        .collect();
    s.update(DT);
    s.update(DT);
    // From beside each: a door far off is not heard, and not every door has a sound.
    let mut heard = false;
    for door in doors {
        let at = s.game.world.object(door).unwrap().position;
        s.game.world.pc_mover.pos = at;
        s.update(DT);
        s.game.world.open_door(door);
        heard |= s.update(DT).iter().any(|c| matches!(c, Cue::Effect(_)));
    }
    assert!(heard, "the door sounds");
}

#[test]
fn particles_draw_on_the_gpu() {
    let Some(mut s) = stage() else { return };
    let Ok(mut gpu) = GpuRenderer::new(320, 180) else {
        return;
    };
    let cam = kotor_render::Camera {
        eye: [0.0, -3.0, 0.0],
        target: [0.0, 0.0, 0.0],
        fov: 55.0,
    };
    let plain = gpu
        .render(&Default::default(), &mut |n| s.texture(n), &cam)
        .unwrap();
    let mut fx = kotor_gpu::effects::Effects::default();
    fx.fire(
        [-2.0, 0.0, 0.0],
        [2.0, 0.0, 0.0],
        "fx_laser_01",
        [1.0, 0.2, 0.2],
    );
    gpu.set_particles(fx.batches(&cam));
    let lit = gpu
        .render(&Default::default(), &mut |n| s.texture(n), &cam)
        .unwrap();
    assert_ne!(plain.rgba, lit.rgba, "the flash and bolt add light");
    let opaque = lit.rgba.chunks(4).all(|p| p[3] == 255);
    assert!(opaque, "the picture stays opaque");
}

#[test]
fn a_chest_lid_moves_when_it_opens() {
    let Some(mut s) = stage() else { return };
    past_the_talk(&mut s);
    assert!(s.stand_near("end_locker01"));
    s.update(DT);
    let id = s.game.world.by_tag("end_locker01")[0];
    let at = s.game.world.object(id).unwrap().position;
    let near = |scene: &kotor_render::Scene| -> Vec<[f32; 3]> {
        scene
            .meshes
            .iter()
            .flat_map(|m| m.positions.iter().copied())
            .filter(|p| (p[0] - at[0]).hypot(p[1] - at[1]) < 0.9)
            .collect()
    };
    let shut = near(&s.dynamic_scene());
    assert!(!shut.is_empty(), "the locker is drawn each frame");
    s.game.world.click(id);
    for _ in 0..40 {
        s.update(DT);
    }
    assert_ne!(near(&s.dynamic_scene()), shut, "the lid moved");
}

#[test]
fn walls_hide_enemies_and_stop_the_camera() {
    let Some(mut s) = stage() else { return };
    // The player starts in a closed bunk room: nobody on the far side of its walls notices.
    for _ in 0..90 {
        s.update(DT);
        s.continue_line();
        s.number_key(1);
    }
    let w = &s.game.world;
    let pc = w.pc;
    let noticed: Vec<_> = w
        .combat
        .perceived
        .iter()
        .filter(|(_, b)| *b == pc)
        .map(|(a, _)| w.object(*a).map(|o| (o.tag.clone(), o.position)))
        .collect();
    let here = w.object(pc).unwrap().position;
    for (tag, at) in noticed.into_iter().flatten() {
        let d = (at[0] - here[0]).hypot(at[1] - here[1]);
        assert!(
            d < 25.0,
            "{tag} noticed the player through walls from {d} m"
        );
    }
    // Backed against something, the camera stays on this side of it.
    let eye_clear = |s: &Stage| {
        let c = s.camera();
        let p = s.game.world.object(s.game.world.pc).unwrap().position;
        let t = [p[0], p[1], p[2] + 1.2];
        let d = [c.eye[0] - t[0], c.eye[1] - t[1], c.eye[2] - t[2]];
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        (len, t, d)
    };
    for k in 0..24 {
        s.yaw = k as f32 * std::f32::consts::TAU / 24.0;
        let (len, _, _) = eye_clear(&s);
        assert!(
            (0.7..=3.5).contains(&len),
            "camera {len} m away at turn {k}"
        );
    }
}

#[test]
fn stock_visual_effects_play_and_fade() {
    let Some(mut s) = stage() else { return };
    let eye = s.camera().eye;
    let pc = s.game.world.pc;
    // VFX_IMP_HEALING_SMALL names v_heal_imp, a model of fountains.
    let rec = {
        let w = &mut s.game.world;
        let mut r = kotor_game::EffectRec::new("EffectVisualEffect");
        r.nums = vec![1001.0];
        w.effects.push(r);
        w.effects.len() as u32
    };
    s.game.world.events.push(format!(
        "ApplyEffectToObject(0, Engine(0, {rec}), #{pc}, 0)"
    ));
    s.update(DT);
    assert!(s.effects.playing() > 0, "the effect started");
    for _ in 0..30 {
        s.effects.update(DT, eye);
    }
    assert!(!s.particles().is_empty(), "its particles show");
    for _ in 0..600 {
        s.effects.update(DT, eye);
    }
    assert_eq!(s.effects.playing(), 0, "and it ends");
}

#[test]
fn the_underwear_style_reaches_what_a_conversation_draws() {
    let Ok(mut s) = Stage::new("end_m01aa") else {
        return;
    };
    // What is drawn each frame, in a conversation as anywhere: the hero's own rig and the textures it names.
    let names: Vec<String> = s
        .dynamic_scene()
        .meshes
        .iter()
        .filter_map(|m| m.texture.clone())
        .collect();
    let body = names
        .iter()
        .find(|n| kotor_render::underwear::body_of_texture(n).is_some())
        .cloned()
        .expect("the hero's bare body is drawn");
    let head = names
        .iter()
        .find(|n| kotor_render::underwear::is_hero_head(n))
        .cloned()
        .expect("the hero's head is drawn");
    let painted = (s.texture(&body).unwrap(), s.texture(&head).unwrap());
    let shape = bytes_of(&s.dynamic_scene());
    s.set_conventional_underwear(false);
    let original = (s.texture(&body).unwrap(), s.texture(&head).unwrap());
    assert_ne!(painted.0.rgba, original.0.rgba, "{body} is painted");
    assert_ne!(painted.1.rgba, original.1.rgba, "{head}'s neck is matched");
    assert_ne!(
        shape,
        bytes_of(&s.dynamic_scene()),
        "and its shading evened out"
    );
}

#[test]
fn the_jedi_duel_ends_without_any_click() {
    let Some(mut s) = stage() else { return };
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    s.pilot = Some(kotor_game::Autopilot::new());
    let began = |s: &Stage| {
        s.game
            .world
            .events
            .iter()
            .any(|e| e.contains("conversation end_cut04 begins"))
    };
    for _ in 0..(900.0 / DT) as usize {
        if began(&s) {
            break;
        }
        s.update(DT);
    }
    assert!(began(&s), "the pilot never reached the duel");
    // From here nobody clicks.
    s.pilot = None;
    s.steer = (0.0, 0.0, false);
    let mut cues = Vec::new();
    let mut ended = None;
    for i in 0..(150.0 / DT) as usize {
        cues.extend(s.update(DT));
        if s.game
            .world
            .events
            .iter()
            .any(|e| e.contains("conversation end_cut04 ends"))
        {
            ended = Some(i as f32 * DT);
            break;
        }
    }
    let ev = &s.game.world.events;
    assert!(ended.is_some(), "end_cut04 waits for a click");
    assert!(ev.iter().any(|e| e.contains("end_darkjedi01) dies")));
}
