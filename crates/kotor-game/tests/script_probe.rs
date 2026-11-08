//! Where a script would send the party, found without playing it (`World::script_destination`).

use kotor_archive::Install;
use kotor_game::World;

fn hawk() -> Option<World> {
    let dir = kotor_core::install_dir()?;
    let mut w = World::new(Install::open(dir).unwrap()).unwrap();
    w.load_module("ebo_m12aa").unwrap();
    Some(w)
}

#[test]
fn the_ramp_of_the_ship_goes_where_the_current_planet_says() {
    let Some(mut w) = hawk() else { return };
    for (planet, module) in [
        (10, "tar_m02aa"),
        (15, "danm13"),
        (20, "kas_m22aa"),
        (25, "manm26ad"),
        (30, "korr_m33aa"),
        (35, "tat_m17ab"),
        (45, "unk_m41aa"),
        (50, "sta_m45aa"),
    ] {
        w.global_numbers.insert("K_CURRENT_PLANET".into(), planet);
        assert_eq!(
            w.script_destination("k_pebn_exithawk", -1).as_deref(),
            Some(module),
            "planet {planet}"
        );
    }
    // Looking changes nothing: no module is asked for, and the globals are as they were.
    assert!(w.transition_request.is_none());
    assert_eq!(w.global_numbers.get("K_CURRENT_PLANET"), Some(&50));
}

#[test]
fn a_ladder_and_a_door_that_fails_to_open_say_where_they_go() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut w = World::new(Install::open(dir).unwrap()).unwrap();
    w.load_module("tar_m05aa").unwrap();
    assert_eq!(
        w.script_destination("k_ptar_go04aai", -1).as_deref(),
        Some("tar_m04aa")
    );
}

#[test]
fn a_script_that_goes_nowhere_says_nothing() {
    let Some(mut w) = hawk() else { return };
    assert_eq!(w.script_destination("k_pebn_galaxy", -1), None);
    assert_eq!(w.script_destination("no_such_script", -1), None);
}

#[test]
fn choosing_a_planet_on_the_map_says_which_module_the_ramp_will_open() {
    let Some(mut w) = hawk() else { return };
    w.global_numbers.insert("K_KOTOR_MASTER".to_string(), 15);
    for (row, module) in [
        (3, "danm13"),
        (4, "tat_m17ab"),
        (5, "kas_m22aa"),
        (6, "manm26ad"),
        (7, "korr_m33aa"),
    ] {
        assert_eq!(
            w.planet_first_module(row, "k_pebn_exithawk").as_deref(),
            Some(module),
            "row {row}"
        );
    }
    // Nothing was done: no travel, no globals written.
    assert!(w.transition_request.is_none());
    assert_eq!(w.global_numbers.get("K_FUTURE_PLANET"), None);
    assert_eq!(w.global_numbers.get("K_CURRENT_PLANET"), None);
}

#[test]
fn a_trigger_that_asks_who_entered_before_it_starts_a_module_is_read_as_the_player() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut w = World::new(Install::open(dir).unwrap()).unwrap();
    w.load_module("unk_m41aa").unwrap();
    // `k_punk_goebon` stops at "is the one who entered the player?": the probe answers yes.
    assert_eq!(
        w.script_destination("k_punk_goebon", -1).as_deref(),
        Some("ebo_m41aa")
    );
    // Looking changes nothing.
    assert!(w.transition_request.is_none());
}
