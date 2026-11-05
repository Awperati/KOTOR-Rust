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
