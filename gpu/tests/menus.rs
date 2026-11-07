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
    // Its own scene shows through it, and only that: see the test of the scene.
    assert!(
        o.rgba.chunks(4).any(|p| p[3] == 255),
        "the logo and buttons"
    );
    assert!(s.frame_scene().2, "nothing else is drawn behind it");

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

/// The temp folders a test's thread has been given. They go when it ends.
struct TempPlaces(std::cell::RefCell<Vec<std::path::PathBuf>>);

impl Drop for TempPlaces {
    fn drop(&mut self) {
        for dir in self.0.get_mut() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

thread_local! {
    static TEMP_PLACES: TempPlaces = const { TempPlaces(std::cell::RefCell::new(Vec::new())) };
}

/// A folder of this run's own: other runs of these tests can be going at the same time, in this checkout or another.
fn temp_place(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("kotor_rs_{name}_{}", std::process::id()));
    TEMP_PLACES.with(|p| p.0.borrow_mut().push(dir.clone()));
    dir
}

/// Only for the test below: a second run of this program says where its folder is.
#[test]
fn temp_place_of_this_run() {
    if std::env::var_os("KOTOR_SHOW_TEMP_PLACE").is_some() {
        let dir = temp_place("two_runs");
        std::fs::create_dir_all(&dir).unwrap();
        println!("place={}", dir.display());
    }
}

#[test]
fn two_runs_at_once_have_different_temp_folders_and_leave_none() {
    let start = || {
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "temp_place_of_this_run", "--nocapture"])
            .env("KOTOR_SHOW_TEMP_PLACE", "1")
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap()
    };
    let runs = [start(), start()];
    let places: Vec<String> = runs
        .into_iter()
        .map(|run| {
            let out = String::from_utf8(run.wait_with_output().unwrap().stdout).unwrap();
            let line = out.lines().find_map(|l| l.split("place=").nth(1));
            line.unwrap_or_else(|| panic!("{out}")).to_string()
        })
        .collect();
    assert_ne!(places[0], places[1]);
    for place in &places {
        assert!(!std::path::Path::new(place).exists(), "{place}");
    }
}

/// A temp folder for saves, empty, so that a test never touches the real ones.
fn temp_saves(s: &mut Stage, name: &str) -> std::path::PathBuf {
    let dir = temp_place(name);
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
    let line = s.state_line();
    assert_eq!(s.game.world.opened, Some(foe), "{line}");
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

    // A click on a row chooses it, and the middle button takes it.
    let items = s.game.world.container_items();
    assert!(items.len() >= 2);
    click_row(&mut s, "LB_ITEMS", 1);
    assert_eq!(s.game.world.container_items(), items, "only chosen so far");
    press(&mut s, "BTN_GIVEITEMS");
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

#[test]
fn a_power_for_oneself_is_in_the_fourth_slot_and_a_click_turns_it_on_and_off() {
    let Some((mut s, _)) = in_game("self_powers") else {
        return;
    };
    let pc = s.game.world.pc;
    // Nothing known, nothing shown: as the video has it on a Soldier.
    assert!(s.quick_slot_shown(2).is_none());
    const BURST_OF_SPEED: i32 = 8;
    const FORCE_ARMOR: i32 = 17;
    const FORCE_PUSH: i32 = 23;
    let o = s.game.world.object_mut(pc).unwrap();
    o.spells.extend([BURST_OF_SPEED, FORCE_ARMOR, FORCE_PUSH]);
    (o.force, o.max_force) = (100, 100);
    s.update(DT);
    let shown = s.quick_slot_shown(2).expect("the slot");
    assert!(shown.more, "two powers for oneself, and not the Push");
    assert!(!shown.lit);

    // Its name under the cursor.
    let slot = s.hud_pixel("BTN_ACTION3", SIZE).unwrap();
    s.hover(slot, SIZE);
    let name = s.combat_words().expect("a name");
    assert!(name.contains("Speed"), "{name}");

    // A click casts it, and it costs Force.
    let effects = |s: &Stage| s.game.world.object(pc).unwrap().effects.len();
    let before = effects(&s);
    s.click(slot, SIZE);
    s.update(DT);
    let paid = s.game.world.object(pc).unwrap().force;
    assert!(paid < 100, "{paid}");
    assert!(effects(&s) > before);
    assert!(s.quick_slot_shown(2).unwrap().lit);

    // Another ends it, and nothing comes back.
    s.click(slot, SIZE);
    assert_eq!(effects(&s), before);
    assert!(!s.quick_slot_shown(2).unwrap().lit);
    assert_eq!(s.game.world.object(pc).unwrap().force, paid);

    // Too dear: it goes from the slot, unless it is on.
    s.click(slot, SIZE);
    s.update(DT);
    s.game.world.object_mut(pc).unwrap().force = 0;
    assert!(s.quick_slot_shown(2).is_some(), "on, so it can go off");
    s.click(slot, SIZE);
    assert_eq!(effects(&s), before);
}

#[test]
fn the_title_card_comes_from_the_assets_folder_and_black_is_seen_through() {
    let Some(mut s) = at_menu() else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("kotor_rs_title_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    // None in the folder: the game's own stays, where the file has it.
    s.set_assets_dir(Some(dir.clone()));
    s.update(DT);
    assert_eq!(s.menu_picture_of("LBL_GAMELOGO"), None);

    // A card: black and gold, twice as wide as high.
    let mut rgba = Vec::new();
    for _ in 0..2 {
        for x in 0..4 {
            let gold = x >= 2;
            rgba.extend_from_slice(if gold {
                &[255, 200, 0, 255]
            } else {
                &[0, 0, 0, 255]
            });
        }
    }
    let card = kotor_image::Image {
        width: 4,
        height: 2,
        rgba,
    };
    std::fs::write(dir.join("title-card.png"), kotor_image::to_png(&card)).unwrap();
    s.set_assets_dir(Some(dir.clone()));
    s.update(DT);
    let name = s.menu_picture_of("LBL_GAMELOGO").expect("the card");
    let shown = s.ui_texture_for_test(&name).expect("its picture");
    assert_eq!((shown.width, shown.height), (4, 2));
    assert_eq!(shown.rgba[3], 0, "black is seen through");
    assert_eq!(shown.rgba[2 * 4 + 3], 255, "gold is not");
    for tag in ["BTN_NEWGAME", "BTN_LOADGAME", "BTN_OPTIONS", "BTN_EXIT"] {
        assert!(s.menu_pixel(tag, None, SIZE).is_some(), "{tag}");
    }

    // Gone again: back to the game's own.
    std::fs::remove_file(dir.join("title-card.png")).unwrap();
    s.set_assets_dir(Some(dir.clone()));
    s.update(DT);
    assert_eq!(s.menu_picture_of("LBL_GAMELOGO"), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_attack_mode_button_beside_the_hourglass_turns_it_on_and_off() {
    let Some((mut s, _)) = in_game("attack_mode") else {
        return;
    };
    assert_eq!(s.action_for_key("KeyB"), Some("AttackMode"));
    s.update(DT);
    assert!(!s.game.world.attack_mode());
    let button = s.hud_pixel(kotor_ui::ATTACK_MODE_BUTTON, SIZE).unwrap();
    s.hover(button, SIZE);
    assert_eq!(s.combat_words().as_deref(), Some("Attack mode: Off"));
    // How much of the button's line shows.
    let line = |s: &mut Stage| -> usize {
        s.update(DT);
        let o = s.overlay().unwrap();
        o.rgba
            .chunks(4)
            .filter(|p| p[0] > 240 && p[1] > 240 && (60..100).contains(&p[2]))
            .count()
    };
    let without = line(&mut s);
    s.click(button, SIZE);
    assert!(s.game.world.attack_mode());
    assert_eq!(s.combat_words().as_deref(), Some("Attack mode: On"));
    assert!(line(&mut s) > without + 100, "the line round it");
    s.attack_mode_key();
    assert!(!s.game.world.attack_mode());
    // Not from behind a screen.
    s.open_inventory();
    s.attack_mode_key();
    assert!(!s.game.world.attack_mode());
}

#[test]
fn a_rounded_button_has_no_square_plate_behind_it() {
    let Some((mut s, _)) = in_game("rounded") else {
        return;
    };
    let pc = s.game.world.pc;
    s.game.world.hold_levels = true;
    s.open_character();
    s.update(DT);
    let without = s.overlay().unwrap().clone();
    s.escape();
    s.game.world.grant_xp(pc, 4000);
    s.open_character();
    s.update(DT);
    let (x, y) = s.menu_pixel("BTN_AUTO", None, SIZE).unwrap();
    let o = s.overlay().unwrap();
    let at = |img: &kotor_image::Image, dx: f32, dy: f32| -> [u8; 4] {
        let (px, py) = ((x + dx * 1.5) as u32, (y + dy * 1.5) as u32);
        let i = ((py * img.width + px) * 4) as usize;
        [
            img.rgba[i],
            img.rgba[i + 1],
            img.rgba[i + 2],
            img.rgba[i + 3],
        ]
    };
    // `BTN_AUTO` is 127 by 52 layout units. In a corner, outside the rounded line, it draws nothing.
    for (dx, dy) in [(-62.0, -25.0), (62.0, -25.0), (-62.0, 25.0), (62.0, 25.0)] {
        assert_eq!(at(o, dx, dy), at(&without, dx, dy), "corner {dx},{dy}");
    }
    // The bar through the middle is gone: its centre is as dark as the rim, on the same ground.
    let centre = at(o, 40.0, 0.0);
    let rim = at(o, 40.0, -18.0);
    let ground = (at(&without, 40.0, 0.0), at(&without, 40.0, -18.0));
    if ground.0 == ground.1 {
        assert_eq!(centre, rim);
    }
}

#[test]
fn every_kind_on_the_bench_has_its_picture_and_what_cannot_be_chosen_is_greyed() {
    let Some((mut s, _)) = in_game("bench_pictures") else {
        return;
    };
    // How many pixels in a kind's box are at least this bright.
    let bright = |s: &mut Stage, tag: &str, least: u8| -> usize {
        s.update(DT);
        let (x, y) = s.menu_pixel(tag, None, SIZE).unwrap();
        let o = s.overlay().unwrap();
        let (half_w, half_h) = (70 * 3 / 2, 64 * 3 / 2);
        let mut count = 0;
        for py in (y as u32 - half_h)..(y as u32 + half_h) {
            for px in (x as u32 - half_w)..(x as u32 + half_w) {
                let p = &o.rgba[((py * o.width + px) * 4) as usize..][..3];
                count += p.iter().all(|c| *c >= least) as usize;
            }
        }
        count
    };
    const KINDS: [&str; 4] = ["BTN_LIGHTSABER", "BTN_RANGED", "BTN_MELEE", "BTN_ARMOR"];
    s.open_bench();
    for tag in KINDS {
        assert!(bright(&mut s, tag, 60) > 300, "{tag} has its picture");
        assert!(
            bright(&mut s, tag, 200) < 300,
            "{tag} has nothing to upgrade"
        );
    }
    s.escape();
    let w = &mut s.game.world;
    let pc = w.pc;
    let saber = w.spawn_item("g_w_lghtsbr01");
    w.carry(pc, saber);
    s.open_bench();
    assert!(
        bright(&mut s, "BTN_LIGHTSABER", 200) > 300,
        "it can be chosen"
    );
    assert!(
        bright(&mut s, "BTN_RANGED", 200) < 300,
        "this still has nothing"
    );
}

/// A creature's remains here, holding this template.
fn remains_at(s: &mut Stage, at: [f32; 3], template: &str, count: i32) -> (u32, u32) {
    let w = &mut s.game.world;
    let body = w.spawn_creature("n_calonord", at);
    let item = w.spawn_item(template);
    w.object_mut(item).unwrap().stack = count;
    let o = w.object_mut(body).unwrap();
    o.dead = true;
    o.useable = true;
    o.inventory = vec![item];
    o.equipped.clear();
    (body, item)
}

#[test]
fn every_pile_of_credits_in_a_container_is_one_row_with_their_sum() {
    let Some((mut s, _)) = in_game("credit_piles") else {
        return;
    };
    let locker = s.game.world.by_tag("end_locker01")[0];
    assert!(s.stand_near("end_locker01"));
    s.update(DT);
    s.game.world.click(locker);
    let w = &mut s.game.world;
    let before = w.container_items().len();
    let piles: Vec<u32> = [10, 20, 30]
        .into_iter()
        .map(|count| {
            let item = w.spawn_item("g_i_credits001");
            w.object_mut(item).unwrap().stack = count;
            w.object_mut(locker).unwrap().inventory.insert(0, item);
            item
        })
        .collect();
    assert_eq!(s.game.world.container_items().len(), before + 1);
    let rows = s.menu_rows("LB_ITEMS");
    assert_eq!(rows[0].count, 60, "{}", rows[0].text);
    assert_eq!(rows[0].stepper, Some(1));

    // The stepper goes up to the sum, and what is left is still one row.
    let gold = s.game.world.gold;
    click_row(&mut s, "LB_ITEMS", 0);
    let step = s.menu_step_pixel("LB_ITEMS", 0, 1, SIZE).unwrap();
    s.set_shift(true);
    for _ in 0..4 {
        s.click(step, SIZE);
    }
    s.set_shift(false);
    assert_eq!(s.menu_rows("LB_ITEMS")[0].stepper, Some(41));
    press(&mut s, "BTN_GIVEITEMS");
    assert_eq!(s.game.world.gold, gold + 41);
    assert_eq!(s.game.world.container_items().len(), before + 1);
    assert_eq!(s.menu_rows("LB_ITEMS")[0].count, 19);

    // Take All gives the rest, and no pile is left behind.
    press(&mut s, "BTN_OK");
    assert_eq!(s.game.world.gold, gold + 60);
    for pile in piles {
        assert!(s.game.world.object(pile).is_none(), "pile {pile}");
    }
    let o = s.game.world.object(locker).unwrap();
    assert!(o.inventory.is_empty());
}

#[test]
fn pickup_opens_what_is_near_as_one_container_and_what_is_taken_leaves_its_own() {
    let Some((mut s, dir)) = in_game("pickup") else {
        return;
    };
    let locker = s.game.world.by_tag("end_locker01")[0];
    assert!(s.stand_near("end_locker01"));
    s.update(DT);
    let pc = s.game.world.pc;
    let here = s.game.world.object(pc).unwrap().position;
    // Clear of everything else near: only what the test puts here counts.
    let others: Vec<u32> = s.game.world.objects.values().map(|o| o.id).collect();
    for id in others.into_iter().filter(|i| *i != pc && *i != locker) {
        let o = s.game.world.object_mut(id).unwrap();
        if matches!(
            o.kind,
            kotor_game::Kind::Creature | kotor_game::Kind::Placeable
        ) {
            o.useable = false;
        }
    }
    // A spot in sight at 3.5 m, past the reach.
    let far_at = (0..16)
        .map(|n| (n as f32 * std::f32::consts::TAU / 16.0).sin_cos())
        .map(|(y, x)| [here[0] + 3.5 * x, here[1] + 3.5 * y, here[2]])
        .find(|at| s.game.world.can_see_along(here, *at))
        .expect("somewhere in sight");
    let (body, medpac) = remains_at(
        &mut s,
        [here[0] + 0.5, here[1], here[2]],
        "g_i_medeqpmnt01",
        2,
    );
    let (far, sword) = remains_at(&mut s, far_at, "g_w_lngswrd01", 1);

    // The slot is the Pickup button, with its name under the cursor.
    let slot = s.hud_pixel("BTN_ACTION0", SIZE).unwrap();
    s.hover(slot, SIZE);
    assert_eq!(s.combat_words().as_deref(), Some("Pickup"));

    // A click: the locker and the near remains are one panel, and the far ones stay out of it.
    s.click(slot, SIZE);
    assert_eq!(s.menu_name(), "container");
    let w = &s.game.world;
    assert_eq!(w.open_sources().len(), 2, "{:?}", w.open_sources());
    let listed: Vec<u32> = w.container_items().iter().map(|i| i.0).collect();
    assert!(listed.contains(&medpac));
    assert!(!listed.contains(&sword), "out of reach");
    let locker_had = w.object(locker).unwrap().inventory.clone();
    assert!(!locker_had.is_empty(), "this footlocker has its own things");
    assert!(locker_had.iter().all(|i| listed.contains(i)));

    // What is taken leaves where it was.
    let row = listed.iter().position(|i| *i == medpac).unwrap();
    click_row(&mut s, "LB_ITEMS", row);
    let step = s.menu_step_pixel("LB_ITEMS", row, 1, SIZE).unwrap();
    s.click(step, SIZE);
    press(&mut s, "BTN_GIVEITEMS");
    let w = &s.game.world;
    assert!(w.object(body).unwrap().inventory.is_empty());
    assert_eq!(w.object(locker).unwrap().inventory, locker_had);
    assert!(w.object(pc).unwrap().inventory.iter().any(|i| {
        w.object(*i)
            .is_some_and(|o| o.template == "g_i_medeqpmnt01" && o.stack >= 2)
    }));

    // Take All empties the rest, and all of it shuts.
    press(&mut s, "BTN_OK");
    let w = &s.game.world;
    assert!(w.object(locker).unwrap().inventory.is_empty());
    assert!(w.opened.is_none() && w.opened_too.is_empty());
    assert_eq!(w.object(far).unwrap().inventory, vec![sword], "left alone");

    // Nothing left in reach: a word over the player, and no panel.
    s.update(DT);
    s.pickup();
    assert_eq!(s.menu_name(), "-");
    assert!(s.floats_shown().iter().any(|f| f == "Nothing nearby"));

    // The reach is a setting.
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("settings.txt"), "pickup_radius=4\n").unwrap();
    s.use_settings_file(dir.join("settings.txt"));
    s.pickup();
    assert_eq!(s.menu_name(), "container");
    assert_eq!(s.game.world.open_sources(), vec![far]);
    s.game.world.close_container();

    // What is locked stays shut, and the log says so.
    let w = &mut s.game.world;
    let again = w.spawn_item("g_w_lngswrd01");
    let l = w.object_mut(locker).unwrap();
    l.locked = true;
    l.inventory = vec![again];
    w.events.clear();
    s.pickup();
    let w = &s.game.world;
    assert_eq!(w.open_sources(), vec![far]);
    assert!(
        w.events
            .iter()
            .any(|e| e.contains("is locked: Pickup left it")),
        "{:?}",
        w.events
    );
}

#[test]
fn a_container_takes_the_rows_chosen_with_control_in_the_numbers_their_steppers_say() {
    let Some((mut s, _)) = in_game("take_selected") else {
        return;
    };
    let pc = s.game.world.pc;
    let locker = s.game.world.by_tag("end_locker01")[0];
    assert!(s.stand_near("end_locker01"));
    s.update(DT);
    s.game.world.click(locker);
    assert!(
        !s.game.world.container_items().is_empty(),
        "bug 12: this footlocker has its own things"
    );
    // Rows 0 to 3: 50 credits, a sword, 3 medpacs and a sword. What the locker had comes after them.
    let w = &mut s.game.world;
    let stack = |w: &mut kotor_game::World, n: usize, template: &str, count: i32| {
        let item = w.spawn_item(template);
        w.object_mut(item).unwrap().stack = count;
        w.object_mut(locker).unwrap().inventory.insert(n, item);
        item
    };
    let credits = stack(w, 0, "g_i_credits001", 50);
    let sword = stack(w, 1, "g_w_lngswrd01", 1);
    let medpacs = stack(w, 2, "g_i_medeqpmnt01", 3);
    let other = stack(w, 3, "g_w_shortswrd01", 1);
    let everything = s.game.world.container_items().len();
    let gold = s.game.world.gold;
    let chosen = |s: &mut Stage| -> Vec<bool> {
        s.menu_rows("LB_ITEMS")[..4]
            .iter()
            .map(|r| r.selected)
            .collect()
    };

    // The buttons have the owner's words, and only a stack has a stepper. It starts at 1.
    let rows = s.menu_rows("LB_ITEMS");
    assert_eq!(rows[0].stepper, Some(1));
    assert_eq!(rows[1].stepper, None);
    assert_eq!(rows[2].stepper, Some(1));
    assert_eq!(rows[3].stepper, None);
    assert_eq!(s.menu_button_words("BTN_OK").as_deref(), Some("Take All"));
    assert_eq!(
        s.menu_button_words("BTN_GIVEITEMS").as_deref(),
        Some("Take Selected")
    );

    // Nothing chosen: the button does nothing.
    press(&mut s, "BTN_GIVEITEMS");
    assert_eq!(s.game.world.container_items().len(), everything);

    // A plain click chooses one row and leaves the last.
    click_row(&mut s, "LB_ITEMS", 0);
    assert_eq!(chosen(&mut s), [true, false, false, false]);
    click_row(&mut s, "LB_ITEMS", 3);
    assert_eq!(chosen(&mut s), [false, false, false, true]);
    click_row(&mut s, "LB_ITEMS", 0);
    // With Control it adds, and takes away again.
    s.set_ctrl(true);
    click_row(&mut s, "LB_ITEMS", 1);
    assert_eq!(chosen(&mut s), [true, true, false, false]);
    click_row(&mut s, "LB_ITEMS", 3);
    assert_eq!(chosen(&mut s), [true, true, false, true]);
    click_row(&mut s, "LB_ITEMS", 3);
    assert_eq!(chosen(&mut s), [true, true, false, false]);
    s.set_ctrl(false);

    // The stepper keeps what is chosen. It stops at 1 and at the whole stack, and goes by ten with Shift.
    let arrow = |s: &mut Stage, row: usize, by: i32| {
        let p = s.menu_step_pixel("LB_ITEMS", row, by, SIZE).unwrap();
        s.hover(p, SIZE);
        s.click(p, SIZE);
    };
    arrow(&mut s, 0, -1);
    assert_eq!(s.menu_rows("LB_ITEMS")[0].stepper, Some(1));
    assert_eq!(chosen(&mut s), [true, true, false, false]);
    s.set_shift(true);
    for _ in 0..6 {
        arrow(&mut s, 0, 1);
    }
    assert_eq!(s.menu_rows("LB_ITEMS")[0].stepper, Some(50));
    arrow(&mut s, 0, -1);
    arrow(&mut s, 0, -1);
    arrow(&mut s, 0, -1);
    s.set_shift(false);
    assert_eq!(s.menu_rows("LB_ITEMS")[0].stepper, Some(20));

    // 20 of the 50 credits and the sword come. The container keeps 30 credits and the rest, and stays up.
    press(&mut s, "BTN_GIVEITEMS");
    assert_eq!(s.game.world.gold, gold + 20);
    let w = &s.game.world;
    assert_eq!(w.object(credits).unwrap().stack, 30);
    assert_eq!(w.container_items().len(), everything - 1);
    assert_eq!(w.container_items()[0].0, credits);
    assert!(w.carried(pc).contains(&sword));
    assert!(w.container_items().iter().any(|i| i.0 == other));
    assert!(w.container_items().iter().any(|i| i.0 == medpacs));
    assert_eq!(s.menu_name(), "container");
    let rows = s.menu_rows("LB_ITEMS");
    assert!(rows.iter().all(|r| !r.selected), "nothing stays chosen");
    assert_eq!(rows[0].stepper, Some(1), "and the stepper starts again");
    assert_eq!(rows[0].count, 30);

    // An arrow on a row not chosen chooses it too. Two of the 3 medpacs come, and the last has no stepper.
    let carried = |s: &Stage| -> i32 {
        let w = &s.game.world;
        w.carried(pc)
            .iter()
            .filter_map(|i| w.object(*i))
            .filter(|o| o.template == "g_i_medeqpmnt01")
            .map(|o| o.stack)
            .sum()
    };
    let before = carried(&s);
    arrow(&mut s, 1, 1);
    assert!(s.menu_rows("LB_ITEMS")[1].selected);
    press(&mut s, "BTN_GIVEITEMS");
    assert_eq!(carried(&s), before + 2);
    assert_eq!(s.game.world.object(medpacs).unwrap().stack, 1);
    assert_eq!(s.menu_rows("LB_ITEMS")[1].stepper, None);
    assert!(s.game.world.errors.is_empty(), "{:?}", s.game.world.errors);

    // Close leaves it all as it is, and it opens with nothing chosen.
    click_row(&mut s, "LB_ITEMS", 0);
    let left = s.game.world.container_items();
    press(&mut s, "BTN_CANCEL");
    assert_eq!(s.game.world.opened, None);
    s.update(DT);
    s.game.world.click(locker);
    assert_eq!(s.game.world.container_items(), left);
    assert!(s.menu_rows("LB_ITEMS").iter().all(|r| !r.selected));

    // Take All takes the rest of the stack too, and the panel goes.
    press(&mut s, "BTN_OK");
    assert_eq!(s.game.world.gold, gold + 50);
    assert!(s.game.world.object(locker).unwrap().inventory.is_empty());
    assert_eq!(s.menu_name(), "-");

    // The last rows taken shut the panel as well.
    let w = &mut s.game.world;
    let last = w.spawn_item("g_w_lngswrd01");
    w.object_mut(locker).unwrap().inventory.push(last);
    w.click(locker);
    click_row(&mut s, "LB_ITEMS", 0);
    press(&mut s, "BTN_GIVEITEMS");
    assert_eq!(s.menu_name(), "-");
    assert_eq!(s.game.world.opened, None);
}

#[test]
fn a_containers_label_fades_when_it_is_emptied_in_play() {
    let Some((mut s, _)) = in_game("empty_label") else {
        return;
    };
    // Past the talk that follows the first, for no plates show in one.
    let mut quiet = 0;
    for _ in 0..20000 {
        s.update(DT);
        if s.game.world.in_dialogue() {
            quiet = 0;
            s.number_key(1);
            s.continue_line();
        } else {
            quiet += 1;
            if quiet > 150 {
                break;
            }
        }
    }
    let locker = s.game.world.by_tag("end_locker01")[0];
    assert!(s.stand_near("end_locker01"));
    // Facing it: a plate behind the camera is not drawn.
    let at = s.game.world.object(locker).unwrap().position;
    let here = s.game.world.pc_mover.pos;
    s.place_player(here, (at[1] - here[1]).atan2(at[0] - here[0]));
    s.update(DT);
    s.update(DT);
    let name = s.game.world.object(locker).unwrap().name.clone();
    let label = |s: &Stage| {
        s.plates_drawn()
            .iter()
            .find(|p| p.text.starts_with(&name))
            .cloned()
            .expect("a label")
    };
    assert_eq!(label(&s).opacity, 1.0, "not looked in yet");
    s.game.world.click(locker);
    press(&mut s, "BTN_CANCEL");
    s.update(DT);
    assert_eq!(
        label(&s).opacity,
        1.0,
        "looked in, and it still holds things"
    );
    s.game.world.click(locker);
    press(&mut s, "BTN_OK");
    s.update(DT);
    assert_eq!(label(&s).opacity, 0.33, "{:?}", label(&s));
}

/// The cursor moves here. What is drawn again is what drawing the whole would give.
fn cursor_to(s: &mut Stage, at: (f32, f32), what: &str) -> bool {
    let seen = s.overlay_version;
    s.hover(at, SIZE);
    s.update(DT);
    if s.overlay_version == seen {
        return false;
    }
    let change = s.overlay_change_since(seen);
    let part = s.overlay().unwrap().clone();
    // A new size forgets what was drawn.
    s.set_view(SIZE);
    let whole = s.overlay().unwrap();
    let wrong = part
        .rgba
        .chunks(4)
        .zip(whole.rgba.chunks(4))
        .position(|(a, b)| a != b);
    assert_eq!(
        wrong.map(|n| (n % part.width as usize, n / part.width as usize)),
        None,
        "{what} on {}: {change:?}",
        s.menu_name()
    );
    change.unwrap_or_else(|| panic!("{what} on {}: all of it", s.menu_name()));
    true
}

#[test]
fn the_cursor_draws_again_only_what_it_changes_and_the_picture_is_the_same() {
    let Some(mut s) = at_menu() else { return };
    let dir = temp_saves(&mut s, "hover_part");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("settings.txt");
    std::fs::write(&file, "").unwrap();
    s.use_settings_file(file);
    s.set_view(SIZE);
    s.update(DT);
    let mut changes = 0;
    let mut through = |s: &mut Stage, tags: &[&str]| {
        for tag in tags {
            let Some(p) = s.menu_pixel(tag, None, SIZE) else {
                continue;
            };
            changes += cursor_to(s, p, tag) as i32;
            changes += cursor_to(s, (2.0, 2.0), "off it") as i32;
        }
    };
    through(
        &mut s,
        &["BTN_NEWGAME", "BTN_LOADGAME", "BTN_OPTIONS", "BTN_EXIT"],
    );
    assert!(changes >= 6, "{changes}");

    // A screen with a list and words that follow it, and the party's tabs.
    press(&mut s, "BTN_NEWGAME");
    for _ in 0..12000 {
        s.update(DT);
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        } else if s.game.world.transcript.len() > 3 && s.game.world.delayed.is_empty() {
            break;
        }
    }
    for template in ["g_w_lngswrd01", "g_i_medeqpmnt01", "g_a_clothes01"] {
        let w = &mut s.game.world;
        let (pc, item) = (w.pc, w.spawn_item(template));
        w.carry(pc, item);
    }
    let mut changes = 0;
    let mut rows = |s: &mut Stage, n: usize| {
        for row in 0..n {
            if let Some(p) = s.menu_pixel("LB_ITEMS", Some(row), SIZE) {
                changes += cursor_to(s, p, "a row") as i32;
            }
        }
    };
    s.open_inventory();
    s.update(DT);
    assert_eq!(s.menu_name(), "inventory");
    rows(&mut s, 3);
    let tab = s.menu_pixel("BTN_EQU", None, SIZE).unwrap();
    cursor_to(&mut s, tab, "a tab");
    s.click(tab, SIZE);
    s.update(DT);
    assert_eq!(s.menu_name(), "equip");
    let slot = s.menu_pixel("BTN_INV_WEAP_R", None, SIZE).unwrap();
    cursor_to(&mut s, slot, "a slot");
    s.click(slot, SIZE);
    s.update(DT);
    rows(&mut s, 2);
    s.escape();
    s.escape();
    s.update(DT);
    assert!(changes >= 3, "{changes}");
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
    let file = temp_place("settings_test").join("settings.txt");
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
    // The movement box: Modern to begin with, kept in the file, and back again.
    assert!(s.settings().movement_modern);
    press(&mut s, "CB_REVERSE");
    assert!(!s.settings().movement_modern);
    assert!(std::fs::read_to_string(&file)
        .unwrap()
        .contains("movement_modern=false"));
    press(&mut s, "CB_REVERSE");
    assert!(s.settings().movement_modern);
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
    // Invert Camera was ticked above and has been kept: the mouse going down now looks up.
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

/// A door a Security check can open, locked, with the player beside it and nothing else in reach to be the target.
fn beside_a_locked_door(s: &mut Stage) -> Option<u32> {
    let w = &mut s.game.world;
    let doors: Vec<u32> = w
        .objects
        .values()
        .filter(|o| o.kind == kotor_game::Kind::Door && o.link_module.is_empty())
        .map(|o| o.id)
        .collect();
    let door = doors.into_iter().find(|d| w.lock_can_be_picked(*d))?;
    let at = w.object(w.pc)?.position;
    // Trask would be the nearest thing in reach.
    for o in w.objects.values_mut() {
        o.useable = false;
    }
    let o = w.object_mut(door)?;
    o.locked = true;
    o.open = false;
    o.useable = true;
    // This one talks to whoever fails it, and has scripts for that.
    o.conversation.clear();
    o.scripts.clear();
    // The floor keeps the player where it has them, so the door comes here.
    o.position = [at[0] - 0.8, at[1], at[2]];
    Some(door)
}

fn carrying(s: &mut Stage, template: &str) -> u32 {
    let w = &mut s.game.world;
    let pc = w.pc;
    let item = w.spawn_item(template);
    w.object_mut(pc).unwrap().inventory.push(item);
    item
}

#[test]
fn a_locked_door_is_tried_with_the_skill_and_a_spike_only_by_one_who_has_points() {
    let Some((mut s, _)) = in_game("lock_ways") else {
        return;
    };
    let Some(door) = beside_a_locked_door(&mut s) else {
        return;
    };
    let pc = s.game.world.pc;
    let spike = carrying(&mut s, "g_i_secspike02");
    assert!(s.game.world.lock_ways(pc).1.contains(&spike));
    let tried = |s: &Stage| {
        s.game
            .world
            .events
            .iter()
            .filter(|e| e.starts_with("security"))
            .count()
    };
    // No points: they cannot try, even with a spike, and the spike stays.
    s.game.world.object_mut(pc).unwrap().skills = vec![0; 8];
    let by_it = |s: &mut Stage| {
        // Facing it, or its bar has nowhere to be.
        s.yaw = std::f32::consts::PI;
        // Trask goes on talking in this room.
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        }
        s.update(DT);
    };
    // Where someone is drawn catches up with where they are.
    for _ in 0..10 {
        by_it(&mut s);
    }
    let slot = s.hud_pixel("BTN_TARGET0", SIZE).unwrap();
    s.click(slot, SIZE);
    assert_eq!(
        s.menu_name(),
        "-",
        "it is the slot, not what the file has there"
    );
    for _ in 0..30 {
        by_it(&mut s);
    }
    assert_eq!(tried(&s), 0);
    assert!(s.game.world.object(door).unwrap().locked);
    assert!(s.game.world.object(spike).is_some());

    // With points the slot has the skill first, and then the spike, which goes with the try.
    s.game.world.object_mut(pc).unwrap().skills[6] = 20;
    by_it(&mut s);
    let down = s.hud_pixel("BTN_TARGETDOWN0", SIZE).unwrap();
    s.click(down, SIZE);
    by_it(&mut s);
    let slot = s.hud_pixel("BTN_TARGET0", SIZE).unwrap();
    s.click(slot, SIZE);
    for _ in 0..30 {
        by_it(&mut s);
    }
    assert_eq!(tried(&s), 1);
    assert!(s.game.world.object(spike).is_none(), "a spike is used up");
    assert!(!s.game.world.object(door).unwrap().locked);
}

#[test]
fn a_medpac_is_used_from_the_quick_slot() {
    let Some((mut s, _)) = in_game("quick_slot") else {
        return;
    };
    let pc = s.game.world.pc;
    let medpac = carrying(&mut s, "g_i_medeqpmnt01");
    carrying(&mut s, "g_i_adrnaline003");
    let lists = s.game.world.quick_items(pc);
    assert_eq!(lists[0], vec![medpac]);
    assert_eq!(lists[1].len(), 1, "a stim goes in the next slot");
    s.game.world.object_mut(pc).unwrap().hp = 1;
    s.update(DT);
    let slot = s.hud_pixel("BTN_ACTION1", SIZE).unwrap();
    s.click(slot, SIZE);
    for _ in 0..30 {
        s.update(DT);
    }
    assert!(s.game.world.object(pc).unwrap().hp > 1, "it heals");
    assert!(s.game.world.object(medpac).is_none(), "and is used up");
}

#[test]
fn the_arrow_on_a_portrait_goes_to_the_character_when_a_level_waits() {
    let Some((mut s, _)) = in_game("level_arrow") else {
        return;
    };
    let pc = s.game.world.pc;
    let portrait = s.hud_pixel("BTN_CHAR1", SIZE).unwrap();
    s.click(portrait, SIZE);
    assert_eq!(s.menu_name(), "-", "nothing waits");
    s.game.world.hold_levels = true;
    s.game.world.grant_xp(pc, 1000);
    assert!(s.game.world.level_ready(pc));
    s.click(portrait, SIZE);
    assert_eq!(s.menu_name(), "character");
}

#[test]
fn the_mouse_going_down_looks_down_and_invert_camera_flips_it() {
    let Some((mut s, dir)) = in_game("invert_camera") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("settings.txt");
    for (text, flipped) in [
        ("movement_modern=true\n", false),
        ("movement_modern=false\n", false),
        ("invert_y=true\nmovement_modern=true\n", true),
        ("invert_y=true\nmovement_modern=false\n", true),
        // What an old file calls the box does nothing.
        ("invert_camera=true\n", false),
    ] {
        std::fs::write(&file, text).unwrap();
        s.use_settings_file(file.clone());
        s.look(0.0, 50.0);
        let before = s.looks_down();
        s.look(0.0, 10.0);
        let after = s.looks_down();
        assert_eq!(after < before, flipped, "{text}: {before} then {after}");
        assert!((after - before).abs() > 1e-4, "{text}");
    }
}

#[test]
fn invert_camera_begins_off() {
    let Some((mut s, _)) = in_game("invert_label") else {
        return;
    };
    assert!(!s.settings().invert_y, "off to begin with");
    s.escape();
    press(&mut s, "BTN_GAMEPLAY");
    press(&mut s, "CB_INVERTCAM");
    assert!(s.settings().invert_y);
}

#[test]
fn back_on_the_main_menu_nothing_pops_up() {
    let Some((mut s, _)) = in_game("no_popups") else {
        return;
    };
    s.game.world.gold = 77;
    s.update(DT);
    s.enable_tutorials(true);
    s.escape();
    press(&mut s, "BTN_QUIT");
    for _ in 0..30 {
        s.update(DT);
        assert_eq!(s.menu_name(), "main");
    }
}

#[test]
fn a_fight_is_saved_before_it_starts_in_the_one_file() {
    let Some((mut s, dir)) = in_game("fight_save") else {
        return;
    };
    let file = dir.join("autosave.sav");
    s.combat_autosave = true;
    s.enable_auto_pause(false);
    let pc = s.game.world.pc;
    let foes: Vec<u32> = s
        .game
        .world
        .objects
        .values()
        .filter(|o| o.kind == kotor_game::Kind::Creature && s.game.world.reputation(pc, o.id) <= 10)
        .map(|o| o.id)
        .collect();
    assert!(foes.len() >= 2);
    s.update(DT);
    assert!(!file.exists(), "nobody seen yet");
    s.game.world.combat.perceived.insert((pc, foes[0]));
    s.update(DT);
    let info = kotor_game::read_save_info(&file).unwrap();
    assert_eq!(info.name, "Autosave");
    let saved = std::fs::metadata(&file).unwrap().modified().unwrap();

    // Another of the same fight does not save again, and it is the same one file.
    std::thread::sleep(std::time::Duration::from_millis(20));
    s.game.world.combat.perceived.insert((pc, foes[1]));
    s.update(DT);
    assert_eq!(std::fs::metadata(&file).unwrap().modified().unwrap(), saved);
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_game_for_a_test_never_saves_before_a_fight() {
    let Some((mut s, dir)) = in_game("fight_no_save") else {
        return;
    };
    s.enable_auto_pause(true);
    let pc = s.game.world.pc;
    let foe = s
        .game
        .world
        .objects
        .values()
        .find(|o| o.kind == kotor_game::Kind::Creature && s.game.world.reputation(pc, o.id) <= 10)
        .map(|o| o.id)
        .unwrap();
    s.game.world.combat.perceived.insert((pc, foe));
    s.update(DT);
    assert!(!dir.join("autosave.sav").exists());
}

#[test]
fn the_party_screen_shows_who_is_in_a_slot() {
    let Some((mut s, _)) = in_game("party_faces") else {
        return;
    };
    // Trask is in Bastila's slot on the Endar Spire: `k_pend` scripts use slot 0.
    s.game.world.available.insert(0, "end_trask".to_string());
    assert_eq!(s.game.world.party_portrait(0).as_deref(), Some("po_ptrask"));
}

#[test]
fn the_figure_the_mouse_is_on_is_the_big_one() {
    let Some(mut s) = at_chargen("big_one") else {
        return;
    };
    let hole = |s: &mut Stage, tag: &str| {
        let p = s.menu_pixel(tag, None, SIZE).unwrap();
        s.hover(p, SIZE);
        s.update(DT);
        frame(s)
    };
    let first = hole(&mut s, "BTN_SEL1");
    let fifth = hole(&mut s, "BTN_SEL5");
    assert!(first != fifth, "the big one moves");
    let again = hole(&mut s, "BTN_SEL1");
    assert_eq!(first.len(), again.len());
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
    let dir = temp_place("journal");
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
    press(&mut s, "BTN_OK");
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
    press(&mut s, "BTN_OK");
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
    press(&mut s, "BTN_OK");
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
    // The HUD stays under it.
    s.update(DT);
    let portrait = ((720 - 40) * 1280 + 50) * 4;
    let hud = s.overlay().unwrap().rgba[portrait..portrait + 4].to_vec();
    assert_eq!(hud[3], 255, "the player's portrait");
    s.game.world.tutorial_requests.push(41);
    s.update(DT);
    assert_eq!(s.menu_name(), "tutorial");
    s.update(DT);
    assert_eq!(s.overlay().unwrap().rgba[portrait..portrait + 4], hud[..]);
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

#[test]
fn the_feedback_and_autopause_pages_keep_what_they_say_and_it_takes_effect() {
    let Some((mut s, dir)) = in_game("feedback") else {
        return;
    };
    let file = dir.join("settings.txt");
    s.use_settings_file(file.clone());
    s.escape();
    assert_eq!(s.menu_name(), "pause");

    // Feedback: a click on a row flips it, and the file keeps it.
    press(&mut s, "BTN_FEEDBACK");
    assert_eq!(s.menu_name(), "feedback");
    let before = frame(&mut s);
    assert!(s.settings().minimap);
    let row = s.menu_pixel("LB_OPTIONS", Some(3), SIZE).unwrap();
    s.hover(row, SIZE);
    assert!(frame(&mut s) != before, "its words show");
    click_row(&mut s, "LB_OPTIONS", 3);
    assert!(!s.settings().minimap);
    assert!(std::fs::read_to_string(&file)
        .unwrap()
        .contains("minimap=false"));
    click_row(&mut s, "LB_OPTIONS", 0);
    assert!(s.settings().hide_unequippable);
    click_row(&mut s, "LB_OPTIONS", 2);
    assert!(!s.settings().subtitles);
    press(&mut s, "BTN_DEFAULT");
    assert!(s.settings().minimap && s.settings().subtitles);
    click_row(&mut s, "LB_OPTIONS", 3);
    click_row(&mut s, "LB_OPTIONS", 0);
    press(&mut s, "BTN_BACK");
    assert_eq!(s.menu_name(), "pause");

    // Auto-Pause: its boxes flip.
    press(&mut s, "BTN_AUTOPAUSE");
    assert_eq!(s.menu_name(), "autopause");
    assert!(s.settings().pause_enemy);
    press(&mut s, "CB_ENEMYSIGHTED");
    assert!(!s.settings().pause_enemy);
    assert!(s.settings().pause_party);
    press(&mut s, "BTN_DEFAULT");
    assert!(s.settings().pause_enemy);
    press(&mut s, "BTN_BACK");

    // Gameplay: Auto Level Up and Combat Movement.
    press(&mut s, "BTN_GAMEPLAY");
    press(&mut s, "CB_LEVELUP");
    assert!(s.settings().auto_level);
    press(&mut s, "CB_DISABLEMOVE");
    assert!(!s.settings().combat_movement);
    press(&mut s, "BTN_BACK");
    press(&mut s, "BTN_EXIT");
    assert_eq!(s.menu_name(), "-");

    // The small map's frame has gone, and the corner no longer opens the map.
    s.update(DT);
    let corner = s.hud_pixel("BTN_MINIMAP", SIZE).unwrap();
    s.click(corner, SIZE);
    assert_eq!(s.menu_name(), "-");

    // A level is taken by itself.
    let pc = s.game.world.pc;
    let level = s.game.world.object(pc).unwrap().level;
    s.game.world.grant_xp(pc, 1000);
    s.update(DT);
    assert_eq!(s.game.world.object(pc).unwrap().level, level + 1);
    assert_eq!(s.menu_name(), "-");

    // In a fight, the keys and the floor no longer move the player.
    let p = s.game.world.object(pc).unwrap().position;
    let foe = s
        .game
        .world
        .spawn_creature("n_calonord", [p[0] + 3.0, p[1], p[2]]);
    s.game.world.combat.target.insert(foe, pc);
    s.update(DT);
    assert!(s.game.world.steer.is_none());

    // What cannot be worn leaves the list.
    s.game.world.combat.target.clear();
    let armour = s.game.world.spawn_item("g_a_class4001");
    s.game.world.carry(pc, armour);
    s.game.world.object_mut(pc).unwrap().feats.clear();
    s.open_equip();
    press(&mut s, "BTN_INV_BODY");
    let view_rows = s.menu_pixel("LB_ITEMS", Some(1), SIZE);
    click_row(&mut s, "LB_ITEMS", 1);
    press(&mut s, "BTN_EQUIP");
    assert!(view_rows.is_some(), "the place is there, the row is not");
    assert!(!s.game.world.object(pc).unwrap().equipped.contains_key(&1));
    assert!(s.game.world.carried(pc).contains(&armour));
}

#[test]
fn the_slots_are_attacks_powers_and_grenades_and_blows_show_as_numbers() {
    let Some((mut s, _)) = in_game("slots") else {
        return;
    };
    let pc = s.game.world.pc;
    let p = s.game.world.object(pc).unwrap().position;
    let foe = s
        .game
        .world
        .spawn_creature("n_calonord", [p[0] + 0.1, p[1], p[2]]);
    let o = s.game.world.object_mut(foe).unwrap();
    o.faction = 1;
    (o.hp, o.max_hp) = (900, 900);
    let o = s.game.world.object_mut(pc).unwrap();
    o.feats.clear();
    o.spells.clear();
    s.update(DT);
    // No powers and nothing to throw: the middle and the right answer nothing. A click there falls on the floor, and at
    // a run that would take the player past the foe.
    for tag in ["BTN_TARGET1", "BTN_TARGET2"] {
        let slot = s.hud_pixel(tag, SIZE).unwrap();
        s.click(slot, SIZE);
        assert!(s.game.world.combat_queue.is_empty());
        s.game.world.pc_mover.stop();
    }
    assert!(!s.game.world.combat.target.contains_key(&pc));

    // With two feats, the left arrows go from the plain attack through them.
    let o = s.game.world.object_mut(pc).unwrap();
    o.feats.extend([28, 8]);
    o.spells.insert(23);
    (o.force, o.max_force) = (50, 50);
    s.update(DT);
    let first = frame(&mut s);
    let down = s.hud_pixel("BTN_TARGETDOWN0", SIZE).unwrap();
    s.click(down, SIZE);
    assert!(
        s.game.world.combat_queue.is_empty(),
        "an arrow queues nothing"
    );
    assert!(frame(&mut s) != first, "the first feat shows");
    let up = s.hud_pixel("BTN_TARGETUP0", SIZE).unwrap();
    s.click(up, SIZE);
    s.click(down, SIZE);

    // Under the cursor, a slot says what it does. The bar follows the target on the screen, so ask where it is now.
    let shown = frame(&mut s);
    let slot = s.hud_pixel("BTN_TARGET0", SIZE).unwrap();
    s.hover(slot, SIZE);
    assert!(frame(&mut s) != shown);
    let slot = s.hud_pixel("BTN_TARGET0", SIZE).unwrap();

    // A click on the left queues the feat and attacks, and the queue shows it.
    s.click(slot, SIZE);
    assert_eq!(s.game.world.combat_queue.len(), 1);
    assert!(matches!(
        s.game.world.combat_queue[0],
        kotor_game::CombatAction::Feat(_)
    ));
    assert_eq!(s.game.world.combat.target.get(&pc), Some(&foe));

    // The middle is the Force power.
    let power = s.hud_pixel("BTN_TARGET1", SIZE).unwrap();
    s.click(power, SIZE);
    assert_eq!(s.game.world.combat_queue.len(), 2);
    assert_eq!(
        s.game.world.combat_queue[1],
        kotor_game::CombatAction::Power(23)
    );

    // The right throws what the arrows chose, at once: nothing is queued, and one fewer is carried.
    let frag = s.game.world.spawn_item("g_w_fraggren01");
    let stun = s.game.world.spawn_item("g_w_stungren01");
    s.game.world.carry(pc, frag);
    s.game.world.carry(pc, stun);
    frame(&mut s);
    let order: Vec<_> = s
        .game
        .world
        .carried(pc)
        .into_iter()
        .filter(|i| *i == frag || *i == stun)
        .collect();
    let down = s.hud_pixel("BTN_TARGETDOWN2", SIZE).unwrap();
    s.click(down, SIZE);
    let right = s.hud_pixel("BTN_TARGET2", SIZE).unwrap();
    s.click(right, SIZE);
    assert_eq!(s.game.world.combat_queue.len(), 2);
    let carried = s.game.world.carried(pc);
    assert!(
        carried.contains(&order[0]),
        "the arrow moved past the first"
    );
    assert!(!carried.contains(&order[1]), "the second was thrown");

    // Blows show as words over the target, and the option turns them off.
    // Others on the ship fight too, so wait for the player's own.
    for _ in 0..600 {
        s.update(DT);
        if !s.floats_shown().is_empty() {
            break;
        }
    }
    assert!(!s.floats_shown().is_empty(), "{}", s.state_line());
    s.escape();
    press(&mut s, "BTN_FEEDBACK");
    click_row(&mut s, "LB_OPTIONS", 4);
    assert!(!s.settings().floating_numbers);
    press(&mut s, "BTN_BACK");
    press(&mut s, "BTN_EXIT");
    s.update(DT);
    assert!(s.floats_shown().is_empty());
}

#[test]
fn enemies_pause_the_game_and_space_goes_on() {
    let Some((mut s, _)) = in_game("pause") else {
        return;
    };
    let pc = s.game.world.pc;
    s.update(DT);
    // X pauses by hand, with no box, and the hourglass does the same.
    let time = s.game.world.time;
    s.pause_key();
    assert!(s.paused());
    let by_hand = frame(&mut s);
    assert_eq!(s.game.world.time, time, "the game waits");
    s.pause_key();
    assert!(!s.paused());
    s.update(DT);
    assert!(s.game.world.time > time);
    let hourglass = s.hud_pixel("TB_PAUSE", SIZE).unwrap();
    s.click(hourglass, SIZE);
    assert!(s.paused());
    s.click(hourglass, SIZE);
    assert!(!s.paused());

    // By itself only when asked: the first sight of each enemy pauses, with a box.
    let p = s.game.world.object(pc).unwrap().position;
    let foe = s
        .game
        .world
        .spawn_creature("n_calonord", [p[0] + 9.0, p[1], p[2]]);
    s.game.world.object_mut(foe).unwrap().faction = 1;
    s.game.world.combat.perceived.insert((pc, foe));
    s.update(DT);
    assert!(!s.paused(), "off unless asked for");
    s.enable_auto_pause(true);
    s.update(DT);
    assert!(s.paused());
    assert!(frame(&mut s) != by_hand, "the box says why");
    s.pause_key();
    assert!(!s.paused());
    s.update(DT);
    assert!(!s.paused(), "the same enemy does not pause again");

    // Each box of the page has its say: with Enemy Sighted off, another enemy goes by.
    s.escape();
    press(&mut s, "BTN_AUTOPAUSE");
    press(&mut s, "CB_ENEMYSIGHTED");
    press(&mut s, "CB_ENDROUND");
    press(&mut s, "BTN_BACK");
    press(&mut s, "BTN_EXIT");
    assert_eq!(s.menu_name(), "-");
    let other = s
        .game
        .world
        .spawn_creature("n_calonord", [p[0] + 9.5, p[1], p[2]]);
    s.game.world.object_mut(other).unwrap().faction = 1;
    s.game.world.combat.perceived.insert((pc, other));
    s.update(DT);
    assert!(!s.paused());

    // The end of a round pauses, now that it is ticked.
    s.game.world.combat.target.insert(pc, other);
    s.game.world.combat.round += 1;
    s.update(DT);
    assert!(s.paused());
    s.pause_key();

    // A conversation lifts a pause.
    s.game.world.combat.target.clear();
    s.pause_key();
    assert!(s.paused());
    let trask = s.game.world.by_tag("end_trask")[0];
    s.game.world.talk_to(trask);
    s.update(DT);
    if s.game.world.in_dialogue() {
        assert!(!s.paused());
    }
}

#[test]
fn space_never_pauses_in_a_conversation_nor_just_after_one() {
    let Some((mut s, _)) = in_game("space") else {
        return;
    };
    let trask = s.game.world.by_tag("end_trask")[0];
    s.game.world.talk_to(trask);
    for _ in 0..400 {
        s.update(DT);
        if s.game.world.in_dialogue() {
            break;
        }
    }
    assert!(s.game.world.in_dialogue());
    // However many presses it takes to get through, none of them pauses.
    for _ in 0..4000 {
        if !s.game.world.in_dialogue() {
            break;
        }
        s.jump_key();
        s.number_key(1);
        assert!(!s.paused());
        s.update(DT);
    }
    assert!(!s.game.world.in_dialogue());
    // The press after the last line does nothing.
    s.pause_key();
    assert!(!s.paused());
    // A moment later X pauses again.
    for _ in 0..40 {
        s.update(DT);
    }
    s.pause_key();
    assert!(s.paused());
    s.pause_key();
    assert!(!s.paused());
}

#[test]
fn the_keys_can_be_changed_kept_and_put_back() {
    let Some((mut s, dir)) = in_game("keys") else {
        return;
    };
    let file = dir.join("settings.txt");
    s.use_settings_file(file.clone());
    assert_eq!(s.action_for_key("KeyU"), Some("Equip"));
    assert_eq!(s.action_for_key("KeyW"), Some("MoveForward"));
    assert_eq!(s.action_for_key("KeyP"), None);
    s.escape();
    press(&mut s, "BTN_GAMEPLAY");
    press(&mut s, "BTN_KEYMAP");
    assert_eq!(s.menu_name(), "keymap");

    // Movement first, then Game: Use, Change Leader, Equipment.
    let movement = frame(&mut s);
    press(&mut s, "BTN_Filter_Game");
    assert!(frame(&mut s) != movement);
    // Mini Games has nothing, and stays on the page shown.
    let game = frame(&mut s);
    press(&mut s, "BTN_Filter_Mini");
    s.hover((1.0, 1.0), SIZE);
    assert!(frame(&mut s) != movement);
    let _ = game;

    // A click on a row waits for a key. Esc gives up, and the key stays.
    click_row(&mut s, "LST_EventList", 2);
    assert!(s.capture_key("Escape"));
    assert_eq!(s.action_for_key("KeyU"), Some("Equip"));
    assert!(!s.capture_key("KeyQ"), "not waiting now");

    // A new key takes over.
    click_row(&mut s, "LST_EventList", 2);
    assert!(s.capture_key("KeyP"));
    assert_eq!(s.action_for_key("KeyP"), Some("Equip"));
    assert_eq!(s.action_for_key("KeyU"), None);
    // One in use swaps: Inventory's I goes to Equip, and P to Inventory.
    click_row(&mut s, "LST_EventList", 2);
    assert!(s.capture_key("KeyI"));
    assert_eq!(s.action_for_key("KeyI"), Some("Equip"));
    assert_eq!(s.action_for_key("KeyP"), Some("Inventory"));

    // Cancel puts back what the page found, and Esc does the same.
    press(&mut s, "BTN_Cancel");
    assert_eq!(s.menu_name(), "gameplay");
    assert_eq!(s.action_for_key("KeyU"), Some("Equip"));
    press(&mut s, "BTN_KEYMAP");
    press(&mut s, "BTN_Filter_Game");
    click_row(&mut s, "LST_EventList", 2);
    s.capture_key("KeyP");
    s.escape();
    assert_eq!(s.menu_name(), "gameplay");
    assert_eq!(s.action_for_key("KeyU"), Some("Equip"));

    // OK keeps it, in the file too, and a new stage reads it.
    press(&mut s, "BTN_KEYMAP");
    press(&mut s, "BTN_Filter_Game");
    click_row(&mut s, "LST_EventList", 2);
    s.capture_key("KeyP");
    press(&mut s, "BTN_Accept");
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("key.Equip=KeyP"), "{text}");
    let mut again = at_menu().unwrap();
    again.use_settings_file(file.clone());
    assert_eq!(again.action_for_key("KeyP"), Some("Equip"));

    // Default gives the first keys back.
    press(&mut s, "BTN_KEYMAP");
    press(&mut s, "BTN_Default");
    assert_eq!(s.action_for_key("KeyU"), Some("Equip"));
    assert_eq!(s.action_for_key("KeyP"), None);

    // Then the same button is called Original, and gives the game's own keys, from its table.
    let ours = frame(&mut s);
    press(&mut s, "BTN_Default");
    s.hover((1.0, 1.0), SIZE);
    assert!(frame(&mut s) != ours);
    for (key, action) in [
        ("KeyP", "Character"),
        ("KeyL", "Quests"),
        ("KeyZ", "MoveLeft"),
        ("KeyC", "MoveRight"),
        ("KeyA", "TurnLeft"),
        ("KeyD", "TurnRight"),
        ("KeyW", "MoveForward"),
        ("KeyS", "MoveBack"),
        ("KeyR", "Use"),
        // F is Cancel Combat too, in a fight: see the end of this test.
        ("KeyF", "Pickup"),
        ("KeyO", "Options"),
        ("KeyU", "Equip"),
        ("KeyI", "Inventory"),
        ("KeyM", "Map"),
        ("Tab", "ChangeLeader"),
        ("KeyX", "Pause"),
        ("Space", "Jump"),
    ] {
        assert_eq!(s.action_for_key(key), Some(action), "{key}");
    }
    assert_eq!(s.action_for_key("ArrowLeft"), None);
    // It is kept like any other, and the button goes back.
    press(&mut s, "BTN_Accept");
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("key.Character=KeyP"), "{text}");
    press(&mut s, "BTN_KEYMAP");
    press(&mut s, "BTN_Default");
    press(&mut s, "BTN_Accept");
    assert_eq!(s.action_for_key("KeyC"), Some("Character"));
    assert_eq!(s.action_for_key("ArrowLeft"), Some("TurnLeft"));

    // The Cancel Combat key is the Disengage button.
    press(&mut s, "BTN_BACK");
    press(&mut s, "BTN_EXIT");
    let pc = s.game.world.pc;
    let p = s.game.world.object(pc).unwrap().position;
    let foe = s
        .game
        .world
        .spawn_creature("n_calonord", [p[0] + 3.0, p[1], p[2]]);
    s.game.world.combat.target.insert(pc, foe);
    s.update(DT);
    assert_eq!(s.action_for_key("KeyF"), Some("CancelCombat"));
    s.hud_disengage();
    assert_eq!(s.game.world.combat.target.get(&pc), None);
    assert_eq!(s.action_for_key("KeyF"), Some("Pickup"));
}

#[test]
fn mouse_sensitivity_turns_the_camera_faster_and_buttons_can_swap() {
    let Some((mut s, dir)) = in_game("mouse") else {
        return;
    };
    s.use_settings_file(dir.join("settings.txt"));
    let turn = |s: &mut Stage| {
        let before = s.camera().eye;
        s.look(40.0, 0.0);
        let after = s.camera().eye;
        (after[0] - before[0]).hypot(after[1] - before[1])
    };
    let normal = turn(&mut s);
    s.escape();
    press(&mut s, "BTN_GAMEPLAY");
    press(&mut s, "BTN_MOUSE");
    assert_eq!(s.menu_name(), "mouse");
    assert_eq!(s.settings().mouse_sensitivity, 0.5);
    let bar = s.menu_pixel("SLI_MOUSESEN", None, SIZE).unwrap();
    s.hover((bar.0 + 200.0, bar.1), SIZE);
    s.click((bar.0 + 200.0, bar.1), SIZE);
    let high = s.settings().mouse_sensitivity;
    assert!(high > 0.7, "{high}");
    press(&mut s, "CB_REVBUTTONS");
    assert!(s.settings().reverse_buttons);
    // The words about it show under the cursor.
    s.hover((1.0, 1.0), SIZE);
    let plain = frame(&mut s);
    let tick = s.menu_pixel("CB_REVBUTTONS", None, SIZE).unwrap();
    s.hover(tick, SIZE);
    assert!(frame(&mut s) != plain);
    press(&mut s, "BTN_DEFAULT");
    assert!(!s.settings().reverse_buttons);
    assert_eq!(s.settings().mouse_sensitivity, 0.5);
    let bar = s.menu_pixel("SLI_MOUSESEN", None, SIZE).unwrap();
    s.click((bar.0 + 200.0, bar.1), SIZE);
    press(&mut s, "BTN_BACK");
    press(&mut s, "BTN_BACK");
    press(&mut s, "BTN_EXIT");
    assert_eq!(s.menu_name(), "-");
    assert!(turn(&mut s) > normal);
}

#[test]
fn vsync_is_the_advanced_graphics_box_that_works_and_the_rest_say_why_not() {
    let Some((mut s, dir)) = in_game("vsync") else {
        return;
    };
    let file = dir.join("settings.txt");
    s.use_settings_file(file.clone());
    assert_eq!(
        s.take_wanted_vsync(),
        Some(true),
        "the saved choice goes to the window"
    );
    s.escape();
    press(&mut s, "BTN_GRAPHICS");

    // A greyed box does nothing, and its words say why.
    let plain = frame(&mut s);
    let shadows = s.menu_pixel("CB_SHADOWS", None, SIZE).unwrap();
    s.hover(shadows, SIZE);
    assert!(frame(&mut s) != plain);
    s.click(shadows, SIZE);
    assert_eq!(s.menu_name(), "graphics");

    // V-Sync goes to the window, and OK keeps it.
    press(&mut s, "BTN_ADVANCED");
    assert_eq!(s.menu_name(), "graphics_advanced");
    assert!(s.menu_pixel("BTN_ANTIALIASLEFT", None, SIZE).is_none());
    press(&mut s, "CB_VSYNC");
    assert!(!s.settings().vsync);
    assert_eq!(s.take_wanted_vsync(), Some(false));
    assert!(std::fs::read_to_string(&file)
        .unwrap()
        .contains("vsync=false"));
    press(&mut s, "BTN_BACK");
    assert_eq!(s.menu_name(), "graphics");
    assert!(!s.settings().vsync);

    // Cancel puts back what the page found.
    press(&mut s, "BTN_ADVANCED");
    press(&mut s, "CB_VSYNC");
    assert!(s.settings().vsync);
    s.take_wanted_vsync();
    press(&mut s, "BTN_CANCEL");
    assert!(!s.settings().vsync);
    assert_eq!(s.take_wanted_vsync(), Some(false));
    // Default is on.
    press(&mut s, "BTN_ADVANCED");
    press(&mut s, "BTN_DEFAULT");
    assert!(s.settings().vsync);
}

#[test]
fn a_screen_or_the_hud_changes_to_another_member_with_the_bag() {
    let Some((mut s, _)) = in_game("switch") else {
        return;
    };
    let w = &mut s.game.world;
    for id in w.party.clone() {
        w.object_mut(id).unwrap().party_member = false;
    }
    w.party.clear();
    w.npc_slots.clear();
    w.available.clear();
    w.available.insert(2, "n_calonord".to_string());
    w.available.insert(6, "n_calonord".to_string());
    let hero = w.pc;

    // Alone there is nobody to change to.
    s.open_inventory();
    assert!(s.menu_pixel("BTN_CHANGE1", None, SIZE).is_none());
    s.escape();

    let w = &mut s.game.world;
    w.choose_party(&[2, 6], "");
    let (first, second) = (w.party[0], w.party[1]);
    let medpack = w.spawn_item("g_i_medeqpmnt01");
    w.carry(hero, medpack);
    let bag = w.carried(hero);
    assert!(!bag.is_empty());
    let worn = w.spawn_item("g_a_class4001");
    w.object_mut(hero).unwrap().equipped.insert(1, worn);

    // Each of the three screens has the portraits, and a click changes to that member, bag and all.
    s.open_inventory();
    assert!(s.menu_pixel("BTN_CHANGE2", None, SIZE).is_some());
    press(&mut s, "BTN_CHANGE1");
    assert_eq!(s.game.world.pc, first);
    let now = s.game.world.carried(first);
    assert!(bag.iter().all(|i| now.contains(i)), "{now:?} has {bag:?}");
    assert!(s.game.world.carried(hero).is_empty());
    assert_eq!(s.menu_name(), "inventory");
    s.open_equip();
    press(&mut s, "BTN_INV_BODY");
    press(&mut s, "BTN_CHANGE1");
    assert_ne!(
        s.game.world.pc, first,
        "the list closes and the next one leads"
    );
    assert_eq!(s.menu_name(), "equip");
    s.open_character();
    assert_eq!(s.menu_name(), "character");
    press(&mut s, "BTN_CHANGE1");
    s.escape();
    assert_eq!(s.menu_name(), "-");

    // What is worn stays on who wears it.
    assert_eq!(
        s.game.world.object(hero).unwrap().equipped.get(&1),
        Some(&worn)
    );

    // The HUD's small portraits do the same.
    s.update(DT);
    let leader = s.game.world.pc;
    let portrait = s.hud_pixel("BTN_CHAR3", SIZE).unwrap();
    s.click(portrait, SIZE);
    assert_ne!(s.game.world.pc, leader);
    s.update(DT);
    assert!(s.game.world.party.contains(&leader));
    let _ = second;

    // A level waits for whoever earns it, leading or not.
    let member = s.game.world.party[0];
    let o = s.game.world.object_mut(member).unwrap();
    (o.level, o.xp) = (1, 0);
    s.game.world.grant_xp(member, 90000);
    assert_eq!(s.game.world.object(member).unwrap().level, 1);
    assert!(s.game.world.level_ready(member));
}

#[test]
fn the_sheet_and_the_level_panel_have_a_figure_behind_a_hole() {
    let Some((mut s, _)) = in_game("figure") else {
        return;
    };
    s.update(DT);
    assert!(s.figure_camera().is_none(), "not in the game");
    let (_, _, alone) = s.frame_scene();
    assert!(!alone);

    // On the sheet the picture has a hole where the figure stands, and only the leader is drawn.
    s.open_character();
    s.update(DT);
    let o = s.overlay().unwrap();
    let at = |x: u32, y: u32| ((y * o.width + x) * 4 + 3) as usize;
    let (hx, hy) = (184 * SIZE.0 / 640, 257 * SIZE.1 / 480);
    assert_eq!(o.rgba[at(hx, hy)], 0, "the middle of the hole");
    assert_eq!(o.rgba[at(5 * SIZE.0 / 640 + 2, 2)], 255, "the rest stays");
    let pc = s.game.world.pc;
    let here = s.game.world.object(pc).unwrap().position;
    let (scene, camera, alone) = s.frame_scene();
    assert!(alone);
    assert!(!scene.meshes.is_empty());
    let whole = s.dynamic_scene();
    assert!(scene.meshes.len() < whole.meshes.len(), "only the leader");
    // The camera stands before them, away from their feet, and looks their way.
    let away = (camera.eye[0] - here[0]).hypot(camera.eye[1] - here[1]);
    assert!(away > 1.0, "{away}");
    let looks = (camera.target[0] - here[0]).hypot(camera.target[1] - here[1]);
    assert!(looks < away, "{looks} {away}");
    s.escape();

    // Other screens draw the game behind them as before.
    s.open_inventory();
    assert!(s.figure_camera().is_none());
    s.escape();

    // The level-up panel has one as well.
    s.game.world.grant_xp(pc, 1000);
    s.open_character();
    press(&mut s, "BTN_LEVELUP");
    assert!(s.figure_camera().is_some());
    s.update(DT);
    let o = s.overlay().unwrap();
    let (hx, hy) = (236 * SIZE.0 / 640, 182 * SIZE.1 / 480);
    assert_eq!(o.rgba[((hy * o.width + hx) * 4 + 3) as usize], 0);
}

/// The owner's Nilbin had his head cut off by the top of the sheet's hole. Whatever the body, all of it stands inside, with
/// at least 3% of the hole to spare on each side, at every size of window.
#[test]
fn every_body_stands_inside_the_hole_of_its_screen() {
    let Some((mut s, _)) = in_game("figure_fits") else {
        return;
    };
    let pc = s.game.world.pc;
    let inside = |s: &mut Stage, hole: [f32; 4], what: &str| {
        let [left, top, width, height] = hole;
        let (scene, camera, alone) = s.frame_scene();
        assert!(alone && !scene.meshes.is_empty(), "{what}");
        let (mut x0, mut x1, mut y0, mut y1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for p in scene.meshes.iter().flat_map(|m| &m.positions) {
            let [x, y] = s.figure_on_window(&camera, *p);
            // In the 640 by 480 that the holes are given in.
            let (x, y) = ((x + 1.0) * 320.0, (1.0 - y) * 240.0);
            (x0, x1, y0, y1) = (x0.min(x), x1.max(x), y0.min(y), y1.max(y));
        }
        let (mx, my) = (width * 0.03, height * 0.03);
        assert!(
            x0 >= left + mx && x1 <= left + width - mx && y0 >= top + my && y1 <= top + height - my,
            "{what}: {x0:.1} to {x1:.1} and {y0:.1} to {y1:.1} in {hole:?}"
        );
        y1 - y0
    };
    // T3-M4, HK-47, Bastila, Canderous, Carth, Zaalbar, Mission, and the player's bodies, female and male, three sizes each.
    let bodies = [2, 3, 4, 5, 6, 9, 10, 91, 92, 93, 136, 137, 138];
    let mut sizes = Vec::new();
    for size in [
        (1280, 720),
        (1920, 1080),
        (2560, 1440),
        (3840, 2160),
        (1024, 768),
    ] {
        s.set_view(size);
        s.open_character();
        for &body in &bodies {
            s.game.world.objects.get_mut(&pc).unwrap().appearance = body;
            let what = format!("body {body} at {size:?}");
            // The idle moves them about a little.
            let mut high = 0.0f32;
            for _ in 0..3 {
                high = inside(&mut s, [55.0, 110.0, 259.0, 295.0], &what);
                for _ in 0..40 {
                    s.update(DT);
                }
            }
            sizes.push((body, high));
        }
        s.escape();
    }
    // Everyone of a person's height nearly fills it, and a short one is not blown up to that.
    let t3 = sizes.iter().find(|x| x.0 == 2).unwrap().1;
    let zaalbar = sizes.iter().find(|x| x.0 == 9).unwrap().1;
    assert!(t3 < zaalbar * 0.9, "{t3} {zaalbar}");

    // The level-up panel, and the party screen.
    s.set_view(SIZE);
    s.game.world.objects.get_mut(&pc).unwrap().appearance = 138;
    s.game.world.grant_xp(pc, 1000);
    s.open_character();
    press(&mut s, "BTN_LEVELUP");
    inside(&mut s, [155.0, 89.0, 162.0, 186.0], "level-up");
    s.escape();
    s.escape();
    s.game.world.available.insert(8, "p_zaalbar".to_string());
    s.open_party();
    press(&mut s, "BTN_NPC8");
    inside(
        &mut s,
        [326.0, 109.0, 258.0, 213.0],
        "Zaalbar on the party screen",
    );
}

#[test]
fn the_abilities_tab_lists_what_the_leader_has() {
    let Some((mut s, _)) = in_game("abilities") else {
        return;
    };
    let pc = s.game.world.pc;
    s.open_character();
    press(&mut s, "BTN_ABI");
    assert_eq!(s.menu_name(), "abilities");
    let skills = frame(&mut s);
    assert_eq!(s.game.world.skill_choices().len(), 8);

    // A row's words and numbers follow the cursor, from the sheet's ranks and modifier.
    let skills_had = &mut s.game.world.objects.get_mut(&pc).unwrap().skills;
    skills_had.resize(8, 0);
    skills_had[1] = 3;
    let p = s.menu_pixel("LB_ABILITY", Some(1), SIZE).unwrap();
    s.hover(p, SIZE);
    assert!(frame(&mut s) != skills);
    let modifier = s.game.world.skill_modifier(pc, 1);
    let int = s.game.world.object(pc).unwrap().abilities[3];
    assert_eq!(modifier, (int - 10).div_euclid(2));

    // A soldier has no powers, so that tab cannot be chosen. Give one and it can.
    assert!(s.game.world.owned_powers(pc).is_empty());
    press(&mut s, "BTN_FEATS");
    let feats = frame(&mut s);
    assert!(feats != skills);
    let owned = s.game.world.owned_feats(pc);
    assert!(!owned.is_empty());
    assert!(owned.iter().all(|c| c.owned && c.name > 0), "{owned:?}");
    press(&mut s, "BTN_POWERS");
    s.hover((0.0, 0.0), SIZE);
    assert_eq!(frame(&mut s), feats, "greyed");
    let spells = s.game.world.power_choices(pc);
    assert!(spells.is_empty(), "a soldier's list");
    s.game.world.objects.get_mut(&pc).unwrap().spells.insert(8);
    assert_eq!(s.game.world.owned_powers(pc).len(), 1);
    press(&mut s, "BTN_POWERS");
    assert!(frame(&mut s) != feats);

    // The tabs go on to the other screens, and Close closes.
    press(&mut s, "BTN_MSG");
    assert_eq!(s.menu_name(), "messages");
    press(&mut s, "BTN_ABI");
    press(&mut s, "BTN_EXIT");
    assert!(!s.menu_open());
    s.open_abilities();
    assert_eq!(s.menu_name(), "abilities");
    s.escape();
    s.open_messages();
    assert_eq!(s.menu_name(), "messages");
}

#[test]
fn the_messages_tab_shows_what_was_said_and_fought() {
    let Some((mut s, _)) = in_game("messages") else {
        return;
    };
    let pc = s.game.world.pc;
    s.open_messages();
    let words = s.messages_words();
    let said = s.game.world.transcript.clone();
    assert!(said.len() > 3);
    assert!(said.iter().all(|l| words.contains(&l.text)), "{words}");
    let last = frame(&mut s);

    // The wheel goes back through it, and no further than its start.
    let at = (0..400).find(|_| {
        s.menu_wheel(1.0);
        frame(&mut s) != last
    });
    assert!(at.is_some(), "more than a page has been said");
    s.menu_wheel(-1.0);
    assert_eq!(frame(&mut s), last);

    // The other list holds the party's attacks, with the talk table's words. The Jedi of the opening are not theirs.
    assert!(!s.game.world.combat.log.is_empty());
    press(&mut s, "BTN_SHOW");
    assert_eq!(s.messages_words(), "");
    let empty = frame(&mut s);
    assert!(empty != last);
    s.game.world.combat.log.push(kotor_game::AttackLog {
        round: 1,
        attacker: pc,
        target: pc,
        roll: 14,
        bonus: 5,
        defense: 12,
        outcome: kotor_game::Outcome::Hit,
        damage: 6,
        target_hp: 10,
    });
    let line = s.messages_words();
    assert!(line.contains("Hit, 14 +5 / 12"), "{line}");
    assert!(line.ends_with("Damage: 6"), "{line}");
    assert!(frame(&mut s) != empty);
    press(&mut s, "BTN_SHOW");
    assert_eq!(frame(&mut s), last);
}

#[test]
fn the_party_screen_shows_whoever_is_chosen_in_3d() {
    let Some((mut s, _)) = in_game("party_figure") else {
        return;
    };
    s.game.world.available.insert(2, "end_trask".to_string());
    s.open_party();
    s.update(DT);
    assert!(s.figure_camera().is_none(), "nobody is chosen yet");

    // Someone who is not here comes from their creature file.
    assert!(!s.game.world.npc_slots.contains_key(&2));
    press(&mut s, "BTN_NPC2");
    s.update(DT);
    let o = s.overlay().unwrap();
    let (hx, hy) = (455 * SIZE.0 / 640, 215 * SIZE.1 / 480);
    assert_eq!(o.rgba[((hy * o.width + hx) * 4 + 3) as usize], 0, "a hole");
    let (scene, camera, alone) = s.frame_scene();
    assert!(alone);
    assert!(!scene.meshes.is_empty());
    let pc = s.game.world.pc;
    let here = s.game.world.object(pc).unwrap().position;
    assert!((camera.eye[0] - here[0]).hypot(camera.eye[1] - here[1]) > 1.0);

    // Once with the party, they are here and look the same.
    press(&mut s, "BTN_ACCEPT");
    press(&mut s, "BTN_DONE");
    press(&mut s, "BTN_OK");
    assert!(s.game.world.npc_slots.contains_key(&2));
    s.open_party();
    press(&mut s, "BTN_NPC2");
    let (now, _, alone) = s.frame_scene();
    assert!(alone);
    assert_eq!(now.meshes.len(), scene.meshes.len());
}

#[test]
fn the_main_menu_has_its_scene_behind_a_hole_and_it_moves() {
    let Some(mut s) = at_menu() else { return };
    s.set_view(SIZE);
    s.update(DT);
    // A veil over the hole, not the picture that was there, and the logo and buttons over it.
    let o = s.overlay().unwrap();
    let alpha = o.rgba[3];
    assert!(alpha > 0 && alpha < 255, "{alpha}");
    let (first, camera, alone) = s.frame_scene();
    assert!(alone);
    assert!(!first.meshes.is_empty());
    assert!(first
        .meshes
        .iter()
        .any(|m| m.texture.as_deref() == Some("N_DarthMalak01")));
    assert!(camera.fov > 10.0 && camera.fov < 40.0, "{}", camera.fov);

    // Time moves him and fills the room with fog.
    for _ in 0..60 {
        s.update(DT);
    }
    let (later, _, _) = s.frame_scene();
    let moved = first
        .meshes
        .iter()
        .zip(&later.meshes)
        .any(|(a, b)| a.positions != b.positions);
    assert!(moved);
    assert!(!s.frame_particles().is_empty(), "the fog");

    // It belongs to this screen alone.
    press(&mut s, "BTN_OPTIONS");
    let (_, _, alone) = s.frame_scene();
    assert!(!alone);
    assert!(s.frame_particles().is_empty());
}

/// At the character's screens, from the main menu.
#[test]
fn a_new_game_has_early_force_when_the_viewer_allows_it_and_settings_say_so() {
    let Some(mut s) = at_menu() else {
        return;
    };
    s.enable_early_force(true);
    press(&mut s, "BTN_NEWGAME");
    let pc = s.game.world.pc;
    assert!(s.game.world.early_force);
    let o = s.game.world.object(pc).unwrap();
    assert!(o.spells.contains(&kotor_game::FORCE_PUSH));
    assert!(o.max_force >= 12);
    assert!(!s.game.world.combat_powers(pc).is_empty());

    // Tests and the pilot play the game's own rules, whatever Settings says.
    let Some(mut s) = at_menu() else {
        return;
    };
    press(&mut s, "BTN_NEWGAME");
    assert!(!s.game.world.early_force);
    let pc = s.game.world.pc;
    assert_eq!(s.game.world.object(pc).unwrap().max_force, 0);
}

fn at_chargen(name: &str) -> Option<Stage> {
    let mut s = at_menu()?;
    temp_saves(&mut s, name);
    s.enable_chargen(true);
    press(&mut s, "BTN_NEWGAME");
    assert_eq!(s.menu_name(), "chargen_class");
    Some(s)
}

#[test]
fn a_custom_character_goes_through_every_step_into_a_new_game() {
    let Some(mut s) = at_chargen("chargen_custom") else {
        return;
    };
    assert!(!s.in_game(), "no game yet");
    // Six figures, each in a hole, and the words follow the cursor.
    let (scene, _, alone) = s.frame_scene();
    assert!(alone);
    assert!(!scene.meshes.is_empty());
    let blank = frame(&mut s);
    assert!(!s.frame_particles().is_empty(), "the mist");
    let p = s.menu_pixel("BTN_SEL3", None, SIZE).unwrap();
    s.hover(p, SIZE);
    assert!(frame(&mut s) != blank);

    // A male Soldier. Quick or Custom first, then the steps in order.
    press(&mut s, "BTN_SEL3");
    assert_eq!(s.menu_name(), "chargen");
    let chosen = frame(&mut s);
    press(&mut s, "CUST_CHAR_BTN");
    assert!(frame(&mut s) != chosen, "the steps' panel");
    press(&mut s, "BTN_STEPNAME3");
    assert_eq!(s.menu_name(), "chargen", "skills wait for the portrait");

    // The portrait: the arrows change it, Cancel puts it back, and OK keeps it.
    press(&mut s, "BTN_STEPNAME1");
    assert_eq!(s.menu_name(), "chargen_portrait");
    let w = &s.game.world;
    let first = w.object(w.pc).unwrap().portrait.clone();
    press(&mut s, "BTN_ARRR");
    let w = &s.game.world;
    let next = w.object(w.pc).unwrap().portrait.clone();
    assert_ne!(first, next);
    press(&mut s, "BTN_BACK");
    let w = &s.game.world;
    assert_eq!(w.object(w.pc).unwrap().portrait, first);
    press(&mut s, "BTN_STEPNAME1");
    press(&mut s, "BTN_ARRR");
    press(&mut s, "BTN_ACCEPT");
    assert_eq!(s.menu_name(), "chargen");

    // Attributes: from 8 with 30 points. OK waits until they are spent. Recommended spends them.
    press(&mut s, "BTN_STEPNAME2");
    assert_eq!(s.menu_name(), "chargen_attributes");
    assert!(s.menu_pixel("STR_MINUS_BTN", None, SIZE).is_none());
    press(&mut s, "STR_PLUS_BTN");
    assert!(s.menu_pixel("STR_MINUS_BTN", None, SIZE).is_some());
    press(&mut s, "BTN_ACCEPT");
    assert_eq!(s.menu_name(), "chargen_attributes", "points are left");
    press(&mut s, "BTN_RECOMMENDED");
    assert!(
        s.menu_pixel("STR_PLUS_BTN", None, SIZE).is_none(),
        "all spent"
    );
    press(&mut s, "BTN_ACCEPT");
    assert_eq!(s.menu_name(), "chargen");

    // Skills and feats are the level-up's pages.
    press(&mut s, "BTN_STEPNAME3");
    assert_eq!(s.menu_name(), "level_skills");
    press(&mut s, "BTN_RECOMMENDED");
    press(&mut s, "BTN_ACCEPT");
    assert_eq!(s.menu_name(), "chargen");
    press(&mut s, "BTN_STEPNAME4");
    assert_eq!(s.menu_name(), "level_feats");
    press(&mut s, "BTN_RECOMMENDED");
    press(&mut s, "BTN_ACCEPT");
    assert_eq!(s.menu_name(), "chargen");
    let w = &mut s.game.world;
    let pc = w.pc;
    let (ranks, feats) = {
        let o = w.object(pc).unwrap();
        (o.skills.clone(), o.feats.clone())
    };
    assert!(ranks.iter().sum::<i32>() > 0);

    // The name is asked for, and the button makes one.
    press(&mut s, "BTN_STEPNAME5");
    assert_eq!(s.menu_name(), "chargen_name");
    assert!(s.typing());
    press(&mut s, "END_BTN");
    assert_eq!(s.menu_name(), "chargen_name", "a name is needed");
    press(&mut s, "BTN_RANDOM");
    for _ in 0..40 {
        s.backspace();
    }
    s.type_text("Revan");
    press(&mut s, "END_BTN");
    assert_eq!(s.menu_name(), "chargen");

    // Play: a game, with this character.
    press(&mut s, "BTN_STEPNAME6");
    assert_eq!(s.menu_name(), "-");
    assert!(s.in_game());
    let w = &s.game.world;
    let o = w.object(w.pc).unwrap();
    assert_eq!(o.name, "Revan");
    assert_eq!((o.class, o.level, o.gender), (0, 1, 0));
    assert_eq!(o.portrait, next);
    assert_eq!(o.skills, ranks);
    assert_eq!(o.feats, feats);
    assert_eq!(o.feat_slots, (0, 0));
    assert!(o.max_hp > 0);
}

#[test]
fn quick_character_takes_what_the_class_recommends() {
    let Some(mut s) = at_chargen("chargen_quick") else {
        return;
    };
    // A female Scoundrel. Her frame is the small one.
    press(&mut s, "BTN_SEL6");
    press(&mut s, "QUICK_CHAR_BTN");
    press(&mut s, "BTN_STEPNAME1");
    press(&mut s, "BTN_ACCEPT");
    press(&mut s, "BTN_STEPNAME2");
    s.type_text("Mira");
    press(&mut s, "END_BTN");
    press(&mut s, "BTN_STEPNAME3");
    assert_eq!(s.menu_name(), "-", "{}", s.menu_message());
    let w = &mut s.game.world;
    let pc = w.pc;
    let wanted = w.recommended_abilities(2).unwrap();
    let portrait_rows = w.player_portraits(1);
    let o = w.object(pc).unwrap().clone();
    assert_eq!(o.name, "Mira");
    assert_eq!((o.class, o.gender), (2, 1));
    assert_eq!(o.abilities, wanted);
    assert!(o.skills.iter().sum::<i32>() > 0, "{:?}", o.skills);
    assert_eq!(o.feat_slots, (0, 0));
    assert!(portrait_rows
        .iter()
        .any(|p| w.portrait_appearance(*p, kotor_game::BodySize::Small) == Some(o.appearance)));
}

#[test]
fn underwear_is_chosen_on_the_portrait_page_and_goes_with_the_character() {
    let on_offer: Vec<_> = kotor_render::underwear::list_underwear_variants()
        .into_iter()
        .filter(|v| v.fits)
        .collect();
    let Some(mut s) = at_chargen("chargen_underwear") else {
        return;
    };
    // A male body has no choice to make.
    press(&mut s, "BTN_SEL1");
    press(&mut s, "QUICK_CHAR_BTN");
    press(&mut s, "BTN_STEPNAME1");
    assert!(s.menu_pixel("BTN_UNDERWEAR_RIGHT", None, SIZE).is_none());
    s.escape();
    press(&mut s, "BTN_CANCEL");
    press(&mut s, "BTN_BACK");
    press(&mut s, "BTN_SEL6");
    press(&mut s, "QUICK_CHAR_BTN");
    press(&mut s, "BTN_STEPNAME1");
    if on_offer.len() < 2 {
        // Nothing to choose: no row.
        assert!(s.menu_pixel("BTN_UNDERWEAR_RIGHT", None, SIZE).is_none());
        return;
    }
    // The row changes the picture, and Cancel takes it back.
    let head = frame(&mut s);
    press(&mut s, "BTN_UNDERWEAR_RIGHT");
    assert_ne!(frame(&mut s), head, "its name is on the row");
    press(&mut s, "BTN_BACK");
    press(&mut s, "BTN_STEPNAME1");
    assert_eq!(frame(&mut s), head);
    // Taken, it is on the character the game starts with.
    press(&mut s, "BTN_UNDERWEAR_RIGHT");
    press(&mut s, "BTN_ACCEPT");
    press(&mut s, "BTN_STEPNAME2");
    s.type_text("Mira");
    press(&mut s, "END_BTN");
    press(&mut s, "BTN_STEPNAME3");
    assert_eq!(s.menu_name(), "-", "{}", s.menu_message());
    let pc = s.game.world.pc;
    assert_eq!(s.game.world.object(pc).unwrap().underwear, on_offer[1].id);
}

#[test]
fn the_underwear_chosen_comes_back_with_a_save_from_the_screens() {
    let on_offer: Vec<_> = kotor_render::underwear::list_underwear_variants()
        .into_iter()
        .filter(|v| v.fits)
        .collect();
    if on_offer.len() < 2 {
        return;
    }
    let Some(mut s) = at_chargen("chargen_underwear_save") else {
        return;
    };
    press(&mut s, "BTN_SEL6");
    press(&mut s, "QUICK_CHAR_BTN");
    press(&mut s, "BTN_STEPNAME1");
    press(&mut s, "BTN_UNDERWEAR_RIGHT");
    press(&mut s, "BTN_ACCEPT");
    press(&mut s, "BTN_STEPNAME2");
    s.type_text("Mira");
    press(&mut s, "END_BTN");
    press(&mut s, "BTN_STEPNAME3");
    for _ in 0..12000 {
        s.update(DT);
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        } else if s.game.world.transcript.len() > 3 && s.game.world.delayed.is_empty() {
            break;
        }
    }
    assert!(!s.game.world.in_dialogue(), "past the opening");
    let wears = |s: &Stage| {
        let pc = s.game.world.pc;
        s.game.world.object(pc).unwrap().underwear.clone()
    };
    assert_eq!(wears(&s), on_offer[1].id);
    // Written and read back by the buttons, in a world that has been wearing something else.
    s.escape();
    press(&mut s, "BTN_SAVEGAME");
    assert_eq!(s.menu_name(), "save");
    press(&mut s, "BTN_SAVELOAD");
    assert_eq!(s.menu_name(), "name");
    s.type_text("Blue");
    press(&mut s, "BTN_OK");
    assert_eq!(s.saves_listed(), 1, "{}", s.menu_message());
    press(&mut s, "BTN_BACK");
    press(&mut s, "BTN_EXIT");
    s.game.world.set_player_underwear("");
    assert_eq!(wears(&s), "");
    s.escape();
    press(&mut s, "BTN_LOADGAME");
    assert_eq!(s.menu_name(), "load");
    click_row(&mut s, "LB_GAMES", 0);
    press(&mut s, "BTN_SAVELOAD");
    assert_eq!(s.menu_name(), "-", "{}", s.menu_message());
    assert_eq!(wears(&s), on_offer[1].id, "{}", s.menu_message());
}

#[test]
fn a_party_member_at_the_cameras_eye_is_not_drawn() {
    let Some((mut s, _)) = in_game("at_the_eye") else {
        return;
    };
    let pc = s.game.world.pc;
    let p = s.game.world.object(pc).unwrap().position;
    let friend = s
        .game
        .world
        .spawn_creature("n_calonord", [p[0] + 30.0, p[1], p[2]]);
    s.game.world.object_mut(friend).unwrap().party_member = true;
    let eye = s.camera().eye;
    let foot = eye[2] - 1.7;
    let place = |s: &mut Stage, away: f32| {
        let o = s.game.world.object_mut(friend).unwrap();
        o.position = [eye[0] + away, eye[1], foot];
        s.update(DT);
        s.dynamic_scene().triangle_count()
    };
    let far = place(&mut s, 3.0);
    let at_eye = place(&mut s, 0.2);
    assert!(at_eye < far, "{at_eye} of {far} triangles");
    // A little further off they stay gone, so they do not flicker at the edge; then they come back.
    assert_eq!(place(&mut s, 0.6), at_eye, "still gone");
    assert_eq!(place(&mut s, 0.8), far, "back");
    // Someone who is not in the party is drawn wherever they stand.
    s.game.world.object_mut(friend).unwrap().party_member = false;
    assert_eq!(place(&mut s, 0.2), far);
}

#[test]
fn a_fresh_settings_file_has_invert_camera_off_and_the_mouse_going_down_looks_down() {
    let Some((mut s, dir)) = in_game("invert_fresh") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("no_such_settings.txt");
    let _ = std::fs::remove_file(&file);
    s.use_settings_file(file.clone());
    assert!(!s.settings().invert_y);
    let written = std::fs::read_to_string(&file).unwrap();
    assert!(written.contains("invert_y=false"), "{written}");
    assert!(!written.contains("invert_camera"), "{written}");
    assert!(!written.contains("mouse_look"), "{written}");
    let before = s.looks_down();
    s.look(0.0, 10.0);
    assert!(s.looks_down() > before);
    // The file read again says the same.
    let mut again = at_menu().unwrap();
    again.use_settings_file(file);
    assert!(!again.settings().invert_y);
}

#[test]
fn the_ways_back_from_the_characters_screens() {
    let Some(mut s) = at_chargen("chargen_back") else {
        return;
    };
    press(&mut s, "BTN_SEL1");
    press(&mut s, "CUST_CHAR_BTN");
    // Back undoes a step, and Cancel asks Quick or Custom again.
    press(&mut s, "BTN_STEPNAME1");
    press(&mut s, "BTN_ACCEPT");
    press(&mut s, "BTN_BACK");
    let step_one = frame(&mut s);
    press(&mut s, "BTN_BACK");
    assert_eq!(frame(&mut s), step_one, "nothing left to undo");
    press(&mut s, "BTN_CANCEL");
    assert!(s.menu_pixel("QUICK_CHAR_BTN", None, SIZE).is_some());
    // That panel's Cancel, and Esc, go back to the classes and the main menu, and no game has begun.
    press(&mut s, "BTN_BACK");
    assert_eq!(s.menu_name(), "chargen_class");
    s.escape();
    assert_eq!(s.menu_name(), "main");
    assert!(!s.in_game());
}

#[test]
fn a_random_name_comes_from_the_games_letter_tables() {
    let Some(mut s) = at_menu() else { return };
    for gender in [0, 1] {
        for _ in 0..20 {
            let name = s.game.world.random_name(gender);
            let parts: Vec<&str> = name.split(' ').collect();
            assert_eq!(parts.len(), 2, "{name}");
            for part in parts {
                assert!(part.len() >= 4, "{name}");
                assert!(part.chars().next().unwrap().is_ascii_uppercase(), "{name}");
            }
        }
    }
}

#[test]
fn quest_items_lists_what_is_carried_for_a_quest() {
    let Some((mut s, _)) = in_game("quest_items") else {
        return;
    };
    let w = &mut s.game.world;
    let pc = w.pc;
    let plain = w.spawn_item("g_i_medeqpmnt01");
    let quest = w.spawn_item("g_w_blstrrfl001");
    w.carry(pc, plain);
    w.carry(pc, quest);
    w.object_mut(quest).unwrap().plot = true;
    s.open_journal();
    press(&mut s, "BTN_QUESTITEMS");
    assert_eq!(s.menu_name(), "quest_items");
    let listed = frame(&mut s);
    // One row, and its words show when it is clicked.
    let second = s.menu_pixel("LB_ITEMS", Some(1), SIZE).unwrap();
    s.hover(second, SIZE);
    assert_eq!(frame(&mut s), listed, "only that one");
    let row = s.menu_pixel("LB_ITEMS", Some(0), SIZE).unwrap();
    s.hover(row, SIZE);
    s.click(row, SIZE);
    s.hover((1.0, 1.0), SIZE);
    assert!(frame(&mut s) != listed, "its words");
    press(&mut s, "BTN_BACK");
    assert_eq!(s.menu_name(), "journal");
    // The inventory has the same button, and Esc leaves it.
    s.escape();
    s.open_inventory();
    press(&mut s, "BTN_QUESTITEMS");
    assert_eq!(s.menu_name(), "quest_items");
    s.escape();
    assert_eq!(s.menu_name(), "inventory");
}

#[test]
fn the_upgrade_bench_puts_a_crystal_in_a_lightsaber() {
    let Some((mut s, _)) = in_game("bench") else {
        return;
    };
    let w = &mut s.game.world;
    let pc = w.pc;
    let saber = w.spawn_item("g_w_lghtsbr01");
    let crystal = w.spawn_item("g_w_sbrcrstl01");
    w.carry(pc, saber);
    w.carry(pc, crystal);
    assert_eq!(w.bench_kind(saber), Some(kotor_game::BenchKind::Lightsaber));
    let before = w.item_properties(saber);

    // A script opens it: only the kind with something to upgrade can be chosen.
    s.game.world.upgrade_screen = true;
    s.update(DT);
    assert_eq!(s.menu_name(), "bench_kinds");
    press(&mut s, "BTN_UPGRADEITEMS");
    assert_eq!(s.menu_name(), "bench_kinds", "no kind chosen yet");
    press(&mut s, "BTN_MELEE");
    press(&mut s, "BTN_UPGRADEITEMS");
    assert_eq!(s.menu_name(), "bench_kinds", "nothing of that kind");
    press(&mut s, "BTN_LIGHTSABER");
    press(&mut s, "BTN_UPGRADEITEMS");
    assert_eq!(s.menu_name(), "bench_items");

    // The lightsaber, then a slot, then the crystal in its list.
    let row = s.menu_pixel("LB_ITEMS", Some(0), SIZE).unwrap();
    s.hover(row, SIZE);
    s.click(row, SIZE);
    press(&mut s, "BTN_UPGRADEITEM");
    assert_eq!(s.menu_name(), "bench");
    let (scene, _, alone) = s.frame_scene();
    assert!(alone);
    assert!(!scene.meshes.is_empty(), "the item in its hole");
    press(&mut s, "BTN_UPGRADE41");
    let row = s.menu_pixel("LB_ITEMS", Some(1), SIZE).unwrap();
    s.hover(row, SIZE);
    s.click(row, SIZE);

    // Cancel changes nothing.
    press(&mut s, "BTN_BACK");
    assert_eq!(s.menu_name(), "bench_items");
    assert!(s.game.world.carried(pc).contains(&crystal));
    assert!(s.game.world.object(saber).unwrap().upgrades.is_empty());

    // Assemble does: the crystal leaves the bag, and the property that waited for it is listed.
    press(&mut s, "BTN_UPGRADEITEM");
    press(&mut s, "BTN_UPGRADE41");
    let row = s.menu_pixel("LB_ITEMS", Some(1), SIZE).unwrap();
    s.hover(row, SIZE);
    s.click(row, SIZE);
    press(&mut s, "BTN_ASSEMBLE");
    assert_eq!(s.menu_name(), "bench_items");
    assert!(!s.game.world.carried(pc).contains(&crystal));
    assert_eq!(
        s.game.world.object(saber).unwrap().upgrades.get(&0),
        Some(&0)
    );
    assert!(s.game.world.item_properties(saber).len() > before.len());

    // A save keeps it.
    let dir = temp_place("bench_save");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("b.sav");
    s.game.world.save_to(&path, "bench").unwrap();
    s.game.world.load_from(&path).unwrap();
    let w = &s.game.world;
    let found = w.objects.values().any(|o| o.upgrades.get(&0) == Some(&0));
    assert!(found);

    // Taking it out gives it back.
    let saber = s
        .game
        .world
        .objects
        .values()
        .find(|o| !o.upgrades.is_empty())
        .map(|o| o.id)
        .unwrap();
    let pc = s.game.world.pc;
    s.game.world.remove_upgrade(pc, saber, 0);
    let back = s.game.world.bench_choices(pc, 0);
    assert_eq!(back.len(), 1);
}

#[test]
fn the_one_being_fought_stays_the_target_when_the_cursor_leaves() {
    let Some((mut s, _)) = in_game("fought_target") else {
        return;
    };
    let w = &mut s.game.world;
    let pc = w.pc;
    let here = w.object(pc).unwrap().position;
    let foe = w.spawn_creature("n_sithsoldier003", [here[0] + 40.0, here[1], here[2]]);
    s.hover((1.0, 1.0), SIZE);
    assert!(
        !s.state_line().contains("target=Sith"),
        "too far to be chosen"
    );
    s.game.world.combat.target.insert(pc, foe);
    assert!(s.state_line().contains("target=Sith"), "{}", s.state_line());
    s.game.world.object_mut(foe).unwrap().dead = true;
    assert!(!s.state_line().contains("target=Sith"), "and not once dead");
}

#[test]
fn a_conversations_words_are_blue_and_its_bars_have_nothing_else() {
    let Some(mut s) = at_menu() else { return };
    temp_saves(&mut s, "talk_colors");
    press(&mut s, "BTN_NEWGAME");
    let talking = (0..12000).find(|_| {
        s.update(DT);
        if s.state_line().contains("replies=0") {
            s.continue_line();
            false
        } else {
            true
        }
    });
    assert!(talking.is_some(), "{}", s.state_line());
    s.update(DT);
    let o = s.overlay().unwrap();
    let top_bar = (o.width * 90 * 4) as usize;
    let bar = &o.rgba[..top_bar];
    let bright = |p: &&[u8]| p[3] > 200 && p[2] > 180;
    assert!(bar.chunks(4).filter(bright).count() > 50, "the line");
    let white = bar
        .chunks(4)
        .filter(|p| p[0] > 200 && p[1] > 200 && p[3] > 200);
    assert_eq!(white.count(), 0, "no white words, and no caption");
}

#[test]
fn the_party_screens_ok_asks_first() {
    let Some((mut s, _)) = in_game("party_ok") else {
        return;
    };
    s.game.world.available.insert(2, "end_trask".to_string());
    s.open_party();
    press(&mut s, "BTN_NPC2");
    press(&mut s, "BTN_ACCEPT");
    let chosen = frame(&mut s);
    press(&mut s, "BTN_DONE");
    assert_eq!(s.menu_name(), "party");
    assert!(frame(&mut s) != chosen, "the question");
    assert!(!s.game.world.npc_slots.contains_key(&2), "nothing yet");
    // Cancel goes back to choosing, as Esc does.
    press(&mut s, "BTN_CANCEL");
    assert_eq!(s.menu_name(), "party");
    press(&mut s, "BTN_DONE");
    s.escape();
    assert_eq!(s.menu_name(), "party");
    assert!(!s.game.world.npc_slots.contains_key(&2));
    press(&mut s, "BTN_DONE");
    press(&mut s, "BTN_OK");
    assert_eq!(s.menu_name(), "-");
    assert!(s.game.world.npc_slots.contains_key(&2));
}

#[test]
fn x_ends_a_pause_that_began_just_after_a_talk() {
    let Some((mut s, _)) = in_game("pause_after_talk") else {
        return;
    };
    s.toggle_pause();
    assert!(s.paused());
    // The game's clock stands still in a pause, so it would never get past the wait after a talk.
    s.game.world.time = 0.0;
    s.pause_key();
    assert!(!s.paused());
    s.pause_key();
    assert!(!s.paused(), "and that wait still holds when not paused");
}

#[test]
fn the_pilot_hands_a_fight_over_to_the_player() {
    let Some(mut s) = at_menu() else { return };
    press(&mut s, "BTN_NEWGAME");
    s.pilot = Some(kotor_game::Autopilot::new());
    s.speed = 16.0;
    let arrived = (0..3000).find(|_| {
        s.update(DT);
        s.pilot_arrived()
    });
    assert!(arrived.is_some(), "{}", s.state_line());
    s.take_over_from_pilot(true);
    assert!(s.pilot.is_none());
    assert_eq!(s.speed, 1.0);
    assert!(s.game.world.hold_levels);
    assert!(s.game.world.party_screen);
    let pc = s.game.world.object(s.game.world.pc).unwrap();
    assert_eq!(pc.hp, pc.max_hp);
    assert!(s.seen_enemy().is_some());
    // The enemy it stopped for pauses the game, as a player's first sight of them does.
    s.update(DT);
    assert!(s.paused());
    assert!(s.state_line().contains("note=ENEMY"), "{}", s.state_line());
    // For a level, the player is put one kill of that enemy short of it.
    let id = s.seen_enemy().unwrap();
    let w = &s.game.world;
    let (xp, level) = {
        let o = w.object(w.pc).unwrap();
        (o.xp, o.level)
    };
    assert!(w.level_for_xp(xp + w.kill_xp(id)) <= level);
    s.one_kill_short_of_a_level();
    let w = &s.game.world;
    let short = w.object(w.pc).unwrap().xp;
    assert!(w.level_for_xp(short) <= level, "not there yet");
    assert!(
        w.level_for_xp(short + w.kill_xp(id)) > level,
        "that kill does it"
    );
    // A driver's player can be kept standing.
    s.toggle_pause();
    let pc = s.game.world.pc;
    s.game.world.object_mut(pc).unwrap().hp = 1;
    s.keep_player_standing(true);
    s.update(DT);
    let o = s.game.world.object(pc).unwrap();
    assert_eq!(o.hp, o.max_hp);
}

/// How many short sounds come out of the next update.
fn sounds(s: &mut Stage) -> usize {
    let cues = s.update(DT);
    cues.iter()
        .filter(|c| matches!(c, kotor_gpu::stage::Cue::Effect(b) if !b.is_empty()))
        .count()
}

#[test]
fn the_interface_makes_its_sounds() {
    let Some((mut s, _)) = in_game("sounds") else {
        return;
    };
    sounds(&mut s);
    assert_eq!(sounds(&mut s), 0, "nothing happening");

    // A screen opening, a new place under the cursor, a click, and a screen closing.
    s.open_inventory();
    assert!(sounds(&mut s) >= 1, "the panel opens");
    let exit = s.menu_pixel("BTN_EXIT", None, SIZE).unwrap();
    s.hover(exit, SIZE);
    assert!(sounds(&mut s) >= 1, "the cursor enters a button");
    s.hover(exit, SIZE);
    assert_eq!(sounds(&mut s), 0, "and stays on it");
    s.hover((1.0, 1.0), SIZE);
    sounds(&mut s);
    s.click(exit, SIZE);
    assert!(sounds(&mut s) >= 2, "the click, and the panel closing");

    // On the HUD too.
    s.update(DT);
    let map = s.hud_pixel("BTN_MAP", SIZE).unwrap();
    s.click(map, SIZE);
    assert!(sounds(&mut s) >= 2, "the click, and the panel opening");
    s.escape();
    sounds(&mut s);

    // A box has its own, and so does being refused.
    s.game
        .world
        .object_mut(s.game.world.pc)
        .unwrap()
        .feats
        .clear();
    let armour = s.game.world.spawn_item("g_a_class4001");
    let pc = s.game.world.pc;
    s.game.world.carry(pc, armour);
    s.open_equip();
    sounds(&mut s);
    press(&mut s, "BTN_INV_BODY");
    click_row(&mut s, "LB_ITEMS", 1);
    sounds(&mut s);
    press(&mut s, "BTN_EQUIP");
    assert!(sounds(&mut s) >= 2, "the click and the refusal");
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
    press(&mut s, "BTN_ABI");
    assert_eq!(s.menu_name(), "abilities");
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

/// Windows of each shape: letters take the most of the 640 wide layout in a 4:3 one.
const FIT_SIZES: [(u32, u32); 3] = [(1280, 720), (3840, 2160), (800, 600)];

/// What does not fit on the screen showing now, at each size.
fn words_too_big(s: &mut Stage, screen: &str, found: &mut Vec<String>) {
    for size in FIT_SIZES {
        s.words_fit();
        s.set_view(size);
        s.update(0.0);
        s.overlay();
        let (drawn, big) = s.words_fit();
        assert!(drawn > 0, "{screen} at {size:?} drew no words");
        found.extend(big.iter().map(|b| format!("{screen} {size:?}: {b}")));
    }
    s.set_view(SIZE);
}

#[test]
fn the_screens_before_a_game_keep_their_words_inside_their_boxes() {
    let Some(mut s) = at_chargen("fit_chargen") else {
        return;
    };
    let mut found = Vec::new();
    words_too_big(&mut s, "class", &mut found);
    press(&mut s, "BTN_SEL4");
    words_too_big(&mut s, "quick or custom", &mut found);
    press(&mut s, "CUST_CHAR_BTN");
    words_too_big(&mut s, "custom", &mut found);
    press(&mut s, "BTN_STEPNAME1");
    words_too_big(&mut s, "portrait", &mut found);
    press(&mut s, "BTN_ACCEPT");
    press(&mut s, "BTN_STEPNAME2");
    words_too_big(&mut s, "attributes", &mut found);
    press(&mut s, "BTN_RECOMMENDED");
    press(&mut s, "BTN_ACCEPT");
    press(&mut s, "BTN_STEPNAME3");
    words_too_big(&mut s, "skills", &mut found);
    press(&mut s, "BTN_RECOMMENDED");
    press(&mut s, "BTN_ACCEPT");
    press(&mut s, "BTN_STEPNAME4");
    words_too_big(&mut s, "feats", &mut found);
    press(&mut s, "BTN_RECOMMENDED");
    press(&mut s, "BTN_ACCEPT");
    press(&mut s, "BTN_STEPNAME5");
    press(&mut s, "BTN_RANDOM");
    words_too_big(&mut s, "name", &mut found);
    while s.menu_name() != "main" {
        s.escape();
    }
    words_too_big(&mut s, "main", &mut found);
    press(&mut s, "BTN_OPTIONS");
    words_too_big(&mut s, "options", &mut found);
    for page in ["BTN_GAMEPLAY", "BTN_GRAPHICS", "BTN_SOUND"] {
        press(&mut s, page);
        words_too_big(&mut s, page, &mut found);
        press(&mut s, "BTN_BACK");
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
}

#[test]
fn the_screens_of_a_game_keep_their_words_inside_their_boxes() {
    let Some((mut s, _dir)) = in_game("fit_game") else {
        return;
    };
    let mut found = Vec::new();
    s.escape();
    words_too_big(&mut s, "pause", &mut found);
    for page in ["BTN_FEEDBACK", "BTN_AUTOPAUSE"] {
        press(&mut s, page);
        words_too_big(&mut s, page, &mut found);
        press(&mut s, "BTN_BACK");
    }
    press(&mut s, "BTN_SAVEGAME");
    words_too_big(&mut s, "save", &mut found);
    press(&mut s, "BTN_BACK");
    s.escape();
    let pc = s.game.world.pc;
    for t in [
        "g_a_class5001",
        "g_w_blstrpstl001",
        "g_w_lngswrd01",
        "g_i_medeqpmnt01",
        "g_w_lghtsbr01",
    ] {
        let item = s.game.world.spawn_item(t);
        s.game.world.carry(pc, item);
    }
    s.open_inventory();
    click_row(&mut s, "LB_ITEMS", 1);
    words_too_big(&mut s, "inventory", &mut found);
    s.open_equip();
    press(&mut s, "BTN_INV_BODY");
    words_too_big(&mut s, "equipment", &mut found);
    s.escape();
    // Level 4 has an attribute point.
    s.game.world.grant_xp(pc, 6000);
    for _ in 0..2 {
        s.game.world.level_up(pc);
    }
    s.update(0.1);
    s.open_character();
    words_too_big(&mut s, "character", &mut found);
    press(&mut s, "BTN_LEVELUP");
    words_too_big(&mut s, "level", &mut found);
    press(&mut s, "BTN_STEPNAME1");
    words_too_big(&mut s, "level attributes", &mut found);
    while s.menu_name() != "-" {
        s.escape();
    }
    s.open_character();
    press(&mut s, "BTN_ABI");
    words_too_big(&mut s, "skills", &mut found);
    press(&mut s, "BTN_FEATS");
    words_too_big(&mut s, "abilities feats", &mut found);
    press(&mut s, "BTN_MSG");
    words_too_big(&mut s, "messages", &mut found);
    press(&mut s, "BTN_JOU");
    words_too_big(&mut s, "journal", &mut found);
    assert!(found.is_empty(), "{}", found.join("\n"));

    // A shop's lists, and its question.
    let Some((mut s, _, _)) = at_shop("fit_shop") else {
        return;
    };
    words_too_big(&mut s, "shop", &mut found);
    click_row(&mut s, "LB_SHOPITEMS", 1);
    words_too_big(&mut s, "shop item", &mut found);
    press(&mut s, "BTN_Accept");
    words_too_big(&mut s, "shop question", &mut found);
    press(&mut s, "BTN_CANCEL");
    press(&mut s, "BTN_Examine");
    words_too_big(&mut s, "shop sell", &mut found);
    assert!(found.is_empty(), "{}", found.join("\n"));

    // Pazaak: the wager, the side deck, and a table with a message.
    let Some(mut s) = at_pazaak("fit_pazaak") else {
        return;
    };
    words_too_big(&mut s, "wager", &mut found);
    press(&mut s, "BTN_WAGER");
    words_too_big(&mut s, "setup", &mut found);
    pazaak_side_deck(&mut s);
    press(&mut s, "BTN_ATEXT");
    words_too_big(&mut s, "table", &mut found);
    for _ in 0..30 {
        if s.menu_pixel("BTN_OK", None, SIZE).is_some() {
            break;
        }
        press(&mut s, "BTN_YTEXT");
    }
    words_too_big(&mut s, "message", &mut found);
    assert!(found.is_empty(), "{}", found.join("\n"));

    // The galaxy map, on each planet the story can have.
    let Some(mut s) = at_galaxy("fit_galaxy") else {
        return;
    };
    for planet in [
        "LBL_Planet_Dantooine",
        "LBL_Planet_Tatooine",
        "LBL_Planet_Kashyyyk",
    ] {
        press(&mut s, planet);
        words_too_big(&mut s, planet, &mut found);
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
}

/// A game of Pazaak up to its table, with 500 credits and the most to wager 100.
fn at_pazaak(name: &str) -> Option<Stage> {
    let (mut s, _) = in_game(name)?;
    s.game.world.gold = 500;
    s.game.world.start_pazaak(1, "", 100, None);
    s.update(DT);
    Some(s)
}

fn pazaak_side_deck(s: &mut Stage) {
    for tag in [
        "BTN_AVAIL00",
        "BTN_AVAIL01",
        "BTN_AVAIL02",
        "BTN_AVAIL03",
        "BTN_AVAIL04",
    ] {
        for _ in 0..2 {
            press(s, tag);
            press(s, "BTN_YTEXT");
        }
    }
}

#[test]
fn pazaak_goes_from_wager_through_side_deck_to_the_table_by_its_buttons() {
    use kotor_game::PazaakView as V;
    let Some(mut s) = at_pazaak("pazaak") else {
        return;
    };
    assert_eq!(s.menu_name(), "pazaak");
    let time = s.game.world.time;
    frame(&mut s);
    assert_eq!(s.game.world.time, time, "the game waits");

    // The wager, up and down, and its words change with it.
    let wager = |s: &Stage| match s.game.world.pazaak_view() {
        Some(V::Wager { value, .. }) => value,
        other => panic!("{other:?}"),
    };
    let before = wager(&s);
    let shown = frame(&mut s);
    press(&mut s, "BTN_MORE");
    assert!(wager(&s) > before);
    assert!(frame(&mut s) != shown);
    press(&mut s, "BTN_LESS");
    assert_eq!(wager(&s), before);
    press(&mut s, "BTN_WAGER");

    // The side deck: none left is no button, a click picks, and Add Card takes it.
    assert!(matches!(s.game.world.pazaak_view(), Some(V::Setup { .. })));
    assert!(
        s.menu_pixel("BTN_AVAIL05", None, SIZE).is_none(),
        "no +6 owned"
    );
    press(&mut s, "BTN_YTEXT");
    let chosen = |s: &Stage| match s.game.world.pazaak_view() {
        Some(V::Setup { chosen, .. }) => chosen.len(),
        other => panic!("{other:?}"),
    };
    assert_eq!(chosen(&s), 0, "nothing picked yet");
    press(&mut s, "BTN_ATEXT");
    assert_eq!(chosen(&s), 0, "not ten yet");
    press(&mut s, "BTN_AVAIL00");
    press(&mut s, "BTN_YTEXT");
    assert_eq!(chosen(&s), 1);
    press(&mut s, "BTN_CHOSEN0");
    assert_eq!(chosen(&s), 0, "and goes back");
    pazaak_side_deck(&mut s);
    assert_eq!(chosen(&s), 10);
    press(&mut s, "BTN_ATEXT");

    // The table: a card drawn, only what has been played, and a hand of four.
    let table = |s: &Stage| match s.game.world.pazaak_view() {
        Some(V::Table { player, .. }) => *player,
        other => panic!("{other:?}"),
    };
    assert_eq!(table(&s).played.len(), 1);
    assert_eq!(table(&s).hand.len(), 4);
    assert!(s.menu_pixel("BTN_PLR0", None, SIZE).is_some());
    assert!(s.menu_pixel("BTN_PLR1", None, SIZE).is_none());
    assert!(
        s.menu_pixel("BTN_FLIP0", None, SIZE).is_none(),
        "no flip on a plain card"
    );
    press(&mut s, "BTN_PLRSIDE0");
    assert_eq!(table(&s).hand.len(), 3);
    assert_eq!(table(&s).played.len(), 2);
    press(&mut s, "BTN_XTEXT");

    // Esc does not leave a game.
    s.escape();
    assert_eq!(s.menu_name(), "pazaak");

    // Played out, the wager moves and the game goes on.
    for _ in 0..500 {
        if s.game.world.pazaak.is_none() {
            break;
        }
        match s.menu_pixel("BTN_OK", None, SIZE) {
            Some(ok) => {
                s.hover(ok, SIZE);
                s.click(ok, SIZE);
            }
            None => press(&mut s, "BTN_YTEXT"),
        }
    }
    assert!(s.game.world.pazaak.is_none());
    assert_eq!(s.menu_name(), "-");
    let won = s.game.world.last_pazaak_result == 1;
    assert_eq!(
        s.game.world.gold,
        if won { 500 + before } else { 500 - before }
    );
    s.update(DT);
    assert!(s.game.world.time > time);
}

#[test]
fn quit_and_esc_leave_pazaak_before_the_table_and_cost_nothing() {
    let Some(mut s) = at_pazaak("pazaak_quit") else {
        return;
    };
    press(&mut s, "BTN_QUIT");
    assert_eq!(s.menu_name(), "-");
    assert_eq!(s.game.world.gold, 500);
    let Some(mut s) = at_pazaak("pazaak_esc") else {
        return;
    };
    press(&mut s, "BTN_WAGER");
    s.escape();
    assert_eq!(s.menu_name(), "-");
    assert_eq!(s.game.world.gold, 500);
}

fn at_galaxy(name: &str) -> Option<Stage> {
    let (mut s, _) = in_game(name)?;
    // Taris, where we are, Dantooine and Tatooine to go to, and Kashyyyk not yet. Manaan is not on the map.
    let w = &mut s.game.world;
    for (row, selectable) in [(1, true), (3, true), (4, true), (5, false)] {
        w.set_planet_available(row, true);
        w.set_planet_selectable(row, selectable);
    }
    w.galaxy = Some(kotor_game::GalaxyOpen { current: 1 });
    s.update(DT);
    Some(s)
}

#[test]
fn the_galaxy_map_shows_what_the_story_allows_and_travel_runs_the_stock_script() {
    let Some(mut s) = at_galaxy("galaxy") else {
        return;
    };
    assert_eq!(s.menu_name(), "galaxy");
    let time = s.game.world.time;
    frame(&mut s);
    assert_eq!(s.game.world.time, time, "the game waits");
    assert_eq!(s.galaxy_shown(), Some(1));

    // Off the map: no button. Where we are: nowhere to go.
    assert!(s.menu_pixel("LBL_Planet_Manaan", None, SIZE).is_none());
    assert!(s.menu_pixel("LBL_Planet_Korriban", None, SIZE).is_none());
    press(&mut s, "BTN_ACCEPT");
    assert_eq!(s.menu_name(), "galaxy");
    assert_eq!(s.game.world.selected_planet, -1);

    // A click shows a planet, with its words.
    let taris = frame(&mut s);
    press(&mut s, "LBL_Planet_Dantooine");
    assert_eq!(s.galaxy_shown(), Some(3));
    assert!(frame(&mut s) != taris, "its name and words");
    let (scene, _, alone) = s.frame_scene();
    assert!(alone && !scene.meshes.is_empty(), "the planet is drawn");
    assert!(!s.frame_particles().is_empty(), "the galaxy is drawn");

    // It can be looked at and not gone to.
    press(&mut s, "LBL_Planet_Kashyyyk");
    assert_eq!(s.galaxy_shown(), Some(5));
    press(&mut s, "BTN_ACCEPT");
    assert_eq!(s.menu_name(), "galaxy");

    // Travel chooses it, closes the map, and `k_sup_galaxymap` starts a module.
    press(&mut s, "LBL_Planet_Tatooine");
    s.game.world.transition_request = None;
    press(&mut s, "BTN_ACCEPT");
    assert_eq!(s.menu_name(), "-");
    assert_eq!(s.game.world.selected_planet, 4);
    assert!(s.game.world.galaxy.is_none());
    let w = &s.game.world;
    assert!(
        w.transition_request.is_some(),
        "the script went somewhere: {:?}",
        w.errors.last()
    );
}

#[test]
fn cancel_and_esc_leave_the_galaxy_map_with_nothing_chosen() {
    let Some(mut s) = at_galaxy("galaxy_cancel") else {
        return;
    };
    press(&mut s, "BTN_BACK");
    assert_eq!(s.menu_name(), "-");
    assert_eq!(s.game.world.selected_planet, -1);
    assert!(s.game.world.galaxy.is_none());
    assert!(s.game.world.transition_request.is_none());
    let Some(mut s) = at_galaxy("galaxy_esc") else {
        return;
    };
    s.escape();
    assert_eq!(s.menu_name(), "-");
    assert!(s.game.world.galaxy.is_none());
    s.update(DT);
    assert_eq!(s.menu_name(), "-", "it does not come back");
}

/// A shop with a cheap thing and a dear one, and some credits.
fn at_shop(name: &str) -> Option<(Stage, u32, u32)> {
    let (mut s, _) = in_game(name)?;
    let w = &mut s.game.world;
    let keeper = w.spawn_creature("n_calonord", [0.0; 3]);
    let cheap = w.spawn_item("g_i_medeqpmnt01");
    let dear = w.spawn_item("g_a_class5001");
    w.object_mut(keeper).unwrap().inventory = vec![cheap, dear];
    w.gold = 1000;
    w.store = Some(kotor_game::StoreOpen {
        store: keeper,
        mark_up: 100,
        mark_down: 50,
        can_buy: true,
        can_sell: true,
    });
    s.update(DT);
    Some((s, cheap, dear))
}

#[test]
fn a_shop_buys_and_sells_asks_over_a_dear_item_and_holds_the_game() {
    let Some((mut s, cheap, dear)) = at_shop("shop") else {
        return;
    };
    let pc = s.game.world.pc;
    assert_eq!(s.menu_name(), "store");
    let time = s.game.world.time;
    frame(&mut s);
    assert_eq!(s.game.world.time, time, "the game waits");

    // Nothing chosen: the button does nothing. A click chooses, and it buys one.
    press(&mut s, "BTN_Accept");
    assert_eq!(s.game.world.gold, 1000);
    let nothing = frame(&mut s);
    click_row(&mut s, "LB_SHOPITEMS", 0);
    assert!(frame(&mut s) != nothing, "its words and price show");
    press(&mut s, "BTN_Accept");
    let price = 1000 - s.game.world.gold;
    assert!(price > 0 && price <= 150, "{price}");
    let carried = |s: &Stage, item: u32| s.game.world.carried(pc).contains(&item);
    assert!(carried(&s, cheap), "not infinite, so that one comes over");
    assert_eq!(s.game.world.store_lines().len(), 1);

    // A dear one asks. Cancel and Esc keep the credits, OK buys it.
    click_row(&mut s, "LB_SHOPITEMS", 0);
    let credits = s.game.world.gold;
    let before = frame(&mut s);
    press(&mut s, "BTN_Accept");
    assert!(frame(&mut s) != before, "the question shows");
    press(&mut s, "BTN_CANCEL");
    assert_eq!(s.game.world.gold, credits);
    press(&mut s, "BTN_Accept");
    s.escape();
    assert_eq!(s.menu_name(), "store", "Esc answers the question and stays");
    assert_eq!(s.game.world.gold, credits);
    press(&mut s, "BTN_Accept");
    press(&mut s, "BTN_OK");
    assert!(s.game.world.gold < credits);
    assert!(carried(&s, dear));

    // Too dear says so, and no question.
    let w = &mut s.game.world;
    let also = w.spawn_item("g_a_class5001");
    let keeper = w.store.as_ref().unwrap().store;
    w.object_mut(keeper).unwrap().inventory.push(also);
    w.gold = 1;
    click_row(&mut s, "LB_SHOPITEMS", 0);
    press(&mut s, "BTN_Accept");
    assert_eq!(s.game.world.gold, 1);
    assert_eq!(s.menu_message(), "You do not have enough credits.");
    assert!(!carried(&s, also));

    // The other list: selling pays, and what is sold goes to the shop.
    press(&mut s, "BTN_Examine");
    assert!(s.menu_pixel("LB_SHOPITEMS", Some(0), SIZE).is_none());
    let sold = s.game.world.sell_lines()[0].clone();
    click_row(&mut s, "LB_INVITEMS", 0);
    press(&mut s, "BTN_Accept");
    assert_eq!(s.game.world.gold, 1 + sold.price);
    assert!(s
        .game
        .world
        .store_lines()
        .iter()
        .any(|l| l.name == sold.name));
    press(&mut s, "BTN_Examine");
    assert!(s.menu_pixel("LB_SHOPITEMS", Some(0), SIZE).is_some());

    // Close, and the game goes on.
    press(&mut s, "BTN_Cancel");
    assert!(s.game.world.store.is_none());
    assert_eq!(s.menu_name(), "-");
    s.update(DT);
    assert!(s.game.world.time > time);
}

#[test]
fn a_shop_that_only_buys_has_only_the_sell_list_and_esc_closes_it() {
    let Some((mut s, _, _)) = at_shop("shop_sell") else {
        return;
    };
    s.game.world.store.as_mut().unwrap().can_buy = false;
    assert!(s.menu_pixel("LB_SHOPITEMS", Some(0), SIZE).is_none());
    press(&mut s, "BTN_Examine");
    assert!(s.menu_pixel("LB_SHOPITEMS", Some(0), SIZE).is_none());
    words_too_big(&mut s, "shop", &mut Vec::new());
    s.escape();
    assert!(s.game.world.store.is_none());
    assert_eq!(s.menu_name(), "-");
}

/// The mist's colour, over all that is drawn of it.
fn mist_colour(s: &mut Stage) -> [f32; 3] {
    let mut sum = [0.0; 3];
    let mut count = 0.0;
    for batch in s.frame_particles() {
        for v in &batch.vertices {
            for (total, c) in sum.iter_mut().zip(v.color) {
                *total += c;
            }
            count += 1.0;
        }
    }
    assert!(count > 0.0, "there is mist");
    sum.map(|c| c / count)
}

/// How many clear pixels lie between the needle's point and the alignment bar, on the row through the needle's middle.
/// The sheet leaves a hole for the figure behind them, so what is clear there is the gap.
fn needle_gap(s: &mut Stage, size: (u32, u32)) -> usize {
    s.set_view(size);
    s.update(DT);
    let needle = s.alignment_needle().unwrap();
    let o = s.overlay().unwrap().clone();
    let (sx, sy) = (size.0 as f32 / 640.0, size.1 as f32 / 480.0);
    let y = ((needle[1] as f32 + needle[3] as f32 / 2.0) * sy) as usize;
    let solid = |x: usize| o.rgba[(y * size.0 as usize + x) * 4 + 3] > 100;
    // From inside the needle, back past its point: its own pixels, then any gap, then the bar's.
    let mut x = ((needle[0] as f32 + needle[2] as f32 * 0.7) * sx) as usize;
    while !solid(x) {
        x -= 1;
    }
    while solid(x) && x > 0 {
        x -= 1;
    }
    // Solid past the needle's own left: it runs straight into the bar.
    if (x as f32) < needle[0] as f32 * sx {
        return 0;
    }
    let mut gap = 0;
    while !solid(x) && gap < 200 {
        x -= 1;
        gap += 1;
    }
    gap
}

#[test]
fn the_needle_beside_the_alignment_bar_follows_the_alignment() {
    let Some((mut s, _)) = in_game("needle") else {
        return;
    };
    let pc = s.game.world.pc;
    s.open_character();
    s.set_view(SIZE);
    let mut at = |alignment: i32| {
        s.game.world.object_mut(pc).unwrap().alignment = alignment;
        s.update(DT);
        let n = s.alignment_needle().unwrap();
        n[1] as f32 + n[3] as f32 / 2.0
    };
    let places = [0, 20, 50, 80, 100].map(&mut at);
    assert!(
        places.windows(2).all(|p| p[0] > p[1]),
        "light is higher: {places:?}"
    );
    // Evenly along the bar, which runs about 152 to 352 down the screen.
    let (dark, light) = (places[0], places[4]);
    assert!(
        (dark - 352.0).abs() < 6.0 && (light - 152.0).abs() < 6.0,
        "{places:?}"
    );
    for (place, alignment) in places.iter().zip([0.0, 20.0, 50.0, 80.0, 100.0]) {
        let even = dark + (light - dark) * alignment / 100.0;
        assert!((place - even).abs() < 3.0, "{alignment}: {places:?}");
    }
}

#[test]
fn the_needle_rests_against_the_bar_at_every_size_and_alignment() {
    let Some((mut s, _)) = in_game("needle_gap") else {
        return;
    };
    let pc = s.game.world.pc;
    s.open_character();
    for size in [(1280, 720), (2560, 1440), (3840, 2160)] {
        for alignment in [0, 20, 50, 80, 100] {
            s.game.world.object_mut(pc).unwrap().alignment = alignment;
            let gap = needle_gap(&mut s, size);
            assert!(gap <= 1, "{gap} pixels at {size:?} and {alignment}");
        }
    }
    s.set_view(SIZE);
}

#[test]
fn a_number_key_picks_its_own_reply_and_goes_on_from_a_line_with_none() {
    let Some(mut s) = at_menu() else { return };
    press(&mut s, "BTN_NEWGAME");
    let (mut picked, mut went_on) = (false, false);
    for _ in 0..4000 {
        s.update(DT);
        let Some(view) = s.game.world.dialogue_view() else {
            if picked && went_on {
                break;
            }
            continue;
        };
        let said = s.game.world.transcript.len();
        if view.3.is_empty() {
            s.number_pressed(2);
            assert_eq!(s.game.world.transcript.len(), said, "only 1 goes on");
            s.number_pressed(1);
            went_on |= s.game.world.dialogue_view() != Some(view);
        } else {
            let n = view.3.len();
            s.number_pressed(n + 1);
            assert_eq!(s.game.world.transcript.len(), said, "no such reply");
            s.number_pressed(n);
            let heard = format!("{:?}", &s.game.world.transcript[said..]);
            let last = view.3[n - 1].split('[').next().unwrap().trim().to_string();
            assert!(heard.contains(&last), "{last} in {heard}");
            picked = true;
        }
    }
    assert!(picked && went_on);
}

#[test]
fn a_number_key_is_a_step_on_the_numbered_panels() {
    // Level-up: the first step is there, and one further on is not yet.
    let Some((mut s, _)) = in_game("number_level") else {
        return;
    };
    let pc = s.game.world.pc;
    s.game.world.grant_xp(pc, 6000);
    for _ in 0..2 {
        s.game.world.level_up(pc);
    }
    s.update(0.1);
    s.open_character();
    press(&mut s, "BTN_LEVELUP");
    assert_eq!(s.menu_name(), "levelup");
    s.number_pressed(3);
    assert_eq!(s.menu_name(), "levelup", "not yet");
    s.number_pressed(1);
    assert_ne!(s.menu_name(), "levelup");

    // Making a character: not before there are steps, and then the first.
    let Some(mut s) = at_chargen("number_chargen") else {
        return;
    };
    s.number_pressed(1);
    assert_eq!(s.menu_name(), "chargen_class", "no numbers here");
    press(&mut s, "BTN_SEL4");
    press(&mut s, "CUST_CHAR_BTN");
    assert_eq!(s.menu_name(), "chargen");
    s.number_pressed(3);
    assert_eq!(s.menu_name(), "chargen", "not yet");
    s.number_pressed(1);
    assert_eq!(s.menu_name(), "chargen_portrait");
}

#[test]
fn a_slot_under_the_cursor_says_its_key() {
    let Some((mut s, _)) = in_game("slot_words") else {
        return;
    };
    let pc = s.game.world.pc;
    let p = s.game.world.object(pc).unwrap().position;
    let foe = s
        .game
        .world
        .spawn_creature("n_calonord", [p[0] + 0.1, p[1], p[2]]);
    s.game.world.object_mut(foe).unwrap().faction = 1;
    s.update(DT);
    let plain = frame(&mut s);
    // The words run from the left slot across the other two, a little below their middles.
    let next = s.hud_pixel("BTN_TARGET1", SIZE).unwrap();
    let last = s.hud_pixel("BTN_TARGET2", SIZE).unwrap();
    let row = (next.1 + 1.5 * 21.0 - 1.5 * 29.5 + 1.5 * 7.0) as usize;
    let strip = |o: &[u8]| -> Vec<u8> {
        let w = SIZE.0 as usize;
        (row - 8..row + 8)
            .flat_map(|y| (next.0 as usize..last.0 as usize).map(move |x| (y * w + x) * 4))
            .map(|i| o[i])
            .collect()
    };
    let slot = s.hud_pixel("BTN_TARGET0", SIZE).unwrap();
    s.hover(slot, SIZE);
    let lit = frame(&mut s);
    assert!(strip(&lit) != strip(&plain), "the words show");
    s.hover((1.0, 300.0), SIZE);
    let (drawn, too_big) = {
        frame(&mut s);
        s.words_fit()
    };
    assert!(drawn > 0 && too_big.is_empty(), "{too_big:?}");
}

#[test]
fn keys_one_two_and_three_are_the_slots_under_an_enemy() {
    let Some((mut s, _)) = in_game("number_slots") else {
        return;
    };
    let pc = s.game.world.pc;
    // Nobody to fight: nothing.
    s.number_pressed(1);
    assert!(!s.game.world.combat.target.contains_key(&pc));

    let p = s.game.world.object(pc).unwrap().position;
    let foe = s
        .game
        .world
        .spawn_creature("n_calonord", [p[0] + 0.1, p[1], p[2]]);
    let o = s.game.world.object_mut(foe).unwrap();
    o.faction = 1;
    (o.hp, o.max_hp) = (900, 900);
    let grenade = s.game.world.spawn_item("g_w_adhsvgren001");
    s.game.world.carry(pc, grenade);
    let o = s.game.world.object_mut(pc).unwrap();
    o.spells.insert(23);
    (o.force, o.max_force) = (50, 50);
    s.update(DT);
    s.number_pressed(1);
    assert_eq!(s.game.world.combat.target.get(&pc), Some(&foe));
    s.number_pressed(2);
    assert_eq!(
        s.game.world.combat_queue.back(),
        Some(&kotor_game::CombatAction::Power(23))
    );
    s.number_pressed(3);
    assert!(!s.game.world.carried(pc).contains(&grenade));
}

#[test]
fn a_shift_from_the_games_script_shows_on_the_sheet_and_in_what_a_power_costs() {
    let Some((mut s, _)) = in_game("shift") else {
        return;
    };
    let pc = s.game.world.pc;
    s.game.world.object_mut(pc).unwrap().alignment = 50;
    s.open_character();
    let needle = s.alignment_needle().unwrap();
    s.game.world.run_script("k_act_lighthigh", pc).unwrap();
    s.update(DT);
    assert!(s.game.world.object(pc).unwrap().alignment > 50);
    assert_ne!(s.alignment_needle().unwrap(), needle);
    s.escape();

    // Choke, a dark power: at 100 it costs double what it does at 50, and the screen says so.
    s.game.world.objects.get_mut(&pc).unwrap().spells.insert(9);
    s.open_abilities();
    press(&mut s, "BTN_POWERS");
    s.game.world.object_mut(pc).unwrap().alignment = 50;
    let middle = frame(&mut s);
    s.game.world.object_mut(pc).unwrap().alignment = 100;
    assert!(frame(&mut s) != middle);
    let cost = s.game.world.spell_cost_and_range(9).unwrap().0;
    assert_eq!(s.game.world.power_cost(pc, 9), Some(cost * 2));
}

#[test]
fn solo_mode_asks_shows_its_button_and_scripts_take_people_off_the_party_screen() {
    let Some((mut s, _)) = in_game("solo") else {
        return;
    };
    assert_eq!(s.action_for_key("KeyV"), Some("SoloMode"));
    let button = s.hud_pixel("TB_SOLO", SIZE).unwrap();
    s.click(button, SIZE);
    assert_eq!(s.menu_name(), "-", "off, it does not show");

    // No leaves it off, and the game waits for the answer.
    s.solo_key();
    assert_eq!(s.menu_name(), "solo");
    words_too_big(&mut s, "solo", &mut Vec::new());
    s.escape();
    assert_eq!(s.menu_name(), "-");
    assert!(!s.game.world.solo_mode);

    // Yes turns it on, and its button shows and turns it off again.
    s.solo_key();
    press(&mut s, "BTN_OK");
    assert_eq!(s.menu_name(), "-");
    assert!(s.game.world.solo_mode);
    s.update(DT);
    s.click(button, SIZE);
    assert_eq!(s.menu_name(), "solo");
    press(&mut s, "BTN_OK");
    assert!(!s.game.world.solo_mode);

    // Someone a script has taken away cannot be chosen.
    s.game.world.available.insert(1, "p_carth".to_string());
    s.open_party();
    assert!(s.menu_pixel("BTN_NPC1", None, SIZE).is_some());
    s.escape();
    s.game.world.unselectable.insert(1);
    s.open_party();
    assert!(s.menu_pixel("BTN_NPC1", None, SIZE).is_none());
}

#[test]
fn the_mist_behind_the_figure_follows_the_alignment() {
    let Some((mut s, _)) = in_game("figure_mist") else {
        return;
    };
    let pc = s.game.world.pc;
    s.open_character();
    let at = |s: &mut Stage, alignment: i32| {
        s.game.world.object_mut(pc).unwrap().alignment = alignment;
        s.update(DT);
        mist_colour(s)
    };
    let light = at(&mut s, 100);
    assert!(light[2] > light[0] * 1.5, "blue: {light:?}");
    let video = at(&mut s, 65);
    assert!(video[2] > video[0] * 1.5, "still blue: {video:?}");
    let neutral = at(&mut s, 50);
    let spread = neutral.iter().cloned().fold(f32::MIN, f32::max)
        - neutral.iter().cloned().fold(f32::MAX, f32::min);
    assert!(spread < 0.02, "grey: {neutral:?}");
    let dark = at(&mut s, 0);
    assert!(dark[0] > dark[2] * 1.5, "red: {dark:?}");
    // It moves.
    let before = s.frame_particles();
    s.update(0.2);
    assert!(s.frame_particles() != before);
}

#[test]
fn a_power_too_dear_shows_greyed_and_a_click_does_nothing() {
    let Some(mut s) = at_menu() else {
        return;
    };
    s.enable_early_force(true);
    press(&mut s, "BTN_NEWGAME");
    for _ in 0..12000 {
        s.update(DT);
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        } else if s.game.world.transcript.len() > 3 && s.game.world.delayed.is_empty() {
            break;
        }
    }
    let pc = s.game.world.pc;
    let p = s.game.world.object(pc).unwrap().position;
    let foe = s
        .game
        .world
        .spawn_creature("n_calonord", [p[0] + 0.1, p[1], p[2]]);
    s.game.world.object_mut(foe).unwrap().faction = 1;
    s.game.world.object_mut(pc).unwrap().force = 0;
    let dear = frame(&mut s);
    let power = s.hud_pixel("BTN_TARGET1", SIZE).unwrap();
    s.click(power, SIZE);
    assert!(s.game.world.combat_queue.is_empty());
    s.game.world.pc_mover.stop();
    let o = s.game.world.object_mut(pc).unwrap();
    o.force = o.max_force;
    assert!(frame(&mut s) != dear, "with the Force for it, it lights up");
    let power = s.hud_pixel("BTN_TARGET1", SIZE).unwrap();
    s.click(power, SIZE);
    assert_eq!(
        s.game.world.combat_queue.front(),
        Some(&kotor_game::CombatAction::Power(kotor_game::FORCE_PUSH))
    );
}

#[test]
fn the_options_box_gives_the_build_and_the_fonts_notice() {
    let words = kotor_gpu::stage::about_words();
    assert!(words.starts_with(&kotor_gpu::build_line()), "{words}");
    assert!(words.contains("Noto Project Authors") && words.contains("Open Font License"));
}

#[test]
fn the_font_and_box_rows_change_the_picture_and_the_file_keeps_them() {
    let Some((mut s, dir)) = in_game("text_style") else {
        return;
    };
    let file = dir.join("settings.txt");
    s.use_settings_file(file.clone());
    assert!(s.settings().noto_font && s.settings().glow_boxes);
    s.escape();
    press(&mut s, "BTN_FEEDBACK");
    let both = frame(&mut s);

    // The font: it changes the picture, and comes back the same.
    click_row(&mut s, "LB_OPTIONS", 5);
    assert!(!s.settings().noto_font);
    let hover_away = s.menu_pixel("BTN_BACK", None, SIZE).unwrap();
    s.hover(hover_away, SIZE);
    let stock = frame(&mut s);
    assert!(stock != both, "the words are in another font");
    click_row(&mut s, "LB_OPTIONS", 5);
    assert!(s.settings().noto_font);
    s.hover(hover_away, SIZE);
    let back = frame(&mut s);
    assert!(back != stock, "the font came back");

    // The boxes: their row is the seventh, and six fit, so the wheel brings it up.
    s.menu_wheel(-1.0);
    click_row(&mut s, "LB_OPTIONS", 6);
    assert!(!s.settings().glow_boxes);
    let saved = std::fs::read_to_string(&file).unwrap();
    assert!(saved.contains("noto_font=true") && saved.contains("glow_boxes=false"));
    s.hover(hover_away, SIZE);
    assert!(frame(&mut s) != back, "its row says so");
    click_row(&mut s, "LB_OPTIONS", 5);
    let saved = std::fs::read_to_string(&file).unwrap();
    assert!(saved.contains("noto_font=false"));

    // Another run reads them back.
    s.use_settings_file(file.clone());
    assert!(!s.settings().noto_font && !s.settings().glow_boxes);

    // The last two rows are further down the list. Early Force asks a new game for it, and this game keeps what it has.
    let before = s.game.world.early_force;
    s.menu_wheel(-1.0);
    s.menu_wheel(-1.0);
    assert!(s.settings().conventional_underwear && s.settings().early_force);
    click_row(&mut s, "LB_OPTIONS", 7);
    click_row(&mut s, "LB_OPTIONS", 8);
    assert!(!s.settings().conventional_underwear && !s.settings().early_force);
    assert_eq!(s.game.world.early_force, before);
    let saved = std::fs::read_to_string(&file).unwrap();
    assert!(saved.contains("conventional_underwear=false") && saved.contains("early_force=false"));
    press(&mut s, "BTN_BACK");
    press(&mut s, "BTN_FEEDBACK");
    let top = s.menu_pixel("LB_OPTIONS", Some(0), SIZE).unwrap();
    s.hover(top, SIZE);
    assert!(frame(&mut s) != back, "the list starts from the top again");
}

/// Paced, what comes into view comes over a few frames, and a frame reads only a few pictures.
#[test]
fn a_frame_does_not_pay_for_all_that_comes_into_view() {
    let Some((mut s, _)) = in_game("paced") else {
        return;
    };
    s.pace_loading = true;
    s.update(DT);
    // Stock pictures, none read yet: past the frame's few, one with no pixels says to try again.
    let names = ["n_commm03", "n_commm04", "n_commm05", "n_walrusman01"];
    let widths: Vec<u32> = names
        .iter()
        .map(|n| s.texture_soon(n).map_or(0, |i| i.width))
        .collect();
    assert!(widths[0] > 0, "{widths:?}");
    assert_eq!(widths[3], 0, "the fourth waits: {widths:?}");
    s.update(DT);
    assert!(s.texture_soon(names[3]).is_some_and(|i| i.width > 0));
    let creatures = |s: &mut Stage| s.dynamic_scene().meshes.len();
    let first = creatures(&mut s);
    for _ in 0..240 {
        s.update(DT);
        creatures(&mut s);
    }
    let settled = creatures(&mut s);
    s.pace_loading = false;
    assert_eq!(settled, creatures(&mut s), "all of them come in the end");
    assert!(first <= settled);
}

/// Item 3 of the owner's list of 6 Nov: figures shown on screens with no game running stood in the rest pose.
/// However the leader was moving when the screen opened, the sheet's figure stands idle with nothing in the hands.
#[test]
fn the_sheet_shows_the_leader_idle_with_empty_hands() {
    let Some((mut s, _)) = in_game("sheet_idle") else {
        return;
    };
    let pc = s.game.world.pc;
    s.open_character();
    s.update(DT);
    let idle = s.figure_scene();
    let idle_reach = {
        let at = s.game.world.object(pc).unwrap().position;
        let all = idle.meshes.iter().flat_map(|m| m.positions.iter());
        all.fold(0.0f32, |most, p| {
            most.max((p[0] - at[0]).hypot(p[1] - at[1]))
        })
    };
    s.escape();
    // A pistol in the hand, and running.
    let pistol = s.game.world.spawn_item("g_w_blstrpstl001");
    s.game.world.carry(pc, pistol);
    s.game.world.equip(pc, pistol, None).unwrap();
    s.steer = (1.0, 0.0, false);
    for _ in 0..40 {
        s.update(DT);
    }
    let clip = s.game.clip(pc).expect("a clip").name;
    assert_ne!(clip, "pause1", "running");
    s.open_character();
    s.update(DT);
    let figure = s.figure_scene();
    assert_eq!(
        figure.meshes.len(),
        idle.meshes.len(),
        "the same body, with nothing more in its hands"
    );
    // A stride reaches further from the feet than standing does.
    let reach = |scene: &kotor_render::Scene, s: &Stage| {
        let at = s.game.world.object(pc).unwrap().position;
        let all = scene.meshes.iter().flat_map(|m| m.positions.iter());
        all.fold(0.0f32, |most, p| {
            most.max((p[0] - at[0]).hypot(p[1] - at[1]))
        })
    };
    let (now, standing) = (reach(&figure, &s), idle_reach);
    assert!((now - standing).abs() < 0.05, "{now} against {standing}");
}

#[test]
fn figures_on_the_party_screen_breathe() {
    let Some((mut s, _)) = in_game("party_idle") else {
        return;
    };
    s.game.world.available.insert(2, "end_trask".to_string());
    s.open_party();
    press(&mut s, "BTN_NPC2");
    s.update(DT);
    let (first, _, _) = s.frame_scene();
    for _ in 0..45 {
        s.update(DT);
    }
    let (later, _, _) = s.frame_scene();
    assert!(
        first
            .meshes
            .iter()
            .zip(&later.meshes)
            .any(|(a, b)| a.positions != b.positions),
        "the figure idles"
    );
}

/// The Override rows flip and the file keeps them. All are off to begin with, as the folder does not pass the gate with them on.
#[test]
fn the_override_rows_flip_and_start_off() {
    let Some((mut s, dir)) = in_game("override_rows") else {
        return;
    };
    let file = dir.join("settings.txt");
    s.use_settings_file(file.clone());
    let set = s.settings();
    assert!(
        !set.override_folder
            && !set.override_scripts
            && !set.override_tables
            && !set.override_screens
            && !set.override_talk
    );
    s.escape();
    press(&mut s, "BTN_FEEDBACK");
    for _ in 0..18 {
        s.menu_wheel(-1.0);
    }
    click_row(&mut s, "LB_OPTIONS", 19);
    click_row(&mut s, "LB_OPTIONS", 23);
    let set = s.settings();
    let flags = (
        set.override_folder,
        set.override_scripts,
        set.override_tables,
        set.override_screens,
        set.override_talk,
    );
    assert_eq!(flags, (true, false, false, false, true));
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("override_folder=true") && text.contains("override_talk=true"));
    assert!(text.contains("override_tables=false"));
}

/// The last row of Feedback is Seamless areas: it flips, the file keeps it, and with it off no door joins its neighbour.
#[test]
fn the_seamless_areas_row_flips_and_turns_the_joining_off() {
    let Some((mut s, dir)) = in_game("seamless_row") else {
        return;
    };
    let file = dir.join("settings.txt");
    s.use_settings_file(file.clone());
    assert!(s.settings().seamless_areas, "on to begin with");
    s.escape();
    press(&mut s, "BTN_FEEDBACK");
    for _ in 0..10 {
        s.menu_wheel(-1.0);
    }
    click_row(&mut s, "LB_OPTIONS", 11);
    assert!(!s.settings().seamless_areas);
    assert!(std::fs::read_to_string(&file)
        .unwrap()
        .contains("seamless_areas=false"));
    press(&mut s, "BTN_BACK");
    press(&mut s, "BTN_EXIT");
    s.update(DT);
    assert!(
        !s.game.world.seamless.enabled,
        "off: doors load as they did"
    );
    // And back on.
    s.escape();
    press(&mut s, "BTN_FEEDBACK");
    for _ in 0..10 {
        s.menu_wheel(-1.0);
    }
    click_row(&mut s, "LB_OPTIONS", 11);
    assert!(s.settings().seamless_areas);
    press(&mut s, "BTN_BACK");
    press(&mut s, "BTN_EXIT");
    s.update(DT);
    assert!(s.game.world.seamless.enabled);
}

/// The Cel shading row of Feedback goes round Off, Subtle, Strong; the file keeps it; and F7's toggle turns it off and
/// brings back the level it had.
#[test]
fn the_cel_shading_row_goes_round_and_the_key_toggles_it() {
    let Some((mut s, dir)) = in_game("cel_row") else {
        return;
    };
    let file = dir.join("settings.txt");
    s.use_settings_file(file.clone());
    let start = s.settings().cel_shading;
    s.escape();
    press(&mut s, "BTN_FEEDBACK");
    for _ in 0..30 {
        s.menu_wheel(-1.0);
    }
    click_row(&mut s, "LB_OPTIONS", 26);
    assert_eq!(s.settings().cel_shading, (start + 1) % 3);
    assert!(std::fs::read_to_string(&file)
        .unwrap()
        .contains(&format!("cel_shading={}", (start + 1) % 3)));
    press(&mut s, "BTN_BACK");
    press(&mut s, "BTN_EXIT");
    s.update(DT);
    // The key: off and back to the level it was, or on and off again when the row left it off.
    let level = s.cel_shading();
    s.toggle_cel_shading();
    if level > 0 {
        assert_eq!(s.cel_shading(), 0, "off");
        s.toggle_cel_shading();
        assert_eq!(s.cel_shading(), level, "back to what it was");
    } else {
        assert!(s.cel_shading() > 0, "on");
        s.toggle_cel_shading();
        assert_eq!(s.cel_shading(), 0, "off again");
    }
}
