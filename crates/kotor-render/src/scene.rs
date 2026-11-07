use kotor_archive::Install;
use kotor_core::{Error, ResType, Result};
use kotor_image::Image;
use kotor_model::{parse_model, Model, Pose};
use kotor_walkmesh::parse_lyt;
use std::collections::HashMap;

pub(crate) type Mat = [[f32; 4]; 4];

pub(crate) const IDENTITY: [[f32; 4]; 4] = [
    [1., 0., 0., 0.],
    [0., 1., 0., 0.],
    [0., 0., 1., 0.],
    [0., 0., 0., 1.],
];

pub(crate) fn mat_mul(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut o = [[0.0; 4]; 4];
    for (i, row) in o.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (0..4).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    o
}

/// Inverse of a rotation + translation matrix (no scale or shear).
pub(crate) fn rigid_inverse(m: &Mat) -> Mat {
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

pub(crate) fn apply(m: &Mat, p: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2] + m[0][3],
        m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2] + m[1][3],
        m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2] + m[2][3],
    ]
}

/// A skeleton in two poses, for blending skinned meshes. Bones are matched by lower-case node name,
/// so a head model can be deformed by the body's skeleton.
pub struct SkinCtx {
    rest: HashMap<String, Mat>,
    posed: HashMap<String, Mat>,
    /// Maps the skinned model's rest space into the skeleton's rest space.
    pub xf_rest: Mat,
}

impl SkinCtx {
    pub fn new(rest: &[(String, Mat)], posed: &[(String, Mat)], xf_rest: Mat) -> SkinCtx {
        let map = |v: &[(String, Mat)]| {
            v.iter()
                .map(|(n, m)| (n.to_ascii_lowercase(), *m))
                .collect()
        };
        SkinCtx {
            rest: map(rest),
            posed: map(posed),
            xf_rest,
        }
    }

    /// How far the bone has moved from rest: `posed * inverse(rest)`.
    fn bone_motion(&self, name: &str) -> Option<Mat> {
        let key = name.to_ascii_lowercase();
        Some(mat_mul(
            self.posed.get(&key)?,
            &rigid_inverse(self.rest.get(&key)?),
        ))
    }
}

/// One drawable mesh with its vertices already in world space.
#[derive(Clone, Debug, Default)]
pub struct SceneMesh {
    pub name: String,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub uvs2: Vec<[f32; 2]>,
    pub triangles: Vec<[u32; 3]>,
    pub texture: Option<String>,
    pub lightmap: Option<String>,
    /// How much of the light it gets, where it is not all: 0 means all, for most things.
    pub shade: f32,
}

#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub meshes: Vec<SceneMesh>,
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Scene {
    pub fn triangle_count(&self) -> usize {
        self.meshes.iter().map(|m| m.triangles.len()).sum()
    }

    /// A copy turned `yaw` radians about the vertical axis and then shifted, for a neighbouring area stitched on to this one.
    pub fn moved(&self, yaw: f32, shift: [f32; 3]) -> Scene {
        let (s, c) = yaw.sin_cos();
        let spin = |p: [f32; 3]| [c * p[0] - s * p[1], s * p[0] + c * p[1], p[2]];
        let mut out = self.clone();
        for m in &mut out.meshes {
            for p in &mut m.positions {
                let q = spin(*p);
                *p = [q[0] + shift[0], q[1] + shift[1], q[2] + shift[2]];
            }
            for n in &mut m.normals {
                *n = spin(*n);
            }
        }
        out.min = [f32::MAX; 3];
        out.max = [f32::MIN; 3];
        for m in &out.meshes {
            for p in &m.positions {
                for (k, v) in p.iter().enumerate() {
                    out.min[k] = out.min[k].min(*v);
                    out.max[k] = out.max[k].max(*v);
                }
            }
        }
        out
    }

    /// Add another scene's meshes to this one.
    pub fn append(&mut self, other: Scene) {
        if self.meshes.is_empty() {
            self.min = other.min;
            self.max = other.max;
        } else {
            for k in 0..3 {
                self.min[k] = self.min[k].min(other.min[k]);
                self.max[k] = self.max[k].max(other.max[k]);
            }
        }
        self.meshes.extend(other.meshes);
    }

    fn grow(&mut self, p: [f32; 3]) {
        for (k, v) in p.iter().enumerate() {
            self.min[k] = self.min[k].min(*v);
            self.max[k] = self.max[k].max(*v);
        }
    }

    /// Add every render-flagged mesh of `model`, translated by `offset`.
    pub fn add_model(&mut self, model: &Model, offset: [f32; 3]) {
        let mut xf = IDENTITY;
        for (k, o) in offset.iter().enumerate() {
            xf[k][3] = *o;
        }
        self.add_model_with(model, None, &xf, &[], None, None);
    }

    /// Copy `other` into this scene, transformed by `xf` (rotation + translation).
    pub fn append_transformed(&mut self, other: &Scene, xf: &[[f32; 4]; 4]) {
        for m in &other.meshes {
            let mut c = m.clone();
            for p in &mut c.positions {
                *p = apply(xf, *p);
            }
            for n in &mut c.normals {
                let o = apply(xf, *n);
                let t = apply(xf, [0.0; 3]);
                *n = [o[0] - t[0], o[1] - t[1], o[2] - t[2]];
            }
            for &p in &c.positions {
                self.grow(p);
            }
            self.meshes.push(c);
        }
    }

    /// Add the meshes of `model` as posed by `pose`, placed by `xf` (applied after the model's own
    /// node transforms). Nodes named in `skip` are left out. Meshes with no texture name use
    /// `default_texture`. With `skin`, skinned meshes are blended by the skeleton's bone motion
    /// instead of being placed rigidly (their result is in the skeleton's posed space; `xf` is not applied).
    pub fn add_model_with(
        &mut self,
        model: &Model,
        pose: Option<&Pose>,
        xf: &[[f32; 4]; 4],
        skip: &[&str],
        default_texture: Option<&str>,
        skin: Option<&SkinCtx>,
    ) {
        let rest_by_name: HashMap<String, Mat> = skin
            .map(|_| {
                model
                    .world_matrices(None)
                    .into_iter()
                    .map(|(n, m)| (n.to_ascii_lowercase(), m))
                    .collect()
            })
            .unwrap_or_default();
        let mut names_by_index: HashMap<i32, String> = HashMap::new();
        if skin.is_some() {
            model.root.walk(&mut |n| {
                names_by_index.insert(n.index as i32, n.name.clone());
            });
        }
        for (node, nm) in model.mesh_nodes(pose) {
            if skip.iter().any(|s| node.name.eq_ignore_ascii_case(s)) {
                continue;
            }
            let m = mat_mul(xf, &nm);
            let Some(mesh) = &node.mesh else { continue };
            if mesh.positions.is_empty() || mesh.faces.is_empty() {
                continue;
            }
            // AABB nodes are the walkmesh (collision geometry, not drawn); a mesh with neither a
            // texture nor a lightmap has nothing to show.
            if node.flags & kotor_model::NODE_AABB != 0
                || (mesh.texture.is_none() && mesh.lightmap.is_none() && default_texture.is_none())
            {
                continue;
            }
            // Skinned mesh with a skeleton available: blend vertex by vertex.
            let skinned = match (skin, &mesh.skin) {
                (Some(ctx), Some(sk)) => {
                    let rest_world = rest_by_name
                        .get(&node.name.to_ascii_lowercase())
                        .copied()
                        .unwrap_or(IDENTITY);
                    let base = mat_mul(&ctx.xf_rest, &rest_world);
                    // `bone_map` is indexed by the node's name-table index and holds that node's bone
                    // number (-1 if it is not a bone of this mesh); vertices refer to bone numbers.
                    // Checked on the head: its weights land on head_g, f_jaw_g, f_um_g and the lip bones.
                    let mut motions: Vec<Option<Mat>> = Vec::new();
                    for (name_idx, &raw) in sk.bone_map.iter().enumerate() {
                        let bone = f32::from_bits(raw);
                        if !(0.0..4096.0).contains(&bone) {
                            continue;
                        }
                        let bone = bone as usize;
                        if motions.len() <= bone {
                            motions.resize(bone + 1, None);
                        }
                        if let Some(name) = names_by_index.get(&(name_idx as i32)) {
                            motions[bone] = ctx.bone_motion(name);
                        }
                    }
                    let blend = |v: usize| -> [f32; 3] {
                        let p0 = apply(&base, mesh.positions[v]);
                        let (mut acc, mut total) = ([0.0f32; 3], 0.0f32);
                        for k in 0..4 {
                            let w = sk.weights.get(v).map_or(0.0, |w| w[k]);
                            if w <= 0.0 {
                                continue;
                            }
                            let slot = sk.bones.get(v).map_or(-1.0, |b| b[k]);
                            let moved = if slot >= 0.0 {
                                motions
                                    .get(slot as usize)
                                    .copied()
                                    .flatten()
                                    .map_or(p0, |s| apply(&s, p0))
                            } else {
                                p0
                            };
                            for (a, q) in acc.iter_mut().zip(moved) {
                                *a += w * q;
                            }
                            total += w;
                        }
                        if total > 1e-6 {
                            acc.map(|a| a / total)
                        } else {
                            p0
                        }
                    };
                    Some((
                        (0..mesh.positions.len()).map(blend).collect::<Vec<_>>(),
                        base,
                    ))
                }
                _ => None,
            };
            let at = |p: [f32; 3]| -> [f32; 3] {
                [
                    m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2] + m[0][3],
                    m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2] + m[1][3],
                    m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2] + m[2][3],
                ]
            };
            // Directions: rotation part only (uniform scale is not used by room meshes).
            let dir = |n: [f32; 3]| -> [f32; 3] {
                [
                    m[0][0] * n[0] + m[0][1] * n[1] + m[0][2] * n[2],
                    m[1][0] * n[0] + m[1][1] * n[1] + m[1][2] * n[2],
                    m[2][0] * n[0] + m[2][1] * n[1] + m[2][2] * n[2],
                ]
            };
            let mut out = SceneMesh {
                shade: 0.0,
                name: node.name.clone(),
                positions: match &skinned {
                    Some((p, _)) => p.clone(),
                    None => mesh.positions.iter().map(|&p| at(p)).collect(),
                },
                normals: match &skinned {
                    Some((_, base)) => mesh.normals.iter().map(|&n| apply_dir(base, n)).collect(),
                    None => mesh.normals.iter().map(|&n| dir(n)).collect(),
                },
                uvs: mesh.uvs.clone(),
                uvs2: mesh.uvs2.clone(),
                triangles: mesh
                    .faces
                    .iter()
                    .map(|f| f.indices.map(|i| i as u32))
                    .collect(),
                texture: mesh
                    .texture
                    .clone()
                    .or_else(|| default_texture.map(str::to_string)),
                lightmap: mesh.lightmap.clone(),
            };
            if out.uvs.len() != out.positions.len() {
                out.uvs.clear();
            }
            if out.uvs2.len() != out.positions.len() {
                out.uvs2.clear();
            }
            if out.normals.len() != out.positions.len() {
                out.normals.clear();
            }
            for &p in &out.positions {
                self.grow(p);
            }
            self.meshes.push(out);
        }
    }
}

fn apply_dir(m: &Mat, n: [f32; 3]) -> [f32; 3] {
    let o = apply(m, n);
    let t = apply(m, [0.0; 3]);
    [o[0] - t[0], o[1] - t[1], o[2] - t[2]]
}

/// Build the scene for an area: every room model of its layout, placed where the layout says.
pub fn load_area(inst: &mut Install, area: &str) -> Result<Scene> {
    let lyt = parse_lyt(&inst.read(area, ResType::LYT)?)?;
    let mut scene = Scene {
        min: [f32::MAX; 3],
        max: [f32::MIN; 3],
        ..Scene::default()
    };
    for room in &lyt.rooms {
        let mdl = match inst.read(&room.model, ResType::MDL) {
            Ok(d) => d,
            Err(_) => continue, // layout rooms without a model are placeholders
        };
        let mdx = inst.read(&room.model, ResType::MDX).unwrap_or_default();
        let model = parse_model(&mdl, &mdx)?;
        scene.add_model(&model, room.position);
    }
    if scene.meshes.is_empty() {
        return Err(Error::format(format!(
            "area {area} has no drawable room models"
        )));
    }
    Ok(scene)
}

/// Texture by name from the texture packs (TPC) or the base data (TGA). The caller must
/// have called `Install::load_texture_packs` for TPC lookups.
pub fn load_texture(inst: &mut Install, name: &str) -> Option<Image> {
    let mut txi = inst
        .read(name, ResType::TXI)
        .map(|b| String::from_utf8_lossy(&b).to_string())
        .unwrap_or_default();
    // A TGA in the `Override` folder beats the game's own TPC, and a TPC there beats a TGA there.
    let loose_tpc = inst.in_override(name, ResType::TPC);
    let loose_tga = !loose_tpc && inst.in_override(name, ResType::TGA);
    if loose_tga {
        let parsed = inst
            .read(name, ResType::TGA)
            .map_err(|e| e.to_string())
            .and_then(|d| kotor_image::parse_tga(&d).map_err(|e| e.to_string()));
        match parsed {
            Ok(mut img) => {
                apply_txi_alpha(&mut img, &txi.to_ascii_lowercase());
                return Some(img);
            }
            Err(e) => kotor_archive::note_override_problem(format!(
                "{name}.tga: failed to parse ({e}), so the stock one is used"
            )),
        }
    }
    let tpc = |inst: &mut Install, stock: bool| {
        let d = if stock {
            inst.read_stock(name, ResType::TPC)
        } else {
            inst.read(name, ResType::TPC)
        };
        d.ok().map(|d| kotor_image::parse_tpc(&d))
    };
    let parsed = match tpc(inst, loose_tga) {
        Some(Err(e)) if loose_tpc => {
            kotor_archive::note_override_problem(format!(
                "{name}.tpc: failed to parse ({e}), so the stock one is used"
            ));
            tpc(inst, true)
        }
        p => p,
    };
    let mut img = if let Some(t) = parsed {
        let t = t.ok()?;
        txi.push_str(&t.txi);
        // TPC rows are stored bottom-up; `Image` and the renderers have the first row on top.
        flip_rows(t.image)
    } else {
        kotor_image::parse_tga(&inst.read_stock(name, ResType::TGA).ok()?).ok()?
    };
    apply_txi_alpha(&mut img, &txi.to_ascii_lowercase());
    Some(img)
}

fn flip_rows(img: Image) -> Image {
    let row = (img.width as usize * 4).max(1);
    let rgba = img.rgba.chunks(row).rev().flatten().copied().collect();
    Image { rgba, ..img }
}

/// Make the alpha channel mean transparency. With an environment or bump map named in the TXI it is a shininess mask,
/// so the picture becomes opaque. With additive blending, alpha follows brightness, so dark parts add nothing.
fn apply_txi_alpha(img: &mut Image, txi: &str) {
    let shiny = ["envmaptexture", "bumpyshinytexture", "bumpmaptexture"]
        .iter()
        .any(|k| txi.contains(k));
    let additive = txi.contains("blending") && txi.contains("additive");
    if !shiny && !additive {
        return;
    }
    for p in img.rgba.chunks_mut(4) {
        p[3] = if additive {
            p[0].max(p[1]).max(p[2])
        } else {
            255
        };
    }
}

#[cfg(test)]
mod alpha_tests {
    use super::*;

    #[test]
    fn tpc_rows_are_turned_top_side_up() {
        let bottom_up = Image {
            width: 1,
            height: 2,
            rgba: vec![1, 1, 1, 255, 2, 2, 2, 255],
        };
        assert_eq!(flip_rows(bottom_up).rgba, vec![2, 2, 2, 255, 1, 1, 1, 255]);
    }

    #[test]
    fn the_txi_decides_what_alpha_means() {
        let image = || Image {
            width: 1,
            height: 1,
            rgba: vec![200, 10, 10, 50],
        };
        let mut a = image();
        apply_txi_alpha(&mut a, "envmaptexture cm_chrome");
        assert_eq!(a.rgba[3], 255);
        let mut b = image();
        apply_txi_alpha(&mut b, "blending additive");
        assert_eq!(b.rgba[3], 200);
        let mut c = image();
        apply_txi_alpha(&mut c, "blending punchthrough");
        assert_eq!(c.rgba[3], 50);
    }
}
