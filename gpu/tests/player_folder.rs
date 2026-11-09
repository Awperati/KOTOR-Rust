//! Only the game writes in the player's folder, `%LOCALAPPDATA%\kotor-rs`: a test, a driver or a probe that reads their
//! settings leaves the file as it was. A probe once rewrote it through `use_saved_settings`.
//!
//! The one test in this file points `LOCALAPPDATA` at a folder of its own, so the player's real one is never touched.
use kotor_gpu::stage::{default_settings_file, may_write, Stage};

#[test]
fn only_the_game_writes_the_players_settings_and_saves() {
    let local = std::env::temp_dir().join(format!("kotor_player_folder_{}", std::process::id()));
    let home = local.join("kotor-rs");
    std::fs::create_dir_all(&home).unwrap();
    let text = "minimap=false\nkey.Use=KeyR\n";
    std::fs::write(home.join("settings.txt"), text).unwrap();
    std::env::set_var("LOCALAPPDATA", &local);
    let file = default_settings_file().unwrap();
    assert_eq!(file, home.join("settings.txt"));

    // The folder and all in it, but nothing beside it.
    assert!(!may_write(&file));
    assert!(!may_write(&home));
    assert!(!may_write(&home.join("saves").join("autosave.sav")));
    assert!(!may_write(&home.join("Saves").join("save_0001.sav")));
    assert!(may_write(
        &local.join("kotor-rs-other").join("settings.txt")
    ));
    assert!(may_write(&std::env::temp_dir().join("kotor_settings.txt")));

    if kotor_core::install_dir().is_none() {
        std::fs::remove_dir_all(&local).ok();
        return;
    }
    // The player's settings are read, and a change to them is not written back.
    let mut s = Stage::menu("end_m01aa", true).unwrap();
    s.use_saved_settings();
    assert!(!s.settings().minimap, "read");
    s.set_game_mode(kotor_game::GameMode::Modern);
    s.update(1.0 / 60.0);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), text, "written");
    assert!(!home.join("saves").exists());
    // A file of the test's own is written as ever.
    let own = local.join("own_settings.txt");
    s.use_settings_file(own.clone());
    assert!(own.is_file());
    std::fs::remove_dir_all(&local).ok();
}
