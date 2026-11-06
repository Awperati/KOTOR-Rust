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

/// A place of this run's own: other runs of these tests can be going at the same time, in this checkout or another.
fn temp_place(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("kotor_rs_{name}_{}", std::process::id()))
}

#[test]
fn a_save_keeps_the_leader_the_hero_and_who_cannot_fall() {
    let Some(mut w) = world() else { return };
    let hero = w.pc;
    let spec = w.recommended_character("Player", 0, 1).unwrap();
    w.create_character(&spec).unwrap();
    let carth = w.by_tag("end_trask")[0];
    w.party.push(carth);
    w.object_mut(carth).unwrap().party_member = true;
    w.object_mut(carth).unwrap().min_one_hp = true;
    assert!(w.switch_leader(carth));
    let bytes = w.save_bytes("leader").unwrap();

    let mut back = fresh();
    back.load_save(&bytes).unwrap();
    assert_eq!(back.hero, hero);
    assert_ne!(back.pc, hero);
    assert_eq!(back.object(back.pc).unwrap().tag, "end_trask");
    assert!(back.object(back.pc).unwrap().min_one_hp);
    assert_eq!(back.party, vec![hero]);
    // And a world led by someone else takes it too.
    let again = back.save_bytes("again").unwrap();
    back.load_save(&again).unwrap();
    assert_eq!(back.hero, hero);
    assert_eq!(back.party, vec![hero]);
}

#[test]
fn a_tutorial_comes_once_and_a_save_remembers() {
    let Some(mut w) = world() else { return };
    let pages = w.tutorial_once(41);
    assert_eq!(pages, vec![47886, 47887], "two pages");
    assert!(w.tutorial_once(41).is_empty());
    let bytes = w.save_bytes("tutorial").unwrap();
    let mut back = fresh();
    back.load_save(&bytes).unwrap();
    assert!(back.tutorial_once(41).is_empty());
    assert_eq!(back.tutorial_once(14), vec![41882]);
}

#[test]
fn what_has_been_seen_of_a_map_grows_and_is_saved() {
    let Some(mut w) = world() else { return };
    let map = w.area_map().expect("the first area has a map");
    let first = w.reveal(&map, 20.0, 20.0);
    assert!(first > 0 && first < 100, "{first}");
    assert_eq!(
        w.reveal(&map, 20.0, 20.0),
        first,
        "the same place adds nothing"
    );
    assert!(w.reveal(&map, 30.0, 100.0) > first);
    // Off the picture nothing happens.
    let both = w.reveal(&map, 30.0, 100.0);
    assert_eq!(w.reveal(&map, 9000.0, 9000.0), both);

    let bytes = w.save_bytes("seen").unwrap();
    let mut back = fresh();
    back.load_save(&bytes).unwrap();
    assert_eq!(back.explored, w.explored);
}

#[test]
fn a_member_left_behind_keeps_levels_and_items_through_a_save() {
    let Some(mut w) = world() else { return };
    w.available.insert(2, "end_trask".to_string());
    w.choose_party(&[2], "");
    let id = w.npc_slots[&2];
    let (pistol, medpack) = (
        w.spawn_item("g_w_blstrpstl001"),
        w.spawn_item("g_i_medeqpmnt01"),
    );
    let o = w.object_mut(id).unwrap();
    o.level = 7;
    o.xp = 21000;
    o.max_hp = 77;
    o.hp = 0;
    o.feats.insert(13);
    o.spells.insert(23);
    o.skill_points = 4;
    o.abilities[3] = 18;
    o.inventory = vec![medpack];
    o.equipped.clear();
    o.equipped.insert(4, pistol);
    w.object_mut(medpack).unwrap().stack = 3;

    // Left behind, they and their things are gone from the world.
    w.choose_party(&[], "");
    assert!(w.party.is_empty());
    assert!(w.npc_slots.is_empty());
    assert!(w.object(id).is_none());
    assert!(w.object(pistol).is_none());
    assert_eq!(w.party_candidate(2).unwrap().1, 7);

    // A save and a load keep them.
    let bytes = w.save_bytes("bench").unwrap();
    let mut back = fresh();
    back.load_save(&bytes).unwrap();
    assert_eq!(back.bench, w.bench);
    assert_eq!(
        back.bench[&2].carried,
        vec![("g_i_medeqpmnt01".to_string(), 3)]
    );

    // They come back as they were, without what their template would give them.
    back.choose_party(&[2], "");
    assert!(back.bench.is_empty());
    let id = back.npc_slots[&2];
    let o = back.object(id).unwrap().clone();
    assert_eq!((o.level, o.xp, o.max_hp, o.hp), (7, 21000, 77, 1));
    assert!(o.feats.contains(&13) && o.spells.contains(&23));
    assert_eq!((o.skill_points, o.abilities[3]), (4, 18));
    assert_eq!(o.inventory.len(), 1);
    assert_eq!(back.object(o.inventory[0]).unwrap().stack, 3);
    assert_eq!(o.equipped.len(), 1);
    assert_eq!(
        back.object(o.equipped[&4]).unwrap().template,
        "g_w_blstrpstl001"
    );
    assert_eq!(back.object(id).unwrap().faction, 2);
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
    let path = temp_place("test_save.sav");
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

/// A bare world: only the player exists and no module is loaded, as on the main menu.
fn bare() -> World {
    let dir = kotor_core::install_dir().unwrap();
    World::new(Install::open(dir).unwrap()).unwrap()
}

#[test]
fn a_save_loads_from_another_module_and_from_no_module() {
    let Some(mut a) = world() else { return };
    play(&mut a);
    let trask = a.by_tag("end_trask")[0];
    a.transition("end_m01ab", "").unwrap();
    assert_eq!(a.module, "end_m01ab");
    assert_eq!(a.by_tag("end_trask"), vec![trask], "the party came along");
    let there = a.save_bytes("Second module").unwrap();

    // Into a world in the first module, and into one with nothing loaded.
    for mut b in [fresh(), bare()] {
        b.load_save(&there).unwrap();
        assert_eq!(a.state_digest(), b.state_digest());
        assert_eq!(b.module, "end_m01ab");
        let area = b.object(b.area).unwrap();
        assert_eq!(area.kind, Kind::Area);
        assert!(area.tag.to_ascii_lowercase().starts_with("m01ab") || !area.tag.is_empty());
        assert!(b.nav.is_some(), "the walkmesh is of the saved module");
        let t = b.by_tag("end_trask")[0];
        assert_eq!(b.party, vec![t]);
        // An object whose template is in another module keeps what the template gave it.
        let (ours, theirs) = (b.object(t).unwrap(), a.object(trask).unwrap());
        assert_eq!(ours.scripts, theirs.scripts);
        assert_eq!(ours.conversation, theirs.conversation);
        assert_eq!(ours.natural_ac, theirs.natural_ac);
        assert_eq!(ours.appearance, theirs.appearance);
        // It plays on from there.
        for _ in 0..20 {
            b.step(0.05);
        }
        assert!(b.errors.is_empty(), "{:?}", b.errors);
    }

    // And back: a save of the first module, loaded while in the second.
    let mut c = fresh();
    play(&mut c);
    let here = c.save_bytes("First module").unwrap();
    let mut d = fresh();
    d.transition("end_m01ab", "").unwrap();
    d.load_save(&here).unwrap();
    assert_eq!(c.state_digest(), d.state_digest());
    assert_eq!(d.module, "end_m01aa");
}

#[test]
fn loading_leaves_nothing_of_the_game_before_it() {
    let Some(mut a) = world() else { return };
    a.interactive_dialogue = true;
    let bytes = a.save_bytes("Clean").unwrap();

    let mut b = fresh();
    b.interactive_dialogue = true;
    let trask = b.by_tag("end_trask")[0];
    let pc = b.pc;
    b.attack(pc, trask);
    b.opened = Some(trask);
    b.game_over = true;
    b.converse(trask, pc, "", &mut kotor_game::FirstReply).ok();
    b.load_save(&bytes).unwrap();
    assert!(b.combat.target.is_empty());
    assert_eq!(b.opened, None);
    assert!(!b.game_over);
    assert!(!b.in_dialogue());
    assert_eq!(a.state_digest(), b.state_digest());
}

#[test]
fn a_file_that_will_not_load_leaves_the_world_alone() {
    let Some(mut w) = world() else { return };
    let before = w.state_digest();
    assert!(w.load_save(b"not an archive").is_err());
    assert_eq!(w.state_digest(), before);

    // A save of a module the install does not have.
    let erf = Erf::open(Source::memory(w.save_bytes("x").unwrap())).unwrap();
    let mut nfo = Gff::parse(&erf.get("savenfo", ResType(0)).unwrap()).unwrap();
    nfo.root.set(
        "LASTMODULE",
        kotor_gff::Value::String("no_such_module".to_string()),
    );
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
    assert_eq!(w.state_digest(), before);
}

#[test]
fn a_save_keeps_its_header_and_a_replaced_file_is_whole() {
    let Some(w) = world() else { return };
    let dir = temp_place("save_header");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("one.sav");
    w.save_to(&path, "Header").unwrap();
    w.save_to(&path, "Header again").unwrap();
    let info = kotor_game::read_save_info(&path).unwrap();
    assert_eq!(info.name, "Header again");
    assert_eq!(info.module, "end_m01aa");
    assert_eq!(info.portraits.len(), 1);
    assert_eq!(
        std::fs::read_dir(&dir).unwrap().count(),
        1,
        "no part file is left behind"
    );
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::write(&path, b"junk").unwrap();
    assert!(kotor_game::read_save_info(&path).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A soldier of the first module, killed, and the world back in that module after going elsewhere.
fn kill_leave_and_return(through_a_save: bool) -> Option<()> {
    let mut w = world()?;
    let pc = w.pc;
    let foe = w
        .objects
        .values()
        .find(|o| o.template == "n_sithsoldier003")
        .map(|o| o.id)?;
    let (tag, nth) = {
        let tag = w.object(foe)?.tag.clone();
        let nth = w.by_tag(&tag).iter().position(|id| *id == foe)?;
        (tag, nth)
    };
    let door = w.by_tag("end_door01")[0];
    assert!(w.open_door(door));
    for _ in 0..30 {
        w.step(0.05);
    }
    assert!(w.hurt(pc, foe, 100_000));
    assert!(w.object(foe)?.dead);
    let xp = w.object(pc)?.xp;
    assert!(xp > 0, "the kill paid");
    w.transition("end_m01ab", "").unwrap();
    if through_a_save {
        let bytes = w.save_bytes("away").unwrap();
        let again = w.save_bytes("away").unwrap();
        assert_eq!(bytes, again);
        w.load_save(&bytes).unwrap();
        assert_eq!(
            w.save_bytes("away").unwrap(),
            bytes,
            "a save survives a load"
        );
    }
    w.transition("end_m01aa", "").unwrap();
    let back = w.by_tag(&tag)[nth];
    assert!(w.object(back)?.dead, "still dead");
    assert_eq!(w.object(w.pc)?.xp, xp, "not paid twice");
    // The door stays open, and walking through it works.
    let door = w.by_tag("end_door01")[0];
    assert_eq!(w.object(door)?.door, kotor_game::DoorState::Open);
    let nav = w.nav.as_ref()?;
    let a = w.object(door)?.position;
    assert!(!nav.crosses_block([a[0] - 2.0, a[1], a[2]], [a[0] + 2.0, a[1], a[2]]));
    Some(())
}

#[test]
fn a_module_stays_as_it_was_left() {
    if world().is_some() {
        kill_leave_and_return(false).unwrap();
    }
}

#[test]
fn a_module_left_behind_survives_a_save_and_load() {
    if world().is_some() {
        kill_leave_and_return(true).unwrap();
    }
}
