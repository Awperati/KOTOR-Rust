//! The owner's own clips: a `*.anim.txt` file is played as a clip of its name, replaces a stock clip of the same name, and a
//! bad file is named in `anim_problems.txt` and left out. Its own test binary: the clips are registered for the whole process.

use kotor_gpu::stage::Stage;

const DT: f32 = 1.0 / 30.0;

/// The positions of the hero's body parts as drawn, hashed: it changes when a limb moves.
fn shape(s: &mut Stage) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for m in &s.dynamic_scene().meshes {
        if m.texture
            .as_deref()
            .is_some_and(|t| t.to_ascii_lowercase().starts_with("pfba"))
        {
            for p in &m.positions {
                p.map(f32::to_bits).hash(&mut h);
            }
        }
    }
    h.finish()
}

#[test]
fn a_clip_of_his_is_played_replaces_a_stock_clip_and_a_bad_file_is_named() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let folder = std::env::temp_dir().join(format!("kotor_rs_anims_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    std::env::set_var("KOTOR_RS_ANIMS", &folder);
    // A wave: the right forearm raised over ten frames. The keys name bones as the game does, and one it does not have.
    std::fs::write(
        folder.join("wave.anim.txt"),
        "version = 2\nname = wave\nfps = 30\nloop = yes\n\
         key rbicep_g 0 0.0 0.0 0.0 1 0 0 0\n\
         key rbicep_g 10 0.0 0.0 0.0 0.7071 0.7071 0 0\n\
         key rbicep_g 20 0.0 0.0 0.0 1 0 0 0\n\
         key nosuchbone_g 0 0 0 0 1 0 0 0\n",
    )
    .unwrap();
    std::fs::write(
        folder.join("broken.anim.txt"),
        "version = 1\nname = broken\n",
    )
    .unwrap();
    let mut s = Stage::open("end_m01aa", true).unwrap();
    // A hero with a body to draw, as a character screen leaves one.
    let pc = s.game.world.pc;
    s.game.world.object_mut(pc).unwrap().appearance = 92;
    // Out of the opening conversation, which poses the player itself.
    for _ in 0..600 {
        s.update(DT);
        if !s.game.world.in_dialogue() {
            break;
        }
        s.game.world.dialogue_choose(0);
    }
    for _ in 0..40 {
        s.update(DT);
    }
    let stock = shape(&mut s);
    s.reload_custom_clips();
    let problems = s.anim_problems().join("\n");
    assert!(problems.contains("nosuchbone_g"), "{problems}");
    assert!(problems.contains("broken.anim.txt"), "{problems}");
    let text = std::fs::read_to_string(folder.join("anim_problems.txt")).unwrap();
    assert!(
        text.contains("broken.anim.txt") && text.contains("nosuchbone_g"),
        "{text}"
    );
    assert_eq!(kotor_render::custom_clip::names(), vec!["wave".to_string()]);

    // F9: the player plays the wave; the shape changes as it goes.
    s.cycle_custom_clip();
    let mut seen = std::collections::HashSet::new();
    for _ in 0..20 {
        for _ in 0..3 {
            s.update(DT);
        }
        let sh = shape(&mut s);
        seen.insert(sh);
    }
    assert!(
        seen.len() >= 4,
        "the wave moves the body: {} shapes",
        seen.len()
    );
    assert!(!seen.contains(&stock) || seen.len() > 3);
    // Again: back to the game's own.
    s.cycle_custom_clip();
    for _ in 0..3 {
        s.update(DT);
    }

    // A clip named like a stock one replaces it: the player standing is now the wave's first frame, and the pose differs.
    std::fs::write(
        folder.join("pause1.anim.txt"),
        "version = 2\nname = pause1\nfps = 30\nloop = yes\n\
         key rbicep_g 0 0.0 0.0 0.0 0.7071 0.7071 0 0\n",
    )
    .unwrap();
    s.reload_custom_clips();
    for _ in 0..40 {
        s.update(DT);
    }
    let replaced = shape(&mut s);
    assert_ne!(replaced, stock, "the standing clip is his");
    // Take it away and press F8: the game's own idle returns.
    std::fs::remove_file(folder.join("pause1.anim.txt")).unwrap();
    s.reload_custom_clips();
    for _ in 0..40 {
        s.update(DT);
    }
    assert_ne!(shape(&mut s), replaced);

    // Clips named for the moves take the place of their pose files. The example file, under another name.
    let wave = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../docs/examples/wave.anim.txt"
    ))
    .unwrap();
    let arm = |s: &mut Stage| s.bent_joints().unwrap()["rhand_g"];
    type Preview = fn(&mut Stage, f32);
    let moves: [(&str, Preview); 3] = [
        ("jump", Stage::preview_jump),
        ("roll", Stage::preview_roll),
        ("landing", Stage::preview_landing),
    ];
    for (name, preview) in moves {
        preview(&mut s, 0.5);
        let from_file = arm(&mut s);
        let file = folder.join(format!("{name}.anim.txt"));
        std::fs::write(
            &file,
            wave.replace("name = wave", &format!("name = {name}")),
        )
        .unwrap();
        s.reload_custom_clips();
        assert!(
            s.anim_problems().iter().all(|p| !p.contains(name)),
            "{:?}",
            s.anim_problems()
        );
        let from_clip = arm(&mut s);
        let moved = (0..3)
            .map(|k| (from_clip[k] - from_file[k]).abs())
            .fold(0.0, f32::max);
        assert!(moved > 0.1, "{name}: the hand moved {moved} m");
        // Its first frame is what the body was doing, and not a jump to the clip.
        preview(&mut s, 0.0);
        let start = arm(&mut s);
        preview(&mut s, 0.5);
        assert!(
            (0..3).any(|k| (start[k] - from_clip[k]).abs() > 0.05),
            "{name} comes in"
        );
        // Gone, and the file is back.
        std::fs::remove_file(&file).unwrap();
        s.reload_custom_clips();
        assert_eq!(arm(&mut s), from_file, "{name}");
    }
    let _ = std::fs::remove_dir_all(&folder);
}
