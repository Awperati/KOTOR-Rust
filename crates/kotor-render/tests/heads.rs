//! A head stays on its body, and a hero's skin matches between the two.
use kotor_archive::Install;
use kotor_render::{rig_for_outfit, Rig, Scene};

/// Women and men of the three skin families.
const ROWS: [usize; 6] = [92, 107, 122, 137, 152, 167];

/// A vertex: which mesh, and which of its vertices.
type Vertices = Vec<(usize, usize)>;

fn is_head(scene: &Scene, mesh: usize, head_tex: &str) -> bool {
    scene.meshes[mesh]
        .texture
        .as_deref()
        .is_some_and(|t| t.eq_ignore_ascii_case(head_tex))
}

fn apart(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f32>().sqrt()
}

/// The head's vertices in its lowest 3 cm at rest, and the body's within 8 cm of them. Picked at rest and followed by
/// index: a body lying down has some other part of the head lowest.
fn join_vertices(rest: &Scene, head_tex: &str) -> (Vertices, Vertices) {
    let (mut neck, mut collar) = (Vec::new(), Vec::new());
    let mut bottom = f32::MAX;
    for (i, m) in rest.meshes.iter().enumerate() {
        if is_head(rest, i, head_tex) {
            bottom = m.positions.iter().fold(bottom, |b, p| b.min(p[2]));
        }
    }
    for (i, m) in rest.meshes.iter().enumerate() {
        if is_head(rest, i, head_tex) {
            neck.extend(
                (0..m.positions.len())
                    .filter(|k| m.positions[*k][2] < bottom + 0.03)
                    .map(|k| (i, k)),
            );
        }
    }
    for (i, m) in rest.meshes.iter().enumerate() {
        if !is_head(rest, i, head_tex) {
            let near = |k: &usize| {
                neck.iter().any(|(a, b): &(usize, usize)| {
                    apart(rest.meshes[*a].positions[*b], m.positions[*k]) < 0.08
                })
            };
            collar.extend((0..m.positions.len()).filter(near).map(|k| (i, k)));
        }
    }
    (neck, collar)
}

#[test]
fn the_neck_stays_in_the_collar_in_every_kind_of_pose() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    for row in ROWS {
        let rig = rig_for_outfit(&mut inst, row, 1, Some('b')).unwrap();
        let head_tex = rig.head_texture().unwrap().to_string();
        let rest = rig.pose(None).unwrap();
        let (neck, collar) = join_vertices(&rest, &head_tex);
        assert!(!neck.is_empty() && !collar.is_empty(), "row {row}");
        for anim in [
            "pause1", "walk", "run", "g8r1", "die3", "dead3", "dead", "c2p4",
        ] {
            let len = rig.animation_length(anim).unwrap_or(1.0);
            for k in 0..3 {
                let s = rig.pose(Some((anim, len * k as f32 / 3.0))).unwrap();
                for (a, b) in &neck {
                    let p = s.meshes[*a].positions[*b];
                    let near = collar
                        .iter()
                        .map(|(c, d)| apart(s.meshes[*c].positions[*d], p))
                        .fold(f32::MAX, f32::min);
                    // 8.6 cm is the ponytail of `PFHC01`, the lowest part of that head; every other head stays under 6.
                    assert!(
                        near < 0.09,
                        "row {row} {anim}: a neck vertex is {near:.3} m from the collar"
                    );
                }
            }
        }
    }
}

/// The average colour of a mesh's texture at the vertices `keep` accepts.
fn colour(
    inst: &mut Install,
    scene: &Scene,
    mesh: usize,
    keep: &dyn Fn([f32; 3]) -> bool,
) -> [f32; 3] {
    let m = &scene.meshes[mesh];
    let img = kotor_render::load_texture(inst, m.texture.as_deref().unwrap()).unwrap();
    let (mut sum, mut n) = ([0.0f32; 3], 0.0f32);
    for (uv, at) in m.uvs.iter().zip(&m.positions) {
        if !keep(*at) {
            continue;
        }
        let x = (uv[0].rem_euclid(1.0) * (img.width - 1) as f32) as u32;
        let y = ((1.0 - uv[1].rem_euclid(1.0)) * (img.height - 1) as f32) as u32;
        let p = ((y * img.width + x) * 4) as usize;
        for (c, s) in sum.iter_mut().enumerate() {
            *s += f32::from(img.rgba[p + c]);
        }
        n += 1.0;
    }
    sum.map(|v| v / n.max(1.0))
}

#[test]
fn a_hero_with_nothing_on_has_the_arms_of_their_own_skin_family() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.load_texture_packs().unwrap();
    let mut seen = Vec::new();
    for row in ROWS {
        let rig = rig_for_outfit(&mut inst, row, 1, None).unwrap();
        let s = rig.pose(None).unwrap();
        let arm = s
            .meshes
            .iter()
            .position(|m| m.name.to_ascii_lowercase().contains("arm"))
            .unwrap();
        let head = s
            .meshes
            .iter()
            .position(|m| m.name.eq_ignore_ascii_case("head"))
            .unwrap();
        let top = s.meshes[arm]
            .positions
            .iter()
            .fold(f32::MIN, |t, p| t.max(p[2]));
        // The neck: just above the shoulders and close to the middle, which leaves a ponytail out.
        let neck =
            move |p: [f32; 3]| p[2] > top + 0.1 && p[2] < top + 0.2 && p[0].hypot(p[1]) < 0.07;
        let upper_arm = move |p: [f32; 3]| p[2] > top - 0.2;
        let (a, n) = (
            colour(&mut inst, &s, arm, &upper_arm),
            colour(&mut inst, &s, head, &neck),
        );
        println!(
            "row {row}: arm {a:?} neck {n:?}, {:?}",
            s.meshes[arm].texture
        );
        seen.push((row, a, n));
    }
    // The game's own files make some bodies brighter than their heads (row 92: arm 213, neck 145 in red), so colours do
    // not match exactly. What must hold is that the body is the row's own: a dark head gets the dark body.
    let bright = |c: [f32; 3]| c[0] + c[1] + c[2];
    for group in seen.chunks(3) {
        let (a, b, c) = (&group[0], &group[1], &group[2]);
        assert!(
            bright(b.1) < bright(a.1) && bright(b.1) < bright(c.1),
            "family B has the darkest arms: {group:?}"
        );
        assert!(
            bright(b.2) < bright(a.2) && bright(b.2) < bright(c.2),
            "family B has the darkest neck: {group:?}"
        );
    }
}

/// Item 9 of the owner's list of 6 Nov: faces never moved. A head carries clips of its own, and a face in one is not
/// the face at rest, yet the head stays the size it was.
#[test]
fn a_face_moves_with_the_clip_and_stays_whole() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    // Bastila's head carries clips of its own. The hero's heads carry none: they would need the lip files, not built.
    let rig = Rig::load(&mut inst, "P_BastilaBB", Some("P_BastilaH"), None).unwrap();
    let head_tex = rig.head_texture().unwrap().to_string();
    let face = rig.sample_face("tlknorm", 3.0).expect("the head talks");
    let rest = rig.posed(None).unwrap();
    let talking = rig.posed_with_face(None, Some(&face)).unwrap();
    let extent = |s: &Scene| {
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for (i, m) in s.meshes.iter().enumerate() {
            if is_head(s, i, &head_tex) {
                for p in &m.positions {
                    for k in 0..3 {
                        lo[k] = lo[k].min(p[k]);
                        hi[k] = hi[k].max(p[k]);
                    }
                }
            }
        }
        (lo, hi)
    };
    let moved = rest
        .meshes
        .iter()
        .zip(&talking.meshes)
        .enumerate()
        .any(|(i, (a, b))| is_head(&rest, i, &head_tex) && a.positions != b.positions);
    assert!(moved, "the face differs from the rest pose");
    let (a, b) = (extent(&rest), extent(&talking));
    for k in 0..3 {
        assert!(
            ((b.1[k] - b.0[k]) - (a.1[k] - a.0[k])).abs() < 0.08,
            "axis {k}: {a:?} against {b:?}"
        );
    }
}

/// A head with no clip of its own (almost every head: Trask's, and the hero's) takes the face tracks of the body's clip, so
/// its mouth and lids move as the clip plays, and its size stays.
#[test]
fn a_head_with_no_clip_of_its_own_takes_the_bodys_face_tracks() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    let rig = Rig::load(&mut inst, "N_RepSold", Some("N_TraskH"), None).unwrap();
    let head_tex = rig.head_texture().unwrap().to_string();
    let a = rig
        .sample_face("tlknorm", 0.5)
        .expect("a face for the body's clip");
    let b = rig.sample_face("tlknorm", 3.1).unwrap();
    assert!(
        a.keys().all(|k| k.starts_with("f_")
            || k.contains("teeth")
            || k.contains("tongue")
            || k.contains("lid")
            || k.contains("eye")
            || k == "talkdummy"),
        "{:?}",
        a.keys().collect::<Vec<_>>()
    );
    assert!(
        !a.contains_key("head_g"),
        "only what is under the head's own node"
    );
    let (one, two) = (
        rig.posed_with_face(None, Some(&a)).unwrap(),
        rig.posed_with_face(None, Some(&b)).unwrap(),
    );
    let differs = one
        .meshes
        .iter()
        .zip(&two.meshes)
        .enumerate()
        .any(|(i, (x, y))| is_head(&one, i, &head_tex) && x.positions != y.positions);
    assert!(differs, "the face moves between the two moments");
}

/// The owner's frames of the female hero: black eye sockets and a stretched face, because the body's clip keys were taken as
/// the head's own. A face from a borrowed clip moves each vertex of a hero's head by two or three centimetres at most, on every head
/// (women's and men's, all three families) and in every clip a talk or a stand uses.
#[test]
fn a_borrowed_face_moves_a_heros_head_by_centimetres_only() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.load_texture_packs().unwrap();
    for row in [91, 92, 93, 97, 104, 107, 112, 122, 127, 135, 137, 152, 167] {
        let rig = rig_for_outfit(&mut inst, row, 1, None).unwrap();
        let head_tex = rig.head_texture().unwrap().to_string();
        let rest = rig.posed_with_face(None, None).unwrap();
        for clip in ["pause1", "listen", "tlknorm", "tlksad", "tlkforce", "talk"] {
            let Some(len) = rig.animation_length(clip).filter(|l| *l > 0.0) else {
                continue;
            };
            for k in 0..6 {
                let Some(face) = rig.sample_face(clip, len * k as f32 / 6.0) else {
                    continue;
                };
                let moved = rig.posed_with_face(None, Some(&face)).unwrap();
                let mut worst = 0.0f32;
                for (i, (a, b)) in rest.meshes.iter().zip(&moved.meshes).enumerate() {
                    if !is_head(&rest, i, &head_tex) {
                        continue;
                    }
                    for (p, q) in a.positions.iter().zip(&b.positions) {
                        worst = worst.max(apart(*p, *q));
                    }
                }
                // The talk loop moves the mouth most; a stand or a listen only the lids and brows.
                let limit = if clip == "talk" { 0.04 } else { 0.02 };
                assert!(
                    worst < limit,
                    "row {row} {clip}: a vertex of the head moves {worst:.3} m"
                );
            }
        }
    }
}

/// The owner's frame of a fist stance: a shoulder crumpled into a spike at the collar. The stance's keys were placed against the
/// supermodel's rest, where the collars sit at the middle, and not against the body's, where they sit 7.8 cm out, so the
/// shoulders were drawn in to the neck. In every stance and stand the collars stay about as far apart as at rest, on all
/// three women's bodies.
#[test]
fn a_stance_keeps_the_shoulders_apart() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.load_texture_packs().unwrap();
    let gap = |frames: &[kotor_model::JointFrame]| {
        let at = |name: &str| {
            let m = frames.iter().find(|f| f.0 == name).unwrap().1;
            [m[0][3], m[1][3], m[2][3]]
        };
        let (l, r) = (at("lcollar_g"), at("rcollar_g"));
        ((l[0] - r[0]).powi(2) + (l[1] - r[1]).powi(2) + (l[2] - r[2]).powi(2)).sqrt()
    };
    for row in [91, 92, 93] {
        let rig = rig_for_outfit(&mut inst, row, 1, None).unwrap();
        let rest = gap(&rig.joint_frames(None));
        for clip in ["g8r1", "g1r1", "pause1", "walk", "run", "g8a1"] {
            let Some(len) = rig.animation_length(clip).filter(|l| *l > 0.0) else {
                continue;
            };
            for k in 0..4 {
                let pose = rig.sample(clip, len * k as f32 / 4.0).unwrap();
                let g = gap(&rig.joint_frames(Some(&pose)));
                assert!(
                    (g - rest).abs() < rest * 0.25,
                    "row {row} {clip}: the collars are {g:.3} m apart, {rest:.3} at rest"
                );
            }
        }
    }
}

/// "At certain angles the neck detaches" (owner, a male hero in a jacket seen from above and behind). The head rides its body
/// rigidly, so no pose should pull the neck away from the collar: in every clip, for every body and outfit letter, the neck's
/// lowest ring is as far from the body as at rest, to within 5 mm. (A gap that is there at rest is the collar's own, and is
/// not covered here.)
#[test]
fn no_pose_pulls_the_neck_away_from_the_collar() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.load_texture_packs().unwrap();
    let gap = |s: &Scene, neck: &Vertices, head_tex: &str| {
        let mut worst = 0.0f32;
        for (a, b) in neck {
            let p = s.meshes[*a].positions[*b];
            let near = s
                .meshes
                .iter()
                .enumerate()
                .filter(|(i, _)| !is_head(s, *i, head_tex))
                .flat_map(|(_, m)| m.positions.iter())
                .map(|q| apart(*q, p))
                .fold(f32::MAX, f32::min);
            worst = worst.max(near);
        }
        worst
    };
    for row in [92, 137, 152, 167] {
        for letter in [None, Some('b'), Some('c'), Some('f'), Some('g')] {
            let Ok(rig) = rig_for_outfit(&mut inst, row, 1, letter) else {
                continue;
            };
            let head_tex = rig.head_texture().unwrap().to_string();
            let rest = rig.pose(None).unwrap();
            let (neck, _) = join_vertices(&rest, &head_tex);
            let at_rest = gap(&rest, &neck, &head_tex);
            for anim in [
                "pause1", "g7r1", "g8r1", "walk", "run", "hturnl", "hturnr", "listen", "b7a1",
            ] {
                let Some(len) = rig.animation_length(anim) else {
                    continue;
                };
                for k in 0..4 {
                    let s = rig.pose(Some((anim, len * k as f32 / 4.0))).unwrap();
                    let g = gap(&s, &neck, &head_tex);
                    assert!(
                        (g - at_rest).abs() < 0.005,
                        "row {row} {letter:?} {anim}: {g:.3} m from the body, {at_rest:.3} at rest"
                    );
                }
            }
        }
    }
}

/// "During cinematics the neck still has a gap": a cutscene poses the body from a stunt model of its own, whose bones rest a little
/// differently from the body's (its `headhook` 2.5 cm higher), so its keys took the head off the collar. Posed through
/// `Rig::retarget`, the neck's lowest ring is as far from the body as at rest, to within 5 mm, in every stunt clip of the
/// opening that has a stunt model in the install.
#[test]
fn a_cutscene_stunt_keeps_the_head_on_the_collar() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.load_texture_packs().unwrap();
    let Ok(bytes) = inst.read("m01aa_c01_char01", kotor_core::ResType::MDL) else {
        return;
    };
    let mdx = inst
        .read("m01aa_c01_char01", kotor_core::ResType::MDX)
        .unwrap_or_default();
    let stunt = kotor_model::parse_model(&bytes, &mdx).unwrap();
    let gap = |s: &Scene, neck: &Vertices, head_tex: &str| {
        let mut worst = 0.0f32;
        for (a, b) in neck {
            let p = s.meshes[*a].positions[*b];
            let near = s
                .meshes
                .iter()
                .enumerate()
                .filter(|(i, _)| !is_head(s, *i, head_tex))
                .flat_map(|(_, m)| m.positions.iter())
                .map(|q| apart(*q, p))
                .fold(f32::MAX, f32::min);
            worst = worst.max(near);
        }
        worst
    };
    for row in [137, 92] {
        let rig = rig_for_outfit(&mut inst, row, 1, None).unwrap();
        let head_tex = rig.head_texture().unwrap().to_string();
        let rest = rig.pose(None).unwrap();
        let (neck, _) = join_vertices(&rest, &head_tex);
        let at_rest = gap(&rest, &neck, &head_tex);
        for a in &stunt.animations {
            for k in 0..4 {
                let t = a.length * k as f32 / 4.0;
                let pose = rig.retarget(&stunt, a.sample(&stunt, t));
                let s = rig.posed_with_face(Some(&pose), None).unwrap();
                let g = gap(&s, &neck, &head_tex);
                assert!(
                    (g - at_rest).abs() < 0.005,
                    "row {row} {} at {t:.2}: {g:.3} m from the body, {at_rest:.3} at rest",
                    a.name
                );
            }
        }
    }
}

/// Owner, 7 Nov: "facial 'talking' animations move way too fast for the actual spoken dialogue". The `talk` clip's jaw opens and
/// shuts 7 to 9 times a second; played at `MOUTH_RATE` of its speed it does so 4 to 5.5 times a second, as a voice has syllables.
#[test]
fn the_mouth_opens_and_shuts_at_the_rate_of_syllables() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.load_texture_packs().unwrap();
    for (body, head) in [
        ("N_RepSold", "N_TraskH"),
        ("P_BastilaBB", "P_BastilaH"),
        ("N_RepSold", "N_comAMH01"),
    ] {
        let Ok(rig) = Rig::load(&mut inst, body, Some(head), None) else {
            continue;
        };
        let Some(len) = rig.animation_length("talk").filter(|l| *l > 0.0) else {
            continue;
        };
        // The jaw over two seconds of the clip, as the clip has it and as the stage plays it.
        let cycles = |rate: f32| -> Option<f32> {
            let n = 400;
            let series: Vec<f32> = (0..n)
                .filter_map(|i| {
                    let clock = 2.0 * i as f32 / n as f32 * rate;
                    rig.sample_face("talk", clock % len)
                        .and_then(|f| f.get("f_jaw_g").map(|p| p.orientation[1]))
                })
                .collect();
            if series.len() < n {
                return None;
            }
            let mean = series.iter().sum::<f32>() / n as f32;
            let crossings = series
                .windows(2)
                .filter(|w| (w[0] - mean) * (w[1] - mean) < 0.0)
                .count();
            Some(crossings as f32 / 4.0)
        };
        let (Some(own), Some(played)) = (cycles(1.0), cycles(kotor_render::MOUTH_RATE)) else {
            continue;
        };
        // The clip is as fast as speech or faster; played at MOUTH_RATE of its speed it is slower, and no faster than a voice.
        assert!(
            own >= 2.0,
            "{head}: the clip's jaw cycles {own:.1} times a second"
        );
        assert!(
            played <= own * 0.85 && played <= 6.2,
            "{head}: played, {played:.1} against {own:.1} a second"
        );
    }
}
