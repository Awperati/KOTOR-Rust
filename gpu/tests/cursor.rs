//! When the window hides the cursor and holds it: the look button turning the camera, with no edge to stop the mouse.
use kotor_gpu::stage::Stage;

const DT: f32 = 1.0 / 60.0;

#[test]
fn the_look_button_holds_the_cursor_and_a_menu_replies_or_lost_focus_let_it_go() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let dir = std::env::temp_dir().join(format!("kotor_cursor_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = Stage::open("end_m01aa", true).unwrap();
    s.set_saves_dir(dir.join("saves"));
    s.use_settings_file(dir.join("settings.txt"));
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    s.set_view((1280, 720));
    // Trask's replies on screen: the cursor is there to choose one.
    let mut replies = false;
    for _ in 0..3000 {
        s.update(DT);
        if s.game
            .world
            .dialogue_view()
            .is_some_and(|v| !v.3.is_empty())
        {
            replies = true;
            break;
        }
        s.continue_line();
    }
    assert!(replies, "Trask asks");
    assert!(!s.cursor_held(true, true), "replies to click");
    for _ in 0..3000 {
        if !s.game.world.in_dialogue() && s.game.world.transcript.len() > 16 {
            break;
        }
        s.continue_line();
        s.number_key(1);
        s.update(DT);
    }
    for _ in 0..10 {
        s.update(DT);
    }
    assert!(!s.game.world.in_dialogue());
    // Modern, the default: the mouse looks by itself, held with no button, and only with the window in use.
    assert!(s.cursor_held(false, true), "Modern: no button needed");
    assert!(s.cursor_held(true, true));
    assert!(
        !s.cursor_held(false, false),
        "no focus: a driver's window, or the player went elsewhere"
    );
    // Caps Lock gives the cursor back, and takes it again. The button still pans.
    s.caps_lock_key();
    assert!(s.cursor_freed());
    assert!(!s.cursor_held(false, true), "Caps Lock: the cursor is back");
    assert!(s.cursor_held(true, true), "the button still pans");
    s.caps_lock_key();
    assert!(s.cursor_held(false, true));
    // A menu lets it go, and closing it takes it again.
    s.escape();
    s.update(DT);
    assert!(!s.cursor_held(false, true), "a menu");
    assert!(!s.cursor_held(true, true), "a menu");
    s.escape();
    s.update(DT);
    assert!(s.cursor_held(false, true));
    // The on-screen controls keep the cursor.
    s.toggle_touch();
    assert!(!s.cursor_held(false, true), "touch");
    s.toggle_touch();
    // Classic: the button pans, as ever, and Caps Lock does nothing.
    s.set_movement_modern(false);
    assert!(!s.cursor_held(false, true), "Classic");
    assert!(s.cursor_held(true, true));
    s.caps_lock_key();
    assert!(!s.cursor_freed());
    std::fs::remove_dir_all(&dir).ok();
}
