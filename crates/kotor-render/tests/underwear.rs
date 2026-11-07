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
            // Down the middle of the belly, where the middle body had its centre bar.
            assert!(r.midline < 8.0, "{name}: {r:?}");
            // Neither flat nor rougher than the skin beside it.
            let ratio = r.grain / r.skin_grain;
            assert!((1.0 / 3.0..3.0).contains(&ratio), "{name}: {r:?}");
        }
    }
}

#[test]
fn the_edge_of_the_bra_is_crisp_at_any_size() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.load_texture_packs().unwrap();
    for body in ["pfbas", "pfbam", "pfbal"] {
        let plan = underwear::plan_for(&mut inst, body).unwrap();
        // In texels of the game's own picture: two at most, whatever the size of the upscaled one.
        for size in [512, 1024] {
            let width = plan.bra_edge_width(size, size);
            assert!(width <= 2.01, "{body} at {size}: {width}");
        }
    }
}

#[test]
fn a_head_meets_each_body_of_its_skin_at_the_same_colour() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.load_texture_packs().unwrap();
    let plans: Vec<_> = ["pfbas", "pfbam", "pfbal"]
        .iter()
        .map(|b| (*b, underwear::plan_for(&mut inst, b).unwrap()))
        .collect();
    for (letter, family) in [("a", "a"), ("b", "b"), ("c", "")] {
        for (body, plan) in &plans {
            let name = format!("{body}{family}01");
            let mut skin = load_texture(&mut inst, &name).unwrap();
            plan.apply(&mut skin);
            let at_body = plan.neck_colour(&skin);
            for number in 1..=5 {
                let head = format!("pfh{letter}{number:02}");
                let Some(mut face) = load_texture(&mut inst, &head) else {
                    continue;
                };
                let neck = underwear::neck_plan_for(&mut inst, &head).unwrap();
                neck.apply(&mut face);
                let at_head = neck.ring_colour(&face);
                let step = (0..3).map(|k| (at_head[k] - at_body[k]).abs()).sum::<f32>() / 3.0;
                assert!(
                    step < NECK_LIMIT,
                    "{head} on {name}: {at_head:?} and {at_body:?}"
                );
            }
        }
    }
}

/// Levels of 255, as a mean over red, green and blue.
const NECK_LIMIT: f32 = 3.0;

#[test]
fn the_neck_reaches_down_and_the_rest_of_the_head_stays() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    let mdl = inst.read("pfha01", kotor_core::ResType::MDL).unwrap();
    let mdx = inst.read("pfha01", kotor_core::ResType::MDX).unwrap();
    let before = kotor_model::parse_model(&mdl, &mdx).unwrap();
    let mut after = kotor_model::parse_model(&mdl, &mdx).unwrap();
    underwear::close_neck_gap(&mut after, "pfha01");
    let mesh = |m: &'_ kotor_model::Model| m.root.find("head").unwrap().mesh.clone().unwrap();
    let (old, new) = (mesh(&before), mesh(&after));
    let moved = old
        .positions
        .iter()
        .zip(&new.positions)
        .filter(|(a, b)| a != b)
        .count();
    assert!(moved > 0, "the lowest of the neck moves");
    assert!(
        moved * 4 < old.positions.len(),
        "{moved} of {}",
        old.positions.len()
    );
    assert_eq!(old.uvs, new.uvs);
}

#[test]
fn only_a_heros_head_has_its_neck_changed() {
    for name in ["pfha01", "PFHB05", "pfhc03", "pmha01", "pmhc05"] {
        assert!(underwear::is_hero_head(name), "{name}");
    }
    for name in ["pfbam01", "n_commf01", "pfha", "pfhd01", "p_bastilah"] {
        assert!(!underwear::is_hero_head(name), "{name}");
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

/// Where a woman's neck meets her body the two are lit by the same normals, so the join shows no step in brightness.
#[test]
fn the_neck_is_lit_like_the_body_where_they_meet() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.load_texture_packs().unwrap();
    // Every size, and heads whose hair leaves the back of the neck bare (rows 97 and 112); a ponytail that hangs below the neck is no neck.
    for row in [91, 92, 93, 97, 112, 118, 121, 127, 133] {
        let mut rig = kotor_render::rig_for_appearance(&mut inst, row, 1).unwrap();
        let plain = rig.pose(None).unwrap();
        rig.soften_underwear_shading();
        let scene = rig.pose(None).unwrap();
        let head = |n: &str| underwear::is_hero_head(n);
        // The head's lowest vertices, and the nearest body normal to each.
        let mut worst_before = 0.0f32;
        let mut worst_after = 0.0f32;
        for (which, worst) in [(&plain, &mut worst_before), (&scene, &mut worst_after)] {
            let is = |m: &kotor_render::SceneMesh| m.texture.as_deref().is_some_and(head);
            let low = which
                .meshes
                .iter()
                .filter(|m| is(m))
                .flat_map(|m| m.positions.iter())
                .fold(f32::MAX, |b, p| b.min(p[2]));
            let body: Vec<_> = which
                .meshes
                .iter()
                .filter(|m| !is(m))
                .flat_map(|m| m.positions.iter().zip(m.normals.iter()))
                .collect();
            for m in which.meshes.iter().filter(|m| is(m)) {
                for (p, n) in m.positions.iter().zip(&m.normals) {
                    // As far up as the neck shows above the collar from behind.
                    if p[2] > low + 0.09 {
                        continue;
                    }
                    let near = body
                        .iter()
                        .min_by(|a, b| dist2(a.0, p).total_cmp(&dist2(b.0, p)))
                        .unwrap();
                    if dist2(near.0, p) > 0.16 * 0.16 {
                        continue;
                    }
                    let dot: f32 = (0..3).map(|k| n[k] * near.1[k]).sum();
                    *worst = worst.max(1.0 - dot);
                }
            }
        }
        assert!(
            worst_after < worst_before * 0.5 + 0.02,
            "row {row}: {worst_before} -> {worst_after}"
        );
        assert!(worst_after < 0.2, "row {row}: {worst_after}");
    }
}

fn dist2(a: &[f32; 3], b: &[f32; 3]) -> f32 {
    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum()
}

/// The choices for a player's underwear: the standard paint and each mod folder, whose blues colour our own bra and briefs.
#[test]
fn the_variants_are_the_standard_paint_and_each_mod_folder() {
    let list = underwear::list_underwear_variants();
    assert_eq!(list[0].id, "", "the standard paint comes first");
    assert!(list[0].fits);
    for v in &list[1..] {
        assert!(!v.id.is_empty() && v.fits, "{v:?}");
        assert!(underwear::variant_dir(&v.id).is_some(), "{v:?}");
        assert!(underwear::variant_palette(&v.id).is_some(), "{v:?}");
    }
    assert!(underwear::variant_dir("..").is_none());
    assert!(underwear::variant_dir("a/b").is_none());
    assert!(underwear::variant_palette("").is_none());
    assert_eq!(underwear::variant_texture_name("pfbam01", ""), "pfbam01");
    assert_eq!(
        underwear::split_variant("pfbam01@Blue"),
        ("pfbam01", "Blue")
    );
    assert_eq!(underwear::split_variant("pfbam01"), ("pfbam01", ""));
}

/// Our bra and briefs take the blues of a variant's sheet on every body and skin, and the skin stays as it was.
#[test]
fn a_variants_blues_colour_our_bra_and_briefs_and_leave_the_skin() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let Some(v) = underwear::list_underwear_variants().into_iter().nth(1) else {
        return;
    };
    let palette = underwear::variant_palette(&v.id).unwrap();
    let mut inst = Install::open(dir).unwrap();
    inst.load_texture_packs().unwrap();
    let blue =
        |p: &[u8; 4]| i32::from(p[2]) > i32::from(p[0]) + 30 && i32::from(p[2]) > i32::from(p[1]);
    for (body, name) in [
        ("pfbas", "pfbasa01"),
        ("pfbam", "pfbam01"),
        ("pfbal", "pfbalb01"),
    ] {
        let plan = underwear::plan_for(&mut inst, body).unwrap();
        let mut plain = load_texture(&mut inst, name).unwrap();
        plan.apply(&mut plain);
        let mut tinted = plain.clone();
        plan.tint(&mut tinted, &palette);
        let before = plain
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| blue(p))
            .count();
        let after = tinted
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| blue(p))
            .count();
        assert!(before < 1000, "{name}: {before} blue texels to start with");
        assert!(after > 3000, "{name}: {after} blue texels once tinted");
        // What is not cloth is where it was: most of the sheet is skin.
        let same = plain
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .zip(tinted.rgba.as_chunks::<4>().0)
            .filter(|(a, b)| a == b)
            .count();
        assert!(
            same > plain.rgba.len() / 4 / 2,
            "{name}: only {same} texels kept"
        );
    }
}

/// The base mesh of a woman's torso is flat under the painted cups, so in profile the bra stood off the skin. The chest is
/// shaped at load: the front of it comes forward by a few centimetres, nothing at the back moves, and no vertex moves far.
#[test]
fn the_chest_has_a_shape_for_the_bra_to_sit_on() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    for body in ["pfbas", "pfbam", "pfbal"] {
        let mdl = inst.read(body, kotor_core::ResType::MDL).unwrap();
        let mdx = inst
            .read(body, kotor_core::ResType::MDX)
            .unwrap_or_default();
        let before = kotor_model::parse_model(&mdl, &mdx).unwrap();
        let mut after = kotor_model::parse_model(&mdl, &mdx).unwrap();
        underwear::shape_chest(&mut after);
        let mesh = |m: &kotor_model::Model| m.root.find("Torso").unwrap().mesh.clone().unwrap();
        let (a, b) = (mesh(&before), mesh(&after));
        let moves: Vec<f32> = a
            .positions
            .iter()
            .zip(&b.positions)
            .map(|(p, q)| (0..3).map(|k| (p[k] - q[k]).powi(2)).sum::<f32>().sqrt())
            .collect();
        let most = moves.iter().cloned().fold(0.0, f32::max);
        let moved = moves.iter().filter(|d| **d > 0.0).count();
        assert!(
            (0.01..0.05).contains(&most),
            "{body}: the chest comes forward by {most:.3} m at most"
        );
        assert!(
            moved > 8 && moved * 4 < moves.len(),
            "{body}: {moved} of {} vertices move",
            moves.len()
        );
    }
}
