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
    /// The body's chain of supermodels, nearest first; each carries some of the animations.
    sup: Vec<Model>,
    body_tex: Option<String>,
    head_tex: Option<String>,
    /// The texture the body's meshes name themselves, which `appearance.2da` replaces: `PFBAM` names `pfbam01` however
    /// dark the skin of the row that wears it.
    replaces: Option<String>,
    /// Whether the head's neck takes the body's shading where they meet: the conventional underwear style, for a woman.
    neck_shading: bool,
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
        let mut sup: Vec<Model> = Vec::new();
        let mut next = body_m.supermodel.clone();
        while let Some(name) = next.take() {
            if sup.len() >= 6 {
                break;
            }
            match load(inst, &name) {
                Ok(m) => {
                    next = m.supermodel.clone();
                    sup.push(m);
                }
                Err(e) if sup.is_empty() => return Err(e),
                Err(_) => break,
            }
        }
        let head_m = match head {
            Some(h) => Some(load(inst, h)?),
            None => None,
        };
        Ok(Rig {
            body_tex: body_texture.or_else(|| main_texture(&body_m)),
            head_tex: head_m.as_ref().and_then(main_texture),
            replaces: None,
            neck_shading: false,
            body: body_m,
            head: head_m,
            sup,
        })
    }

    /// The body's shading for the conventional underwear style, if it is one of the bodies that style is for. See
    /// [`crate::underwear::soften_shading`].
    pub fn soften_underwear_shading(&mut self) {
        let name = self.body_tex.as_deref().unwrap_or("");
        if crate::underwear::body_of_texture(name).is_some() {
            crate::underwear::soften_shading(&mut self.body);
            if let (Some(head), Some(tex)) = (&mut self.head, &self.head_tex) {
                crate::underwear::close_neck_gap(head, tex);
                self.neck_shading = true;
            }
        }
    }

    /// The body, then its chain of supermodels: a droid or a rakghoul carries its clips in its own model, with no chain.
    fn models(&self) -> impl Iterator<Item = &Model> {
        std::iter::once(&self.body).chain(&self.sup)
    }

    /// The name this body has for a clip. The game asks for a person's names. A creature has its own set, `cpause1` and
    /// `cwalk`, and its fighting clips carry digit 0 whatever it holds, or another digit for a droid like T3-M4.
    ///
    /// **Assumption, read from the names in the models:** which of a creature's clips stands in for which.
    pub fn clip_name(&self, want: &str) -> Option<&str> {
        let has = |name: &str| {
            self.models()
                .find_map(|m| m.animation(name))
                .map(|a| a.name.as_str())
        };
        if let Some(found) = has(want) {
            return Some(found);
        }
        let fallback: &[&str] = match want {
            "pause1" => &["cpause1", "creadyr"],
            "walk" => &["cwalk", "crun"],
            "run" => &["crun", "cwalk"],
            "die" => &["cdie"],
            "dead" => &["cdead"],
            _ => &[],
        };
        if let Some(found) = fallback.iter().find_map(|n| has(n)) {
            return Some(found);
        }
        // `g5r1`: a stance letter, the weapon's digit, what it does, which one.
        let b = want.as_bytes();
        if b.len() != 4 || !b[1].is_ascii_digit() {
            return None;
        }
        let (stance, doing) = (b[0] as char, b[2] as char);
        let other_digit = |letter: char| {
            (0..10).find_map(|d| has(&format!("{letter}{d}{doing}{}", b[3] as char)))
        };
        match doing {
            'r' => other_digit('g').or_else(|| has("creadyr")),
            'g' => other_digit('g').or_else(|| has("cdamages")),
            'd' => other_digit('g').or_else(|| has("cdodgeg")),
            'a' => other_digit(stance)
                .or_else(|| ['g', 'b', 'c', 'm'].iter().find_map(|l| other_digit(*l))),
            _ => None,
        }
    }

    /// Every animation the body and its supermodels carry, nearest first.
    pub fn animation_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for m in self.models() {
            for a in &m.animations {
                if !names.iter().any(|n| n.eq_ignore_ascii_case(&a.name)) {
                    names.push(a.name.clone());
                }
            }
        }
        names
    }

    /// Animations in the body model itself.
    pub fn body_animation_names(&self) -> Vec<String> {
        self.body
            .animations
            .iter()
            .map(|a| a.name.clone())
            .collect()
    }

    /// The names of the supermodels, nearest first.
    pub fn chain(&self) -> Vec<String> {
        self.sup.iter().map(|m| m.name.clone()).collect()
    }

    /// The texture the head is drawn with.
    pub fn head_texture(&self) -> Option<&str> {
        self.head_tex.as_deref()
    }

    /// Length in seconds of a named animation of the supermodel.
    pub fn animation_length(&self, name: &str) -> Option<f32> {
        let name = self.clip_name(name)?;
        self.models()
            .find_map(|m| m.animation(name))
            .map(|a| a.length)
    }

    /// How far a clip takes the body from its rest pose, at most, over its length: 0 is standing in the bind pose.
    pub fn distance_from_rest(&self, name: &str) -> f32 {
        let rest = self.body.world_matrices(None);
        let Some(length) = self.animation_length(name) else {
            return 0.0;
        };
        let mut most = 0.0f32;
        for k in 0..=8 {
            let Some(pose) = self.sample(name, length * k as f32 / 8.0) else {
                return 0.0;
            };
            for ((_, a), (_, b)) in self.body.world_matrices(Some(&pose)).iter().zip(&rest) {
                for r in 0..3 {
                    for c in 0..4 {
                        most = most.max((a[r][c] - b[r][c]).abs());
                    }
                }
            }
        }
        most
    }

    /// The character in `animation` (name, time in seconds) at the origin, feet on z = 0, facing +y.
    /// `None`, or a name the supermodel lacks, leaves the rest pose.
    ///
    /// The head is attached at the body's `head_g`: rigid head parts follow the posed node and the
    /// skinned head is blended by the body's bones (see `SkinCtx`).
    pub fn pose(&self, animation: Option<(&str, f32)>) -> Result<Scene> {
        let pose = animation.and_then(|(name, t)| self.sample(name, t));
        let face = animation.and_then(|(name, t)| self.sample_face(name, t));
        self.posed_with_face(pose.as_ref(), face.as_ref())
    }

    /// A named animation sampled at `t`, from the nearest supermodel that has it.
    pub fn sample(&self, name: &str, t: f32) -> Option<kotor_model::Pose> {
        let name = self.clip_name(name)?;
        self.models()
            .find_map(|m| m.animation(name).map(|a| a.sample(m, t)))
    }

    /// The body standing as its model has it, with no animation.
    pub fn bind_pose(&self) -> kotor_model::Pose {
        self.body.bind_pose()
    }

    /// Each node of the body in `pose`: its lower-case name, its transform and its parent's.
    pub fn joint_frames(&self, pose: Option<&kotor_model::Pose>) -> Vec<kotor_model::JointFrame> {
        self.body.joint_frames(pose)
    }

    /// Add `item` to `scene` at a node of the body (`rhand`, `lhand`), for a body in `pose`.
    pub fn attach(
        &self,
        scene: &mut Scene,
        pose: Option<&kotor_model::Pose>,
        node: &str,
        item: &Model,
    ) {
        if let Some(at) = find(&self.body.world_matrices(pose), node) {
            scene.add_model_with(item, None, &at, &[], None, None);
        }
    }

    /// The head's own animation of this name (`pause1`, `talk`, `tlknorm`, `listen`) sampled at `t`: the face moving. A
    /// head carries these the way a body's supermodel does, and a body with no head or a head without the clip has none.
    pub fn sample_face(&self, name: &str, t: f32) -> Option<kotor_model::Pose> {
        let head = self.head.as_ref()?;
        let a = head.animation(name)?;
        Some(a.sample(head, if a.length > 0.0 { t % a.length } else { 0.0 }))
    }

    /// The character in a pose sampled elsewhere, such as a cutscene's stunt model.
    pub fn posed(&self, pose: Option<&kotor_model::Pose>) -> Result<Scene> {
        self.posed_with_face(pose, None)
    }

    /// Like [`Rig::posed`], with the head in a pose of its own (see [`Rig::sample_face`]).
    pub fn posed_with_face(
        &self,
        pose: Option<&kotor_model::Pose>,
        face: Option<&kotor_model::Pose>,
    ) -> Result<Scene> {
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
        let posed_mats = self.body.world_matrices(pose);
        let body_skin = SkinCtx::new(&rest_mats, &posed_mats, identity);
        scene.add_model_with(
            &self.body,
            pose,
            &identity,
            hide_on_body,
            self.body_tex.as_deref(),
            Some(&body_skin),
        );
        if let Some(old) = &self.replaces {
            for m in &mut scene.meshes {
                if m.texture
                    .as_deref()
                    .is_some_and(|t| t.eq_ignore_ascii_case(old))
                {
                    m.texture.clone_from(&self.body_tex);
                }
            }
        }
        let mut head_up = None;
        if let Some(head_m) = &self.head {
            let head_mats = head_m.world_matrices(None);
            // A creature's body has its own `head_g` to line the head's up with, and its bones bend the head too.
            // A player's body has only a `headhook`: the head model has its own skeleton with its origin at the neck
            // (measured: head vertices 0 to 0.26 m above it), and rides the hook rigidly. A body with neither is drawn
            // alone.
            match (
                find(&posed_mats, "head_g"),
                find(&rest_mats, "head_g"),
                find(&head_mats, "head_g"),
            ) {
                (Some(mb), Some(_), Some(mh)) => {
                    // Rigid: bending the head by the body's bones melts the face, because the facial bones are only in
                    // the head's own skeleton.
                    let xf = mat_mul(&mb, &rigid_inverse(&mh));
                    head_up = Some([xf[0][2], xf[1][2], xf[2][2]]);
                    self.add_head(&mut scene, head_m, face, &xf);
                }
                _ => {
                    if let (Some(posed), Some(mut turn)) =
                        (find(&posed_mats, "headhook"), find(&rest_mats, "headhook"))
                    {
                        for row in turn.iter_mut().take(3) {
                            row[3] = 0.0;
                        }
                        let xf = mat_mul(&posed, &rigid_inverse(&turn));
                        head_up = Some([xf[0][2], xf[1][2], xf[2][2]]);
                        self.add_head(&mut scene, head_m, face, &xf);
                    }
                }
            }
        }
        if self.neck_shading {
            if let (Some(tex), Some(up)) = (&self.head_tex, head_up) {
                crate::underwear::blend_neck_normals(&mut scene, tex, up);
            }
        }
        if scene.meshes.is_empty() {
            return Err(Error::format("character has no drawable meshes"));
        }
        Ok(scene)
    }
}

impl Rig {
    /// The head at `xf`, its face in `face` if there is one: the skin then follows the head's own bones, in the head's
    /// own space, and the whole is placed after.
    fn add_head(
        &self,
        scene: &mut Scene,
        head: &Model,
        face: Option<&kotor_model::Pose>,
        xf: &Mat,
    ) {
        let Some(f) = face else {
            scene.add_model_with(head, None, xf, &[], self.head_tex.as_deref(), None);
            return;
        };
        let skin = SkinCtx::new(
            &head.world_matrices(None),
            &head.world_matrices(Some(f)),
            crate::scene::IDENTITY,
        );
        let mut alone = Scene {
            min: [f32::MAX; 3],
            max: [f32::MIN; 3],
            ..Scene::default()
        };
        alone.add_model_with(
            head,
            Some(f),
            &crate::scene::IDENTITY,
            &[],
            self.head_tex.as_deref(),
            Some(&skin),
        );
        scene.append_transformed(&alone, xf);
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
    rig_for_outfit(inst, appearance_row, texture_var, None)
}

/// [`rig_for_appearance`] in armour: `letter` picks the `modelX` and `texX` columns (the `bodyvar` of the armour's
/// `baseitems.2da` row) when the row has that column, and `texture_var` is then the armour's `TextureVar`.
pub fn rig_for_outfit(
    inst: &mut Install,
    appearance_row: usize,
    texture_var: u32,
    letter: Option<char>,
) -> Result<Rig> {
    let app = TwoDa::parse(&inst.read("appearance", ResType::TWODA)?)?;
    let get = |c: &str| {
        app.get(appearance_row, c)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    };
    let worn = letter.and_then(|l| get(&format!("model{}", l.to_ascii_lowercase())));
    // The player's rows name no `race`. With nothing on, a hero wears column a, a vest and shorts in the row's own skin:
    // the reference video shows that until the footlocker's clothes go on (docs/REFERENCE.md).
    let race = worn
        .or_else(|| get("race"))
        .or_else(|| get("modela"))
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
    let named = body_texture.is_some();
    let mut rig = Rig::load(inst, &race, head.as_deref(), body_texture)?;
    if named {
        rig.replaces = main_texture(&rig.body);
    }
    Ok(rig)
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
