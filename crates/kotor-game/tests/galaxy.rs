//! The galaxy map's rules, through the game's own scripts.

use kotor_archive::Install;
use kotor_game::World;

fn world() -> Option<World> {
    let dir = kotor_core::install_dir()?;
    let mut w = World::new(Install::open(dir).unwrap()).unwrap();
    w.load_module("ebo_m12aa").unwrap();
    Some(w)
}

#[test]
fn the_ships_script_says_which_planets_are_on_the_map_and_asks_for_it() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    assert!(w.planets().is_empty());
    // After the Jedi give the player the main quest, as `k_pebn_galaxy` has it.
    w.global_numbers.insert("K_KOTOR_MASTER".to_string(), 15);
    w.run_script("k_pebn_galaxy", pc).unwrap();
    assert!(w.galaxy.is_some(), "it asks for the map");
    for row in [3, 4, 5, 6, 7] {
        let line = w.planets().into_iter().find(|l| l.row == row);
        assert!(line.is_some_and(|l| l.available && l.selectable), "{row}");
    }
    assert!(!w.planets().iter().any(|l| l.row == 1), "Taris is gone");
}

#[test]
fn before_the_main_quest_only_dantooine_is_there() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    w.global_numbers.insert("K_KOTOR_MASTER".to_string(), 10);
    w.run_script("k_pebn_galaxy", pc).unwrap();
    let rows: Vec<i32> = w.planets().iter().map(|l| l.row).collect();
    assert_eq!(rows, [3]);
}

#[test]
fn travel_runs_the_stock_script_and_cancel_runs_nothing() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    w.global_numbers.insert("K_KOTOR_MASTER".to_string(), 15);
    w.run_script("k_pebn_galaxy", pc).unwrap();

    w.galaxy_close();
    assert!(w.galaxy.is_none());
    assert_eq!(w.selected_planet, -1);
    assert!(w.transition_request.is_none());

    w.run_script("k_pebn_galaxy", pc).unwrap();
    w.galaxy_choose(4, pc);
    assert!(w.galaxy.is_none());
    assert_eq!(w.selected_planet, 4);
    assert!(
        w.transition_request.is_some(),
        "it starts a module: {:?}",
        w.errors
    );
    assert_ne!(w.global_numbers.get("K_FUTURE_PLANET"), None);
}

#[test]
fn the_planets_come_back_with_a_save() {
    let Some(mut w) = world() else { return };
    let dir = kotor_core::install_dir().unwrap();
    w.set_planet_available(5, true);
    w.set_planet_selectable(5, false);
    w.set_planet_available(3, true);
    w.set_planet_selectable(3, true);
    let bytes = w.save_bytes("planets").unwrap();
    let mut again = World::new(Install::open(dir).unwrap()).unwrap();
    again.load_save(&bytes).unwrap();
    assert_eq!(again.planets(), w.planets());
}
