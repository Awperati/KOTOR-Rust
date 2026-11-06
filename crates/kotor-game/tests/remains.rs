//! What a creature carried is in its remains (bug 34).

use kotor_archive::Install;
use kotor_game::World;

#[test]
fn the_remains_of_the_dark_jedi_hold_both_lightsabers() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut base = Install::open(dir).unwrap();
    base.set_prefer_rim(true);
    let mut w = World::new(base).unwrap();
    w.load_module("end_m01aa").unwrap();
    let spec = w.recommended_character("Player", 0, 1).unwrap();
    w.create_character(&spec).unwrap();
    w.enter_area();
    let ids = w.by_tag("end_darkjedi01");
    assert!(!ids.is_empty());
    let jedi = ids[0];
    let o = w.object(jedi).unwrap();
    let held: Vec<_> = o
        .inventory
        .iter()
        .chain(o.equipped.values())
        .copied()
        .collect();
    let sabers = held
        .iter()
        .filter(|i| {
            w.object(**i)
                .is_some_and(|o| o.template.contains("lghtsbr"))
        })
        .count();
    assert!(sabers >= 1, "he carries no lightsaber: {held:?}");
    let o = w.object_mut(jedi).unwrap();
    o.min_one_hp = false;
    let hp = o.hp;
    w.hurt(w.pc, jedi, hp + 1);
    assert!(w.object(jedi).unwrap().dead);
    let after = &w.object(jedi).unwrap().inventory;
    for i in &held {
        assert!(after.contains(i), "{i} was lost");
    }
}
