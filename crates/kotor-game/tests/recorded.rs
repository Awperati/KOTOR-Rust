//! Routines that used to record themselves and do nothing, through the game's own scripts where a test can reach them.

use kotor_archive::Install;
use kotor_game::{CannotEquip, Kind, World};

fn world(module: &str) -> Option<World> {
    let dir = kotor_core::install_dir()?;
    let mut w = World::new(Install::open(dir).unwrap()).unwrap();
    w.load_module(module).unwrap();
    Some(w)
}

fn saved_and_back(w: &World) -> World {
    let bytes = w.save_bytes("recorded").unwrap();
    let mut again = World::new(Install::open(kotor_core::install_dir().unwrap()).unwrap()).unwrap();
    again.load_save(&bytes).unwrap();
    again
}

#[test]
fn the_star_forges_script_turns_solo_mode_on_and_it_stays_with_a_save() {
    let Some(mut w) = world("unk_m44ab") else {
        return;
    };
    let pc = w.pc;
    assert!(!w.solo_mode);
    w.run_script("k_punk_sparty03", pc).unwrap();
    w.run_script("k_punk_sparty", pc).unwrap();
    assert!(w.solo_mode, "{:?}", w.errors);
    assert!(saved_and_back(&w).solo_mode);
}

#[test]
fn in_solo_mode_the_party_stays_where_it_is() {
    let Some(mut w) = world("end_m01aa") else {
        return;
    };
    let pc = w.pc;
    // Someone in the party, a long way behind.
    let friend = w
        .objects
        .values()
        .find(|o| o.kind == Kind::Creature && o.id != pc)
        .map(|o| o.id)
        .unwrap();
    w.party.push(friend);
    w.solo_mode = true;
    let where_they_were = w.object(friend).unwrap().position;
    for _ in 0..60 {
        w.step(0.1);
    }
    assert_eq!(w.object(friend).unwrap().position, where_they_were);
}

#[test]
fn a_script_can_stop_an_item_going_on() {
    let Some(mut w) = world("end_m01aa") else {
        return;
    };
    let pc = w.pc;
    let armour = w.spawn_item("g_i_belt001");
    assert_eq!(w.can_wear(pc, armour), Ok(()));
    w.object_mut(armour).unwrap().non_equippable = true;
    assert_eq!(w.can_wear(pc, armour), Err(CannotEquip::Prerequisites));
}

#[test]
fn map_notes_go_and_come_as_scripts_say_and_stay_with_a_save() {
    let Some(mut w) = world("end_m01aa") else {
        return;
    };
    let notes = w.area_map().unwrap().notes.len();
    assert!(notes > 0, "the Endar Spire has notes");
    let places: Vec<u32> = w
        .objects
        .values()
        .filter(|o| o.kind == Kind::Waypoint)
        .map(|o| o.id)
        .collect();
    for id in &places {
        w.set_map_pin(*id, false);
    }
    assert!(w.area_map().unwrap().notes.is_empty());
    let mut again = saved_and_back(&w);
    assert!(again.area_map().unwrap().notes.is_empty());
    for id in &places {
        w.set_map_pin(*id, true);
    }
    assert!(w.area_map().unwrap().notes.len() >= notes);
}

#[test]
fn the_apartments_script_shows_the_map() {
    let Some(mut w) = world("tar_m02af") else {
        return;
    };
    let pc = w.pc;
    let map = w.area_map().unwrap();
    let seen = |w: &World| {
        w.explored
            .get(&map.picture)
            .map_or(0, |c| c.iter().filter(|c| **c).count())
    };
    let before = seen(&w);
    w.run_script("k_ptar_02af_en", pc).unwrap();
    assert!(seen(&w) > before, "{:?}", w.errors);
}

#[test]
fn a_script_can_take_someone_off_the_party_screen_with_a_save() {
    let Some(mut w) = world("danm13") else {
        return;
    };
    let pc = w.pc;
    assert!(w.unselectable.is_empty());
    w.unselectable.insert(4);
    let again = saved_and_back(&w);
    assert!(again.unselectable.contains(&4));
    // `k_pdan_13_load` asks and sets it, and must run on what it has.
    w.run_script("k_pdan_13_load", pc).unwrap();
    assert!(w.errors.is_empty(), "{:?}", w.errors);
}
