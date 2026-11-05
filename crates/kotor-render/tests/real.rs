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

#[test]
fn endar_spire_doors_and_placeables_are_placed() {
    use kotor_render::add_area_objects;
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.load_module("end_m01aa").unwrap();
    inst.load_texture_packs().unwrap();
    let mut scene = load_area(&mut inst, "m01aa").unwrap();
    let rooms = (scene.meshes.len(), scene.triangle_count());
    let stats = add_area_objects(&mut scene, &mut inst, "m01aa").unwrap();
    assert_eq!(stats.doors_listed, 15);
    assert_eq!(stats.doors_drawn, 15, "{stats:?}");
    assert_eq!(stats.placeables_listed, 61);
    // The rest are invisible helper placeables (appearance model PLC_Invis and the like).
    assert!(stats.placeables_drawn >= 50, "{stats:?}");
    assert_eq!(stats.unresolved, 0, "{stats:?}");
    assert!(scene.meshes.len() > rooms.0 + 200 && scene.triangle_count() > rooms.1 + 30_000);
    // Placed objects stay inside the area's bounds: nothing was thrown far away by a bad transform.
    for m in &scene.meshes[rooms.0..] {
        for p in &m.positions {
            assert!(
                p[0] > -50.0 && p[0] < 150.0 && p[1] > -50.0 && p[1] < 300.0,
                "{}: {p:?}",
                m.name
            );
        }
    }
    // Without the module loaded there is no GIT, so nothing is added and nothing fails.
    let mut inst2 = Install::open(kotor_core::install_dir().unwrap()).unwrap();
    let mut s2 = load_area(&mut inst2, "m01aa").unwrap();
    assert_eq!(
        add_area_objects(&mut s2, &mut inst2, "m01aa")
            .unwrap()
            .doors_listed,
        0
    );
}

#[test]
fn creature_texture_follows_appearance_row_and_variation() {
    use kotor_render::load_creature;
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.load_texture_packs().unwrap();
    // appearance.2da row 6 is Carth: body P_CarthBB with texture P_CarthBB + variation.
    let carth = load_creature(&mut inst, 6, 1, Some(("pause1", 0.0))).unwrap();
    let textures: BTreeSet<String> = carth
        .meshes
        .iter()
        .filter_map(|m| m.texture.clone())
        .map(|t| t.to_ascii_lowercase())
        .collect();
    assert!(textures.contains("p_carthbb01"), "{textures:?}");
    let h = carth.max[2] - carth.min[2];
    assert!((1.6..2.0).contains(&h), "height {h}");
    // A different variation picks a different (existing) texture name, or falls back without failing.
    let v2 = load_creature(&mut inst, 6, 2, None).unwrap();
    assert!(!v2.meshes.is_empty());
}

#[test]
fn creatures_from_the_git_stand_in_the_area() {
    use kotor_render::add_area_creatures;
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.load_module("end_m01aa").unwrap();
    inst.load_texture_packs().unwrap();
    let mut scene = load_area(&mut inst, "m01aa").unwrap();
    let before = scene.meshes.len();
    let stats = add_area_creatures(&mut scene, &mut inst, "m01aa", Some(("pause1", 0.0))).unwrap();
    assert_eq!(stats.listed, 26);
    assert_eq!(stats.drawn, 26, "{stats:?}");
    assert!(stats.distinct >= 8 && stats.distinct <= 26, "{stats:?}");
    assert!(scene.meshes.len() > before + 100);
    // Carth is the first entry of the creature list, at (35.16, 161.43, 0): some added mesh stands there.
    let near_carth = scene.meshes[before..].iter().any(|m| {
        m.positions.iter().any(|p| {
            (p[0] - 35.16).abs() < 0.6 && (p[1] - 161.43).abs() < 0.6 && p[2] > 1.3 && p[2] < 2.0
        })
    });
    assert!(
        near_carth,
        "no creature geometry head-high above Carth's position"
    );
}

#[test]
fn skinning_at_rest_equals_rigid_placement_and_a_pose_moves_the_head() {
    use kotor_model::parse_model;
    use kotor_render::{Scene, SkinCtx};
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    let load = |inst: &mut Install, n: &str| {
        parse_model(
            &inst.read(n, kotor_core::ResType::MDL).unwrap(),
            &inst.read(n, kotor_core::ResType::MDX).unwrap_or_default(),
        )
        .unwrap()
    };
    let body = load(&mut inst, "p_bastilabb");
    let head = load(&mut inst, "p_bastilah");
    let sup = load(&mut inst, body.supermodel.as_deref().unwrap());
    let rest = body.world_matrices(None);
    let pose = sup.animation("pause1").unwrap().sample(&sup, 0.0);
    let posed = body.world_matrices(Some(&pose));
    let find = |v: &[(String, [[f32; 4]; 4])]| {
        v.iter()
            .find(|x| x.0.eq_ignore_ascii_case("head_g"))
            .unwrap()
            .1
    };
    let hrest = head.world_matrices(None);
    let hm = find(&hrest);
    // Translation that puts the head model's head_g where the body's is (rotation-free at rest).
    let br = find(&rest);
    let mut xf = [
        [1., 0., 0., 0.],
        [0., 1., 0., 0.],
        [0., 0., 1., 0.],
        [0., 0., 0., 1.],
    ];
    for (k, row) in xf.iter_mut().take(3).enumerate() {
        row[3] = br[k][3] - hm[k][3];
    }
    let fresh = || Scene {
        min: [f32::MAX; 3],
        max: [f32::MIN; 3],
        ..Scene::default()
    };
    let tex = Some("p_bastilah04");
    let mut rigid = fresh();
    rigid.add_model_with(&head, None, &xf, &[], tex, None);
    let mut at_rest = fresh();
    at_rest.add_model_with(
        &head,
        None,
        &xf,
        &[],
        tex,
        Some(&SkinCtx::new(&rest, &rest, xf)),
    );
    let mut moved = fresh();
    moved.add_model_with(
        &head,
        None,
        &xf,
        &[],
        tex,
        Some(&SkinCtx::new(&rest, &posed, xf)),
    );
    assert_eq!(rigid.meshes.len(), at_rest.meshes.len());
    let (mut worst, mut shift) = (0.0f32, 0.0f32);
    for ((a, b), c) in rigid.meshes.iter().zip(&at_rest.meshes).zip(&moved.meshes) {
        for ((p, q), r) in a.positions.iter().zip(&b.positions).zip(&c.positions) {
            worst = worst.max((0..3).map(|k| (p[k] - q[k]).abs()).fold(0.0, f32::max));
            shift = shift.max((0..3).map(|k| (p[k] - r[k]).abs()).fold(0.0, f32::max));
        }
    }
    assert!(
        worst < 1e-3,
        "skin with no motion differs from rigid by {worst}"
    );
    assert!(
        shift > 0.02,
        "idle pose left the skinned head where it was ({shift})"
    );
}

#[test]
fn a_character_walks_the_first_corridor_on_the_floor() {
    use kotor_render::{
        add_area_objects_with, contact_sheet, render_walk, rig_for_appearance, walk_path, Floor,
        WalkOptions,
    };
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.load_module("end_m01aa").unwrap();
    inst.load_texture_packs().unwrap();
    let floor = Floor::load(&mut inst, "m01aa").unwrap();
    // Spawn point to the far end of the first corridor: every 5 cm step has floor under it.
    let path = [[15.4248f32, 20.1215], [30.4, 20.6], [38.4, 20.6]];
    let steps = walk_path(&floor, &path, -1.275, 0.05).unwrap();
    assert!(steps.len() > 400, "{} steps", steps.len());
    assert!((steps.last().unwrap().distance - 23.0).abs() < 0.2);
    assert!(
        steps.iter().all(|s| (s.pos[2] + 1.275).abs() < 0.3),
        "floor height wandered"
    );
    assert!(steps.windows(2).all(|w| w[1].distance >= w[0].distance));
    // A line through solid wall has no floor, and the error says where.
    let err = walk_path(&floor, &[[15.4, 20.1], [15.4, 60.0]], -1.275, 0.05).unwrap_err();
    assert!(err.to_string().contains("no floor at"), "{err}");

    let mut scene = load_area(&mut inst, "m01aa").unwrap();
    add_area_objects_with(&mut scene, &mut inst, "m01aa", false).unwrap();
    let rig = rig_for_appearance(&mut inst, 4, 1).unwrap();
    let len = rig.animation_length("walk").unwrap();
    assert!(len > 0.3 && len < 5.0, "walk cycle {len}s");
    // The walking character keeps its feet at the floor in every phase of the cycle.
    for i in 0..8 {
        let who = rig.pose(Some(("walk", len * i as f32 / 8.0))).unwrap();
        assert!(
            who.min[2] > -0.15 && who.min[2] < 0.3,
            "phase {i}: lowest point {}",
            who.min[2]
        );
    }
    let opts = WalkOptions {
        frames: 6,
        width: 96,
        height: 54,
        animation: "walk",
        metres_per_cycle: 1.6,
    };
    let mut cache = std::collections::HashMap::new();
    let frames = render_walk(
        &scene,
        &rig,
        &steps,
        &mut |n| {
            cache
                .entry(n.to_string())
                .or_insert_with(|| load_texture(&mut inst, n))
                .clone()
        },
        &opts,
    )
    .unwrap();
    assert_eq!(frames.len(), 6);
    assert!(
        frames.windows(2).all(|w| w[0] != w[1]),
        "consecutive frames are identical"
    );
    let sheet = contact_sheet(&frames, 3).unwrap();
    assert_eq!((sheet.width, sheet.height), (96 * 3, 54 * 2));
}
