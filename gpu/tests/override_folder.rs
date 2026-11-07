//! The install's `Override` folder comes first when it is turned on, what it holds that cannot be used is said and not fatal,
//! and with it off the game is the stock game.
use kotor_archive::{Install, OverrideUse};
use kotor_core::ResType;

/// A file of the folder, as a test can read it. The owner's own install, so nothing here says what must be in it.
fn in_folder(inst: &Install, name: &str) -> Option<Vec<u8>> {
    std::fs::read(inst.dir().join("Override").join(name)).ok()
}

#[test]
fn the_folder_comes_first_only_when_it_is_on_and_each_type_has_its_switch() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(&dir).unwrap();
    let Some(table) = in_folder(&inst, "appearance.2da") else {
        return;
    };
    let stock = inst.read("appearance", ResType::TWODA).unwrap();
    assert!(stock != table, "the stock table, with nothing chosen");

    inst.set_honour_override(true);
    assert_eq!(inst.read("appearance", ResType::TWODA).unwrap(), table);
    assert!(kotor_archive::override_used()
        .iter()
        .any(|(n, replaces)| n == "appearance.2da" && *replaces));

    // Tables off: the rest of the folder still comes first.
    inst.set_override_use(OverrideUse {
        tables: false,
        ..OverrideUse::ALL
    });
    assert_eq!(inst.read("appearance", ResType::TWODA).unwrap(), stock);
    assert!(
        inst.in_override("k_ai_master", ResType::NCS)
            == in_folder(&inst, "k_ai_master.ncs").is_some()
    );

    // Scripts off.
    inst.set_override_use(OverrideUse {
        scripts: false,
        ..OverrideUse::ALL
    });
    assert!(!inst.in_override("k_ai_master", ResType::NCS));

    // Screens off: this game's menus are laid out from the stock files.
    inst.set_override_use(OverrideUse {
        screens: false,
        ..OverrideUse::ALL
    });
    assert!(!inst.in_override("mainmenu", ResType(2047)));

    // And off altogether.
    inst.set_honour_override(false);
    assert_eq!(inst.read("appearance", ResType::TWODA).unwrap(), stock);
}

#[test]
fn the_talk_table_stays_the_games_until_it_is_asked_for() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(&dir).unwrap();
    inst.set_honour_override(true);
    assert_eq!(inst.talk_table_file(), dir.join("dialog.tlk"));
    inst.set_override_use(OverrideUse {
        talk: true,
        ..OverrideUse::ALL
    });
    if dir.join("Override").join("dialog.tlk").is_file() {
        assert_eq!(
            inst.talk_table_file(),
            dir.join("Override").join("dialog.tlk")
        );
    }
}

/// A TGA in the folder beats the game's own TPC of that name: the game's own would otherwise be found first.
#[test]
fn a_picture_in_the_folder_wins_whatever_its_type() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(&dir).unwrap();
    inst.load_texture_packs().unwrap();
    inst.set_honour_override(true);
    let folder = dir.join("Override");
    let Ok(read) = std::fs::read_dir(&folder) else {
        return;
    };
    let mut tested = 0;
    for entry in read.flatten() {
        let path = entry.path();
        let is_tga = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("tga"));
        if !is_tga || entry.metadata().map_or(0, |m| m.len()) > 16 << 20 {
            continue;
        }
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        if inst.in_override(&stem, ResType::TPC) || !inst.contains(&stem, ResType::TPC) {
            continue;
        }
        let theirs = kotor_image::parse_tga(&std::fs::read(&path).unwrap()).unwrap();
        let got = kotor_render::load_texture(&mut inst, &stem).unwrap();
        assert_eq!(
            (got.width, got.height),
            (theirs.width, theirs.height),
            "{stem}"
        );
        tested += 1;
        if tested == 3 {
            break;
        }
    }
    println!("{tested} pictures compared");
}

/// Nothing in the folder stops the game, and what cannot be used is said.
#[test]
fn a_look_through_the_folder_says_what_cannot_be_used() {
    let folder = std::env::temp_dir().join(format!("override_scan_{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    for (name, bytes) in [
        ("bad.2da", &b"not a table"[..]),
        ("bad.utc", &b"not a creature"[..]),
        ("broken.mdl", &[0u8; 8][..]),
        ("broken.mdx", &[0u8; 8][..]),
        ("line.lip", &b"LIP V1.0"[..]),
        ("sounds.ssf", &b"SSF V1.1"[..]),
        ("rooms.vis", &b"a 1"[..]),
        ("dialog.tlk", &b"TLK V3.0"[..]),
        ("a_name_of_over_sixteen.ncs", &b"NCS V1.0"[..]),
        ("fine.ncs", &b"NCS V1.0"[..]),
    ] {
        std::fs::write(folder.join(name), bytes).unwrap();
    }
    let lines = kotor_gpu::stage::scan_override_folder(&folder);
    let said = |name: &str| lines.iter().any(|l| l.starts_with(name));
    for name in [
        "bad.2da",
        "bad.utc",
        "broken.mdl",
        "line.lip",
        "sounds.ssf",
        "rooms.vis",
        "dialog.tlk",
        "a_name_of_over_sixteen.ncs",
    ] {
        assert!(said(name), "{name}: {lines:?}");
    }
    assert!(!said("fine.ncs"), "{lines:?}");

    // A problems file that says something else about the folder is looked at again, and starts with how it stands.
    let path = folder.with_extension("txt");
    std::fs::write(&path, "stale\n").unwrap();
    kotor_gpu::stage::scan_override_in_background(folder.clone(), path.clone());
    let began = std::time::Instant::now();
    let mut text = String::new();
    while !text.starts_with("# Override") && began.elapsed().as_secs() < 30 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        text = std::fs::read_to_string(&path).unwrap();
    }
    assert!(text.starts_with("# Override folder: 10 files"), "{text}");
    assert!(text.contains("bad.2da"), "{text}");
    let _ = std::fs::remove_file(&path);
}

/// Bytes that are not what their type should be are passed over for the stock file, with a note.
#[test]
fn a_file_of_the_wrong_kind_is_passed_over() {
    for (restype, bytes) in [
        (ResType::TWODA, &b"junk"[..]),
        (ResType::NCS, &b"junk"[..]),
        (ResType::MDL, &b"# ascii model"[..]),
        (ResType::DLG, &b"UTC V3.2"[..]),
    ] {
        assert!(!kotor_archive::looks_right(restype, bytes), "{restype}");
    }
    assert!(kotor_archive::looks_right(ResType::DLG, b"DLG V3.2"));
    assert!(kotor_archive::looks_right(ResType::MDL, &[0, 0, 0, 0, 1]));
}
