//! The screens over the game, driven headless through the same clicks a window sends.
use kotor_gpu::stage::{Cue, Stage};

const DT: f32 = 1.0 / 30.0;
const SIZE: (u32, u32) = (1280, 720);

fn at_menu() -> Option<Stage> {
    kotor_core::install_dir()?;
    Some(Stage::menu("end_m01aa", true).unwrap())
}

fn press(s: &mut Stage, tag: &str) {
    let p = s
        .menu_pixel(tag, None, SIZE)
        .unwrap_or_else(|| panic!("no {tag} on {}", s.menu_name()));
    s.hover(p, SIZE);
    s.click(p, SIZE);
}

#[test]
fn the_main_menu_starts_a_game_and_exit_quits() {
    let Some(mut s) = at_menu() else { return };
    assert_eq!(s.menu_name(), "main");
    // Nothing runs under it: no module, no bodies, no clock.
    s.update(DT);
    s.update(DT);
    assert_eq!(s.game.world.time, 0.0);
    assert!(s.game.world.module.is_empty());
    assert!(s.dynamic_scene().meshes.is_empty());
    let o = s.overlay().expect("the menu is drawn").clone();
    assert_eq!((o.width, o.height), SIZE);
    assert!(o.rgba.chunks(4).all(|p| p[3] == 255), "it hides the scene");

    // Stock strings come from the talk table: a button with text differs under the cursor.
    let new_game = s.menu_pixel("BTN_NEWGAME", None, SIZE).unwrap();
    s.hover(new_game, SIZE);
    s.update(DT);
    assert_ne!(s.overlay().unwrap().rgba, o.rgba, "the button lights up");
    assert!(s.menu_pixel("BTN_WARP", None, SIZE).is_none());
    s.hover((1.0, 1.0), SIZE);
    s.update(DT);
    assert_eq!(s.overlay().unwrap().rgba, o.rgba, "and goes out again");

    // Hidden buttons do not take clicks.
    assert_eq!(s.menu_name(), "main");
    // Esc does not leave the main menu, and keys for the game do nothing on it.
    s.escape();
    s.interact();
    s.number_key(1);
    s.continue_line();
    assert_eq!(s.menu_name(), "main");

    // The same button at the same place at 4K.
    s.set_view((3840, 2160));
    s.update(DT);
    let at4k = s.menu_pixel("BTN_NEWGAME", None, (3840, 2160)).unwrap();
    assert!(
        (at4k.0 / 3.0 - new_game.0).abs() < 2.0,
        "{at4k:?} {new_game:?}"
    );
    assert!(
        (at4k.1 / 3.0 - new_game.1).abs() < 2.0,
        "{at4k:?} {new_game:?}"
    );
    assert_eq!(s.overlay().unwrap().width, 3840);
    s.set_view(SIZE);

    assert!(!s.wants_exit());
    press(&mut s, "BTN_NEWGAME");
    assert_eq!(s.menu_name(), "-");
    assert_eq!(s.game.world.module, "end_m01aa");
    assert!(s.static_version > 0, "the area is drawn");
    s.update(DT);
    s.update(DT);
    assert!(s.game.world.time > 0.0, "the clock runs");
    assert!(!s.dynamic_scene().meshes.is_empty());
    assert!(s.game.world.errors.is_empty(), "{:?}", s.game.world.errors);

    let Some(mut s) = at_menu() else { return };
    press(&mut s, "BTN_EXIT");
    assert!(s.wants_exit());
}

#[test]
fn test_modes_still_start_in_the_game() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let s = Stage::open("end_m01aa", true).unwrap();
    assert!(!s.menu_open());
    assert_eq!(s.game.world.module, "end_m01aa");
}

/// A temp folder for saves, empty, so that a test never touches the real ones.
fn temp_saves(s: &mut Stage, name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("kotor_rs_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    s.set_saves_dir(dir.clone());
    dir
}

/// The screen as it is now.
fn frame(s: &mut Stage) -> Vec<u8> {
    s.update(DT);
    s.overlay().unwrap().rgba.clone()
}

fn click_row(s: &mut Stage, tag: &str, row: usize) {
    let p = s.menu_pixel(tag, Some(row), SIZE).unwrap();
    s.hover(p, SIZE);
    s.click(p, SIZE);
}

fn in_game(name: &str) -> Option<(Stage, std::path::PathBuf)> {
    let mut s = at_menu()?;
    let dir = temp_saves(&mut s, name);
    press(&mut s, "BTN_NEWGAME");
    // Past the opening: conversations refuse saves.
    for _ in 0..12000 {
        s.update(DT);
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        } else if s.game.world.transcript.len() > 3 && s.game.world.delayed.is_empty() {
            break;
        }
    }
    Some((s, dir))
}

#[test]
fn esc_pauses_the_game_and_exit_game_goes_back_to_the_main_menu() {
    let Some((mut s, _)) = in_game("pause") else {
        return;
    };
    let t = s.game.world.time;
    s.escape();
    assert_eq!(s.menu_name(), "pause");
    s.update(DT);
    s.update(DT);
    assert_eq!(s.game.world.time, t, "the clock stops");
    let drawn = s.overlay().unwrap().clone();
    assert!(drawn.rgba.chunks(4).all(|p| p[3] == 255));
    // The greyed pages do nothing.
    press(&mut s, "BTN_FEEDBACK");
    assert_eq!(s.menu_name(), "pause");
    // Close and Esc both go back to the game.
    press(&mut s, "BTN_EXIT");
    assert_eq!(s.menu_name(), "-");
    s.escape();
    s.escape();
    assert_eq!(s.menu_name(), "-");
    s.update(DT);
    s.update(DT);
    assert!(s.game.world.time > t, "the clock runs on");

    s.escape();
    press(&mut s, "BTN_QUIT");
    assert_eq!(s.menu_name(), "main");
    assert!(s.game.world.module.is_empty());
    assert!(s.dynamic_scene().meshes.is_empty());
    assert!(s.static_scene().meshes.is_empty());
    let cues = s.update(DT);
    assert!(cues.contains(&kotor_gpu::stage::Cue::StopMusic), "{cues:?}");
    // And a new game starts from nothing.
    press(&mut s, "BTN_NEWGAME");
    assert_eq!(s.menu_name(), "-");
    assert_eq!(s.game.world.module, "end_m01aa");
    assert!(s.game.world.time < 1.0);
    s.update(DT);
    assert!(!s.dynamic_scene().meshes.is_empty());
}

/// Click down the two rows from `shift` until the save named Bridge is chosen.
fn choose_bridge(s: &mut Stage, shift: usize) {
    for r in shift..shift + 2 {
        click_row(s, "LB_GAMES", r);
        if s.selected_save_name().as_deref() == Some("Bridge") {
            return;
        }
    }
    panic!("no save named Bridge");
}

#[test]
fn saves_are_listed_written_loaded_and_deleted_from_the_screens() {
    let Some((mut s, dir)) = in_game("saves") else {
        return;
    };
    let pc = s.game.world.pc;
    s.escape();
    press(&mut s, "BTN_SAVEGAME");
    assert_eq!(s.menu_name(), "save");
    assert_eq!(s.saves_listed(), 0);
    // The first row is a new save. Save asks for the name, Esc takes it back.
    press(&mut s, "BTN_SAVELOAD");
    assert_eq!(s.menu_name(), "name");
    assert!(s.typing());
    s.escape();
    assert_eq!(s.menu_name(), "save");
    press(&mut s, "BTN_SAVELOAD");
    while !s.save_name_typed().is_empty() {
        s.backspace();
    }
    s.type_text("Bridge\n1");
    assert_eq!(s.save_name_typed(), "Bridge1", "no control characters");
    s.backspace();
    press(&mut s, "BTN_OK");
    assert_eq!(s.menu_name(), "save", "{}", s.menu_message());
    assert_eq!(s.saves_listed(), 1);
    let first = dir.join("save_0001.sav");
    assert!(first.is_file(), "{first:?}");
    assert_eq!(kotor_game::read_save_info(&first).unwrap().name, "Bridge");
    let digest = s.game.world.state_digest();

    // A second, new one; then the first again, overwritten under the same file.
    click_row(&mut s, "LB_GAMES", 0);
    press(&mut s, "BTN_SAVELOAD");
    s.type_text(" two");
    s.continue_line();
    assert_eq!(s.saves_listed(), 2);
    assert!(dir.join("save_0002.sav").is_file());
    choose_bridge(&mut s, 1);
    press(&mut s, "BTN_SAVELOAD");
    assert_eq!(s.save_name_typed(), "Bridge", "it offers the old name");
    s.continue_line();
    assert_eq!(s.saves_listed(), 2);
    press(&mut s, "BTN_BACK");
    assert_eq!(s.menu_name(), "pause");
    press(&mut s, "BTN_EXIT");

    // Something changes, and loading takes it back.
    let at = s.game.world.object(pc).unwrap().position;
    s.game.world.teleport(pc, [at[0] + 0.5, at[1], at[2]]);
    s.game.world.global_numbers.insert("K_TEST".into(), 77);
    s.game.world.object_mut(pc).unwrap().hp = 1;
    assert_ne!(s.game.world.state_digest(), digest);
    s.escape();
    press(&mut s, "BTN_LOADGAME");
    assert_eq!(s.menu_name(), "load");
    assert_eq!(s.saves_listed(), 2);
    choose_bridge(&mut s, 0);
    press(&mut s, "BTN_SAVELOAD");
    assert_eq!(s.menu_name(), "-", "{}", s.menu_message());
    assert_eq!(s.game.world.state_digest(), digest);
    let a = s.static_version;
    s.update(DT);
    assert_eq!(s.static_version, a);
    assert!(!s.dynamic_scene().meshes.is_empty());
    assert!(s.game.world.errors.is_empty(), "{:?}", s.game.world.errors);

    // From the main menu too, with no game running.
    s.escape();
    press(&mut s, "BTN_QUIT");
    let saved = s.game.world.state_digest();
    assert_ne!(saved, digest);
    press(&mut s, "BTN_LOADGAME");
    assert_eq!(s.menu_name(), "load");
    // Nothing is chosen yet, so Load does nothing; Back returns to the main menu.
    press(&mut s, "BTN_SAVELOAD");
    assert_eq!(s.menu_name(), "load");
    press(&mut s, "BTN_BACK");
    assert_eq!(s.menu_name(), "main");
    press(&mut s, "BTN_LOADGAME");
    choose_bridge(&mut s, 0);
    s.continue_line();
    assert_eq!(s.menu_name(), "-", "{}", s.menu_message());
    assert_eq!(s.game.world.state_digest(), digest);
    s.update(DT);
    assert!(s.game.world.time > 0.0);
    assert!(!s.dynamic_scene().meshes.is_empty());

    // Delete asks first, then takes the file away.
    s.escape();
    press(&mut s, "BTN_LOADGAME");
    click_row(&mut s, "LB_GAMES", 0);
    press(&mut s, "BTN_DELETE");
    assert_eq!(s.saves_listed(), 2);
    assert!(s.menu_message().contains("again"), "{}", s.menu_message());
    press(&mut s, "BTN_DELETE");
    assert_eq!(s.saves_listed(), 1);
    click_row(&mut s, "LB_GAMES", 0);
    press(&mut s, "BTN_DELETE");
    press(&mut s, "BTN_DELETE");
    assert_eq!(s.saves_listed(), 0);
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_conversation_cannot_be_saved_and_a_save_goes_only_to_its_folder() {
    let Some(mut s) = at_menu() else { return };
    let dir = temp_saves(&mut s, "refuse");
    press(&mut s, "BTN_NEWGAME");
    for _ in 0..600 {
        s.update(DT);
        if s.game.world.in_dialogue() {
            break;
        }
    }
    assert!(s.game.world.in_dialogue());
    s.escape();
    press(&mut s, "BTN_SAVEGAME");
    assert_eq!(s.menu_name(), "pause");
    assert!(s.menu_message().contains("conversation"));
    assert!(!dir.exists());
    // The default folder is the one the owner asked for.
    let want = std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap())
        .join("kotor-rs")
        .join("saves");
    assert_eq!(kotor_gpu::stage::default_saves_dir(), Some(want));
}

#[test]
fn the_inventory_lists_what_is_carried_and_a_click_shows_its_words() {
    let Some((mut s, _)) = in_game("inventory") else {
        return;
    };
    let pc = s.game.world.pc;
    let armour = s.game.world.spawn_item("g_a_class4001");
    let medpack = s.game.world.spawn_item("g_i_medeqpmnt01");
    s.game.world.carry(pc, armour);
    s.game.world.carry(pc, medpack);
    s.game.world.gold = 123;
    let t = s.game.world.time;
    s.open_inventory();
    assert_eq!(s.menu_name(), "inventory");
    s.update(DT);
    assert_eq!(s.game.world.time, t, "the game waits");
    let before = s.overlay().unwrap().clone();

    // Under the cursor, an item shows its words.
    let row = s.menu_pixel("LB_ITEMS", Some(1), SIZE).unwrap();
    s.hover(row, SIZE);
    s.update(DT);
    assert_ne!(s.overlay().unwrap().rgba, before.rgba);
    // A click leaves everything in the bag: wearing is the equipment screen's.
    s.click(row, SIZE);
    click_row(&mut s, "LB_ITEMS", 0);
    assert_eq!(s.game.world.carried(pc), vec![armour, medpack]);
    assert!(s.game.world.object(pc).unwrap().equipped.is_empty());
    s.update(DT);
    // I, Esc and the button each close it.
    s.open_inventory();
    assert_eq!(s.menu_name(), "-");
    s.open_inventory();
    s.escape();
    assert_eq!(s.menu_name(), "-");
    s.open_inventory();
    press(&mut s, "BTN_EXIT");
    assert_eq!(s.menu_name(), "-");
    // U swaps to the equipment from the inventory, but neither opens over the pause menu.
    s.open_inventory();
    s.open_equip();
    assert_eq!(s.menu_name(), "equip");
    s.escape();
    assert_eq!(s.menu_name(), "-");
    s.escape();
    s.open_inventory();
    assert_eq!(s.menu_name(), "pause");
    s.escape();
}

#[test]
fn a_slot_lists_none_then_what_fits_and_ok_or_cancel_decide() {
    let Some((mut s, _)) = in_game("equip") else {
        return;
    };
    let pc = s.game.world.pc;
    let worn = |s: &Stage, slot: i32| {
        s.game
            .world
            .object(s.game.world.pc)
            .unwrap()
            .equipped
            .get(&slot)
            .copied()
    };
    let mut items = Vec::new();
    for t in ["g_a_class4001", "g_w_blstrpstl001", "g_i_medeqpmnt01"] {
        let i = s.game.world.spawn_item(t);
        s.game.world.carry(pc, i);
        items.push(i);
    }
    let (armour, pistol) = (items[0], items[1]);
    s.open_equip();
    assert!(
        s.menu_pixel("BTN_EQUIP", None, SIZE).is_none(),
        "no OK until a slot is clicked"
    );
    let empty = frame(&mut s);

    // A slot under the cursor lists what fits it, and the list stays when the cursor leaves it. Its rows do nothing.
    let body = s.menu_pixel("BTN_INV_BODY", None, SIZE).unwrap();
    s.hover(body, SIZE);
    s.hover((1.0, 1.0), SIZE);
    let listed = frame(&mut s);
    assert!(listed != empty);
    click_row(&mut s, "LB_ITEMS", 1);
    assert_eq!(worn(&s, 1), None);
    s.hover((1.0, 1.0), SIZE);
    assert!(frame(&mut s) == listed, "the list is still the body's");

    // A click on the slot: the slots go, OK and Cancel come. Choosing a row wears nothing yet.
    press(&mut s, "BTN_INV_BODY");
    assert!(s.menu_pixel("BTN_INV_BODY", None, SIZE).is_none());
    assert!(s.menu_pixel("BTN_EQUIP", None, SIZE).is_some());
    assert!(frame(&mut s) != listed);
    // Hidden buttons do not take clicks: where the slot was, nothing happens.
    s.click(body, SIZE);
    assert!(s.menu_pixel("BTN_EQUIP", None, SIZE).is_some());
    click_row(&mut s, "LB_ITEMS", 1);
    assert_eq!(worn(&s, 1), None);
    // Cancel leaves it off, and the slots come back.
    press(&mut s, "BTN_BACK");
    assert_eq!(s.menu_name(), "equip");
    assert_eq!(worn(&s, 1), None);
    assert!(s.menu_pixel("BTN_INV_BODY", None, SIZE).is_some());
    // So does Esc, once, and it stays on the screen.
    press(&mut s, "BTN_INV_BODY");
    s.escape();
    assert_eq!(s.menu_name(), "equip");
    assert!(s.menu_pixel("BTN_INV_BODY", None, SIZE).is_some());

    // OK wears it, and the slot's picture and the defense change.
    let bare = s.game.world.defense_of(pc);
    press(&mut s, "BTN_INV_BODY");
    click_row(&mut s, "LB_ITEMS", 1);
    press(&mut s, "BTN_EQUIP");
    assert_eq!(worn(&s, 1), Some(armour));
    assert!(s.game.world.carried(pc).iter().all(|i| *i != armour));
    assert!(s.game.world.defense_of(pc) > bare);
    assert!(s.menu_pixel("BTN_EQUIP", None, SIZE).is_none());
    assert!(frame(&mut s) != empty);
    // Its row now comes first after None, and OK on it changes nothing.
    press(&mut s, "BTN_INV_BODY");
    click_row(&mut s, "LB_ITEMS", 1);
    press(&mut s, "BTN_EQUIP");
    assert_eq!(worn(&s, 1), Some(armour));
    // None takes it off.
    press(&mut s, "BTN_INV_BODY");
    click_row(&mut s, "LB_ITEMS", 0);
    press(&mut s, "BTN_EQUIP");
    assert_eq!(worn(&s, 1), None);
    assert!(s.game.world.carried(pc).contains(&armour));

    // A head's list has None only: a click past it chooses nothing, and OK leaves things as they are.
    press(&mut s, "BTN_INV_HEAD");
    click_row(&mut s, "LB_ITEMS", 1);
    press(&mut s, "BTN_EQUIP");
    assert_eq!(worn(&s, 0), None);
    assert!(s.game.world.carried(pc).contains(&armour));

    // Without the feat an item stays off, the list stays up, and the screen says so.
    press(&mut s, "BTN_INV_WEAP_R");
    click_row(&mut s, "LB_ITEMS", 1);
    press(&mut s, "BTN_EQUIP");
    assert_eq!(worn(&s, 4), Some(pistol));
    press(&mut s, "BTN_INV_BODY");
    let feats = std::mem::take(&mut s.game.world.object_mut(pc).unwrap().feats);
    click_row(&mut s, "LB_ITEMS", 1);
    let before = frame(&mut s);
    press(&mut s, "BTN_EQUIP");
    assert_eq!(worn(&s, 1), None);
    assert!(s.menu_pixel("BTN_EQUIP", None, SIZE).is_some());
    assert!(frame(&mut s) != before);
    s.game.world.object_mut(pc).unwrap().feats = feats;
    s.escape();

    press(&mut s, "BTN_BACK");
    assert_eq!(s.menu_name(), "-");
    // What is worn shows on the body.
    s.update(DT);
    let scene = s.dynamic_scene();
    assert!(
        scene.meshes.iter().any(|m| m
            .texture
            .as_ref()
            .is_some_and(|t| t.to_ascii_lowercase().starts_with("w_"))),
        "the pistol is drawn in the hand"
    );
}

#[test]
fn a_body_with_loot_is_a_target_and_opens_as_a_list() {
    let Some((mut s, _)) = in_game("loot") else {
        return;
    };
    let pc = s.game.world.pc;
    let at = s.game.world.object(pc).unwrap().position;
    let foe = s
        .game
        .world
        .spawn_creature("n_calonord", [at[0] + 0.1, at[1], at[2]]);
    let hp = s.game.world.object(foe).unwrap().hp;
    s.game.world.hurt(pc, foe, hp + 1);
    s.update(DT);
    assert!(
        s.state_line().contains("target=Remains"),
        "{}",
        s.state_line()
    );
    // E uses it, and it opens as a list on the screen.
    s.interact();
    s.update(DT);
    assert_eq!(s.game.world.opened, Some(foe), "{}", s.state_line());
    assert!(!s.game.world.container_items().is_empty());
    let before = s.game.world.carried(pc).len();
    s.number_key(1);
    assert!(
        s.game.world.carried(pc).len() > before,
        "one item was taken"
    );
    // Esc closes the list before it pauses anything.
    s.escape();
    assert_eq!(s.game.world.opened, None);
    assert_eq!(s.menu_name(), "-");
    assert!(s.game.world.errors.is_empty(), "{:?}", s.game.world.errors);
}

#[test]
fn a_container_opens_its_own_window_over_the_scene() {
    let Some((mut s, _)) = in_game("container") else {
        return;
    };
    let pc = s.game.world.pc;
    let locker = s.game.world.by_tag("end_locker01")[0];
    assert!(s.stand_near("end_locker01"));
    s.update(DT);
    assert_eq!(s.menu_name(), "-");
    s.game.world.click(locker);
    assert_eq!(s.menu_name(), "container");
    s.update(DT);
    let o = s.overlay().unwrap();
    assert_eq!(o.rgba[3], 0, "the scene shows around the panel");
    // The game runs on behind it, unlike the other screens.
    let t = s.game.world.time;
    s.update(DT);
    s.update(DT);
    assert!(s.game.world.time > t);
    // The keys for the game do not reach it: I, U and E do nothing, and Esc closes the window before it pauses.
    s.open_inventory();
    s.open_equip();
    s.interact();
    assert_eq!(s.menu_name(), "container");

    // A click on a row takes that item.
    let items = s.game.world.container_items();
    assert!(items.len() >= 2);
    click_row(&mut s, "LB_ITEMS", 1);
    let left = s.game.world.container_items();
    assert_eq!(left.len(), items.len() - 1);
    assert!(!left.iter().any(|i| i.0 == items[1].0));
    // Close keeps the rest in it, and it opens again with them.
    press(&mut s, "BTN_CANCEL");
    assert_eq!(s.menu_name(), "-");
    assert_eq!(s.game.world.opened, None);
    s.game.world.click(locker);
    assert_eq!(s.menu_name(), "container");
    assert_eq!(s.game.world.container_items(), left);
    // Take All empties it and closes.
    let gold = s.game.world.gold;
    press(&mut s, "BTN_OK");
    assert_eq!(s.menu_name(), "-");
    assert!(s.game.world.object(locker).unwrap().inventory.is_empty());
    assert!(!s.game.world.carried(pc).is_empty() || s.game.world.gold > gold);
    // Space closes an open one, and Esc does, and then Esc pauses.
    s.game.world.click(locker);
    s.continue_line();
    assert_eq!(s.menu_name(), "-");
    s.game.world.click(locker);
    s.escape();
    assert_eq!(s.menu_name(), "-");
    s.escape();
    assert_eq!(s.menu_name(), "pause");
}

/// The middle of a slider is at this share of its width, on the window.
fn slide(s: &mut Stage, tag: &str, along: f32) {
    let (x, y) = s.menu_pixel(tag, None, SIZE).unwrap();
    // A slider is 240 units wide, at 2 pixels to a unit across.
    let px = x + (along - 0.5) * 240.0 * 2.0;
    s.hover((px, y), SIZE);
    s.click((px, y), SIZE);
}

#[test]
fn the_option_pages_change_what_they_say_and_keep_it() {
    let Some(mut s) = at_menu() else { return };
    let file = std::env::temp_dir()
        .join("kotor_rs_settings_test")
        .join("settings.txt");
    let _ = std::fs::remove_file(&file);
    s.use_settings_file(file.clone());
    press(&mut s, "BTN_OPTIONS");
    assert_eq!(s.menu_name(), "options");

    // Sound: the levels reach the window as a cue.
    press(&mut s, "BTN_SOUND");
    assert_eq!(s.menu_name(), "sound");
    s.update(DT);
    let before = s.overlay().unwrap().clone();
    slide(&mut s, "SLI_MUSIC", 0.25);
    let cues = s.update(DT);
    let level = s.settings().music;
    assert!((level - 0.25).abs() < 0.02, "{level}");
    assert!(
        cues.iter()
            .any(|c| matches!(c, Cue::Levels(m, 1.0, 1.0) if *m == level)),
        "{cues:?}"
    );
    assert_ne!(s.overlay().unwrap().rgba, before.rgba, "the handle moved");
    slide(&mut s, "SLI_VO", 0.5);
    slide(&mut s, "SLI_FX", 0.01);
    assert!(s.settings().effects < 0.02);
    press(&mut s, "BTN_BACK");
    assert_eq!(s.menu_name(), "options");

    // Graphics: brightness shows on the picture, and the size is asked of the window.
    press(&mut s, "BTN_GRAPHICS");
    s.update(DT);
    let normal = s.overlay().unwrap().clone();
    slide(&mut s, "SLI_GAMMA", 0.99);
    s.update(DT);
    let bright = s.overlay().unwrap().clone();
    let sum = |o: &kotor_image::Image| o.rgba.iter().map(|b| *b as u64).sum::<u64>();
    assert!(sum(&bright) > sum(&normal));
    slide(&mut s, "SLI_GAMMA", 0.01);
    s.update(DT);
    assert!(sum(s.overlay().unwrap()) < sum(&normal));
    press(&mut s, "BTN_DEFAULT");
    s.update(DT);
    assert_eq!(s.settings().brightness, 0.5);
    assert_eq!(
        s.overlay().unwrap().rgba,
        normal.rgba,
        "a half changes nothing"
    );
    assert_eq!(s.take_wanted_size(), None);
    press(&mut s, "BTN_RESOLUTION");
    assert_eq!(s.take_wanted_size(), Some((1280, 720)));
    press(&mut s, "BTN_RESOLUTION");
    assert_eq!(s.take_wanted_size(), Some((1920, 1080)));
    assert_eq!(s.take_wanted_size(), None, "asked once");
    // Greyed boxes do nothing.
    let same = s.settings();
    press(&mut s, "CB_SHADOWS");
    assert_eq!(s.settings(), same);
    press(&mut s, "BTN_BACK");

    // Gameplay: difficulty and the boxes.
    press(&mut s, "BTN_GAMEPLAY");
    assert_eq!(s.settings().difficulty, 1);
    press(&mut s, "BTN_DIFFRIGHT");
    press(&mut s, "BTN_DIFFRIGHT");
    assert_eq!(s.settings().difficulty, 2, "it stops at the last");
    assert_eq!(s.game.world.difficulty, 2);
    assert!((s.game.world.difficulty_multiplier() - 1.5).abs() < 0.01);
    s.update(DT);
    let ticked = s.overlay().unwrap().clone();
    press(&mut s, "CB_AUTOSAVE");
    assert!(s.settings().autosave);
    press(&mut s, "CB_INVERTCAM");
    s.update(DT);
    assert_ne!(s.overlay().unwrap().rgba, ticked.rgba, "the box is ticked");
    press(&mut s, "BTN_BACK");
    press(&mut s, "BTN_BACK");
    assert_eq!(s.menu_name(), "main");

    // A new start reads them back, and New Game plays by them.
    let kept = s.settings();
    let mut again = at_menu().unwrap();
    assert_eq!(again.settings(), kotor_gpu::stage::Settings::default());
    again.use_settings_file(file.clone());
    assert_eq!(again.settings(), kept);
    assert_eq!(again.take_wanted_size(), Some((1920, 1080)));
    press(&mut again, "BTN_NEWGAME");
    assert_eq!(again.game.world.difficulty, 2);
    // The camera turns the other way on the pitch.
    let before = again.pitch;
    again.look(0.0, 10.0);
    assert!(again.pitch < before);
    let _ = std::fs::remove_file(&file);

    // Also from the pause menu.
    let Some((mut p, _)) = in_game("settings") else {
        return;
    };
    p.escape();
    press(&mut p, "BTN_SOUND");
    assert_eq!(p.menu_name(), "sound");
    p.escape();
    assert_eq!(p.menu_name(), "pause");
}

#[test]
fn autosave_writes_on_a_change_of_module_when_ticked() {
    let Some((mut s, dir)) = in_game("autosave") else {
        return;
    };
    let file = dir.join("autosave.sav");
    s.game.world.transition_request = Some(("end_m01ab".to_string(), String::new()));
    for _ in 0..5 {
        s.update(DT);
    }
    assert_eq!(s.game.world.module, "end_m01ab");
    assert!(!file.exists(), "not ticked");

    let settings = dir.with_extension("settings.txt");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&settings, "autosave=true\n").unwrap();
    s.use_settings_file(settings.clone());
    s.game.world.transition_request = Some(("end_m01aa".to_string(), String::new()));
    for _ in 0..5 {
        s.update(DT);
    }
    assert_eq!(s.game.world.module, "end_m01aa");
    let info = kotor_game::read_save_info(&file).unwrap();
    assert_eq!(info.name, "Autosave");
    assert_eq!(info.module, "end_m01aa");
    let _ = std::fs::remove_file(&settings);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_character_screen_shows_the_sheet_and_use_item_heals() {
    let Some((mut s, _)) = in_game("character") else {
        return;
    };
    let pc = s.game.world.pc;
    s.open_character();
    assert_eq!(s.menu_name(), "character");
    let t = s.game.world.time;
    let before = frame(&mut s);
    assert_eq!(s.game.world.time, t, "the game waits");
    s.game.world.object_mut(pc).unwrap().hp = 3;
    assert_ne!(frame(&mut s), before, "the numbers are live");
    // C closes it, and it swaps with the other two.
    s.open_character();
    assert_eq!(s.menu_name(), "-");
    s.open_inventory();
    s.open_character();
    assert_eq!(s.menu_name(), "character");
    press(&mut s, "BTN_EXIT");
    assert_eq!(s.menu_name(), "-");

    // Use Item needs an item with a power on show.
    let armour = s.game.world.spawn_item("g_a_class4001");
    let pack = s.game.world.spawn_item("g_i_medeqpmnt01");
    s.game.world.carry(pc, armour);
    s.game.world.carry(pc, pack);
    s.open_inventory();
    press(&mut s, "BTN_USEITEM");
    assert_eq!(s.game.world.object(pc).unwrap().hp, 3, "nothing on show");
    let row = s.menu_pixel("LB_ITEMS", Some(1), SIZE).unwrap();
    s.hover(row, SIZE);
    press(&mut s, "BTN_USEITEM");
    assert!(s.game.world.object(pc).unwrap().hp > 3, "it healed");
    assert_eq!(
        s.game.world.carried(pc),
        vec![armour],
        "the last one is gone"
    );
    assert!(s.game.world.errors.is_empty(), "{:?}", s.game.world.errors);
}

#[test]
fn a_level_waits_for_the_screen_and_its_pages_spend_and_cancel() {
    let Some((mut s, _)) = in_game("levelup") else {
        return;
    };
    let pc = s.game.world.pc;
    let start = s.game.world.object(pc).unwrap().clone();
    assert!(s.menu_name() == "-");

    // Experience comes in and the level waits, with a mark on the portrait.
    let hud = frame_of_hud(&mut s);
    s.open_character();
    assert!(
        s.menu_pixel("BTN_LEVELUP", None, SIZE).is_none(),
        "no level is waiting"
    );
    s.escape();
    s.game.world.grant_xp(pc, 1000);
    assert_eq!(s.game.world.object(pc).unwrap().level, start.level);
    assert!(frame_of_hud(&mut s) != hud);

    // Back on the panel puts everything back.
    s.open_character();
    press(&mut s, "BTN_LEVELUP");
    assert_eq!(s.menu_name(), "levelup");
    let o = s.game.world.object(pc).unwrap();
    assert_eq!(o.level, start.level + 1);
    assert!(o.max_hp > start.max_hp);
    frame(&mut s);
    press(&mut s, "BTN_BACK");
    assert_eq!(s.menu_name(), "character");
    let o = s.game.world.object(pc).unwrap();
    assert_eq!((o.level, o.max_hp), (start.level, start.max_hp));
    assert_eq!(o.skill_points, start.skill_points);

    // A step past the next one does nothing. Level 2 has no attribute point, so skills come first.
    press(&mut s, "BTN_LEVELUP");
    assert!(s.menu_pixel("BTN_STEPNAME1", None, SIZE).is_none());
    press(&mut s, "BTN_STEPNAME5");
    assert_eq!(s.menu_name(), "levelup", "Accept waits for the steps");
    press(&mut s, "BTN_STEPNAME2");
    assert_eq!(s.menu_name(), "level_skills");
    frame(&mut s);

    // An arrow spends a point and the other takes it back. Cancel leaves the page as it was found.
    let ranks = s.game.world.skill_ranks(pc, 0);
    let points = s.game.world.object(pc).unwrap().skill_points;
    assert!(points > 0);
    assert!(
        s.menu_pixel("COM_MINUS_BTN", None, SIZE).is_none(),
        "nothing to take back"
    );
    press(&mut s, "COM_PLUS_BTN");
    assert_eq!(s.game.world.skill_ranks(pc, 0), ranks + 1);
    let cost = points - s.game.world.object(pc).unwrap().skill_points;
    assert!(cost == 1 || cost == 2, "{cost}");
    press(&mut s, "COM_MINUS_BTN");
    assert_eq!(s.game.world.skill_ranks(pc, 0), ranks);
    assert_eq!(s.game.world.object(pc).unwrap().skill_points, points);
    press(&mut s, "COM_PLUS_BTN");
    press(&mut s, "BTN_BACK");
    assert_eq!(s.menu_name(), "levelup");
    assert_eq!(s.game.world.skill_ranks(pc, 0), ranks);
    // OK keeps them. The words follow the cursor.
    press(&mut s, "BTN_STEPNAME2");
    let plain = frame(&mut s);
    let row = s.menu_pixel("COMPUTER_USE_LBL", None, SIZE).unwrap();
    s.hover(row, SIZE);
    assert!(frame(&mut s) != plain, "the words about the skill");
    press(&mut s, "COM_PLUS_BTN");
    press(&mut s, "BTN_ACCEPT");
    assert_eq!(s.game.world.skill_ranks(pc, 0), ranks + 1);

    // Feats: chains of pictures. Add takes a feat and a slot, Remove gives them back.
    press(&mut s, "BTN_STEPNAME3");
    assert_eq!(s.menu_name(), "level_feats");
    let slots = s.game.world.object(pc).unwrap().feat_slots;
    let chains = s.game.world.feat_choices(pc);
    assert!(chains.iter().all(|c| c.len() <= 3 && !c.is_empty()));
    let (r, c, feat) = chains
        .iter()
        .enumerate()
        .flat_map(|(r, chain)| chain.iter().enumerate().map(move |(c, f)| (r, c, f)))
        .find(|(r, _, f)| f.can_take && !f.owned && *r < 4)
        .map(|(r, c, f)| (r, c, f.id))
        .expect("a feat in the first rows");
    let p = s.menu_cell_pixel("LB_FEATS", r, c, SIZE).unwrap();
    s.hover(p, SIZE);
    s.click(p, SIZE);
    frame(&mut s);
    press(&mut s, "BTN_SELECT");
    let o = s.game.world.object(pc).unwrap();
    assert!(o.feats.contains(&feat));
    assert_eq!(o.feat_slots.0 + o.feat_slots.1, slots.0 + slots.1 - 1);
    press(&mut s, "BTN_SELECT");
    let o = s.game.world.object(pc).unwrap();
    assert!(!o.feats.contains(&feat));
    assert_eq!(o.feat_slots, slots);
    // Recommended spends every slot.
    press(&mut s, "BTN_RECOMMENDED");
    let o = s.game.world.object(pc).unwrap();
    assert_eq!(o.feat_slots, (0, 0), "{:?}", o.feats);
    press(&mut s, "BTN_ACCEPT");

    // Accept keeps it all, and no level is waiting.
    press(&mut s, "BTN_STEPNAME5");
    assert_eq!(s.menu_name(), "character");
    assert_eq!(s.game.world.object(pc).unwrap().level, start.level + 1);
    assert!(s.menu_pixel("BTN_LEVELUP", None, SIZE).is_none());
    s.escape();

    // Level 4 has an attribute point: OK asks for it, and Constitution pays back every level.
    s.game.world.grant_xp(pc, 6000);
    s.game.world.level_up(pc);
    let level = s.game.world.object(pc).unwrap().level;
    assert_eq!(level, 3);
    s.open_character();
    press(&mut s, "BTN_LEVELUP");
    assert_eq!(s.game.world.object(pc).unwrap().level, 4);
    // Odd to even, so the modifier moves.
    s.game.world.object_mut(pc).unwrap().abilities[2] = 13;
    press(&mut s, "BTN_STEPNAME1");
    assert_eq!(s.menu_name(), "level_attributes");
    frame(&mut s);
    press(&mut s, "BTN_ACCEPT");
    assert_eq!(s.menu_name(), "level_attributes");
    assert!(s.menu_message().contains("point"), "{}", s.menu_message());
    let hp = s.game.world.object(pc).unwrap().max_hp;
    press(&mut s, "CON_PLUS_BTN");
    assert!(
        s.menu_pixel("CON_PLUS_BTN", None, SIZE).is_none(),
        "the point is spent"
    );
    press(&mut s, "BTN_ACCEPT");
    assert_eq!(s.game.world.object(pc).unwrap().abilities[2], 14);
    assert_eq!(s.game.world.object(pc).unwrap().max_hp, hp + 4);
    assert_eq!(s.menu_name(), "levelup");
    s.escape();
    assert_eq!(s.menu_name(), "character");
    assert_eq!(s.game.world.object(pc).unwrap().level, 3);

    // A Jedi has powers to choose too. Auto Level Up spends everything at once.
    let o = s.game.world.object_mut(pc).unwrap();
    o.class = 3;
    o.feat_slots = (0, 0);
    o.skill_points = 0;
    press(&mut s, "BTN_LEVELUP");
    press(&mut s, "BTN_STEPNAME1");
    press(&mut s, "BTN_RECOMMENDED");
    press(&mut s, "BTN_ACCEPT");
    press(&mut s, "BTN_STEPNAME2");
    press(&mut s, "BTN_ACCEPT");
    let next = if s.menu_pixel("BTN_STEPNAME3", None, SIZE).is_some() {
        press(&mut s, "BTN_STEPNAME3");
        press(&mut s, "BTN_ACCEPT");
        true
    } else {
        false
    };
    if s.game.world.power_slots_at(3, 4) > 0 {
        press(&mut s, "BTN_STEPNAME4");
        assert_eq!(s.menu_name(), "level_powers", "{next}");
        frame(&mut s);
        press(&mut s, "RECOMMENDED_BTN");
        press(&mut s, "ACCEPT_BTN");
        assert!(!s.game.world.object(pc).unwrap().spells.is_empty());
    }
    press(&mut s, "BTN_STEPNAME5");
    assert_eq!(s.game.world.object(pc).unwrap().level, 4);
    s.game.world.grant_xp(pc, 4000);
    press(&mut s, "BTN_AUTO");
    let o = s.game.world.object(pc).unwrap();
    assert_eq!(o.level, 5);
    assert_eq!(o.feat_slots.0 + o.feat_slots.1, 0);
    assert_eq!(s.menu_name(), "character");
}

#[test]
fn the_journal_lists_quests_swaps_to_completed_and_sorts() {
    let Some((mut s, _)) = in_game("journal") else {
        return;
    };
    // The file has quests with words, and two tags that need their last state to end.
    let tags = s.game.world.quest_tags();
    assert!(tags.len() > 20, "{}", tags.len());
    let w = &mut s.game.world;
    w.journal.clear();
    w.journal_order.clear();
    let (a, b) = (tags[0].clone(), tags[1].clone());
    w.journal.insert(a.clone(), 1);
    w.journal.insert(b.clone(), 1);
    w.journal_order = vec![a.clone(), b.clone()];
    let quests = w.quests();
    assert_eq!(quests.len(), 2, "{quests:?}");
    assert_eq!(quests[0].tag, a, "the newest first");
    assert!(!quests[0].name.is_empty());

    // J opens it, from the game and from the tabs, and closes it. It runs from the HUD's button too.
    s.open_journal();
    assert_eq!(s.menu_name(), "journal");
    s.open_journal();
    assert_eq!(s.menu_name(), "-");
    s.update(DT);
    let p = s.hud_pixel("BTN_JOU", SIZE).unwrap();
    s.click(p, SIZE);
    assert_eq!(s.menu_name(), "journal");

    // A click on a row chooses it, and the words follow.
    s.hover((1.0, 1.0), SIZE);
    let first = frame(&mut s);
    click_row(&mut s, "LB_ITEMS", 1);
    s.hover((1.0, 1.0), SIZE);
    let second = frame(&mut s);
    assert!(first != second);
    // Sort changes the title and the button.
    press(&mut s, "BTN_SORT");
    s.hover((1.0, 1.0), SIZE);
    let sorted = frame(&mut s);
    assert!(sorted != second);

    // Active and completed quests are apart. Nothing is completed yet, so the list is empty.
    press(&mut s, "BTN_SWAPTEXT");
    s.hover((1.0, 1.0), SIZE);
    let done = frame(&mut s);
    assert!(done != sorted);
    let p = s.menu_pixel("LB_ITEMS", Some(0), SIZE).unwrap();
    s.click(p, SIZE);
    s.hover((1.0, 1.0), SIZE);
    assert!(frame(&mut s) == done, "no row there to choose");
    press(&mut s, "BTN_EXIT");
    assert_eq!(s.menu_name(), "-");
    s.game.world.events.clear();

    // A save keeps the order.
    let dir = std::env::temp_dir().join(format!("kotor_rs_journal_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("j.sav");
    s.game.world.save_to(&path, "journal").unwrap();
    s.game.world.journal_order.clear();
    s.game.world.load_from(&path).unwrap();
    assert_eq!(s.game.world.journal_order, vec![a, b]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_party_screen_adds_removes_and_answers_a_script() {
    let Some((mut s, _)) = in_game("party") else {
        return;
    };
    let w = &mut s.game.world;
    w.party_request = None;
    w.available.clear();
    for id in w.party.clone() {
        w.object_mut(id).unwrap().party_member = false;
    }
    w.party.clear();
    w.npc_slots.clear();
    w.available.insert(2, "end_trask".to_string());
    w.available.insert(6, "n_calonord".to_string());
    w.available.insert(8, "n_calonord".to_string());

    // Only who is offered can be chosen, and the game waits.
    s.open_party();
    assert_eq!(s.menu_name(), "party");
    assert!(s.menu_pixel("BTN_NPC0", None, SIZE).is_none());
    let empty = frame(&mut s);
    let t = s.game.world.time;
    s.update(DT);
    assert_eq!(s.game.world.time, t);

    // A click shows someone, and Add takes them. The words change, and the screen says how many places are left.
    press(&mut s, "BTN_NPC2");
    s.hover((1.0, 1.0), SIZE);
    let shown = frame(&mut s);
    assert!(shown != empty);
    press(&mut s, "BTN_ACCEPT");
    s.hover((1.0, 1.0), SIZE);
    assert!(frame(&mut s) != shown);
    assert!(s.game.world.party.is_empty(), "not until OK");

    // Cancel changes nothing.
    press(&mut s, "BTN_BACK");
    assert_eq!(s.menu_name(), "-");
    assert!(s.game.world.party.is_empty());

    // OK brings them. There are only two places.
    s.open_party();
    for slot in ["BTN_NPC2", "BTN_NPC6", "BTN_NPC8"] {
        press(&mut s, slot);
        press(&mut s, "BTN_ACCEPT");
    }
    press(&mut s, "BTN_DONE");
    assert_eq!(s.menu_name(), "-");
    let w = &s.game.world;
    assert_eq!(w.party.len(), 2);
    assert_eq!(w.party_slots(), vec![2, 6]);
    let member = w.npc_slots[&2];
    assert_eq!(w.object(member).unwrap().faction, 2);

    // Taking someone off sends them away.
    s.open_party();
    press(&mut s, "BTN_NPC2");
    press(&mut s, "BTN_ACCEPT");
    press(&mut s, "BTN_DONE");
    let w = &s.game.world;
    assert_eq!(w.party_slots(), vec![6]);
    assert!(w.object(member).is_none());

    // A script asks for the screen. Who it forces cannot be taken off, and it cannot be cancelled.
    s.game.world.party_request = Some(kotor_game::PartyRequest {
        script: String::new(),
        forced: vec![8],
    });
    s.update(DT);
    assert_eq!(s.menu_name(), "party");
    press(&mut s, "BTN_BACK");
    assert_eq!(s.menu_name(), "party");
    s.escape();
    assert_eq!(s.menu_name(), "party");
    press(&mut s, "BTN_NPC8");
    press(&mut s, "BTN_ACCEPT");
    press(&mut s, "BTN_DONE");
    assert_eq!(s.menu_name(), "-");
    assert_eq!(s.game.world.party_slots(), vec![6, 8]);
}

#[test]
fn the_map_shows_the_area_the_party_and_its_notes() {
    let Some((mut s, _)) = in_game("map") else {
        return;
    };
    // The numbers place the player on the picture.
    let map = s.game.world.area_map().expect("the first area has a map");
    assert_eq!(map.picture, "lbl_mapm01aa");
    let p = s.game.world.object(s.game.world.pc).unwrap().position;
    let [u, v] = map.place(p[0], p[1]);
    assert!(
        (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v),
        "{u} {v}"
    );

    // The small map on the HUD: a click on it opens the map, and it moves and turns with the player.
    s.update(DT);
    let moved_from = frame(&mut s);
    let w = &mut s.game.world;
    let heading = w.pc_mover.heading;
    w.pc_mover.heading = heading + 1.5;
    assert!(frame(&mut s) != moved_from, "the arrow turns");
    s.game.world.pc_mover.heading = heading;
    assert!(frame(&mut s) == moved_from, "and back");
    let corner = s.hud_pixel("BTN_MINIMAP", SIZE).unwrap();
    assert!(corner.0 < 200.0 && corner.1 < 120.0, "{corner:?}");
    s.click(corner, SIZE);
    assert_eq!(s.menu_name(), "map");
    s.escape();

    // M opens it and closes it, so do the HUD's button and the tab.
    s.open_map();
    assert_eq!(s.menu_name(), "map");
    let open = frame(&mut s);
    s.open_map();
    assert_eq!(s.menu_name(), "-");
    s.update(DT);
    let hud = s.hud_pixel("BTN_MAP", SIZE).unwrap();
    s.click(hud, SIZE);
    assert_eq!(s.menu_name(), "map");
    press(&mut s, "BTN_INV");
    press(&mut s, "BTN_MAP");
    assert_eq!(s.menu_name(), "map");
    s.hover((1.0, 1.0), SIZE);
    assert!(frame(&mut s) == open);

    // Up and down go through the notes, if the area has more than one.
    if map.notes.len() > 1 {
        press(&mut s, "BTN_DOWN");
        s.hover((1.0, 1.0), SIZE);
        assert!(frame(&mut s) != open);
        press(&mut s, "BTN_UP");
        s.hover((1.0, 1.0), SIZE);
        assert!(frame(&mut s) == open);
    }

    // The party button opens that screen, and comes back here.
    press(&mut s, "BTN_PRTYSLCT");
    assert_eq!(s.menu_name(), "party");
    press(&mut s, "BTN_BACK");
    assert_eq!(s.menu_name(), "map");
    // Moving moves the mark, and what is there has now been seen, also after going back.
    let seen = |s: &Stage| {
        let cells = s.game.world.explored.get("lbl_mapm01aa").expect("seen");
        cells.iter().filter(|c| **c).count()
    };
    let before = seen(&s);
    assert!(before > 0);
    let o = s.game.world.object_mut(s.game.world.pc).unwrap();
    o.position[1] += 30.0;
    s.hover((1.0, 1.0), SIZE);
    assert!(frame(&mut s) != open);
    assert!(seen(&s) > before);
    let o = s.game.world.object_mut(s.game.world.pc).unwrap();
    o.position[1] -= 30.0;
    s.hover((1.0, 1.0), SIZE);
    assert!(frame(&mut s) != open, "more of the map shows than before");
    press(&mut s, "BTN_EXIT");
    assert_eq!(s.menu_name(), "-");
}

#[test]
fn a_fight_brings_up_its_panel_and_the_target_slot_attacks() {
    let Some((mut s, _)) = in_game("combat") else {
        return;
    };
    let pc = s.game.world.pc;
    let p = s.game.world.object(pc).unwrap().position;
    let foe = s
        .game
        .world
        .spawn_creature("n_calonord", [p[0] + 0.1, p[1], p[2]]);
    s.update(DT);
    // Not an enemy yet: no red bar, no slot, no panel. A click where they would be walks.
    let friendly = frame(&mut s);
    assert!(!s.state_line().contains("target=-"), "{}", s.state_line());
    s.game.world.object_mut(foe).unwrap().faction = 1;
    let hostile = frame(&mut s);
    assert!(hostile != friendly, "the bar turns red and the slots come");

    // The slot under the bar attacks, and the panel comes up with Disengage.
    let slot = s.hud_pixel("BTN_TARGET0", SIZE).unwrap();
    s.click(slot, SIZE);
    assert_eq!(s.game.world.combat.target.get(&pc), Some(&foe));
    let engaged = frame(&mut s);
    assert!(engaged != hostile);
    let middle = s.hud_pixel("BTN_CLEARALL", SIZE).unwrap();
    assert!(middle.1 > SIZE.1 as f32 * 0.8, "{middle:?}");

    // Disengage stops it.
    s.click(middle, SIZE);
    assert_eq!(s.game.world.combat.target.get(&pc), None);
}

#[test]
fn a_tutorial_comes_once_over_a_screen_and_notices_flash() {
    let Some((mut s, _)) = in_game("tutorial") else {
        return;
    };
    s.enable_tutorials(true);
    s.game.world.tutorial_seen.clear();

    // The first look at the inventory brings its pop-up over it, and only the button answers.
    s.open_inventory();
    s.update(DT);
    assert_eq!(s.menu_name(), "tutorial");
    assert!(s.menu_pixel("BTN_EXIT", None, SIZE).is_none());
    press(&mut s, "BTN_OK");
    assert_eq!(s.menu_name(), "inventory");
    s.escape();
    // The second brings nothing.
    s.open_inventory();
    s.update(DT);
    assert_eq!(s.menu_name(), "inventory");
    s.escape();

    // A script asks for one with two pages: Continue, then OK. It pauses the game. Esc does what the button does.
    s.game.world.tutorial_requests.push(41);
    s.update(DT);
    assert_eq!(s.menu_name(), "tutorial");
    let time = s.game.world.time;
    s.update(DT);
    assert_eq!(s.game.world.time, time, "the game waits");
    let first = frame(&mut s);
    press(&mut s, "BTN_OK");
    assert_eq!(s.menu_name(), "tutorial");
    s.hover((1.0, 1.0), SIZE);
    assert!(frame(&mut s) != first, "the next page");
    s.escape();
    assert_eq!(s.menu_name(), "-");

    // Things that change flash an icon on the HUD, with their pop-up the first time.
    s.update(DT);
    let plain = frame(&mut s);
    s.game.world.gold += 7;
    s.update(DT);
    assert_eq!(s.menu_name(), "tutorial");
    s.escape();
    assert!(frame(&mut s) != plain, "the credits icon");
    // It goes after a while.
    for _ in 0..200 {
        s.game.world.time += 0.05;
        s.update(DT);
        if s.menu_name() != "-" {
            s.escape();
        }
    }
    s.enable_tutorials(false);
    assert_eq!(s.menu_name(), "-");
    assert!(frame(&mut s) != first);
}

/// The picture of the game, with nothing open.
fn frame_of_hud(s: &mut Stage) -> Vec<u8> {
    s.update(DT);
    s.overlay().unwrap().rgba.clone()
}

#[test]
fn the_tabs_change_screen_and_the_hud_buttons_open_them() {
    let Some((mut s, _)) = in_game("tabs") else {
        return;
    };
    s.update(DT);
    // The HUD's buttons, at the top right.
    let p = s.hud_pixel("BTN_INV", SIZE).unwrap();
    assert!(p.0 > SIZE.0 as f32 * 0.7 && p.1 < 60.0, "{p:?}");
    s.click(p, SIZE);
    assert_eq!(s.menu_name(), "inventory");
    s.update(DT);
    press(&mut s, "BTN_EQU");
    assert_eq!(s.menu_name(), "equip");
    press(&mut s, "BTN_CHAR");
    assert_eq!(s.menu_name(), "character");
    press(&mut s, "BTN_OPT");
    assert_eq!(s.menu_name(), "pause");
    press(&mut s, "BTN_INV");
    assert_eq!(s.menu_name(), "inventory");
    // The tabs for screens not built say so and stay.
    press(&mut s, "BTN_ABI");
    assert_eq!(s.menu_name(), "inventory");
    assert!(
        s.menu_message().contains("not built"),
        "{}",
        s.menu_message()
    );
    s.escape();
    assert_eq!(s.menu_name(), "-");
    // The screens fill the window: nothing is left black at the sides.
    s.open_equip();
    s.update(DT);
    let o = s.overlay().unwrap();
    let px = |x: u32, y: u32| &o.rgba[((y * o.width + x) * 4) as usize..][..3];
    assert!(
        px(4, SIZE.1 / 2).iter().any(|c| *c > 8),
        "{:?}",
        px(4, SIZE.1 / 2)
    );
    s.escape();
    // In a conversation the bars are black and the scene shows between them.
    s.game.world.talk_to(s.game.world.by_tag("end_trask")[0]);
    for _ in 0..200 {
        s.update(DT);
        if s.game.world.in_dialogue() {
            break;
        }
    }
    assert!(s.game.world.in_dialogue());
    s.update(DT);
    let o = s.overlay().unwrap();
    let alpha = |x: u32, y: u32| o.rgba[((y * o.width + x) * 4 + 3) as usize];
    assert_eq!(alpha(SIZE.0 - 1, 20), 255, "the top bar");
    assert_eq!(alpha(SIZE.0 - 1, SIZE.1 - 2), 255, "the bottom bar");
    assert_eq!(alpha(SIZE.0 / 2, SIZE.1 / 2), 0, "the scene");
}
