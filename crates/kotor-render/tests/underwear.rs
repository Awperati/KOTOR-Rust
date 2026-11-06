//! The conventional underwear style: which textures it touches, and that it leaves what it should.

use kotor_archive::Install;
use kotor_render::{load_texture, underwear};

#[test]
fn only_the_three_adult_player_bodies_are_touched() {
    for name in ["pfbam01", "PFBAMA03", "pfbasb05", "pfbal02"] {
        assert!(underwear::body_of_texture(name).is_some(), "{name}");
    }
    // Armour, men, children, and every named character, Mission included.
    for name in [
        "pfbbm01",
        "pmbam01",
        "n_commkidf01",
        "n_commkidm01",
        "p_missionba01",
        "p_bastilaba01",
        "p_juhaniba01",
        "n_commf01",
        "pfbam",
    ] {
        assert!(underwear::body_of_texture(name).is_none(), "{name}");
    }
}

#[test]
fn every_body_and_skin_loses_cloth_and_keeps_its_size_and_its_lower_half() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.load_texture_packs().unwrap();
    for body in ["pfbas", "pfbam", "pfbal"] {
        let plan = underwear::plan_for(&mut inst, body).unwrap();
        let (cleared, _) = plan.counts();
        assert!(cleared > 4000, "{body}: {cleared} texels");
        for skin in ["", "a", "b"] {
            for var in 1..=5 {
                let name = format!("{body}{skin}{var:02}");
                let Some(before) = load_texture(&mut inst, &name) else {
                    continue;
                };
                let mut after = before.clone();
                plan.apply(&mut after);
                assert_eq!((after.width, after.height), (before.width, before.height));
                assert_ne!(after.rgba, before.rgba, "{name} changed");
                let same = before.rgba.iter().zip(&after.rgba).filter(|(a, b)| a == b);
                // Most of the sheet is the rest of the body, which stays as it was.
                let share = same.count() as f32 / before.rgba.len() as f32;
                assert!(share > 0.7, "{name}: only {share} left alone");
                // Alpha is never touched.
                let alpha =
                    |i: &kotor_image::Image| i.rgba.chunks(4).map(|p| p[3]).collect::<Vec<u8>>();
                assert_eq!(alpha(&before), alpha(&after), "{name}");
            }
        }
    }
}

#[test]
fn the_painted_skin_cannot_be_told_from_the_skin_beside_it_in_any_family() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.load_texture_packs().unwrap();
    for body in ["pfbas", "pfbam", "pfbal"] {
        let plan = underwear::plan_for(&mut inst, body).unwrap();
        for family in ["", "a", "b"] {
            let name = format!("{body}{family}01");
            let mut t = load_texture(&mut inst, &name).unwrap();
            plan.apply(&mut t);
            let r = plan.seam_report(&t);
            // Levels of 255. Skin's own painted shading is about 2 from one texel to the next.
            assert!(r.edge < 3.0, "{name}: {r:?}");
            assert!(r.wrap < 4.0, "{name}: {r:?}");
            assert!(r.colour < 8.0, "{name}: {r:?}");
            assert!(r.worst_colour < 22.0, "{name}: {r:?}");
            assert!(r.cloth_left < 0.01, "{name}: {r:?}");
        }
    }
}

#[test]
fn evening_out_the_shading_leaves_the_shape_and_the_lower_body() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    let mdl = inst.read("pfbam", kotor_core::ResType::MDL).unwrap();
    let mdx = inst.read("pfbam", kotor_core::ResType::MDX).unwrap();
    let before = kotor_model::parse_model(&mdl, &mdx).unwrap();
    let mut after = kotor_model::parse_model(&mdl, &mdx).unwrap();
    underwear::soften_shading(&mut after);
    let mesh =
        |m: &'_ kotor_model::Model, name: &str| m.root.find(name).unwrap().mesh.clone().unwrap();
    let (old, new) = (mesh(&before, "Torso"), mesh(&after, "Torso"));
    assert_eq!(old.positions, new.positions);
    assert_eq!(old.normals.len(), new.normals.len());
    assert_ne!(old.normals, new.normals);
    let same = old
        .normals
        .iter()
        .zip(&new.normals)
        .filter(|(a, b)| a == b)
        .count();
    assert!(same > 0, "what is below the waist stays");
    for n in &new.normals {
        let long = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        assert!((long - 1.0).abs() < 0.2, "{long}");
    }
}

#[test]
fn the_rule_works_at_any_size_of_texture() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.load_texture_packs().unwrap();
    let plan = underwear::plan_for(&mut inst, "pfbam").unwrap();
    let base = load_texture(&mut inst, "pfbam01").unwrap();
    // A picture twice the size, as an upscaled one is.
    let (w, h) = (base.width as usize, base.height as usize);
    let mut big = kotor_image::Image {
        width: base.width * 2,
        height: base.height * 2,
        rgba: vec![0; w * h * 16],
    };
    for y in 0..h * 2 {
        for x in 0..w * 2 {
            let from = (y / 2 * w + x / 2) * 4;
            let to = (y * w * 2 + x) * 4;
            big.rgba[to..to + 4].copy_from_slice(&base.rgba[from..from + 4]);
        }
    }
    let mut small = base.clone();
    plan.apply(&mut small);
    plan.apply(&mut big);
    // The share of the sheet that changes is the same at both sizes, within a little.
    let changed = |a: &kotor_image::Image, b: &kotor_image::Image| {
        let n = a
            .rgba
            .chunks(4)
            .zip(b.rgba.chunks(4))
            .filter(|(p, q)| p != q)
            .count();
        n as f32 / (a.width * a.height) as f32
    };
    let mut twice = kotor_image::Image {
        width: base.width * 2,
        height: base.height * 2,
        rgba: vec![0; w * h * 16],
    };
    for y in 0..h * 2 {
        for x in 0..w * 2 {
            let from = (y / 2 * w + x / 2) * 4;
            let to = (y * w * 2 + x) * 4;
            twice.rgba[to..to + 4].copy_from_slice(&base.rgba[from..from + 4]);
        }
    }
    let (s, b) = (changed(&base, &small), changed(&twice, &big));
    assert!((s - b).abs() < 0.02, "{s} and {b}");
}
