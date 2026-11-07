//! The compass and the tracked quest, without a window.

use kotor_game::Kind;
use kotor_gpu::stage::Stage;
use kotor_ui::{CompassIcon, TOUCH_BUTTON};

const DT: f32 = 1.0 / 30.0;
const SIZE: (u32, u32) = (1280, 720);
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
    let dir = std::env::temp_dir().join(format!("kotor_compass_{name}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    s.set_saves_dir(dir.join("saves"));
    let _ = std::fs::remove_file(dir.join("settings.txt"));
    s.use_settings_file(dir.join("settings.txt"));
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
    // Nobody starts a conversation, which would swing the camera.
    let w = &mut s.game.world;
    let pc = w.pc;
    for o in w.objects.values_mut().filter(|o| o.id != pc) {
        o.conversation.clear();
        o.scripts.clear();
    }
    w.delayed.clear();
    for _ in 0..30 {
        s.update(DT);
    }
    assert!(!s.game.world.in_dialogue());
    Some(s)
}

fn here(s: &Stage) -> [f32; 3] {
    s.game.world.pc_mover.pos
}

/// A shop this far from the player, this many radians to the right of where the camera looks.
fn shop_at(s: &mut Stage, far: f32, right: f32) -> u32 {
    let at = here(s);
    let bearing = s.yaw - right;
    let spot = [
        at[0] + bearing.cos() * far,
        at[1] + bearing.sin() * far,
        at[2],
    ];
    let w = &mut s.game.world;
    let id = w
        .objects
        .values()
        .find(|o| o.kind == Kind::Placeable)
        .map(|o| o.id)
        .expect("a thing");
    let o = w.object_mut(id).unwrap();
    o.kind = Kind::Store;
    o.position = spot;
    id
}

fn shops(s: &mut Stage) -> Vec<kotor_ui::CompassMark> {
    let view = s.compass_now();
    view.marks
        .into_iter()
        .filter(|m| m.icon == CompassIcon::Shop)
        .collect()
}

#[test]
fn marks_slide_along_the_strip_as_the_camera_turns_and_fade_with_distance() {
    let Some(mut s) = on_floor("slide") else {
        return;
    };
    assert!(s.settings().compass, "on to begin with");
    shop_at(&mut s, 10.0, 0.5);
    let found = shops(&mut s);
    assert_eq!(found.len(), 1);
    assert!((found[0].right - 0.5).abs() < 0.03, "{found:?}");
    let near = found[0].near;
    assert!(near > 0.5 && near < 1.0, "{near}");

    // The camera turns to the right: it comes towards the middle.
    s.yaw -= 0.5;
    let found = shops(&mut s);
    assert!(found[0].right.abs() < 0.03, "{found:?}");
    let turned = s.compass_now().facing;
    assert!((kotor_ui::wrapped(turned - s.yaw)).abs() < 0.02);

    // Further off, less of it. Past its range, nothing.
    let w = &mut s.game.world;
    let id = w
        .objects
        .values()
        .find(|o| o.kind == Kind::Store)
        .map(|o| o.id)
        .unwrap();
    let at = here(&s);
    let o = s.game.world.object_mut(id).unwrap();
    o.position = [at[0] + 30.0, at[1], at[2]];
    let far = shops(&mut s);
    assert!(far[0].near < near, "{far:?}");
    let o = s.game.world.object_mut(id).unwrap();
    o.position = [at[0] + 300.0, at[1], at[2]];
    assert!(shops(&mut s).is_empty());
}

#[test]
fn each_kind_of_thing_has_its_own_icon() {
    let Some(mut s) = on_floor("kinds") else {
        return;
    };
    let at = here(&s);
    let w = &mut s.game.world;
    let spot = [at[0] + 2.0, at[1], at[2]];
    let person = w.spawn_creature("n_calonord", spot);
    let o = w.object_mut(person).unwrap();
    o.name = "Someone".to_string();
    o.conversation = "anything".to_string();
    o.faction = 5;
    o.useable = true;
    let view = s.compass_now();
    let icons: Vec<CompassIcon> = view.marks.iter().map(|m| m.icon).collect();
    let liked = s.game.world.reputation(s.game.world.pc, person) > 10;
    if liked {
        assert!(icons.contains(&CompassIcon::Talk), "{icons:?}");
    }
    // A way out of the area, and an elevator if it says so.
    let w = &mut s.game.world;
    let door = w
        .objects
        .values()
        .find(|o| o.kind == Kind::Door)
        .map(|o| o.id)
        .expect("a door");
    let o = w.object_mut(door).unwrap();
    o.link_module = "somewhere".to_string();
    o.name = "A door".to_string();
    o.position = [at[0] + 5.0, at[1], at[2]];
    let icons: Vec<CompassIcon> = s.compass_now().marks.iter().map(|m| m.icon).collect();
    assert!(icons.contains(&CompassIcon::Area), "{icons:?}");
    s.game.world.object_mut(door).unwrap().name = "Taris Elevator".to_string();
    let icons: Vec<CompassIcon> = s.compass_now().marks.iter().map(|m| m.icon).collect();
    assert!(icons.contains(&CompassIcon::Elevator), "{icons:?}");
    // No limit of the door to the room is shown as a mark.
    s.game.world.object_mut(door).unwrap().link_module.clear();
    let icons: Vec<CompassIcon> = s.compass_now().marks.iter().map(|m| m.icon).collect();
    assert!(!icons.contains(&CompassIcon::Elevator), "{icons:?}");
    // An enemy, once seen.
    let w = &mut s.game.world;
    let foe = w.spawn_creature("n_calonord", spot);
    w.object_mut(foe).unwrap().faction = 1;
    let pc = w.pc;
    w.combat.perceived.insert((pc, foe));
    let icons: Vec<CompassIcon> = s.compass_now().marks.iter().map(|m| m.icon).collect();
    assert!(icons.contains(&CompassIcon::Enemy), "{icons:?}");
}

#[test]
fn it_stays_off_the_map_the_menu_keys_and_the_blue_button_at_every_size() {
    let Some(mut s) = on_floor("places") else {
        return;
    };
    let over = |a: [i32; 4], b: [i32; 4]| {
        a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
    };
    for size in SIZES {
        s.set_view(size);
        let width = s.ui_layout_width() as i32;
        for on in [false, true] {
            if s.touch_on() != on {
                let blue = s.hud_pixel(TOUCH_BUTTON, size).unwrap();
                s.finger_down(1, blue, size);
                s.finger_up(1, blue, size);
            }
            s.update(DT);
            let c = s.compass_place().expect("the compass");
            assert!(c[0] >= 0 && c[0] + c[2] <= width, "{size:?} {c:?}");
            let mid = c[0] * 2 + c[2];
            assert!((mid - width).abs() <= 3, "in the middle: {size:?} {c:?}");
            assert!(
                c[2] as f32 * size.0 as f32 / width as f32 >= 100.0,
                "{size:?} {c:?}"
            );
            let (_, hud) = s.touch_boxes().unwrap();
            for (tag, h) in &hud {
                if tag.starts_with("BTN_ACTION") || tag.starts_with("LBL_ACTION") {
                    continue;
                }
                if tag.starts_with("LBL_BACK") || tag.starts_with("LBL_CHAR") {
                    continue;
                }
                assert!(!over(c, *h), "{size:?} touch {on}: {c:?} is on {tag} {h:?}");
            }
        }
    }
}

#[test]
fn the_switch_and_the_size_are_on_the_feedback_page() {
    let Some(mut s) = on_floor("page") else {
        return;
    };
    let press = |s: &mut Stage, tag: &str| {
        let p = s.menu_pixel(tag, None, SIZE).unwrap();
        s.hover(p, SIZE);
        s.click(p, SIZE);
    };
    s.escape();
    press(&mut s, "BTN_FEEDBACK");
    let rows = s.menu_rows("LB_OPTIONS");
    let row_of = |rows: &[kotor_ui::Row], word: &str| {
        rows.iter()
            .position(|r| r.text.starts_with(word))
            .unwrap_or_else(|| panic!("no {word}"))
    };
    let (switch, size) = (row_of(&rows, "Compass: On"), row_of(&rows, "Compass size"));
    let click = |s: &mut Stage, row: usize| {
        // The row at the top of the list, or as near as the list goes.
        for _ in 0..rows.len() {
            s.menu_wheel(1.0);
        }
        for _ in 0..row {
            s.menu_wheel(-1.0);
        }
        let p = s.menu_pixel("LB_OPTIONS", Some(row), SIZE).unwrap();
        s.hover(p, SIZE);
        s.click(p, SIZE);
    };
    let was = s.settings().compass_size;
    click(&mut s, size);
    assert_ne!(s.settings().compass_size, was);
    click(&mut s, switch);
    assert!(!s.settings().compass);
    s.escape();
    s.escape();
    assert_eq!(s.menu_name(), "-");
    s.update(DT);
    assert!(!s.compass_now().on, "off, nothing to show");
    assert!(s.compass_place().is_none());
}

#[test]
fn a_quest_is_tracked_from_its_row_one_at_a_time_and_a_save_keeps_it() {
    let Some(mut s) = on_floor("track") else {
        return;
    };
    let quests = {
        let w = &mut s.game.world;
        // Two quests that have begun.
        let all = w.quest_tags();
        assert!(all.len() >= 2, "{all:?}");
        w.journal.clear();
        w.journal.insert(all[0].clone(), 1);
        w.journal.insert(all[1].clone(), 1);
        w.journal_order = vec![all[0].clone(), all[1].clone()];
        w.quests()
    };
    assert!(quests.len() >= 2, "{quests:?}");
    s.open_journal();
    assert_eq!(s.menu_name(), "journal");
    let rows = s.menu_rows("LB_ITEMS");
    assert!(rows
        .iter()
        .all(|r| r.end_button.as_ref().is_some_and(|b| !b.lit)));

    // A click on the button of one row, and that row only is tracked, and it is chosen as well.
    let tap = |s: &mut Stage, row: usize| {
        let p = s.menu_row_end_pixel("LB_ITEMS", row, SIZE).unwrap();
        s.hover(p, SIZE);
        s.click(p, SIZE);
    };
    tap(&mut s, 0);
    let first = s.tracked_quest().map(str::to_string).expect("tracked");
    let rows = s.menu_rows("LB_ITEMS");
    assert!(rows[0].end_button.as_ref().unwrap().lit);
    assert!(!rows[1].end_button.as_ref().unwrap().lit);
    // The other: it takes over.
    tap(&mut s, 1);
    let second = s.tracked_quest().map(str::to_string).expect("tracked");
    assert_ne!(first, second);
    let rows = s.menu_rows("LB_ITEMS");
    assert!(!rows[0].end_button.as_ref().unwrap().lit);
    assert!(rows[1].end_button.as_ref().unwrap().lit);
    // The rest of a row still chooses it.
    let p = s.menu_pixel("LB_ITEMS", Some(0), SIZE).unwrap();
    s.hover(p, SIZE);
    s.click(p, SIZE);
    assert_eq!(s.tracked_quest(), Some(second.as_str()));
    assert!(s.menu_rows("LB_ITEMS")[0].selected);
    // Again on its own button: not tracked at all.
    tap(&mut s, 1);
    assert_eq!(s.tracked_quest(), None);
    tap(&mut s, 1);
    assert_eq!(s.tracked_quest(), Some(second.as_str()));
    s.open_journal();
    assert_eq!(s.menu_name(), "-");

    // A line under the compass, a pin at the next step, and the pin goes where the step is.
    s.update(DT);
    // The step first: the index behind it may be building still, and a step known before the view is known to it.
    let step = s.game.world.next_step(&second);
    let view = s.compass_now();
    assert!(!view.line.is_empty());
    if let Some(step) = step {
        let name = view.line.split(':').next().unwrap().to_string();
        assert!(!name.is_empty());
        assert!(view.line.contains(&step.label), "{}", view.line);
        let pin = view
            .marks
            .iter()
            .find(|m| m.icon == CompassIcon::Pin)
            .expect("the pin");
        let at = here(&s);
        let bearing = (step.position[1] - at[1]).atan2(step.position[0] - at[0]);
        let right = kotor_ui::wrapped(s.yaw - bearing);
        assert!((pin.right - right).abs() < 0.03, "{} {right}", pin.right);
        // Behind the camera, the pin stays on the strip, held at its end, and nothing else on the screen points to it. The
        // owner: "have the quest arrow only appear on the compass itself".
        s.yaw = bearing + std::f32::consts::PI;
        let view = s.compass_now();
        let pin = view.marks.iter().find(|m| m.icon == CompassIcon::Pin);
        assert!(pin.is_some_and(|p| p.right.abs() > 2.5), "{pin:?}");
    }

    // A save keeps it, and one from before knows of none.
    let bytes_dir = std::env::temp_dir().join(format!("kotor_compass_keep_{}", std::process::id()));
    std::fs::create_dir_all(&bytes_dir).unwrap();
    let file = bytes_dir.join("kept.sav");
    s.game.world.save_to(&file, "Kept").unwrap();
    s.game.world.tracked_quest = None;
    s.game.world.load_from(&file).unwrap();
    assert_eq!(s.tracked_quest(), Some(second.as_str()));
    let _ = std::fs::remove_dir_all(&bytes_dir);

    // A quest that is done is not tracked any more, and has no button.
    let done = (0..400).find(|state| {
        let w = &mut s.game.world;
        w.journal.insert(second.clone(), *state);
        w.quests().iter().any(|q| q.tag == second && q.completed)
    });
    if done.is_some() {
        let view = s.compass_now();
        assert_eq!(s.tracked_quest(), None);
        assert!(view.line.is_empty());
    }
    // Nor one the file has not got.
    s.game.world.tracked_quest = Some("no such quest".to_string());
    let view = s.compass_now();
    assert_eq!(s.tracked_quest(), None);
    assert!(view.line.is_empty());
}

#[test]
fn the_hud_drawn_in_part_for_a_turning_camera_is_the_hud_drawn_whole() {
    let Some(mut s) = on_floor("part") else {
        return;
    };
    shop_at(&mut s, 8.0, 0.3);
    let w = &mut s.game.world;
    let tag = w.quest_tags()[0].clone();
    w.journal.insert(tag.clone(), 1);
    w.journal_order = vec![tag.clone()];
    s.toggle_track(&tag);
    let _ = s.compass_now();
    s.update(0.0);
    for n in 1..4 {
        let version = s.overlay_version;
        s.yaw += n as f32 * 0.1;
        s.update(0.0);
        assert!(s.overlay_version > version, "drawn again");
    }
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
