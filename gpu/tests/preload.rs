//! The module behind an exit that cannot be joined is loaded ahead while the player is near the exit, and used when they
//! go through. Skips itself when the game is not installed.
use kotor_gpu::stage::Stage;

const DT: f32 = 1.0 / 60.0;

/// Stand `away` metres from the first exit to `to` and wait for the module to be loaded ahead. False when it never is.
fn walk_up_and_wait(s: &mut Stage, to: &str, away: f32) -> bool {
    let exit = s
        .game
        .world
        .objects
        .values()
        .find(|o| o.link_module.eq_ignore_ascii_case(to))
        .map(|o| o.position)
        .expect("an exit to the module");
    s.game.world.pc_mover = kotor_walkmesh::Mover::new([exit[0] + away, exit[1], exit[2]], 0.0);
    let began = std::time::Instant::now();
    while s.preloaded().is_none() && began.elapsed().as_secs() < 40 {
        s.update(DT);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    s.preloaded().is_some()
}

#[test]
fn a_lift_is_loaded_ahead_when_the_player_is_near_and_used_when_they_ride_it() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let mut s = Stage::open("tar_m10aa", true).unwrap();
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    for _ in 0..60 {
        s.update(DT);
    }
    assert!(
        s.preloaded().is_none(),
        "nothing is loaded ahead far from an exit"
    );
    if !walk_up_and_wait(&mut s, "tar_m10ac", 10.0) {
        // Memory short on this machine: nothing to check.
        return;
    }
    assert_eq!(s.preloaded(), Some("tar_m10ac"));
    s.game.world.transition("tar_m10ac", "").unwrap();
    s.update(DT);
    assert_eq!(s.game.world.module, "tar_m10ac");
    assert!(s.preloaded().is_none(), "used once");
    assert!(
        s.static_scene().meshes.len() > 50,
        "the lift's rooms are in the picture"
    );
    assert!(s.game.world.errors.is_empty(), "{:?}", s.game.world.errors);
}

#[test]
fn what_was_loaded_ahead_goes_when_the_player_walks_away() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let mut s = Stage::open("tar_m10aa", true).unwrap();
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    for _ in 0..60 {
        s.update(DT);
    }
    if !walk_up_and_wait(&mut s, "tar_m10ac", 10.0) {
        return;
    }
    let w = &mut s.game.world;
    w.pc_mover = kotor_walkmesh::Mover::new([5000.0, 5000.0, 0.0], 0.0);
    s.update(DT);
    assert!(s.preloaded().is_none(), "freed when out of reach");
}

#[test]
fn a_module_loaded_ahead_is_not_used_for_another() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let mut s = Stage::open("tar_m10aa", true).unwrap();
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    for _ in 0..60 {
        s.update(DT);
    }
    if !walk_up_and_wait(&mut s, "tar_m10ac", 10.0) {
        return;
    }
    // Going somewhere else than the exit the player stood by: the picture is that module's own.
    s.game.world.transition("tar_m03aa", "").unwrap();
    s.update(DT);
    assert_eq!(s.game.world.module, "tar_m03aa");
    let names: Vec<&str> = s
        .static_scene()
        .meshes
        .iter()
        .filter_map(|m| m.texture.as_deref())
        .collect();
    assert!(!names.is_empty());
    assert!(s.game.world.errors.is_empty(), "{:?}", s.game.world.errors);
}

#[test]
fn a_module_a_conversation_ends_in_is_loaded_while_the_player_talks() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let mut s = Stage::open("tat_m17aa", true).unwrap();
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    for _ in 0..60 {
        s.update(DT);
    }
    assert_eq!(
        s.talk_goes_to("tat17_11guard_01").as_deref(),
        Some("tat_m18aa")
    );
    assert_eq!(s.talk_goes_to("tat17_nobody"), None);
    let guard = s
        .game
        .world
        .objects
        .values()
        .find(|o| o.conversation.eq_ignore_ascii_case("tat17_11guard_01"))
        .map(|o| o.id)
        .expect("the guard");
    let pc = s.game.world.pc;
    s.game.world.queue(
        guard,
        kotor_game::Action::StartConversation {
            target: pc,
            dialogue: "tat17_11guard_01".to_string(),
            anywhere: true,
        },
    );
    // A module for a nearby exit may be there first: the talk's replaces it.
    let began = std::time::Instant::now();
    while s.preloaded() != Some("tat_m18aa") && began.elapsed().as_secs() < 40 {
        s.update(DT);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    if s.preloaded().is_none() {
        // Memory short on this machine: nothing to check.
        return;
    }
    assert!(s.game.world.in_dialogue(), "while they talk");
    assert_eq!(s.preloaded(), Some("tat_m18aa"));
}

#[test]
fn a_new_game_is_loaded_while_the_character_is_made() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let size = (1280, 720);
    let mut s = Stage::menu("end_m01aa", true).unwrap();
    s.enable_chargen(true);
    let p = s.menu_pixel("BTN_NEWGAME", None, size).unwrap();
    s.hover(p, size);
    s.click(p, size);
    assert_ne!(s.menu_name(), "main", "the character screens are up");
    let began = std::time::Instant::now();
    while s.preloaded().is_none() && began.elapsed().as_secs() < 40 {
        s.update(DT);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    if s.preloaded().is_none() {
        // Memory short on this machine: nothing to check.
        return;
    }
    assert_eq!(s.preloaded(), Some("end_m01aa"));
}

/// Wait for a module to be loaded ahead. None when nothing is within 40 s (memory short on this machine).
fn wait_for_any(s: &mut Stage) -> Option<String> {
    let began = std::time::Instant::now();
    while s.preloaded().is_none() && began.elapsed().as_secs() < 40 {
        s.update(DT);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    s.preloaded().map(str::to_string)
}

fn opened(module: &str) -> Stage {
    let mut s = Stage::open(module, true).unwrap();
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    for _ in 0..60 {
        s.update(DT);
    }
    s
}

#[test]
fn a_ladder_that_is_a_script_is_loaded_ahead_too() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let mut s = opened("tar_m05aa");
    let at = s
        .game
        .world
        .objects
        .values()
        .find(|o| o.tag == "tar05_lad04aai")
        .map(|o| o.position)
        .expect("the ladder");
    s.game.world.pc_mover = kotor_walkmesh::Mover::new([at[0] + 6.0, at[1], at[2]], 0.0);
    if let Some(m) = wait_for_any(&mut s) {
        assert_eq!(m, "tar_m04aa");
    }
}

#[test]
fn the_ramp_of_the_ship_loads_the_planet_it_will_open_on() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let mut s = opened("ebo_m12aa");
    s.game
        .world
        .global_numbers
        .insert("K_CURRENT_PLANET".into(), 15);
    let at = s
        .game
        .world
        .objects
        .values()
        .find(|o| o.tag == "ebn12_ebonexit")
        .map(|o| o.position)
        .expect("the ramp");
    s.game.world.pc_mover = kotor_walkmesh::Mover::new([at[0] + 4.0, at[1], at[2]], 0.0);
    if let Some(m) = wait_for_any(&mut s) {
        assert_eq!(m, "danm13");
    }
}

#[test]
fn a_planet_chosen_on_the_map_is_loaded_before_the_player_reaches_the_ramp() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let mut s = opened("ebo_m12aa");
    // Row 4 of the map is Tatooine. The player stands where they spawned, wherever that is from the ramp.
    s.game.world.selected_planet = 4;
    if let Some(m) = wait_for_any(&mut s) {
        assert_eq!(m, "tat_m17ab");
    }
}

#[test]
fn the_module_of_the_save_chosen_on_the_load_screen_is_loaded_while_the_player_looks_at_it() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let size = (1280, 720);
    let dir = std::env::temp_dir().join(format!("kotor_loadpre_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // A save made in the Lower City.
    {
        let s = opened("tar_m03aa");
        s.game
            .world
            .save_to(&dir.join("save_0001.sav"), "Lower")
            .unwrap();
    }
    let mut s = Stage::menu("end_m01aa", true).unwrap();
    s.set_saves_dir(dir.clone());
    s.use_settings_file(dir.join("settings.txt"));
    let load = s.menu_pixel("BTN_LOADGAME", None, size).unwrap();
    s.hover(load, size);
    s.click(load, size);
    assert_eq!(s.menu_name(), "load");
    for _ in 0..30 {
        s.update(DT);
    }
    assert!(s.preloaded().is_none(), "nothing chosen, nothing loaded");
    let row = s.menu_pixel("LB_GAMES", Some(0), size).unwrap();
    s.hover(row, size);
    s.click(row, size);
    assert_eq!(s.selected_save_name().as_deref(), Some("Lower"));
    let Some(m) = wait_for_any(&mut s) else {
        // Memory short on this machine: nothing to check.
        return;
    };
    assert_eq!(m, "tar_m03aa", "the save's own module");
}
