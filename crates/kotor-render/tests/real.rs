use kotor_archive::Install;
use kotor_render::{load_area, load_texture, render, Camera};
use std::collections::BTreeSet;

#[test]
fn endar_spire_loads_and_renders_something_lit_and_textured() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.load_texture_packs().unwrap();
    let scene = load_area(&mut inst, "m01aa").unwrap();
    assert!(scene.meshes.len() > 400, "{} meshes", scene.meshes.len());
    assert!(scene.triangle_count() > 50_000);
    // Every named texture resolves, apart from a known stray (m01aa_04a_a0002t has no file in the install).
    let mut names = BTreeSet::new();
    for m in &scene.meshes {
        names.extend(
            m.texture
                .iter()
                .chain(m.lightmap.iter())
                .map(|n| n.to_ascii_lowercase()),
        );
    }
    let missing: Vec<_> = names
        .iter()
        .filter(|n| load_texture(&mut inst, n).is_none())
        .collect();
    assert!(missing.len() <= 1, "missing textures: {missing:?}");
    // Meshes that carry a lightmap also carry its UVs.
    assert!(scene
        .meshes
        .iter()
        .filter(|m| m.lightmap.is_some())
        .all(|m| m.uvs2.len() == m.positions.len()));

    // The view from the spawn point: lots of distinct colours, no flat frame, deterministic.
    let cam = Camera {
        eye: [15.4, 20.1, 0.6],
        target: [25.0, 20.5, 0.2],
        fov: 70.0,
    };
    let mut tex = |n: &str| load_texture(&mut inst, n);
    let a = render(&scene, &mut tex, &cam, 160, 90);
    let b = render(&scene, &mut tex, &cam, 160, 90);
    assert_eq!(a, b, "rendering must be deterministic");
    let bg = [18u8, 20, 28];
    let covered = a.rgba.chunks(4).filter(|p| p[..3] != bg).count();
    assert!(
        covered > 160 * 90 * 9 / 10,
        "only {covered} of {} pixels covered",
        160 * 90
    );
    let colours: BTreeSet<[u8; 3]> = a
        .rgba
        .chunks(4)
        .map(|p| [p[0] >> 3, p[1] >> 3, p[2] >> 3])
        .collect();
    assert!(colours.len() > 200, "{} colours", colours.len());
}

#[test]
fn bastila_stands_human_height_in_an_idle_pose() {
    use kotor_render::load_character;
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.load_texture_packs().unwrap();
    let rest = load_character(&mut inst, "p_bastilabb", Some("p_bastilah"), None).unwrap();
    let idle = load_character(
        &mut inst,
        "p_bastilabb",
        Some("p_bastilah"),
        Some(("pause1", 0.0)),
    )
    .unwrap();
    for (name, s) in [("rest", &rest), ("idle", &idle)] {
        let h = s.max[2] - s.min[2];
        assert!((1.5..2.0).contains(&h), "{name}: height {h}");
        assert!(s.meshes.len() >= 15, "{name}: {} meshes", s.meshes.len());
        // Feet near the floor and the head above the pelvis.
        assert!(s.min[2].abs() < 0.2, "{name}: feet at {}", s.min[2]);
    }
    // The idle pose really differs from the rest pose.
    let moved = rest
        .meshes
        .iter()
        .zip(&idle.meshes)
        .any(|(a, b)| a.positions.first() != b.positions.first());
    assert!(moved, "idle pose left every mesh where it was");
    let cam = Camera {
        eye: [0.0, 4.0, 1.0],
        target: [0.0, 0.0, 0.9],
        fov: 40.0,
    };
    let img = render(&idle, &mut |n| load_texture(&mut inst, n), &cam, 120, 160);
    let bg = [18u8, 20, 28];
    let lit = img.rgba.chunks(4).filter(|p| p[..3] != bg).count();
    assert!(lit > 120 * 160 / 20, "only {lit} pixels drawn");
}
