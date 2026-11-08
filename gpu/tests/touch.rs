//! The on-screen controls and fingers on them, without a window.

use kotor_gpu::stage::Stage;
use kotor_ui::{TouchKey, TouchPart, TOUCH_BUTTON};

const DT: f32 = 1.0 / 30.0;
const SIZE: (u32, u32) = (1280, 720);
/// Landscape, the HUD's own 4:3, and narrower than that.
const SIZES: [(u32, u32); 8] = [
    (1280, 720),
    (1920, 1080),
    (3840, 2160),
    (2560, 1080),
    (1024, 768),
    (640, 480),
    (800, 1000),
    (720, 1280),
];

fn on_floor(name: &str) -> Option<Stage> {
    kotor_core::install_dir()?;
    let mut s = Stage::open("end_m01aa", true).unwrap();
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let dir = std::env::temp_dir().join(format!("kotor_touch_{name}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    s.set_saves_dir(dir.join("saves"));
    let _ = std::fs::remove_file(dir.join("settings.txt"));
    s.use_settings_file(dir.join("settings.txt"));
    // The stick and its buttons as they were made, with Modern movement: the game's default is Classic.
    s.set_movement_modern(true);
    s.set_view(SIZE);
    for _ in 0..12000 {
        s.update(DT);
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        } else if s.game.world.transcript.len() > 3 && s.game.world.delayed.is_empty() {
            break;
        }
    }
    for _ in 0..30 {
        s.update(DT);
    }
    assert!(!s.game.world.in_dialogue());
    Some(s)
}

fn where_is(s: &Stage) -> [f32; 3] {
    s.game.world.pc_mover.pos
}

fn tap(s: &mut Stage, at: (f32, f32), size: (u32, u32)) {
    s.finger_down(1, at, size);
    s.finger_up(1, at, size);
}

fn turn_on(s: &mut Stage, size: (u32, u32)) {
    if !s.touch_on() {
        let blue = s.hud_pixel(TOUCH_BUTTON, size).expect("the blue button");
        tap(s, blue, size);
    }
    assert!(s.touch_on());
}

fn key(s: &mut Stage, k: TouchKey) -> (f32, f32) {
    s.touch_pixel(TouchPart::Key(k), SIZE).expect("the key")
}

fn stick(s: &mut Stage) -> (f32, f32) {
    s.touch_pixel(TouchPart::Stick, SIZE).expect("the stick")
}

/// A few frames as the window has them: the stick is what the player steers by.
fn run(s: &mut Stage, frames: usize) {
    for _ in 0..frames {
        s.steer = s.touch_steer().unwrap_or((0.0, 0.0, false));
        s.set_looking(s.touch_looking());
        s.update(DT);
    }
}

#[test]
fn the_blue_button_turns_the_layer_on_and_off_and_the_settings_remember() {
    let Some(mut s) = on_floor("blue") else {
        return;
    };
    assert!(!s.touch_on(), "off until asked for");
    assert!(s.touch_pixel(TouchPart::Stick, SIZE).is_some());
    let at = s.hud_pixel(TOUCH_BUTTON, SIZE).unwrap();
    assert!(
        at.0 > SIZE.0 as f32 * 0.9 && at.1 < SIZE.1 as f32 * 0.25,
        "top right: {at:?}"
    );
    // The window's own pixels of the button are blue.
    s.update(DT);
    let o = s.overlay().unwrap();
    let i = ((at.1 as u32 + 18) * o.width + at.0 as u32) as usize * 4;
    assert!(
        o.rgba[i + 2] > 180 && o.rgba[i] < 90,
        "{:?}",
        &o.rgba[i..i + 4]
    );

    tap(&mut s, at, SIZE);
    assert!(s.touch_on());
    let dir = std::env::temp_dir().join(format!("kotor_touch_blue_{}", std::process::id()));
    let file = dir.join("settings.txt");
    let said = std::fs::read_to_string(&file).unwrap();
    assert!(said.contains("touch_controls=true"), "{said}");

    // It stays where it is: the same tap again is not a menu key.
    assert_eq!(s.hud_pixel(TOUCH_BUTTON, SIZE), Some(at));
    tap(&mut s, at, SIZE);
    assert!(!s.touch_on());
    assert_eq!(s.menu_name(), "-");

    // The mouse does it too.
    s.hover(at, SIZE);
    s.click(at, SIZE);
    assert!(s.touch_on());

    // A new start reads it back.
    let said = std::fs::read_to_string(&file).unwrap();
    assert!(said.contains("touch_controls=true"));
    tap(&mut s, at, SIZE);
    assert!(!s.touch_on());
    std::fs::write(&file, said).unwrap();
    s.use_settings_file(file);
    assert!(s.touch_on());
}

#[test]
fn the_controls_page_has_a_box_for_it() {
    let Some(mut s) = on_floor("page") else {
        return;
    };
    let press = |s: &mut Stage, tag: &str| {
        let p = s
            .menu_pixel(tag, None, SIZE)
            .unwrap_or_else(|| panic!("no {tag} on {}", s.menu_name()));
        s.hover(p, SIZE);
        s.click(p, SIZE);
    };
    s.escape();
    press(&mut s, "BTN_GAMEPLAY");
    assert_eq!(
        s.menu_button_words("BTN_MOUSE").as_deref(),
        Some("Controls")
    );
    press(&mut s, "BTN_MOUSE");
    assert_eq!(
        s.menu_button_words("LBL_TITLE").as_deref(),
        Some("Controls")
    );
    assert_eq!(
        s.menu_button_words("CB_TOUCH").as_deref(),
        Some("On-screen Controls")
    );
    assert!(!s.touch_on());
    press(&mut s, "CB_TOUCH");
    assert!(s.touch_on());
    press(&mut s, "BTN_DEFAULT");
    assert!(!s.touch_on());
}

#[test]
fn the_layer_stays_off_the_hud_and_itself_and_is_big_at_every_size() {
    let Some(mut s) = on_floor("places") else {
        return;
    };
    let over = |a: [i32; 4], b: [i32; 4]| {
        a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
    };
    for size in SIZES {
        s.set_view(size);
        turn_on(&mut s, size);
        let width = s.ui_layout_width() as i32;
        let (across, down) = (size.0 as f32 / width as f32, size.1 as f32 / 480.0);
        let (layer, hud) = s.touch_boxes().unwrap();
        assert_eq!(layer.len(), 9, "two sticks and seven buttons");
        for (n, a) in layer.iter().enumerate() {
            assert!(
                a[0] >= 0 && a[1] >= 0 && a[0] + a[2] <= width && a[1] + a[3] <= 480,
                "{size:?}: {a:?} is off the window"
            );
            let (wide, high) = (a[2] as f32 * across, a[3] as f32 * down);
            assert!(
                wide >= 44.0 && high >= 44.0,
                "{size:?}: {a:?} is {wide} by {high} pixels"
            );
            assert!((wide - high).abs() <= 4.0, "{size:?}: {a:?} is not round");
            for b in &layer[n + 1..] {
                assert!(!over(*a, *b), "{size:?}: {a:?} and {b:?}");
            }
            for (tag, h) in &hud {
                assert!(!over(*a, *h), "{size:?}: {a:?} is on {tag} {h:?}");
            }
        }
        // What was small for a finger is not now.
        for (tag, h) in &hud {
            if tag.starts_with("BTN_") && !tag.contains("ACTIONUP") && !tag.contains("ACTIONDOWN") {
                let (wide, high) = (h[2] as f32 * across, h[3] as f32 * down);
                assert!(
                    wide >= 38.0 && high >= 38.0,
                    "{size:?}: {tag} is {wide} by {high} pixels"
                );
            }
            assert!(
                h[0] + h[2] <= width + 1,
                "{size:?}: {tag} {h:?} goes off the window"
            );
        }
        // Off, the HUD's own keys are back and the blue button is still on the window.
        let blue = s.hud_pixel(TOUCH_BUTTON, size).unwrap();
        tap(&mut s, blue, size);
        assert!(!s.touch_on());
        assert!(blue.0 < size.0 as f32 && blue.1 < size.1 as f32);
    }
}

#[test]
fn a_thumb_on_the_stick_fingers_on_buttons_and_on_the_camera_all_work_at_once() {
    let Some(mut s) = on_floor("fingers") else {
        return;
    };
    turn_on(&mut s, SIZE);
    assert_eq!(s.touch_steer(), None);
    let from = where_is(&s);

    // The left thumb pushes up: ahead, running.
    let thumb = stick(&mut s);
    s.finger_down(11, thumb, SIZE);
    s.finger_moved(11, (thumb.0, thumb.1 - 120.0), SIZE);
    let (ahead, right, walk) = s.touch_steer().unwrap();
    assert!(
        ahead > 0.9 && right.abs() < 0.05 && !walk,
        "{ahead} {right}"
    );
    run(&mut s, 30);
    let at = where_is(&s);
    let gone = (at[0] - from[0]).hypot(at[1] - from[1]);
    assert!(gone > 0.5, "it has run {gone} m");

    // The other thumb jumps, with the first still down.
    let jump = key(&mut s, TouchKey::Jump);
    s.finger_down(12, jump, SIZE);
    assert!(s.game.world.jump_request, "asked to jump");
    assert_eq!(s.fingers_down(), 2);
    assert!(s.touch_steer().unwrap().0 > 0.9, "the stick is still held");

    // A third finger drags on the right half: the camera, and the stick stays.
    let (yaw, free) = (s.yaw, (700.0, 200.0));
    s.finger_down(13, free, SIZE);
    assert!(!s.touch_looking(), "not till it moves");
    s.finger_moved(13, (800.0, 200.0), SIZE);
    assert!(s.touch_looking());
    s.finger_moved(13, (900.0, 200.0), SIZE);
    assert!(
        (s.yaw - yaw).abs() > 0.2,
        "the camera turned: {yaw} {}",
        s.yaw
    );
    assert_eq!(s.fingers_down(), 3);
    assert!(s.touch_steer().unwrap().0 > 0.9);

    // Off in any order. A drag was no tap, so nothing else was asked.
    s.finger_up(12, jump, SIZE);
    assert!(s.touch_steer().is_some());
    s.finger_up(13, (900.0, 200.0), SIZE);
    assert!(!s.touch_looking());
    assert!(s.touch_steer().is_some());
    s.finger_up(11, (thumb.0, thumb.1 - 120.0), SIZE);
    assert_eq!(s.touch_steer(), None);
    assert_eq!(s.fingers_down(), 0);

    // A light push walks.
    s.finger_down(11, thumb, SIZE);
    s.finger_moved(11, (thumb.0 + 30.0, thumb.1), SIZE);
    let (ahead, right, walk) = s.touch_steer().unwrap();
    assert!(
        ahead.abs() < 0.05 && right > 0.2 && walk,
        "{ahead} {right} {walk}"
    );
    // Hardly at all is nothing.
    s.finger_moved(11, (thumb.0 + 3.0, thumb.1), SIZE);
    assert_eq!(s.touch_steer(), Some((0.0, 0.0, false)));
    // The window losing the input lets go of all of it.
    s.finger_down(12, jump, SIZE);
    s.fingers_gone();
    assert_eq!(s.fingers_down(), 0);
    assert_eq!(s.touch_steer(), None);
}

/// Nobody starts a conversation: one swings the camera round to the speaker, whatever the stick says.
fn quiet_room(s: &mut Stage) {
    let w = &mut s.game.world;
    let pc = w.pc;
    for o in w.objects.values_mut().filter(|o| o.id != pc) {
        o.conversation.clear();
        o.scripts.clear();
    }
    w.delayed.clear();
}

fn look_stick(s: &mut Stage) -> (f32, f32) {
    s.touch_pixel(TouchPart::Look, SIZE)
        .expect("the look stick")
}

/// How far the camera turns round and tilts in a second, with the look stick pushed this far from its middle.
fn looked(s: &mut Stage, by: (f32, f32)) -> (f32, f32) {
    let at = look_stick(s);
    s.finger_down(5, at, SIZE);
    s.finger_moved(5, (at.0 + by.0, at.1 + by.1), SIZE);
    s.pitch = 0.5;
    let (yaw, pitch) = (s.yaw, s.pitch);
    run(s, 30);
    assert!(!s.game.world.in_dialogue());
    let turned = (s.yaw - yaw, s.pitch - pitch);
    s.finger_up(5, at, SIZE);
    turned
}

#[test]
fn the_look_stick_turns_the_camera_while_it_is_pushed_the_faster_the_further() {
    let Some(mut s) = on_floor("look") else {
        return;
    };
    quiet_room(&mut s);
    turn_on(&mut s, SIZE);
    // All the way to the right: round to the right, as the mouse turns it.
    let (full, tilt) = looked(&mut s, (200.0, 0.0));
    assert!(full < -1.5 && full > -2.5, "{full}");
    assert!(tilt.abs() < 0.01, "{tilt}");
    assert!(!s.touch_looking());
    // And the same the other way.
    let (left, _) = looked(&mut s, (-200.0, 0.0));
    assert!((left + full).abs() < 0.1, "{left} {full}");
    // Half way is slower.
    let (half, _) = looked(&mut s, (38.0, 0.0));
    assert!(half < -0.3 && half > full * 0.7, "{half} {full}");
    // Hardly at all is nothing.
    let (none, _) = looked(&mut s, (4.0, 0.0));
    assert_eq!(none, 0.0);
    // Let go, it stops.
    let yaw = s.yaw;
    run(&mut s, 20);
    assert_eq!(s.yaw, yaw);
    // It holds still when the stick is held still: its speed is in how far, and the finger need not move.
    let at = look_stick(&mut s);
    s.finger_down(5, at, SIZE);
    s.finger_moved(5, (at.0 + 200.0, at.1), SIZE);
    run(&mut s, 5);
    let first = s.yaw;
    run(&mut s, 5);
    assert!((first - s.yaw) > 0.1, "still turning");
    assert!(s.touch_looking());
    s.finger_up(5, (at.0 + 200.0, at.1), SIZE);
    // Pushed down it looks down, and the Invert Camera box is for it too.
    let (_, tilt) = looked(&mut s, (0.0, 200.0));
    assert!(tilt > 0.5, "{tilt}");
    let press = |s: &mut Stage, tag: &str| {
        let p = s
            .menu_pixel(tag, None, SIZE)
            .unwrap_or_else(|| panic!("no {tag} on {}", s.menu_name()));
        s.hover(p, SIZE);
        s.click(p, SIZE);
    };
    s.escape();
    press(&mut s, "BTN_GAMEPLAY");
    press(&mut s, "CB_INVERTCAM");
    assert!(s.settings().invert_y);
    press(&mut s, "BTN_MOUSE");
    // The speed's row, past the middle of its slider, and it remembers.
    assert_eq!(
        s.menu_button_words("LBL_LOOKSEN").as_deref(),
        Some("Look Stick Speed")
    );
    let middle = s.menu_pixel("SLI_LOOKSEN", None, SIZE).unwrap();
    let far = (middle.0 + 150.0, middle.1);
    s.hover(far, SIZE);
    s.click(far, SIZE);
    let speed = s.settings().look_stick_speed;
    assert!(speed > 0.6, "{speed}");
    let dir = std::env::temp_dir().join(format!("kotor_touch_look_{}", std::process::id()));
    let said = std::fs::read_to_string(dir.join("settings.txt")).unwrap();
    assert!(said.contains("look_stick_speed="), "{said}");
    press(&mut s, "BTN_BACK");
    press(&mut s, "BTN_BACK");
    s.escape();
    assert_eq!(s.menu_name(), "-");
    let (_, tilt) = looked(&mut s, (0.0, 200.0));
    assert!(tilt < -0.5, "the other way: {tilt}");
    let (faster, _) = looked(&mut s, (200.0, 0.0));
    assert!(faster < full * 1.15, "{faster} {full}");
}

#[test]
fn both_sticks_and_a_button_go_together_and_the_mouse_can_hold_either() {
    let Some(mut s) = on_floor("both") else {
        return;
    };
    quiet_room(&mut s);
    let at = look_stick(&mut s);
    assert!(!s.mouse_down_on_layer(at, SIZE), "the layer is off");
    turn_on(&mut s, SIZE);

    let (from, yaw) = (where_is(&s), s.yaw);
    let thumb = stick(&mut s);
    s.finger_down(1, thumb, SIZE);
    s.finger_moved(1, (thumb.0, thumb.1 - 120.0), SIZE);
    s.finger_down(2, at, SIZE);
    s.finger_moved(2, (at.0 + 200.0, at.1), SIZE);
    let jump = key(&mut s, TouchKey::Jump);
    s.finger_down(3, jump, SIZE);
    assert!(s.game.world.jump_request);
    assert_eq!(s.fingers_down(), 3);
    run(&mut s, 30);
    let here = where_is(&s);
    assert!((here[0] - from[0]).hypot(here[1] - from[1]) > 0.5, "it ran");
    assert!(s.yaw < yaw - 1.0, "it looked round: {yaw} {}", s.yaw);
    assert!(s.touch_steer().unwrap().0 > 0.9, "the one is not the other");
    s.fingers_gone();
    assert_eq!(s.touch_steer(), None);
    let yaw = s.yaw;
    run(&mut s, 10);
    assert_eq!(s.yaw, yaw);

    // The mouse holds it as a finger does, and then the other, and neither is steered by the other.
    assert!(s.mouse_down_on_layer(at, SIZE));
    s.mouse_moved_on_layer((at.0 - 200.0, at.1), SIZE);
    assert_eq!(s.touch_steer(), None, "it does not move the player");
    run(&mut s, 10);
    assert!(s.yaw > yaw + 0.2, "{yaw} {}", s.yaw);
    s.mouse_up_on_layer();
    let yaw = s.yaw;
    run(&mut s, 10);
    assert_eq!(s.yaw, yaw);
    assert!(s.mouse_down_on_layer(thumb, SIZE));
    s.mouse_moved_on_layer((thumb.0, thumb.1 - 120.0), SIZE);
    assert!(s.touch_steer().unwrap().0 > 0.9);
    run(&mut s, 5);
    assert_eq!(s.yaw, yaw, "and the other way about");
    s.mouse_up_on_layer();

    // A drag on the right half off the sticks still turns the camera.
    s.finger_down(1, (900.0, 200.0), SIZE);
    s.finger_moved(1, (1000.0, 200.0), SIZE);
    assert!((s.yaw - yaw).abs() > 0.1);
    s.finger_up(1, (1000.0, 200.0), SIZE);
}

#[test]
fn the_jump_button_is_greyed_while_a_light_push_walks() {
    let Some(mut s) = on_floor("walk_grey") else {
        return;
    };
    turn_on(&mut s, SIZE);
    assert!(!s.touch_key_greyed(TouchKey::Jump));
    let thumb = stick(&mut s);
    s.finger_down(1, thumb, SIZE);
    // Held still in the middle: a stand, and a stand jumps.
    assert!(!s.touch_key_greyed(TouchKey::Jump));
    s.finger_moved(1, (thumb.0 + 30.0, thumb.1), SIZE);
    assert!(s.touch_key_greyed(TouchKey::Jump), "a walk");
    assert!(!s.touch_key_greyed(TouchKey::Roll));
    s.finger_moved(1, (thumb.0 + 120.0, thumb.1), SIZE);
    assert!(!s.touch_key_greyed(TouchKey::Jump), "a run");
    s.finger_moved(1, (thumb.0 + 30.0, thumb.1), SIZE);
    assert!(s.touch_key_greyed(TouchKey::Jump));
    s.finger_up(1, thumb, SIZE);
    assert!(!s.touch_key_greyed(TouchKey::Jump), "let go");
}

#[test]
fn the_stick_steers_classic_movement_too_where_the_jump_and_roll_are_greyed() {
    let Some(mut s) = on_floor("classic") else {
        return;
    };
    s.set_movement_modern(false);
    turn_on(&mut s, SIZE);
    let from = where_is(&s);
    let thumb = stick(&mut s);
    s.finger_down(1, thumb, SIZE);
    s.finger_moved(1, (thumb.0, thumb.1 - 120.0), SIZE);
    run(&mut s, 30);
    let at = where_is(&s);
    assert!((at[0] - from[0]).hypot(at[1] - from[1]) > 0.5);
    s.finger_up(1, thumb, SIZE);
    let (jump, roll) = (key(&mut s, TouchKey::Jump), key(&mut s, TouchKey::Roll));
    tap(&mut s, jump, SIZE);
    tap(&mut s, roll, SIZE);
    assert!(!s.game.world.jump_request && !s.game.world.roll_request);
}

#[test]
fn a_tap_is_a_click_and_a_drag_is_not_and_only_the_right_half_turns_the_camera() {
    let Some(mut s) = on_floor("tap") else {
        return;
    };
    // Nobody to walk into, and a clear floor.
    let w = &mut s.game.world;
    let pc = w.pc;
    let others: Vec<u32> = w
        .objects
        .values()
        .filter(|o| o.id != pc && o.kind == kotor_game::Kind::Creature)
        .map(|o| o.id)
        .collect();
    for id in others {
        let at = w.object(id).unwrap().position;
        w.teleport(id, [at[0] + 500.0, at[1], at[2]]);
    }
    turn_on(&mut s, SIZE);
    run(&mut s, 3);
    let floor = (640.0, 560.0);
    let from = where_is(&s);

    // A drag on the right half: the camera, and the player stays.
    let yaw = s.yaw;
    s.finger_down(1, (900.0, 300.0), SIZE);
    s.finger_moved(1, (1000.0, 300.0), SIZE);
    s.finger_up(1, (1000.0, 300.0), SIZE);
    assert!((s.yaw - yaw).abs() > 0.1);
    run(&mut s, 20);
    let at = where_is(&s);
    assert!((at[0] - from[0]).hypot(at[1] - from[1]) < 0.05, "it stayed");

    // On the left half, off the stick: neither.
    let yaw = s.yaw;
    s.finger_down(1, (420.0, 300.0), SIZE);
    s.finger_moved(1, (560.0, 300.0), SIZE);
    s.finger_up(1, (560.0, 300.0), SIZE);
    assert_eq!(s.yaw, yaw);

    // A tap on the floor walks there, as a click does, with Click to move on. A finger shakes a little.
    s.set_click_to_move(true);
    s.finger_down(1, floor, SIZE);
    s.finger_moved(1, (floor.0 + 3.0, floor.1 + 2.0), SIZE);
    s.finger_up(1, (floor.0 + 3.0, floor.1 + 2.0), SIZE);
    run(&mut s, 30);
    let at = where_is(&s);
    assert!(
        (at[0] - from[0]).hypot(at[1] - from[1]) > 0.2,
        "it walked: {from:?} {at:?}"
    );
}

#[test]
fn each_round_button_does_what_its_key_does() {
    let Some(mut s) = on_floor("keys") else {
        return;
    };
    turn_on(&mut s, SIZE);
    run(&mut s, 3);

    let pause = key(&mut s, TouchKey::Pause);
    tap(&mut s, pause, SIZE);
    assert!(s.paused());
    tap(&mut s, pause, SIZE);
    assert!(!s.paused());

    let mode = key(&mut s, TouchKey::AttackMode);
    tap(&mut s, mode, SIZE);
    assert!(s.game.world.attack_mode());
    tap(&mut s, mode, SIZE);
    assert!(!s.game.world.attack_mode());

    let roll = key(&mut s, TouchKey::Roll);
    tap(&mut s, roll, SIZE);
    assert!(s.game.world.roll_request);
    run(&mut s, 30);

    let jump = key(&mut s, TouchKey::Jump);
    tap(&mut s, jump, SIZE);
    assert!(s.game.world.jump_request);
    run(&mut s, 60);

    // Far from anything, the word over the player.
    let w = &mut s.game.world;
    for o in w.objects.values_mut() {
        o.useable = false;
    }
    let pickup = key(&mut s, TouchKey::Pickup);
    tap(&mut s, pickup, SIZE);
    assert!(
        s.floats_shown().iter().any(|f| f == "Nothing nearby"),
        "{:?}",
        s.floats_shown()
    );

    // The action button talks to who is beside the player, or uses what is.
    run(&mut s, 3);
    let w = &mut s.game.world;
    for o in w.objects.values_mut() {
        o.useable = true;
    }
    let trask = w.by_tag("end_trask").first().copied();
    if let Some(trask) = trask {
        let at = w.object(w.pc).unwrap().position;
        w.teleport(trask, [at[0] + 1.0, at[1], at[2]]);
        run(&mut s, 3);
        let action = key(&mut s, TouchKey::Action);
        tap(&mut s, action, SIZE);
        run(&mut s, 60);
        assert!(s.game.world.in_dialogue(), "talking");
        // A finger moves a line on as a click does.
        let serial = s.game.world.dialogue_serial();
        tap(&mut s, (640.0, 300.0), SIZE);
        run(&mut s, 2);
        assert!(s.game.world.dialogue_serial() != serial || !s.game.world.in_dialogue());
        for _ in 0..600 {
            if !s.game.world.in_dialogue() {
                break;
            }
            s.number_key(1);
            s.continue_line();
            s.update(DT);
        }
    }

    // The menu keys and the first slot, by a finger, are the HUD's own.
    let inventory = s.hud_pixel("BTN_INV", SIZE).unwrap();
    tap(&mut s, inventory, SIZE);
    assert_eq!(s.menu_name(), "inventory");
    let close = s.menu_pixel("BTN_EXIT", None, SIZE).unwrap();
    tap(&mut s, close, SIZE);
    assert_eq!(s.menu_name(), "-");
}

#[test]
fn the_attack_button_goes_for_the_enemy_in_sight() {
    let Some(mut s) = on_floor("attack") else {
        return;
    };
    turn_on(&mut s, SIZE);
    let w = &mut s.game.world;
    let (at, heading) = (w.pc_mover.pos, w.pc_mover.heading);
    let foe = w.spawn_creature(
        "n_calonord",
        [
            at[0] + heading.cos() * 3.0,
            at[1] + heading.sin() * 3.0,
            at[2],
        ],
    );
    let o = w.object_mut(foe).unwrap();
    o.faction = 1;
    (o.hp, o.max_hp) = (5000, 5000);
    let pc = w.pc;
    let o = w.object_mut(pc).unwrap();
    (o.hp, o.max_hp) = (5000, 5000);
    for _ in 0..200 {
        s.update(DT);
        if s.seen_enemy().is_some() {
            break;
        }
    }
    assert_eq!(s.seen_enemy(), Some(foe));
    let attack = key(&mut s, TouchKey::Attack);
    tap(&mut s, attack, SIZE);
    run(&mut s, 5);
    assert_eq!(s.game.world.combat.target.get(&pc), Some(&foe));
}

#[test]
fn the_mouse_holds_the_stick_and_presses_the_buttons_as_a_finger_does() {
    let Some(mut s) = on_floor("mouse") else {
        return;
    };
    let thumb = stick(&mut s);
    assert!(!s.mouse_down_on_layer(thumb, SIZE), "the layer is off");
    turn_on(&mut s, SIZE);
    assert!(s.mouse_down_on_layer(thumb, SIZE));
    s.mouse_moved_on_layer((thumb.0, thumb.1 - 120.0), SIZE);
    assert!(s.touch_steer().unwrap().0 > 0.9);
    s.mouse_up_on_layer();
    assert_eq!(s.touch_steer(), None);

    let pause = key(&mut s, TouchKey::Pause);
    assert!(s.mouse_down_on_layer(pause, SIZE));
    assert!(s.paused());
    s.mouse_up_on_layer();
    assert!(s.mouse_down_on_layer(pause, SIZE));
    s.mouse_up_on_layer();
    assert!(!s.paused());

    // Off it, a click is the mouse's as ever, and the stick's edge is no grab.
    assert!(!s.mouse_down_on_layer((640.0, 560.0), SIZE));
    assert_eq!(s.fingers_down(), 0);
    // With a finger on the stick the mouse is another hand.
    s.finger_down(1, thumb, SIZE);
    assert!(s.mouse_down_on_layer(pause, SIZE));
    assert_eq!(s.fingers_down(), 2);
    s.mouse_up_on_layer();
    assert_eq!(s.fingers_down(), 1);
    assert!(s.touch_steer().is_some());
}

#[test]
fn a_finger_on_a_screen_clicks_and_drags_the_list() {
    let Some(mut s) = on_floor("list") else {
        return;
    };
    s.escape();
    let p = s.menu_pixel("BTN_FEEDBACK", None, SIZE).unwrap();
    tap(&mut s, p, SIZE);
    let top_row = |s: &mut Stage| s.menu_pixel("LB_OPTIONS", Some(0), SIZE);
    let before = top_row(&mut s).unwrap();
    // Up the screen, the rows go up.
    s.finger_down(1, (200.0, 500.0), SIZE);
    s.finger_moved(1, (200.0, 500.0 - SIZE.1 as f32 * 0.25), SIZE);
    s.finger_up(1, (200.0, 300.0), SIZE);
    assert_ne!(top_row(&mut s), Some(before), "the list moved");
}

#[test]
fn a_finger_on_the_turret_looks_and_fires() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let mut s = Stage::open("M12ab", true).unwrap();
    s.update(DT);
    assert!(s.turret_on());
    let looks = s.camera().target;
    s.finger_down(1, (600.0, 300.0), SIZE);
    assert!(s.game.world.minigame.as_ref().unwrap().firing);
    s.finger_moved(1, (700.0, 300.0), SIZE);
    s.update(DT);
    assert_ne!(s.camera().target, looks);
    s.finger_up(1, (700.0, 300.0), SIZE);
    assert!(!s.game.world.minigame.as_ref().unwrap().firing);
}

#[test]
fn fingers_steer_a_race_and_shift_its_gears() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let mut s = Stage::open("tar_m03mg", true).unwrap();
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    for _ in 0..(5.2 / DT) as usize {
        s.update(DT);
    }
    s.finger_down(1, (100.0, 300.0), SIZE);
    assert_eq!(s.touch_steer(), Some((0.0, -1.0, false)));
    s.finger_moved(1, (1200.0, 300.0), SIZE);
    assert_eq!(s.touch_steer(), Some((0.0, 1.0, false)));
    s.finger_up(1, (1200.0, 300.0), SIZE);
    assert_eq!(s.touch_steer(), None);
    assert_eq!(s.game.world.global_numbers["MIN_RACE_GEAR"], 0);
    s.finger_down(2, (640.0, 300.0), SIZE);
    s.finger_up(2, (640.0, 300.0), SIZE);
    for _ in 0..30 {
        s.update(DT);
    }
    assert!(
        s.game.world.global_numbers["MIN_RACE_GEAR"] > 0,
        "it shifted"
    );
}

#[test]
fn the_hud_in_a_narrow_window_is_all_on_it_and_a_click_finds_it() {
    let Some(mut s) = on_floor("narrow") else {
        return;
    };
    let size = (720, 1280);
    s.set_view(size);
    s.update(DT);
    // Nothing is cut off at the right: the far slot and the options key are drawn, at the pixels that say so.
    for tag in ["BTN_OPT", "BTN_ACTION3", TOUCH_BUTTON] {
        let at = s.hud_pixel(tag, size).unwrap();
        assert!(at.0 < size.0 as f32, "{tag} at {at:?}");
        let o = s.overlay().unwrap();
        let (mut any, y) = (false, at.1 as u32);
        for x in (at.0 as u32).saturating_sub(20)..(at.0 as u32 + 20).min(o.width) {
            any |= o.rgba[((y * o.width + x) * 4 + 3) as usize] > 0;
        }
        assert!(any, "{tag} is not drawn at {at:?}");
    }
    let inventory = s.hud_pixel("BTN_INV", size).unwrap();
    tap(&mut s, inventory, size);
    assert_eq!(s.menu_name(), "inventory");
}

#[test]
fn the_hud_drawn_in_part_for_a_moved_stick_is_the_hud_drawn_whole() {
    let Some(mut s) = on_floor("part") else {
        return;
    };
    turn_on(&mut s, SIZE);
    for _ in 0..3 {
        s.update(DT);
    }
    let thumb = stick(&mut s);
    let attack = key(&mut s, TouchKey::Attack);
    let look = look_stick(&mut s);
    s.finger_down(1, thumb, SIZE);
    s.finger_down(3, look, SIZE);
    s.finger_moved(3, (look.0 + 20.0, look.1 + 20.0), SIZE);
    s.update(0.0);
    for n in 1..4 {
        let version = s.overlay_version;
        s.finger_moved(1, (thumb.0 + n as f32 * 25.0, thumb.1 - 30.0), SIZE);
        s.update(0.0);
        assert!(s.overlay_version > version, "the knob was drawn again");
    }
    s.finger_down(2, attack, SIZE);
    s.update(0.0);
    let part = s.overlay().unwrap().clone();
    s.set_view(SIZE);
    let whole = s.overlay().unwrap();
    let different = part
        .rgba
        .chunks(4)
        .zip(whole.rgba.chunks(4))
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(different, 0, "pixels differ");
}

/// A light push walks, and there is no jump at a walk: the button asks, and the world says no.
#[test]
fn the_jump_button_does_nothing_under_a_light_push_of_the_stick() {
    let Some(mut s) = on_floor("walkjump") else {
        return;
    };
    turn_on(&mut s, SIZE);
    let thumb = stick(&mut s);
    s.finger_down(11, thumb, SIZE);
    s.finger_moved(11, (thumb.0, thumb.1 - 25.0), SIZE);
    assert!(s.touch_steer().unwrap().2, "a light push walks");
    run(&mut s, 20);
    let jump = key(&mut s, TouchKey::Jump);
    s.finger_down(12, jump, SIZE);
    run(&mut s, 2);
    assert!(s.game.world.pc_body.grounded, "no jump at a walk");
    s.finger_up(12, jump, SIZE);
    // A full push runs, and the same button jumps.
    s.finger_moved(11, (thumb.0, thumb.1 - 120.0), SIZE);
    assert!(!s.touch_steer().unwrap().2);
    run(&mut s, 20);
    s.finger_down(12, jump, SIZE);
    run(&mut s, 2);
    assert!(!s.game.world.pc_body.grounded, "a jump at a run");
}
