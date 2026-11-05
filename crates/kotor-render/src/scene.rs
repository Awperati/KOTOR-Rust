use kotor_archive::Install;
use kotor_core::{Error, ResType, Result};
use kotor_image::Image;
use kotor_model::{parse_model, Model, Pose};
use kotor_walkmesh::parse_lyt;

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
        self.add_model_with(model, None, &xf, &[], None);
    }

    /// Add the meshes of `model` as posed by `pose`, placed by `xf` (applied after the model's own
    /// node transforms). Nodes named in `skip` are left out. Meshes with no texture name use
    /// `default_texture`.
    pub fn add_model_with(
        &mut self,
        model: &Model,
        pose: Option<&Pose>,
        xf: &[[f32; 4]; 4],
        skip: &[&str],
        default_texture: Option<&str>,
    ) {
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
                name: node.name.clone(),
                positions: mesh.positions.iter().map(|&p| at(p)).collect(),
                normals: mesh.normals.iter().map(|&n| dir(n)).collect(),
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
    if let Ok(d) = inst.read(name, ResType::TPC) {
        return kotor_image::parse_tpc(&d).ok().map(|t| t.image);
    }
    let d = inst.read(name, ResType::TGA).ok()?;
    kotor_image::parse_tga(&d).ok()
}
