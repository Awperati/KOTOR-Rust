use crate::scene::{mat_mul, Scene};
use kotor_archive::Install;
use kotor_core::{Error, ResType, Result};
use kotor_model::{parse_model, Model};

type Mat = [[f32; 4]; 4];

fn load(inst: &mut Install, name: &str) -> Result<Model> {
    let mdl = inst.read(name, ResType::MDL)?;
    let mdx = inst.read(name, ResType::MDX).unwrap_or_default();
    parse_model(&mdl, &mdx)
}

/// Inverse of a rotation + translation matrix (no scale or shear).
fn rigid_inverse(m: &Mat) -> Mat {
    let mut o = [[0.0; 4]; 4];
    for i in 0..3 {
        for j in 0..3 {
            o[i][j] = m[j][i];
        }
        o[i][3] = -(0..3).map(|j| o[i][j] * m[j][3]).sum::<f32>();
    }
    o[3][3] = 1.0;
    o
}

fn find(mats: &[(String, Mat)], name: &str) -> Option<Mat> {
    mats.iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, m)| *m)
}

/// The most used texture among a model's textured meshes: body parts that name no texture of their
/// own take it. This is a stand-in for the engine's appearance-based texture choice (texture name
/// plus the creature's texture variation), which is not implemented.
fn main_texture(model: &Model) -> Option<String> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    model.root.walk(&mut |n| {
        if let Some(t) = n.mesh.as_ref().and_then(|m| m.texture.as_ref()) {
            match counts.iter_mut().find(|(c, _)| c.eq_ignore_ascii_case(t)) {
                Some(e) => e.1 += 1,
                None => counts.push((t.clone(), 1)),
            }
        }
    });
    counts.into_iter().max_by_key(|(_, n)| *n).map(|(t, _)| t)
}

/// Assemble a character: a body model, optionally a head model, and an animation from the body's
/// supermodel sampled at `time` seconds (`animation` of `None` leaves the rest pose).
///
/// The head is attached rigidly: its `head_g` node is moved onto the body's posed `head_g`.
/// Skinned meshes are drawn rigidly at their node transform (no vertex blending yet).
pub fn load_character(
    inst: &mut Install,
    body: &str,
    head: Option<&str>,
    animation: Option<(&str, f32)>,
) -> Result<Scene> {
    let body_m = load(inst, body)?;
    let pose = match (animation, &body_m.supermodel) {
        (Some((anim, t)), Some(sup)) => {
            let sup_m = load(inst, sup)?;
            let a = sup_m
                .animation(anim)
                .ok_or_else(|| Error::NotFound(format!("animation {anim} in {sup}")))?;
            Some(a.sample(&sup_m, t))
        }
        _ => None,
    };
    let mut scene = Scene {
        min: [f32::MAX; 3],
        max: [f32::MIN; 3],
        ..Scene::default()
    };
    let identity: Mat = crate::scene::IDENTITY;
    let hide_on_body: &[&str] = if head.is_some() {
        &["head_g", "neck_g", "necklwr_g"]
    } else {
        &[]
    };
    scene.add_model_with(
        &body_m,
        pose.as_ref(),
        &identity,
        hide_on_body,
        main_texture(&body_m).as_deref(),
    );
    if let Some(h) = head {
        let head_m = load(inst, h)?;
        let body_mats = body_m.world_matrices(pose.as_ref());
        let head_mats = head_m.world_matrices(None);
        let (Some(mb), Some(mh)) = (find(&body_mats, "head_g"), find(&head_mats, "head_g")) else {
            return Err(Error::format("body or head model has no head_g node"));
        };
        let xf = mat_mul(&mb, &rigid_inverse(&mh));
        scene.add_model_with(&head_m, None, &xf, &[], main_texture(&head_m).as_deref());
    }
    if scene.meshes.is_empty() {
        return Err(Error::format("character has no drawable meshes"));
    }
    Ok(scene)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rigid_inverse_undoes_the_transform() {
        // 90 degrees about Z, then translate (1, 2, 3).
        let m: Mat = [
            [0., -1., 0., 1.],
            [1., 0., 0., 2.],
            [0., 0., 1., 3.],
            [0., 0., 0., 1.],
        ];
        let i = mat_mul(&m, &rigid_inverse(&m));
        for r in 0..4 {
            for c in 0..4 {
                let want = if r == c { 1.0 } else { 0.0 };
                assert!((i[r][c] - want).abs() < 1e-6, "{i:?}");
            }
        }
    }
}
