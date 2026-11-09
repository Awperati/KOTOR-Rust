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
    s.jump_key();
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

#[test]
fn a_voice_line_is_found_in_the_folder_of_its_module() {
    let Some(s) = stage() else { return };
    // Most lines live at `streamwaves\m02ab\boun01\...`, not at the top.
    let Some(p) = s.sound_file("nm02abboun01004_") else {
        return; // an install without that module
    };
    assert!(
        p.is_file() && p.to_string_lossy().to_lowercase().contains("m02ab"),
        "{p:?}"
    );
    assert!(
        s.sound_file("NM02ABBOUN01004_").is_some(),
        "the case does not matter"
    );
    assert!(s.sound_file("no_such_line_").is_none());
}

#[test]
fn tilted_right_up_the_camera_stays_under_the_ceiling() {
    let Some(mut s) = stage() else {
        return;
    };
    s.enable_tutorials(false);
    s.update(DT);
    let length = |c: &kotor_render::Camera| {
        (0..3)
            .map(|i| (c.eye[i] - c.target[i]).powi(2))
            .sum::<f32>()
            .sqrt()
    };
    s.yaw = 0.0;
    s.pitch = 1.2;
    let high = s.camera();
    // The Endar Spire's cabin: at this tilt the whole distance rose to 3 m, over the ceiling.
    assert!(high.eye[2] < 2.7, "{high:?}");
    assert!(length(&high) < s.distance - 0.1, "{high:?}");
    assert!(length(&high) > 1.0, "still behind the player: {high:?}");
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
    s.set_click_to_move(true);
    s.click(px, SIZE);
    assert!(s.game.world.pc_walking(), "the click started a walk");
    let still = bytes_of(&s.dynamic_scene());
    // Modern movement speeds up from a stand.
    for _ in 0..8 {
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
    // Modern movement's way: the game's default, Classic, turns on the spot.
    s.set_movement_modern(true);
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
    let count: i32 = items
        .iter()
        .map(|i| s.game.world.container_count(i.0))
        .sum();
    let before = s.game.world.object(pc).unwrap().inventory.len();
    s.update(DT);
    // The row list is on screen: a click on the first row chooses it, and the middle button takes it.
    s.set_view((853, 480));
    assert_eq!(s.menu_name(), "container");
    let row = s.menu_pixel("LB_ITEMS", Some(0), (853, 480)).unwrap();
    s.click(row, (853, 480));
    assert_eq!(s.game.world.object(pc).unwrap().inventory.len(), before);
    let take = s.menu_pixel("BTN_GIVEITEMS", None, (853, 480)).unwrap();
    s.click(take, (853, 480));
    assert_eq!(
        s.game.world.object(pc).unwrap().inventory.len(),
        before + 1,
        "Take Selected took one item"
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
    assert_eq!(carried as usize, before + count as usize);
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
        let count = |s: &Stage| -> i32 {
            let w = &s.game.world;
            w.container_items()
                .iter()
                .map(|i| w.container_count(i.0))
                .sum()
        };
        let before = count(&s);
        let row = s.menu_pixel("LB_ITEMS", Some(0), size).unwrap();
        s.click(row, size);
        let take = s.menu_pixel("BTN_GIVEITEMS", None, size).unwrap();
        s.click(take, size);
        assert_eq!(count(&s), before - 1, "{size:?}");
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
    // The video has the player free about 24 s after the door opens (docs/REFERENCE.md). Each pause in the scene used to be
    // counted twice, which made it 37 s.
    let ended = ended.unwrap();
    assert!(
        (18.0..27.0).contains(&ended),
        "the duel scene lasts {ended} s"
    );
    assert!(ev.iter().any(|e| e.contains("end_darkjedi01) dies")));
}

/// Item 10 of the owner's list of 6 Nov: whoever talks in a conversation gestures, and the player listens.
#[test]
fn the_owner_of_a_conversation_talks_and_the_player_listens() {
    let Some(mut s) = stage() else { return };
    let mut seen = Vec::new();
    for _ in 0..(120.0 / DT) as usize {
        s.update(DT);
        let w = &s.game.world;
        if let Some(owner) = w.dialogue_owner().filter(|o| *o != w.pc) {
            let (pc, tag) = (w.pc, w.object(owner).unwrap().tag.clone());
            let o = s.dialogue_clip(owner).map(|c| c.name);
            let p = s.dialogue_clip(pc).map(|c| c.name);
            seen.push((tag, o, p));
        }
        s.continue_line();
        s.number_key(1);
        if seen.len() > 400 {
            break;
        }
    }
    assert!(!seen.is_empty(), "a conversation happened");
    // Outside a conversation nothing is overridden.
    assert!(s.dialogue_clip(s.game.world.pc).is_none() || s.game.world.in_dialogue());
    assert!(
        seen.iter()
            .any(|(_, o, _)| o.is_some_and(|n| n.starts_with("tlk"))),
        "the speaker makes talking gestures: {:?}",
        &seen[..seen.len().min(5)]
    );
    assert!(
        seen.iter().any(|(_, _, p)| *p == Some("listen")),
        "the player listens"
    );
}

/// The opening cutscene moves on by itself, each node after the length of its animation (`docs/CUTSCENE.md`: 4.97, 3.87,
/// 4.87 and 4.13 s), with no click.
#[test]
fn a_cutscene_advances_by_itself_when_its_clip_ends() {
    let Some(mut s) = stage() else { return };
    let mut last = None;
    let mut at = 0.0;
    let mut gaps = Vec::new();
    for f in 0..(30.0 / DT) as usize {
        s.update(DT);
        let now = f as f32 * DT;
        let key = s.game.world.dialogue_serial();
        if key != last {
            if last.is_some() {
                gaps.push(now - at);
            }
            at = now;
            last = key;
        }
        if s.game
            .world
            .dialogue_view()
            .is_some_and(|v| !v.3.is_empty())
        {
            break;
        }
    }
    let expected = [4.97, 3.87, 4.87, 4.13];
    assert!(gaps.len() >= 4, "{gaps:?}");
    for (g, e) in gaps.iter().zip(expected) {
        assert!((g - e).abs() < 0.15, "{gaps:?}");
    }
}

/// Item 40 of the owner's list of 6 Nov: Carth on the comm was shown as a ceiling and the top of his head over a console,
/// because the module's own camera stands 1 m from him and he was not drawn so far from the player. He is now drawn and
/// framed. With `FRAME_OUT` set to a path, the frame is written there as a PNG, and `FRAME_SIZE` (like 3840x2160) sizes it.
#[test]
fn carth_on_the_comm_is_in_frame() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let mut s = Stage::open("end_m01ab", true).unwrap();
    for f in 0..200 {
        s.update(DT);
        if !(s.game.world.in_dialogue() && f > 40) {
            continue;
        }
        let w = &s.game.world;
        let owner = w.object(w.dialogue_owner().unwrap()).unwrap();
        assert_eq!(owner.tag, "Carth");
        let owner_at = owner.position;
        let cam = s.camera();
        let from = (owner_at[0] - cam.eye[0]).hypot(owner_at[1] - cam.eye[1]);
        assert!(from > 1.2, "the camera is {from} m from him");
        // His head is in the view: ahead of the camera and not above or below the middle of it.
        let (ahead, up) = (
            (owner_at[0] - cam.eye[0]) * (cam.target[0] - cam.eye[0])
                + (owner_at[1] - cam.eye[1]) * (cam.target[1] - cam.eye[1]),
            (owner_at[2] + 1.6 - cam.eye[2]).atan2(from).to_degrees(),
        );
        assert!(ahead > 0.0 && up.abs() < 20.0, "{ahead} {up}");
        let scene = s.dynamic_scene();
        let drawn = scene.meshes.iter().any(|m| {
            m.positions
                .iter()
                .any(|p| (p[0] - owner_at[0]).hypot(p[1] - owner_at[1]) < 0.6)
        });
        assert!(drawn, "Carth is drawn though he is far from the player");
        if let Ok(path) = std::env::var("FRAME_OUT") {
            let size = std::env::var("FRAME_SIZE").unwrap_or("1280x720".into());
            let (w, h) = size.split_once('x').unwrap();
            let (w, h) = (w.parse().unwrap(), h.parse().unwrap());
            s.set_view((w, h));
            s.update(DT);
            let mut gpu = GpuRenderer::new(w, h).unwrap();
            let still = s.static_scene().clone();
            gpu.set_static_scene(&still, &mut |n| s.texture(n));
            let moving = s.dynamic_scene();
            let cam = s.camera();
            let mut img = gpu.render(&moving, &mut |n| s.texture(n), &cam).unwrap();
            if let Some(o) = s.overlay() {
                composite(&mut img, o);
            }
            std::fs::write(path, kotor_image::to_png(&img)).unwrap();
        }
        return;
    }
    panic!("no conversation began");
}

/// With "Auto-advance dialogue" off a cutscene line waits for a click or Space, and then moves on.
#[test]
fn a_cutscene_waits_for_a_click_when_auto_advance_is_off() {
    let Some(mut s) = stage() else { return };
    let file = std::env::temp_dir().join(format!("kotor_auto_advance_{}.txt", std::process::id()));
    std::fs::write(&file, "auto_advance=false\n").unwrap();
    s.use_settings_file(file.clone());
    assert!(!s.settings().auto_advance);
    for _ in 0..(12.0 / DT) as usize {
        s.update(DT);
    }
    let held = s.game.world.dialogue_serial();
    assert!(held.is_some(), "the opening is still on its first line");
    assert_eq!(held, Some(1));
    s.continue_line();
    s.update(DT);
    assert_eq!(
        s.game.world.dialogue_serial(),
        Some(2),
        "a click moves it on"
    );
    let _ = std::fs::remove_file(file);
}

/// Bytes of the `.wav` the game ships under `name`.
fn wav(name: &str) -> Vec<u8> {
    let mut inst = kotor_archive::Install::open(kotor_core::install_dir().unwrap()).unwrap();
    inst.read(name, kotor_core::ResType::WAV).unwrap()
}

#[test]
fn a_stun_baton_sounds_unarmed_and_a_long_sword_sounds_like_a_large_blade() {
    let Some(mut s) = stage() else {
        return;
    };
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let pc = s.game.world.pc;
    let at = s.game.world.object(pc).unwrap().position;
    let foe = s
        .game
        .world
        .spawn_creature("n_calonord", [at[0] + 1.0, at[1], at[2]]);
    // `weaponmattype` of `baseitems.2da` names the `weaponsounds.2da` row: baton 0 (unarmed), long sword 3.
    for (item, swing) in [
        ("g_w_stunbaton01", "cb_sw_unarmed1"),
        ("g_w_lngswrd01", "cb_sw_bldlrg1"),
    ] {
        let it = s.game.world.spawn_item(item);
        s.game.world.equip(pc, it, Some(4)).unwrap();
        s.game.world.combat.log.push(kotor_game::AttackLog {
            round: 1,
            attacker: pc,
            target: foe,
            roll: 10,
            bonus: 0,
            defense: 10,
            outcome: kotor_game::Outcome::Hit,
            damage: 1,
            target_hp: 10,
        });
        let cues = s.update(DT);
        let want = wav(swing);
        assert!(
            cues.iter()
                .any(|c| matches!(c, Cue::Effect(b) | Cue::FaintEffect(b, _) if *b == want)),
            "{item} should swing as {swing}"
        );
    }
}

/// A player's chosen underwear is drawn: the bare body and the head go by the variant's name, and the body's sheet is the
/// variant's. The standard paint keeps its names.
#[test]
fn a_chosen_underwear_is_what_the_hero_wears() {
    let Some(mut s) = stage() else { return };
    let Some(variant) = kotor_render::underwear::list_underwear_variants()
        .into_iter()
        .nth(
            std::env::var("FRAME_VARIANT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1),
        )
    else {
        return;
    };
    eprintln!("variant: {}", variant.id);
    let pc = s.game.world.pc;
    // FRAME_APPEARANCE picks the body for the frame (the sheet's number in appearance.2da).
    let look = std::env::var("FRAME_APPEARANCE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(92);
    s.game.world.object_mut(pc).unwrap().appearance = look;
    s.game.world.set_player_underwear("");
    s.update(DT);
    let plain: Vec<String> = s
        .dynamic_scene()
        .meshes
        .iter()
        .filter_map(|m| m.texture.clone())
        .collect();
    assert!(plain.iter().all(|t| !t.contains('@')), "{plain:?}");
    s.game.world.set_player_underwear(&variant.id);
    // As the opening conversation has it too.
    for f in 0..200 {
        s.update(DT);
        if s.game.world.in_dialogue() && f > 40 {
            break;
        }
    }
    let worn: Vec<String> = s
        .dynamic_scene()
        .meshes
        .iter()
        .filter_map(|m| m.texture.clone())
        .collect();
    if std::env::var("FRAME_OUT").is_ok() {
        eprintln!("textures worn: {worn:?}");
    }
    let body = worn
        .iter()
        .find(|t| {
            t.contains('@') && t.to_ascii_lowercase().starts_with("pfba")
                || t.to_ascii_lowercase().starts_with("pmba")
        })
        .unwrap_or_else(|| panic!("no body in the variant: {worn:?}"))
        .clone();
    let sheet = s.texture(&body).unwrap();
    let standard = s.texture(body.split('@').next().unwrap()).unwrap();
    if body.to_ascii_lowercase().starts_with("pfba") {
        assert_ne!(sheet.rgba, standard.rgba, "{body} is the variant's sheet");
    }
    assert!(
        worn.iter().any(
            |t| t.contains('@') && t.to_ascii_lowercase().starts_with("pfh")
                || t.to_ascii_lowercase().starts_with("pmh")
        ),
        "{worn:?}"
    );

    // With FRAME_OUT set to a path, the hero in the variant, in full, is written there as a PNG; FRAME_SIZE (like 3840x2160)
    // sizes it, and FRAME_PLAIN keeps the standard paint, for a frame beside it.
    if let Ok(path) = std::env::var("FRAME_OUT") {
        if std::env::var("FRAME_PLAIN").is_ok() {
            s.game.world.set_player_underwear("");
        }
        let size = std::env::var("FRAME_SIZE").unwrap_or("1280x720".into());
        let (w, h) = size.split_once('x').unwrap();
        let (w, h): (u32, u32) = (w.parse().unwrap(), h.parse().unwrap());
        s.set_view((w, h));
        for _ in 0..400 {
            s.update(DT);
            if !s.game.world.in_dialogue() {
                break;
            }
            s.game.world.dialogue_choose(0);
        }
        s.update(DT);
        let at = s.game.world.object(pc).unwrap().position;
        // FRAME_ARMOUR puts that item on the hero.
        if let Ok(t) = std::env::var("FRAME_ARMOUR") {
            let it = s.game.world.spawn_item(&t);
            s.game.world.object_mut(pc).unwrap().feats.extend(0..300);
            s.game.world.equip(pc, it, None).unwrap();
            s.update(DT);
        }
        // FRAME_STOCK shows the install's own paint, as with conventional underwear off; FRAME_NOHD leaves out the upscaled pictures.
        if std::env::var("FRAME_STOCK").is_ok() {
            s.set_conventional_underwear(false);
        }
        if std::env::var("FRAME_NOHD").is_ok() {
            s.no_hd();
        }
        if let Ok(level) = std::env::var("FRAME_SHADING") {
            s.set_skin_shading(level.parse().unwrap_or(1.0));
        }
        if let Ok(clip) = std::env::var("FRAME_CLIP") {
            // The owner's clips, if KOTOR_RS_ANIMS names a folder of them.
            s.reload_custom_clips();
            s.force_clip(pc, Some(&clip));
        }
        for _ in 0..5 {
            s.update(DT);
        }
        let heading = s.game.world.pc_mover.heading;
        // FRAME_BACK stands the camera behind the hero; FRAME_CLOSE fills the frame with the shoulders and back.
        let side = if std::env::var("FRAME_BACK").is_ok() {
            -1.0
        } else {
            1.0
        };
        let (dist, high, fov) = if std::env::var("FRAME_HAND").is_ok() {
            (0.8, 0.82, 40.0)
        } else if std::env::var("FRAME_BODY").is_ok() {
            (2.0, 0.85, 40.0)
        } else if std::env::var("FRAME_BELLY").is_ok() {
            (1.05, 1.08, 40.0)
        } else if std::env::var("FRAME_FACE").is_ok() {
            (0.75, 1.6, 40.0)
        } else if std::env::var("FRAME_CLOSE").is_ok() {
            (1.1, 1.35, 40.0)
        } else {
            (3.2, 0.95, 40.0)
        };
        // FRAME_SIDE looks from the hero's left, along the profile.
        let (dx, dy) = if std::env::var("FRAME_SIDE").is_ok() {
            (-heading.sin(), heading.cos())
        } else {
            (heading.cos(), heading.sin())
        };
        let cam = kotor_render::Camera {
            eye: [
                at[0] + dx * dist * side,
                at[1] + dy * dist * side,
                at[2] + high + 0.05,
            ],
            target: [at[0], at[1], at[2] + high],
            fov,
        };
        let mut gpu = GpuRenderer::new(w, h).unwrap();
        if let Some(level) = std::env::var("FRAME_CEL").ok().and_then(|v| v.parse().ok()) {
            gpu.set_cel_shading(level);
        }
        let still = s.static_scene().clone();
        gpu.set_static_scene(&still, &mut |n| s.texture(n));
        let moving = s.dynamic_scene();
        let img = gpu.render(&moving, &mut |n| s.texture(n), &cam).unwrap();
        std::fs::write(path, kotor_image::to_png(&img)).unwrap();
    }
}

/// The positions of everything drawn at head height within arm's reach of `at`: the head's meshes, which a face that moves
/// changes from one frame to the next.
fn head_shape(scene: &Scene, at: [f32; 3]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for m in &scene.meshes {
        for p in &m.positions {
            let near = (p[0] - at[0]).hypot(p[1] - at[1]) < 0.6;
            if near && p[2] > at[2] + 1.2 && p[2] < at[2] + 2.1 {
                p.map(f32::to_bits).hash(&mut h);
            }
        }
    }
    h.finish()
}

/// "Still no facial animations": while a line plays its speaker's face moves, frame after frame, and when the line is said the
/// mouth is shut and stops. With `FRAME_STRIP` set to a folder, six frames of the speaker over two seconds of the line are
/// written there as PNGs (`FRAME_SIZE`, like 3840x2160, sizes them).
#[test]
fn a_head_speaks_while_its_line_plays() {
    let Some(mut s) = stage() else { return };
    let mut speaker = None;
    // Walk the opening conversation on until a line is spoken by someone else than the player.
    for f in 0..3000 {
        s.update(DT);
        let w = &s.game.world;
        if f > 30 && w.in_dialogue() {
            let o = w.dialogue_owner().filter(|o| *o != w.pc);
            if let Some(o) = o.filter(|o| s.mouth_of(*o).is_some()) {
                speaker = Some(o);
                break;
            }
            if f % 15 == 0 {
                s.game.world.dialogue_choose(0);
            }
        }
    }
    let speaker = speaker.expect("a line is spoken by someone other than the player");
    let at = s.game.world.object(speaker).unwrap().position;
    let strip = std::env::var("FRAME_STRIP").ok();
    let size = std::env::var("FRAME_SIZE").unwrap_or("1280x720".into());
    let (w, h) = size.split_once('x').unwrap();
    let (w, h): (u32, u32) = (w.parse().unwrap(), h.parse().unwrap());
    let mut gpu = strip.as_ref().map(|_| {
        s.set_view((w, h));
        GpuRenderer::new(w, h).unwrap()
    });
    let mut shapes = std::collections::HashSet::new();
    let mut opened = false;
    // Two seconds, six frames of them.
    let mut taken = 0;
    // FRAME_STRIP_COUNT frames (6), FRAME_STRIP_EVERY steps apart (10).
    let every: usize = std::env::var("FRAME_STRIP_EVERY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let count: usize = std::env::var("FRAME_STRIP_COUNT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(6);
    for step in 0..every * count {
        s.update(DT);
        if s.mouth_of(speaker).is_none() {
            break;
        }
        opened |= s.mouth_of(speaker).is_some_and(|(open, _)| open > 0.2);
        if step % every == 0 {
            let scene = s.dynamic_scene();
            shapes.insert(head_shape(&scene, at));
            if let (Some(dir), Some(gpu)) = (&strip, gpu.as_mut()) {
                std::fs::create_dir_all(dir).unwrap();
                let still = s.static_scene().clone();
                gpu.set_static_scene(&still, &mut |n| s.texture(n));
                let cam = s.speaker_camera().unwrap_or_else(|| s.camera());
                let img = gpu.render(&scene, &mut |n| s.texture(n), &cam).unwrap();
                std::fs::write(
                    format!("{dir}/speak_{taken:03}.png"),
                    kotor_image::to_png(&img),
                )
                .unwrap();
                taken += 1;
            }
        }
    }
    assert!(opened, "the speaker's mouth opens while the line plays");
    assert!(
        shapes.len() >= 4,
        "the head's shape at six moments of the line: {} different",
        shapes.len()
    );
    // Said: the mouth is shut, so the speaker is not talking any more.
    for _ in 0..3000 {
        if s.mouth_of(speaker).is_none() || !s.game.world.in_dialogue() {
            break;
        }
        s.update(DT);
    }
    assert!(
        s.mouth_of(speaker).is_none(),
        "the mouth shuts when the line is said"
    );
}

/// Bug 33: the sonic weapons' impact sound, `cb_ht_sonblclth1`, is in none of the game's archives, so a sonic hit was silent.
/// It falls back to the plain blaster's impact. The shot is the sonic's own.
#[test]
fn a_sonic_weapons_hit_sounds_like_a_blasters_when_its_own_is_missing() {
    let Some(mut s) = stage() else {
        return;
    };
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let pc = s.game.world.pc;
    let at = s.game.world.object(pc).unwrap().position;
    let foe = s
        .game
        .world
        .spawn_creature("n_calonord", [at[0] + 4.0, at[1], at[2]]);
    let gun = s.game.world.spawn_item("g_w_sonicpstl01");
    s.game.world.equip(pc, gun, Some(4)).unwrap();
    s.game.world.combat.log.push(kotor_game::AttackLog {
        round: 1,
        attacker: pc,
        target: foe,
        roll: 10,
        bonus: 0,
        defense: 10,
        outcome: kotor_game::Outcome::Hit,
        damage: 1,
        target_hp: 10,
    });
    // The shot leaves with the attack clip's kick, a little after the blow is dealt.
    let mut cues = Vec::new();
    for _ in 0..(2.0 / DT) as usize {
        cues.extend(s.update(DT));
    }
    let heard = |name: &str| {
        let want = wav(name);
        cues.iter()
            .any(|c| matches!(c, Cue::Effect(b) | Cue::FaintEffect(b, _) if *b == want))
    };
    assert!(heard("cb_sh_sonbl1"), "the sonic's own shot");
    assert!(heard("cb_ht_blastleth1"), "a hit is not silent");
}

/// The owner's bug of 7 Nov: a rifle's attack clip kicks three times and one bolt came out, at the start. One bolt now leaves at
/// each kick, each with its flash and its sound: three for a rifle, two for a pistol.
#[test]
fn a_burst_fires_one_bolt_for_each_shot_of_the_clip() {
    for (item, shots) in [("g_w_blstrrfl001", 3), ("g_w_blstrpstl001", 2)] {
        let Some(mut s) = stage() else {
            return;
        };
        s.enable_tutorials(false);
        s.enable_auto_pause(false);
        for _ in 0..30 {
            s.update(DT);
        }
        let pc = s.game.world.pc;
        let at = s.game.world.object(pc).unwrap().position;
        let foe = s
            .game
            .world
            .spawn_creature("n_calonord", [at[0] + 6.0, at[1], at[2]]);
        let f = s.game.world.object_mut(foe).unwrap();
        (f.hp, f.max_hp) = (900, 900);
        let gun = s.game.world.spawn_item(item);
        s.game.world.equip(pc, gun, Some(4)).unwrap();
        s.game.world.attack(pc, foe);
        let shot = wav(match item {
            "g_w_blstrrfl001" | "g_w_blstrpstl001" => "cb_sh_blast1",
            _ => unreachable!(),
        });
        let mut at_step: Vec<usize> = Vec::new();
        let mut sounds = 0;
        let mut last = s.effects.bolts_fired;
        for i in 0..(7.0 / DT) as usize {
            let cues = s.update(DT);
            sounds += cues
                .iter()
                .filter(|c| matches!(c, Cue::Effect(b) | Cue::FaintEffect(b, _) if *b == shot))
                .count();
            while s.effects.bolts_fired > last {
                at_step.push(i);
                last += 1;
            }
        }
        // The shots of one attack, up to a pause of a second before the next round's.
        let mut first: Vec<usize> = vec![at_step[0]];
        for p in at_step.windows(2) {
            if (p[1] - p[0]) as f32 * DT > 1.0 {
                break;
            }
            first.push(p[1]);
        }
        assert_eq!(first.len(), shots, "{item}: bolts at steps {at_step:?}");
        let gaps: Vec<f32> = first
            .windows(2)
            .map(|p| (p[1] - p[0]) as f32 * DT)
            .collect();
        assert!(
            gaps.iter().all(|g| *g > 0.3),
            "{item}: shots apart by {gaps:?}"
        );
        assert_eq!(
            sounds,
            at_step.len(),
            "{item}: a shot's sound for each bolt"
        );
    }
}

/// The owner's bug of 7 Nov: no death sounds. A creature's soundset (`SoundSetFile`, its `.ssf`) holds its pain grunts and its
/// dying line, and a blow that hurts or kills plays them where it stands.
#[test]
fn the_hurt_cry_out_and_the_dying_say_their_last() {
    let Some(mut s) = stage() else {
        return;
    };
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    for _ in 0..30 {
        s.update(DT);
    }
    let pc = s.game.world.pc;
    let at = s.game.world.object(pc).unwrap().position;
    let foe = s
        .game
        .world
        .spawn_creature("n_calonord", [at[0] + 3.0, at[1], at[2]]);
    let f = s.game.world.object_mut(foe).unwrap();
    (f.hp, f.max_hp) = (20, 20);
    let pains: Vec<Vec<u8>> = [12, 13]
        .iter()
        .filter_map(|slot| s.game.world.soundset_sound(foe, *slot))
        .map(|n| wav(&n))
        .collect();
    let last = s
        .game
        .world
        .soundset_sound(foe, 15)
        .map(|n| wav(&n))
        .expect("Calo Nord has a dying line");
    assert!(!pains.is_empty(), "and pain grunts");
    let heard = |cues: &[Cue], want: &dyn Fn(&Vec<u8>) -> bool| {
        cues.iter()
            .any(|c| matches!(c, Cue::Effect(b) | Cue::FaintEffect(b, _) if want(b)))
    };
    s.game.world.hurt(pc, foe, 5);
    let cues = s.update(DT);
    assert!(
        heard(&cues, &|b| pains.contains(b)),
        "a blow that does not kill is cried out at"
    );
    assert!(!heard(&cues, &|b| *b == last), "and no last words yet");
    s.game.world.hurt(pc, foe, 50);
    let cues = s.update(DT);
    assert!(s.game.world.object(foe).unwrap().dead);
    assert!(heard(&cues, &|b| *b == last), "the dying line at death");
}

/// "No misses": a glancing blow floats over its target as "Glancing" and its damage, and a hit is just the number.
#[test]
fn a_glancing_blow_floats_as_glancing_with_its_damage() {
    let Some(mut s) = stage() else {
        return;
    };
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let pc = s.game.world.pc;
    let at = s.game.world.object(pc).unwrap().position;
    let foe = s
        .game
        .world
        .spawn_creature("n_calonord", [at[0] + 1.0, at[1], at[2]]);
    for (outcome, damage) in [
        (kotor_game::Outcome::Glance, 2),
        (kotor_game::Outcome::Hit, 7),
    ] {
        s.game.world.combat.log.push(kotor_game::AttackLog {
            round: 1,
            attacker: pc,
            target: foe,
            roll: 3,
            bonus: 0,
            defense: 40,
            outcome,
            damage,
            target_hp: 10,
        });
    }
    s.update(DT);
    let shown = s.floats_shown();
    assert!(shown.iter().any(|f| f == "Glancing 2"), "{shown:?}");
    assert!(shown.iter().any(|f| f == "7"), "{shown:?}");
}

/// Ground finishers: a blaster's carrier pitches down at a target on the floor, in its ready and firing clips, and a blade does not.
#[test]
fn a_blaster_aims_down_at_someone_on_the_floor() {
    let Some(mut s) = stage() else {
        return;
    };
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let pc = s.game.world.pc;
    let at = s.game.world.object(pc).unwrap().position;
    let foe = s
        .game
        .world
        .spawn_creature("n_calonord", [at[0] + 3.0, at[1], at[2]]);
    s.game.world.combat.target.insert(pc, foe);
    let clip = |name| {
        Some(kotor_game::Clip {
            name,
            time: 0.0,
            fade: None,
        })
    };
    assert_eq!(s.aim_down_of(pc, &clip("b5a1")), 0.0, "standing target");
    s.game
        .world
        .effects
        .push(kotor_game::EffectRec::new("EffectKnockdown"));
    let handle = s.game.world.effects.len() as u32;
    s.game
        .world
        .apply_effect(pc, foe, handle, kotor_game::DURATION_TEMPORARY, 10.0);
    for name in ["b5a1", "g5r1", "g7r1", "b9a1", "b6a1"] {
        assert!(s.aim_down_of(pc, &clip(name)) > 20.0, "{name}");
    }
    for name in ["g2r1", "c2a1", "pause1", "walk"] {
        assert_eq!(s.aim_down_of(pc, &clip(name)), 0.0, "{name}");
    }
}

/// Armour on the hero is drawn in its own model and picture: the class and the letter of its `baseitems.2da` row.
#[test]
fn armour_on_the_hero_is_drawn_in_its_own_model() {
    let Some(mut s) = stage() else { return };
    let pc = s.game.world.pc;
    s.game.world.object_mut(pc).unwrap().appearance = 92;
    for (item, want) in [
        ("g_a_clothes01", "PFBBM01"),
        ("g_a_class4001", "PFBC01"),
        ("g_a_class5003", "PFBD03"),
        ("g_a_class6001", "PFBE01"),
        ("g_a_class8001", "PFBG01"),
        ("g_a_class9004", "PFBH04"),
    ] {
        let it = s.game.world.spawn_item(item);
        s.game.world.equip(pc, it, None).unwrap();
        for _ in 0..4 {
            s.update(DT);
        }
        let worn: Vec<String> = s
            .dynamic_scene()
            .meshes
            .iter()
            .filter_map(|m| m.texture.clone())
            .filter(|t| t.to_ascii_lowercase().starts_with("pfb"))
            .collect();
        assert!(
            worn.iter().any(|t| t.eq_ignore_ascii_case(want)),
            "{item}: {worn:?}"
        );
    }
}

/// Owner, 8 Nov: "Upon dying an NPC's clothes will change". The armour on a creature goes to its remains when it dies, and the
/// body was drawn without it, in the clothes of its template. A body keeps the look it had.
#[test]
fn a_creature_that_dies_keeps_the_armour_it_was_drawn_in() {
    let Some(mut s) = stage() else { return };
    let pc = s.game.world.pc;
    let Some(npc) = s
        .game
        .world
        .objects
        .values()
        .find(|o| o.kind == kotor_game::Kind::Creature && o.id != pc && !o.dead)
        .map(|o| o.id)
    else {
        return;
    };
    let at = s.game.world.object(pc).unwrap().position;
    s.game.world.teleport(npc, [at[0] + 1.5, at[1], at[2]]);
    s.game.world.object_mut(npc).unwrap().appearance = 137;
    let it = s.game.world.spawn_item("g_a_class8001");
    s.game.world.object_mut(npc).unwrap().feats.extend(0..300);
    s.game.world.equip(npc, it, None).unwrap();
    let armoured = |s: &mut Stage| {
        for _ in 0..4 {
            s.update(DT);
        }
        s.dynamic_scene().meshes.iter().any(|m| {
            m.texture
                .as_deref()
                .is_some_and(|t| t.eq_ignore_ascii_case("PMBG01"))
        })
    };
    assert!(armoured(&mut s), "alive, in the armour");
    let hp = s.game.world.object(npc).unwrap().hp.max(1);
    s.game.world.hurt(pc, npc, hp + 100);
    assert!(s.game.world.object(npc).unwrap().dead);
    assert!(
        s.game.world.object(npc).unwrap().equipped.is_empty(),
        "the armour is on the remains, not on the body"
    );
    // FRAME_OUT: the creature as it lies, beside the frame of the same with the fix off (git stash it), for a look.
    if let Ok(path) = std::env::var("FRAME_OUT") {
        s.set_view((800, 600));
        for _ in 0..60 {
            s.update(DT);
        }
        let at = s.game.world.object(npc).unwrap().position;
        let cam = kotor_render::Camera {
            eye: [at[0] + 2.2, at[1] - 1.2, at[2] + 1.4],
            target: [at[0], at[1], at[2] + 0.2],
            fov: 40.0,
        };
        let mut gpu = GpuRenderer::new(800, 600).unwrap();
        let still = s.static_scene().clone();
        gpu.set_static_scene(&still, &mut |n| s.texture(n));
        let moving = s.dynamic_scene();
        let img = gpu.render(&moving, &mut |n| s.texture(n), &cam).unwrap();
        std::fs::write(path, kotor_image::to_png(&img)).unwrap();
    }
    assert!(armoured(&mut s), "dead, still in the armour");
}

#[test]
fn the_sith_armour_draws_its_wearer_as_a_sith_soldier() {
    let Some(mut s) = stage() else { return };
    let pc = s.game.world.pc;
    for appearance in [92, 137] {
        s.game.world.object_mut(pc).unwrap().appearance = appearance;
        let it = s.game.world.spawn_item("ptar_sitharmor");
        s.game.world.equip(pc, it, None).unwrap();
        for _ in 0..4 {
            s.update(DT);
        }
        let worn: Vec<String> = s
            .dynamic_scene()
            .meshes
            .iter()
            .filter_map(|m| m.texture.clone())
            .filter(|t| t.to_ascii_lowercase().starts_with("n_sithsoldier"))
            .collect();
        assert!(
            !worn.is_empty(),
            "appearance {appearance}: no Sith soldier on the hero"
        );
        let clothes = s.dynamic_scene().meshes.iter().any(|m| {
            m.texture.as_deref().is_some_and(|t| {
                t.eq_ignore_ascii_case("PFBBM01") || t.eq_ignore_ascii_case("PMBBM01")
            })
        });
        assert!(
            !clothes,
            "appearance {appearance}: the hero is still in plain clothes"
        );
    }
}

/// A weapon that is not in the hand hangs at its holster place, and in a fight it is in the hand (`docs/HOLSTERS.md`, stage 1).
#[test]
fn a_stored_weapon_hangs_at_its_holster() {
    for (item, shape) in [
        ("g_w_blstrpstl001", "holster hip_holster"),
        ("g_w_blstrrfl001", "holster sling"),
        ("g_w_lngswrd01", "holster sheath"),
        ("g_w_stunbaton01", "holster hip_holster"),
    ] {
        if std::env::var("FRAME_ITEM").is_ok_and(|w| w != item) {
            continue;
        }
        let Some(mut s) = stage() else { return };
        let pc = s.game.world.pc;
        s.game.world.object_mut(pc).unwrap().appearance = 92;
        // Only the hero's: holsters of other creatures come and go as they walk about.
        let count = |s: &mut Stage| {
            let at = s.game.world.object(pc).unwrap().position;
            s.dynamic_scene()
                .meshes
                .iter()
                .filter(|m| m.name == shape)
                .filter(|m| {
                    m.positions
                        .iter()
                        .all(|p| (p[0] - at[0]).hypot(p[1] - at[1]) < 0.6)
                })
                .count()
        };
        let before = count(&mut s);
        let gun = s.game.world.spawn_item(item);
        // FRAME_SLOT=5 puts it in the left hand.
        let slot: i32 = std::env::var("FRAME_SLOT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(4);
        s.game
            .world
            .object_mut(pc)
            .unwrap()
            .equipped
            .insert(slot, gun);
        s.update(DT);
        let stored = count(&mut s);
        assert_eq!(stored, before + 1, "{item}: its holster is drawn");
        // FRAME_HOLSTER is a folder: a front, side and back frame of the weapon stored and then drawn.
        if let Ok(dir) = std::env::var("FRAME_HOLSTER") {
            for state in ["stored", "reach", "drawn"] {
                s.reach_hold = (state == "reach").then_some(1.0);
                if state == "drawn" {
                    s.game.hold_stance(pc);
                    for _ in 0..(0.45 / DT) as usize {
                        s.update(DT);
                    }
                }
                let at = s.game.world.object(pc).unwrap().position;
                let heading = s.game.world.pc_mover.heading;
                for (view, side, away) in [
                    ("front", 1.0, false),
                    ("side", 1.0, true),
                    ("back", -1.0, false),
                ] {
                    let (dx, dy) = if away {
                        (-heading.sin(), heading.cos())
                    } else {
                        (heading.cos(), heading.sin())
                    };
                    let cam = kotor_render::Camera {
                        eye: [
                            at[0] + dx * 2.2 * side,
                            at[1] + dy * 2.2 * side,
                            at[2] + 1.1,
                        ],
                        target: [at[0], at[1], at[2] + 0.95],
                        fov: 40.0,
                    };
                    let mut gpu = GpuRenderer::new(480, 640).unwrap();
                    let still = s.static_scene().clone();
                    gpu.set_static_scene(&still, &mut |n| s.texture(n));
                    let moving = s.dynamic_scene();
                    let img = gpu.render(&moving, &mut |n| s.texture(n), &cam).unwrap();
                    std::fs::write(
                        format!("{dir}/{item}-{state}-{view}.png"),
                        kotor_image::to_png(&img),
                    )
                    .unwrap();
                }
            }
            continue;
        }
        s.game.hold_stance(pc);
        for _ in 0..(0.45 / DT) as usize {
            s.update(DT);
        }
        assert_eq!(count(&mut s), before, "{item}: no holster in the hand");
        s.game.world.object_mut(pc).unwrap().equipped.remove(&slot);
    }
}

/// Entering attack mode draws the weapon (the draw clip plays and the weapon is in the hand part way through), and leaving it
/// holsters the same way (`docs/HOLSTERS.md`, stages 2 and 3).
#[test]
fn attack_mode_draws_and_holsters_the_weapon() {
    let Some(mut s) = stage() else { return };
    let pc = s.game.world.pc;
    let gun = s.game.world.spawn_item("g_w_blstrpstl001");
    s.game.world.object_mut(pc).unwrap().equipped.insert(4, gun);
    for _ in 0..4 {
        s.update(DT);
    }
    assert!(!s.game.weapons_drawn(pc), "holstered out of a fight");
    s.game.world.set_attack_mode(true);
    s.update(DT);
    assert!(
        !s.game.weapons_drawn(pc),
        "still in the holster as the draw begins"
    );
    assert_eq!(
        s.game.clip(pc).map(|c| c.name),
        Some("equip"),
        "the draw clip plays"
    );
    let steps = (0.45 / DT) as usize;
    for _ in 0..steps {
        s.update(DT);
    }
    assert!(
        s.game.weapons_drawn(pc),
        "in the hand part way through the draw"
    );
    assert!(
        !s.game.weapon_ready(pc),
        "not ready to fire until the draw has finished"
    );
    assert!(s.game.draw_progress(pc).is_some(), "the hand is reaching");
    for _ in 0..(0.5 / DT) as usize {
        s.update(DT);
    }
    assert!(s.game.weapon_ready(pc), "ready to fire after the draw");
    assert!(s.game.draw_progress(pc).is_none());
    s.game.world.set_attack_mode(false);
    // The stance from the fight holds a few seconds; wait it out, then the holster plays.
    let mut left = 0;
    while s.game.weapons_drawn(pc) && left < 3000 {
        s.update(DT);
        left += 1;
    }
    assert!(left < 3000, "the weapon goes back");
    assert!(!s.game.weapons_drawn(pc));
}

/// A soldier that is attacked draws its weapon, as the player does. With `FRAME_NPC` set to a folder, frames of the draw are written there.
#[test]
fn a_soldier_draws_its_weapon_when_a_fight_starts() {
    let Some(mut s) = stage() else { return };
    let pc = s.game.world.pc;
    let Some(foe) = s
        .game
        .world
        .objects
        .values()
        .find(|o| o.template == "n_sithsoldier003")
        .map(|o| o.id)
    else {
        return;
    };
    let at = s.game.world.object(foe).unwrap().position;
    s.game.world.teleport(pc, [at[0] - 3.0, at[1], at[2]]);
    for _ in 0..10 {
        s.update(DT);
    }
    assert!(
        !s.game.weapon_ready(foe),
        "its weapon is away before the fight"
    );
    s.game.world.attack(foe, pc);
    let mut reached = false;
    for i in 0..(1.0 / DT) as usize {
        s.update(DT);
        reached |= s.game.draw_progress(foe).is_some_and(|p| p > 0.9);
        if let Ok(dir) = std::env::var("FRAME_NPC") {
            if i % 10 == 5 && i < 60 {
                let at = s.game.world.object(foe).unwrap().position;
                let cam = kotor_render::Camera {
                    eye: [at[0] - 2.6, at[1], at[2] + 1.5],
                    target: [at[0], at[1], at[2] + 1.0],
                    fov: 40.0,
                };
                let mut gpu = GpuRenderer::new(480, 640).unwrap();
                let still = s.static_scene().clone();
                gpu.set_static_scene(&still, &mut |n| s.texture(n));
                let moving = s.dynamic_scene();
                let img = gpu.render(&moving, &mut |n| s.texture(n), &cam).unwrap();
                std::fs::write(
                    format!("{dir}/soldier-{i:02}.png"),
                    kotor_image::to_png(&img),
                )
                .unwrap();
            }
        }
    }
    assert!(reached, "the soldier's hand reaches for its weapon");
    assert!(s.game.weapon_ready(foe));
}

/// How long a frame's scene step takes in a busy area (`cargo test --test stage skinning_cost -- --nocapture`): printed, and held
/// to a loose limit. Bodies far from the eye or behind the camera are skinned less often or not at all.
#[test]
fn skinning_cost_of_a_busy_area_stays_small() {
    let Some(mut s) = stage() else { return };
    for _ in 0..60 {
        s.update(DT);
    }
    let mut times: Vec<f64> = Vec::new();
    for _ in 0..120 {
        s.update(DT);
        let t = std::time::Instant::now();
        let scene = s.dynamic_scene();
        times.push(t.elapsed().as_secs_f64() * 1000.0);
        std::hint::black_box(scene.meshes.len());
    }
    times.sort_by(|a, b| a.total_cmp(b));
    eprintln!(
        "scene step: median {:.2} ms, worst {:.2} ms",
        times[times.len() / 2],
        times[times.len() - 1]
    );
}

/// The owner, 8 Nov: "Carth isnt speaking and is still displaying facial animations". A line with no voice clip (a hint, the
/// narrator, a voice this install lacks) moves no mouth and starts no talk gesture: nobody's `mouth_of` is set, and the speaker does not
/// play the talk clip.
#[test]
fn a_line_with_no_voice_moves_no_mouth() {
    let Some(mut s) = stage() else { return };
    let (mut silent, mut voiced) = (0, 0);
    let mut shot = 0;
    let mut gpu: Option<GpuRenderer> = None;
    for f in 0..3000 {
        s.update(DT);
        let w = &s.game.world;
        if f > 30 && w.in_dialogue() {
            let ids: Vec<_> = w
                .objects
                .values()
                .filter(|o| o.kind == kotor_game::Kind::Creature)
                .map(|o| o.id)
                .collect();
            if s.line_has_voice() {
                voiced += 1;
            } else {
                silent += 1;
                // FRAME_SILENT=<dir>: up to 4 frames of the speaker, 12 steps apart, while the line has no voice.
                if let Ok(dir) = std::env::var("FRAME_SILENT") {
                    if silent % 12 == 1 && shot < 4 {
                        let g = gpu.get_or_insert_with(|| {
                            s.set_view((1280, 720));
                            GpuRenderer::new(1280, 720).unwrap()
                        });
                        std::fs::create_dir_all(&dir).unwrap();
                        let scene = s.dynamic_scene();
                        let still = s.static_scene().clone();
                        g.set_static_scene(&still, &mut |n| s.texture(n));
                        let cam = s.speaker_camera().unwrap_or_else(|| s.camera());
                        let img = g.render(&scene, &mut |n| s.texture(n), &cam).unwrap();
                        std::fs::write(
                            format!("{dir}/silent_{shot}.png"),
                            kotor_image::to_png(&img),
                        )
                        .unwrap();
                        shot += 1;
                    }
                }
                for id in ids {
                    assert!(
                        s.mouth_of(id).is_none(),
                        "a silent line moves a mouth (frame {f})"
                    );
                    assert_ne!(
                        s.dialogue_clip(id).map(|c| c.name),
                        Some("tlknorm"),
                        "and starts a talk gesture (frame {f})"
                    );
                }
            }
            if f % 15 == 0 {
                s.game.world.dialogue_choose(0);
            }
        }
    }
    eprintln!("silent frames {silent}, voiced frames {voiced}");
}

/// The owner: "Trask's voicelines dont play upon starting a new game in modern mode". A new game says the same lines with the same
/// voices in both modes: the files asked for in the first thirty seconds, with nobody pressing a key.
#[test]
fn a_new_game_asks_for_the_same_voices_in_both_modes() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let mut asked: Vec<Vec<String>> = Vec::new();
    for mode in [kotor_game::GameMode::Classic, kotor_game::GameMode::Modern] {
        let mut s = Stage::open_in_mode("end_m01aa", true, mode).unwrap();
        let mut voices = Vec::new();
        let (mut shown, mut stops) = (0, 0);
        for step in 0..3600 {
            if s.game.world.in_dialogue() && step % 20 == 0 {
                s.continue_line();
                s.number_key(1);
            }
            for cue in s.update(DT) {
                match cue {
                    Cue::Voice(p) => {
                        voices.push(format!(
                            "{step}:{}",
                            p.file_stem().unwrap().to_string_lossy()
                        ));
                    }
                    Cue::StopVoice => stops += 1,
                    _ => {}
                }
            }
            shown = s.lines_shown;
        }
        eprintln!("{mode:?}: lines shown {shown}, stops {stops}, voices {voices:?}");
        asked.push(
            voices
                .into_iter()
                .map(|v| v.split(':').nth(1).unwrap().to_string())
                .collect(),
        );
    }
    assert!(!asked[0].is_empty(), "Classic asks for no voice at all");
    assert_eq!(asked[0], asked[1], "the two modes ask for other voices");
}
