//! Encounters know their creatures, and leave out those too strong for the party (GAPS row 6).

use kotor_archive::Install;
use kotor_game::World;

#[test]
fn encounter_creatures_are_known_and_a_weak_party_meets_only_what_it_can_face() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut w = World::new(Install::open(dir).unwrap()).unwrap();
    w.load_module("tar_m03aa").unwrap();
    let pc = w.pc;
    assert!(w.encounter_creatures.is_empty());
    w.teleport(pc, [379.0, 150.0, 8.3]);
    w.step(0.1);
    assert_eq!(w.encounter_creatures.len(), 2, "{:?}", w.events.last());
    for id in &w.encounter_creatures {
        assert!(w.object(*id).is_some());
    }
    // Everything it brought out was at or under the level plus the encounter's reach, or the weakest on offer.
    let events = w.events.join("\n");
    assert!(!events.contains("keeps"), "{events}");
}
