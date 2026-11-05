use kotor_archive::{Erf, Install, Source};
use kotor_core::ResType;
use kotor_game::{DoorState, Kind, World};
use kotor_gff::Gff;

fn world() -> Option<World> {
    let dir = kotor_core::install_dir()?;
    let mut w = World::new(Install::open(dir).unwrap()).unwrap();
    w.load_module("end_m01aa").unwrap();
    Some(w)
}

fn fresh() -> World {
    world().expect("install")
}

/// Play a little: open the first door, give Trask to the party with an item, earn XP, set variables, hurt a creature.
fn play(w: &mut World) {
    let d = *w
        .by_tag("end_door01")
        .iter()
        .find(|id| (w.object(**id).unwrap().position[0] - 21.1).abs() < 1.0)
        .unwrap();
    assert!(w.open_door(d));
    for _ in 0..22 {
        w.step(0.05);
    }
    let trask = w.by_tag("end_trask")[0];
    w.party.push(trask);
    w.npc_slots.insert(0, trask);
    w.object_mut(trask).unwrap().party_member = true;
    let item = w.spawn_item("g_w_blstrpstl001");
    w.object_mut(trask).unwrap().inventory.push(item);
    w.object_mut(trask).unwrap().equipped.insert(4, item);
    w.object_mut(trask).unwrap().hp = 7;
    w.object_mut(trask).unwrap().local_numbers.insert(3, 42);
    w.object_mut(trask).unwrap().local_booleans.insert(5, true);
    let pc = w.pc;
    w.object_mut(pc).unwrap().xp = 1234;
    w.global_numbers.insert("K_TEST".into(), 9);
    w.global_booleans.insert("K_FLAG".into(), true);
    w.journal.insert("end_quest".into(), 3);
    assert!(w.walk_to([30.8, 20.7, -1.275]));
    for _ in 0..40 {
        w.step(0.05);
    }
    w.time += 100.0;
}

#[test]
fn a_save_reloads_to_the_same_state() {
    let Some(mut a) = world() else { return };
    play(&mut a);
    let bytes = a.save_bytes("Test save").unwrap();

    // The archive is a real SAV ERF whose resources are GFFs with what the save screen needs.
    let erf = Erf::open(Source::memory(bytes.clone())).unwrap();
    assert_eq!(erf.kind, kotor_archive::ErfKind::Sav);
    for name in ["savenfo", "globalvars", "partytable", "objects"] {
        let g = Gff::parse(&erf.get(name, ResType(0)).unwrap()).unwrap();
        assert!(!g.root.fields.is_empty(), "{name} is empty");
    }
    let nfo = Gff::parse(&erf.get("savenfo", ResType(0)).unwrap()).unwrap();
    assert_eq!(nfo.root.str("LASTMODULE"), Some("end_m01aa"));
    assert_eq!(nfo.root.str("SAVEGAMENAME"), Some("Test save"));

    let mut b = fresh();
    b.load_save(&bytes).unwrap();
    assert_eq!(a.state_digest(), b.state_digest());
    // And the pieces a digest might miss:
    let pc = b.pc;
    assert_eq!(b.object(pc).unwrap().xp, 1234);
    assert_eq!(b.global_numbers.get("K_TEST"), Some(&9));
    assert_eq!(b.journal.get("end_quest"), Some(&3));
    let trask = b.by_tag("end_trask")[0];
    assert_eq!(b.party, vec![trask]);
    assert_eq!(b.object(trask).unwrap().hp, 7);
    let item = b.object(trask).unwrap().inventory[0];
    assert_eq!(b.object(item).unwrap().template, "g_w_blstrpstl001");
    assert_eq!(b.object(trask).unwrap().equipped.get(&4), Some(&item));
    // The opened door is passable again in the reloaded navigation mesh.
    let d = *b
        .by_tag("end_door01")
        .iter()
        .find(|id| (b.object(**id).unwrap().position[0] - 21.1).abs() < 1.0)
        .unwrap();
    assert_eq!(b.object(d).unwrap().door, DoorState::Open);
    let from = b.object(b.pc).unwrap().position;
    assert!(b
        .nav
        .as_ref()
        .unwrap()
        .find_path(from, [30.8, 20.7, -1.275])
        .is_some());
    // Saving the reloaded world and loading that again changes nothing.
    let mut c = fresh();
    c.load_save(&b.save_bytes("Test save").unwrap()).unwrap();
    assert_eq!(b.state_digest(), c.state_digest());
    assert!(b.errors.is_empty() && c.errors.is_empty());
}

#[test]
fn loading_removes_what_was_destroyed_and_restores_what_was_created() {
    let Some(mut a) = world() else { return };
    // A creature that was in the module is gone; a placeable made by a script is there.
    let trask = a.by_tag("end_trask")[0];
    a.objects.remove(&trask);
    let made = a.spawn_item("g_w_blstrpstl001");
    let _ = made;
    let bytes = a.save_bytes("x").unwrap();
    let mut b = fresh();
    assert_eq!(b.by_tag("end_trask").len(), 1);
    b.load_save(&bytes).unwrap();
    assert!(
        b.by_tag("end_trask").is_empty(),
        "destroyed creature is gone"
    );
    // Creatures come with the items their templates equip, so compare with the saved world, and look for the
    // script-made item by its template.
    let items = |w: &World, t: &str| {
        w.objects
            .values()
            .filter(|o| o.kind == Kind::Item && o.template == t)
            .count()
    };
    assert!(items(&a, "g_w_blstrpstl001") >= 1);
    assert_eq!(items(&b, "g_w_blstrpstl001"), items(&a, "g_w_blstrpstl001"));
    assert_eq!(
        b.objects.values().filter(|o| o.kind == Kind::Item).count(),
        a.objects.values().filter(|o| o.kind == Kind::Item).count()
    );
    assert_eq!(a.state_digest(), b.state_digest());
}

#[test]
fn a_save_never_goes_into_the_install_folder() {
    let Some(w) = world() else { return };
    let dir = kotor_core::install_dir().unwrap();
    let err = w.save_to(&dir.join("test_save.sav"), "x");
    assert!(err.is_err(), "writing into the install must be refused");
    assert!(!dir.join("test_save.sav").exists());
    // A temp file works and reads back.
    let path = std::env::temp_dir().join("kotor_rs_test_save.sav");
    w.save_to(&path, "temp").unwrap();
    let mut b = fresh();
    b.load_from(&path).unwrap();
    assert_eq!(w.state_digest(), b.state_digest());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_foreign_or_newer_save_is_rejected() {
    let Some(mut w) = world() else { return };
    assert!(w.load_save(b"not an archive").is_err());
    let bytes = w.save_bytes("x").unwrap();
    // Corrupt the version inside savenfo by rebuilding the archive with a different number.
    let erf = Erf::open(Source::memory(bytes)).unwrap();
    let mut nfo = Gff::parse(&erf.get("savenfo", ResType(0)).unwrap()).unwrap();
    nfo.root.set("SaveVersion", kotor_gff::Value::Int(99));
    let mut res = Vec::new();
    for e in &erf.entries {
        let data = if e.resref.as_str() == "savenfo" {
            nfo.to_bytes().unwrap()
        } else {
            erf.read(e).unwrap()
        };
        res.push(kotor_archive::NewResource::new(
            e.resref.as_str(),
            e.restype,
            data,
        ));
    }
    let bad = kotor_archive::write_archive(kotor_archive::ErfKind::Sav, &res).unwrap();
    assert!(w.load_save(&bad).is_err());
}
