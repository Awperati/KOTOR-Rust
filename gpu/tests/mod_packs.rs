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

    // The Twi'lek pack, when the player has it: its playable heads and their pictures, and nothing of its characters or bodies.
    let heads: Vec<String> = inst
        .pack_names(ResType::MDL)
        .into_iter()
        .filter(|n| n.starts_with("p_twih"))
        .collect();
    let has_twileks = std::fs::read_dir(&root).ok().is_some_and(|r| {
        r.flatten().any(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("K1 HD Female Twilek Diversity")
        })
    });
    if has_twileks {
        assert!(heads.len() >= 26, "{heads:?}");
        assert!(
            !inst.in_override("n_twilekf", ResType::MDL),
            "an NPC's body is left"
        );
        let (mdl, mdx) = (
            inst.read(&heads[0], ResType::MDL).unwrap(),
            inst.read(&heads[0], ResType(3008)).unwrap(),
        );
        let head = kotor_model::parse_model(&mdl, &mdx).unwrap();
        assert!(
            head.root.find("f_mdbrw_g").is_some(),
            "the face's points, for its sliders"
        );
    } else {
        assert!(heads.is_empty());
    }

    // The playable Jawa, when the player has it: its body under a name of its own, the game's own first man left as he is, the
    // game's Jawa's eyes in its hood where nothing of the hood hides them, and its voice.
    let has_jawa = std::fs::read_dir(&root).ok().is_some_and(|r| {
        r.flatten()
            .any(|e| e.file_name().to_string_lossy().starts_with("playable Jawa"))
    });
    if has_jawa {
        assert!(inst.in_override("p_jawab", ResType::MDL));
        assert!(!inst.in_override("pmbal", ResType::MDL), "the hero's own body");
        assert!(!inst.in_override("pmha01", ResType::MDL), "the hero's own head");
        assert!(inst.contains("p_jawamw_atk1", ResType::WAV));
        let rig = kotor_render::rig_for_race_body(&mut inst, "p_jawab").unwrap();
        assert!(rig.body_model().root.find("eyes").is_some(), "the eyes");
        let scene = rig.pose(Some(("pause1", 0.0))).unwrap();
        let points = |name: &str| {
            scene
                .meshes
                .iter()
                .find(|m| m.name.eq_ignore_ascii_case(name))
                .unwrap()
                .positions
                .clone()
        };
        let eyes = points("eyes");
        let lo = [0, 1, 2].map(|i| eyes.iter().map(|p| p[i]).fold(f32::MAX, f32::min));
        let hi = [0, 1, 2].map(|i| eyes.iter().map(|p| p[i]).fold(f32::MIN, f32::max));
        let hood = points("jawa_torso");
        let top = hood.iter().map(|p| p[2]).fold(f32::MIN, f32::max);
        assert!(
            lo[2] > top - 0.4 && hi[2] < top,
            "in the hood: {lo:?} {hi:?}, its top {top}"
        );
        // In the middle of the opening: the hood in front of them is its chin below and its brow above, and a camera a little
        // above or below still sees them.
        let middle = (lo[2] + hi[2]) / 2.0;
        let front: Vec<f32> = hood
            .iter()
            .filter(|p| (lo[0]..hi[0]).contains(&p[0]) && p[1] > hi[1])
            .map(|p| p[2])
            .collect();
        assert!(!front.iter().any(|z| (lo[2]..hi[2]).contains(z)), "hidden");
        let chin = front.iter().copied().filter(|z| *z < middle).fold(f32::MIN, f32::max);
        let brow = front.iter().copied().filter(|z| *z > middle).fold(f32::MAX, f32::min);
        let at = (middle - chin) / (brow - chin);
        assert!((0.33..0.67).contains(&at), "{at} of the way from the chin {chin} to the brow {brow}");
    } else {
        assert!(!inst.in_override("p_jawab", ResType::MDL));
    }
    kotor_archive::set_mod_packs(false);
}
