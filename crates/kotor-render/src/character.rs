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
/// own take it. Stand-in used when no appearance-based texture name is available.
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

/// A character's parsed models, ready to be posed many times (one pose per frame) without
/// touching the install again.
pub struct Rig {
    body: Model,
    head: Option<Model>,
    /// The body's supermodel, which carries the animations.
    sup: Option<Model>,
    body_tex: Option<String>,
    head_tex: Option<String>,
}

impl Rig {
    /// Load a body, optionally a head, and the body's supermodel. `body_texture` is used for body
    /// meshes that name no texture of their own (else the model's most used texture).
    pub fn load(
        inst: &mut Install,
        body: &str,
        head: Option<&str>,
        body_texture: Option<String>,
    ) -> Result<Rig> {
        let body_m = load(inst, body)?;
        let sup = match &body_m.supermodel {
            Some(s) => Some(load(inst, s)?),
            None => None,
        };
        let head_m = match head {
            Some(h) => Some(load(inst, h)?),
            None => None,
        };
        Ok(Rig {
            body_tex: body_texture.or_else(|| main_texture(&body_m)),
            head_tex: head_m.as_ref().and_then(main_texture),
            body: body_m,
            head: head_m,
            sup,
        })
    }

    /// Length in seconds of a named animation of the supermodel.
    pub fn animation_length(&self, name: &str) -> Option<f32> {
        self.sup.as_ref()?.animation(name).map(|a| a.length)
    }

    /// The character in `animation` (name, time in seconds) at the origin, feet on z = 0, facing +y.
    /// `None`, or a name the supermodel lacks, leaves the rest pose.
    ///
    /// The head is attached at the body's `head_g`: rigid head parts follow the posed node and the
    /// skinned head is blended by the body's bones (see `SkinCtx`).
    pub fn pose(&self, animation: Option<(&str, f32)>) -> Result<Scene> {
        let pose = match (animation, &self.sup) {
            (Some((name, t)), Some(sup)) => sup.animation(name).map(|a| a.sample(sup, t)),
            _ => None,
        };
        let mut scene = Scene {
            min: [f32::MAX; 3],
            max: [f32::MIN; 3],
            ..Scene::default()
        };
        let identity: Mat = crate::scene::IDENTITY;
        let hide_on_body: &[&str] = if self.head.is_some() {
            &["head_g", "neck_g", "necklwr_g"]
        } else {
            &[]
        };
        let rest_mats = self.body.world_matrices(None);
        let posed_mats = self.body.world_matrices(pose.as_ref());
        let body_skin = SkinCtx::new(&rest_mats, &posed_mats, identity);
        scene.add_model_with(
            &self.body,
            pose.as_ref(),
            &identity,
            hide_on_body,
            self.body_tex.as_deref(),
            Some(&body_skin),
        );
        if let Some(head_m) = &self.head {
            let head_mats = head_m.world_matrices(None);
            // A body or head model without a `head_g` node cannot be attached: draw the body alone.
            if let (Some(mb), Some(mbr), Some(mh)) = (
                find(&posed_mats, "head_g"),
                find(&rest_mats, "head_g"),
                find(&head_mats, "head_g"),
            ) {
                let xf = mat_mul(&mb, &rigid_inverse(&mh));
                let xf_rest = mat_mul(&mbr, &rigid_inverse(&mh));
                let head_skin = SkinCtx::new(&rest_mats, &posed_mats, xf_rest);
                scene.add_model_with(
                    head_m,
                    None,
                    &xf,
                    &[],
                    self.head_tex.as_deref(),
                    Some(&head_skin),
                );
            }
        }
        if scene.meshes.is_empty() {
            return Err(Error::format("character has no drawable meshes"));
        }
        Ok(scene)
    }
}

/// Assemble a character: a body model, optionally a head model, and an animation from the body's
/// supermodel sampled at `time` seconds (`animation` of `None` leaves the rest pose).
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
    Rig::load(inst, body, head, body_texture)?.pose(animation)
}

/// Models for a creature from its `appearance.2da` row and texture variation (the `Appearance_Type`
/// and `TextureVar` of a creature template).
///
/// Model type `B` is a body plus a head: the body is the row's `race` model, its texture is the
/// `texX` column of the matching `modelX` column plus the two-digit variation (for example
/// `P_BastilaBB` + 1 = `P_BastilaBB01`), and the head comes from `heads.2da` row `normalhead`.
/// Any other model type is a single model named by `race`. Armour variants and equipped items are
/// not handled.
pub fn rig_for_appearance(
    inst: &mut Install,
    appearance_row: usize,
    texture_var: u32,
) -> Result<Rig> {
    let app = TwoDa::parse(&inst.read("appearance", ResType::TWODA)?)?;
    let get = |c: &str| app.get(appearance_row, c).map(str::to_string);
    let race = get("race")
        .ok_or_else(|| Error::NotFound(format!("appearance row {appearance_row} model")))?;
    if get("modeltype").as_deref() != Some("B") {
        return Rig::load(inst, &race, None, None);
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
    Rig::load(inst, &race, head.as_deref(), body_texture)
}

/// [`rig_for_appearance`] posed once.
pub fn load_creature(
    inst: &mut Install,
    appearance_row: usize,
    texture_var: u32,
    animation: Option<(&str, f32)>,
) -> Result<Scene> {
    rig_for_appearance(inst, appearance_row, texture_var)?.pose(animation)
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
