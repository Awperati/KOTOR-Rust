//! The screens over the game, driven headless through the same clicks a window sends.
use kotor_gpu::stage::Stage;

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
    assert!(s.menu_pixel("BTN_WARP", None, SIZE).is_some());
    s.hover((1.0, 1.0), SIZE);
    s.update(DT);
    assert_eq!(s.overlay().unwrap().rgba, o.rgba, "and goes out again");

    // Options is not built: it says so and nothing changes. Hidden buttons do not take clicks.
    press(&mut s, "BTN_OPTIONS");
    assert_eq!(s.menu_name(), "main");
    let warp = s.menu_pixel("BTN_WARP", None, SIZE).unwrap();
    s.click(warp, SIZE);
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
    press(&mut s, "BTN_GRAPHICS");
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
