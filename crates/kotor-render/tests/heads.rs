//! A head stays on its body: whatever the pose, the collar stays around the neck.
use kotor_archive::Install;
use kotor_render::{rig_for_outfit, Scene, SceneMesh};

fn parts<'a>(scene: &'a Scene, head_tex: &str) -> (Vec<&'a SceneMesh>, Vec<&'a SceneMesh>) {
    scene.meshes.iter().partition(|m| {
        m.texture
            .as_deref()
            .is_some_and(|t| t.eq_ignore_ascii_case(head_tex))
    })
}

fn lowest(meshes: &[&SceneMesh]) -> [f32; 3] {
    let mut low = [0.0, 0.0, f32::MAX];
    for p in meshes.iter().flat_map(|m| m.positions.iter()) {
        if p[2] < low[2] {
            low = *p;
        }
    }
    low
}

fn apart(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f32>().sqrt()
}

/// The body's vertices near the bottom of the head at rest, and how far the farthest of them ends up from it in a pose.
fn collar_drift(rest: &Scene, posed: &Scene, head_tex: &str) -> f32 {
    let (rest_head, rest_body) = parts(rest, head_tex);
    let (posed_head, posed_body) = parts(posed, head_tex);
    let (at_rest, at_pose) = (lowest(&rest_head), lowest(&posed_head));
    let mut worst = 0.0f32;
    for (r, p) in rest_body.iter().zip(&posed_body) {
        for (a, b) in r.positions.iter().zip(&p.positions) {
            if apart(*a, at_rest) < 0.08 {
                worst = worst.max(apart(*b, at_pose));
            }
        }
    }
    worst
}

#[test]
fn the_collar_stays_around_the_neck_in_every_common_pose() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    // Women and men of each of the three skin families, in their starting clothes.
    for row in [92, 107, 122, 137, 152, 167] {
        let rig = rig_for_outfit(&mut inst, row, 1, Some('b')).unwrap();
        let head_tex = rig.head_texture().unwrap().to_string();
        let rest = rig.pose(None).unwrap();
        let (head, body) = parts(&rest, &head_tex);
        let top = body
            .iter()
            .flat_map(|m| m.positions.iter())
            .fold(f32::MIN, |t, p| t.max(p[2]));
        assert!(
            lowest(&head)[2] < top,
            "row {row}: at rest the head's bottom is above the body"
        );
        for anim in ["pause1", "pause2", "walk", "run", "g8r1", "g1r1"] {
            let len = rig.animation_length(anim).unwrap_or(1.0);
            for k in 0..4 {
                let posed = rig.pose(Some((anim, len * k as f32 / 4.0))).unwrap();
                let drift = collar_drift(&rest, &posed, &head_tex);
                assert!(
                    drift < 0.2,
                    "row {row} {anim}: the collar is {drift:.3} m from the neck"
                );
            }
        }
    }
}
