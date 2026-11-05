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
