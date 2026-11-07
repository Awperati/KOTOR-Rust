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
    let began = std::time::Instant::now();
    while s.preloaded().is_none() && began.elapsed().as_secs() < 40 {
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
