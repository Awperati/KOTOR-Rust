use crate::scene::{mat_mul, rigid_inverse, Scene, SkinCtx};
use kotor_archive::Install;
use kotor_core::{Error, ResType, Result};
use kotor_model::{parse_model, Model};
use kotor_tables::TwoDa;

type Mat = [[f32; 4]; 4];

fn load(inst: &mut Install, name: &str) -> Result<Model> {
    let mdl = inst.read(name, ResType::MDL)?;
    let mdx = inst.read(name, ResType::MDX).unwrap_or_default();
    parse_model(&mdl, &mdx)
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
    load_character_with(inst, body, head, animation, None)
}

/// Like [`load_character`], with a texture name for body meshes that name none of their own.
pub fn load_character_with(
    inst: &mut Install,
    body: &str,
    head: Option<&str>,
    animation: Option<(&str, f32)>,
    body_texture: Option<String>,
) -> Result<Scene> {
    let body_m = load(inst, body)?;
    let pose = match (animation, &body_m.supermodel) {
        (Some((anim, t)), Some(sup)) => {
            let sup_m = load(inst, sup)?;
            // A supermodel without that animation leaves the character in its rest pose.
            sup_m.animation(anim).map(|a| a.sample(&sup_m, t))
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
    let body_tex = body_texture.or_else(|| main_texture(&body_m));
    let rest_mats = body_m.world_matrices(None);
    let posed_mats = body_m.world_matrices(pose.as_ref());
    let body_skin = SkinCtx::new(&rest_mats, &posed_mats, identity);
    scene.add_model_with(
        &body_m,
        pose.as_ref(),
        &identity,
        hide_on_body,
        body_tex.as_deref(),
        Some(&body_skin),
    );
    if let Some(h) = head {
        let head_m = load(inst, h)?;
        let head_mats = head_m.world_matrices(None);
        // A body or head model without a `head_g` node cannot be attached: draw the body alone.
        let (Some(mb), Some(mbr), Some(mh)) = (
            find(&posed_mats, "head_g"),
            find(&rest_mats, "head_g"),
            find(&head_mats, "head_g"),
        ) else {
            return finish(scene);
        };
        // Rigid head parts ride the posed head_g; the skinned head is blended by the body's bones,
        // starting from the rest attachment.
        let xf = mat_mul(&mb, &rigid_inverse(&mh));
        let xf_rest = mat_mul(&mbr, &rigid_inverse(&mh));
        let head_skin = SkinCtx::new(&rest_mats, &posed_mats, xf_rest);
        scene.add_model_with(
            &head_m,
            None,
            &xf,
            &[],
            main_texture(&head_m).as_deref(),
            Some(&head_skin),
        );
    }
    finish(scene)
}

fn finish(scene: Scene) -> Result<Scene> {
    if scene.meshes.is_empty() {
        return Err(Error::format("character has no drawable meshes"));
    }
    Ok(scene)
}

/// Build a creature from its `appearance.2da` row and texture variation (the `Appearance_Type` and
/// `TextureVar` of a creature template).
///
/// Model type `B` is a body plus a head: the body is the row's `race` model, its texture is the
/// `texX` column of the matching `modelX` column plus the two-digit variation (for example
/// `P_BastilaBB` + 1 = `P_BastilaBB01`), and the head comes from `heads.2da` row `normalhead`.
/// Any other model type is drawn as a single model named by `race`. Armour variants and equipped
/// items are not handled.
pub fn load_creature(
    inst: &mut Install,
    appearance_row: usize,
    texture_var: u32,
    animation: Option<(&str, f32)>,
) -> Result<Scene> {
    let app = TwoDa::parse(&inst.read("appearance", ResType::TWODA)?)?;
    let get = |c: &str| app.get(appearance_row, c).map(str::to_string);
    let race = get("race")
        .ok_or_else(|| Error::NotFound(format!("appearance row {appearance_row} model")))?;
    if get("modeltype").as_deref() != Some("B") {
        return load_character_with(inst, &race, None, animation, None);
    }
    let letter = "abcdefghij"
        .chars()
        .find(|l| get(&format!("model{l}")).is_some_and(|m| m.eq_ignore_ascii_case(&race)))
        .unwrap_or('a');
    let body_texture = get(&format!("tex{letter}"))
        .map(|t| format!("{t}{texture_var:02}"))
        .filter(|t| inst.contains(t, ResType::TPC) || inst.contains(t, ResType::TGA));
    let head = get("normalhead")
        .and_then(|h| h.parse::<usize>().ok())
        .and_then(|row| {
            let heads = TwoDa::parse(&inst.read("heads", ResType::TWODA).ok()?).ok()?;
            heads.get(row, "head").map(str::to_string)
        });
    load_character_with(inst, &race, head.as_deref(), animation, body_texture)
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
