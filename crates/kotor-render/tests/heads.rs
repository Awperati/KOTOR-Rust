//! A head stays on its body, and a hero's skin matches between the two.
use kotor_archive::Install;
use kotor_render::{rig_for_outfit, Scene};

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
