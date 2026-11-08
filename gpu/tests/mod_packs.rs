//! The packs in the player's own mods folder come after the install's `Override` folder and before the game's own files, only
//! when switched on, and are read from there: nothing of them is in the repository. A mods folder that is not there is no pack.
use kotor_archive::Install;
use kotor_core::ResType;

#[test]
fn the_mod_packs_are_used_when_on_and_read_from_the_mods_folder() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let root = kotor_archive::mods_root();
    let beams = std::fs::read_dir(&root).ok().and_then(|r| {
        r.flatten().map(|e| e.path()).find(|p| {
            p.to_string_lossy()
                .contains("High Resolution Lightsaber Beams")
        })
    });
    let Some(beams) = beams else {
        return;
    };
    // Off, the stock game.
    let stock = Install::open(&dir).unwrap();
    assert!(!stock.in_override("w_lsabreblue01", ResType::TGA));

    kotor_archive::set_mod_packs(true);
    let mut inst = Install::open(&dir).unwrap();
    assert!(inst.in_override("w_lsabreblue01", ResType::TGA));
    let theirs = std::fs::read(beams.join("w_lsabreblue01.tga")).unwrap();
    assert_eq!(inst.read("w_lsabreblue01", ResType::TGA).unwrap(), theirs);
    // The hero's own bodies stay the game's own, as the character pictures pack leaves them.
    assert!(!inst.in_override("pfbam01", ResType::TPC));
    assert!(!inst.in_override("pmbam01", ResType::TPC));

    // Every model the packs bring is one this game's reader takes.
    let mut models = 0;
    for name in ["lts_sky0001", "lda_sky0001", "lka_sky0001"] {
        assert!(
            inst.in_override(name, ResType::TPC)
                || inst.in_override(name, ResType::TGA)
                || !root.exists(),
            "{name}"
        );
    }
    for stem in [
        "m02aa_sky",
        "m02ab_sky",
        "m14aa_01h",
        "m18aa_05a",
        "m26aa_set",
        "m41aa_02d",
        "w_blstrcrbn_001",
        "w_blstrpstl_001",
    ] {
        let (Ok(mdl), Ok(mdx)) = (
            inst.read(stem, ResType::MDL),
            inst.read(stem, ResType(3008)),
        ) else {
            continue;
        };
        if inst.in_override(stem, ResType::MDL) {
            kotor_model::parse_model(&mdl, &mdx).unwrap_or_else(|e| panic!("{stem}: {e}"));
            models += 1;
        }
    }
    assert!(models >= 3, "{models} models from the packs");
    kotor_archive::set_mod_packs(false);
}
