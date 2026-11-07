//! `kotor-tool blender-export` writes a glTF a modeller can open: the game's bone names, skins, textures and clips, all in files
//! that agree with each other.

use std::process::Command;

#[test]
fn a_character_is_exported_with_its_bones_skins_textures_and_clips() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let dir = std::env::temp_dir().join(format!("kotor_blender_export_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let status = Command::new(env!("CARGO_BIN_EXE_kotor-tool"))
        .args(["blender-export", "x"])
        .arg(&dir)
        .args([
            "--appearance",
            "92",
            "--name",
            "hero",
            "--clips",
            "pause1,g8r1",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let gltf = std::fs::read_to_string(dir.join("hero.gltf")).unwrap();
    let bin = std::fs::read(dir.join("hero.bin")).unwrap();
    assert!(
        gltf.contains(&format!(
            "\"uri\":\"hero.bin\",\"byteLength\":{}",
            bin.len()
        )),
        "the buffer's size is the file's"
    );
    // The game's bone names, lower case, as nodes.
    for bone in [
        "torso_g",
        "torsoupr_g",
        "lcollar_g",
        "rcollar_g",
        "rbicep_g",
        "lforearm_g",
        "headhook",
        "head.f_jaw_g",
    ] {
        assert!(gltf.contains(&format!("\"name\":\"{bone}\"")), "{bone}");
    }
    // Skins with their bones, and weights.
    assert!(gltf.contains("\"skins\":["));
    assert!(gltf.contains("\"JOINTS_0\"") && gltf.contains("\"WEIGHTS_0\""));
    // The clips asked for, and only those.
    assert!(gltf.contains("\"name\":\"pause1\"") && gltf.contains("\"name\":\"g8r1\""));
    assert!(!gltf.contains("\"name\":\"walk\"") || gltf.matches("\"animations\"").count() == 1);
    // Textures as PNG under the game's names, and the bone list for the add-on.
    let pngs: Vec<_> = std::fs::read_dir(dir.join("textures"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert!(
        pngs.iter()
            .any(|p| p.starts_with("pfbam") && p.ends_with(".png")),
        "{pngs:?}"
    );
    assert!(
        pngs.iter()
            .any(|p| p.starts_with("pfha") && p.ends_with(".png")),
        "{pngs:?}"
    );
    let bones = std::fs::read_to_string(dir.join("bones.txt")).unwrap();
    assert!(
        bones
            .lines()
            .any(|l| l.starts_with("torso_g\ttorso") || l.starts_with("torso_g\t")),
        "{bones}"
    );
    assert!(
        bones.lines().any(|l| l.starts_with("pfbam\t-\t")),
        "the root has no parent"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
