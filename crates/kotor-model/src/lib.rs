//! Binary MDL/MDX models: node tree, meshes, skins, controllers and animations.
//!
//! Layout knowledge here was worked out against the installed game's own files
//! and is recorded in `docs/FORMATS.md`. Light, emitter, camera and reference
//! nodes keep only their common header (transform, controllers, children).

mod obj;
mod parse;

pub use obj::to_obj;
pub use parse::parse_model;

pub const NODE_HEADER: u16 = 0x0001;
pub const NODE_LIGHT: u16 = 0x0002;
pub const NODE_EMITTER: u16 = 0x0004;
pub const NODE_CAMERA: u16 = 0x0008;
pub const NODE_REFERENCE: u16 = 0x0010;
pub const NODE_MESH: u16 = 0x0020;
pub const NODE_SKIN: u16 = 0x0040;
pub const NODE_ANIM: u16 = 0x0080;
pub const NODE_DANGLY: u16 = 0x0100;
pub const NODE_AABB: u16 = 0x0200;
pub const NODE_SABER: u16 = 0x0800;

/// One animated channel on a node (position, orientation, scale, colour, ...).
#[derive(Clone, Debug, PartialEq)]
pub struct Controller {
    /// 8 = position, 20 = orientation, 36 = scale; others are light/emitter/mesh values.
    pub kind: u32,
    /// Raw column count byte; bit 0x10 marks Bezier keys (three values per column).
    pub columns: u8,
    pub times: Vec<f32>,
    /// `times.len() * columns` floats, row-major.
    pub values: Vec<f32>,
}

impl Controller {
    /// Orientation keys as `[w, x, y, z]`, for controller kind 20. Handles both the
    /// 4-float form and the packed single-word form (columns == 2).
    pub fn quaternions(&self) -> Option<Vec<[f32; 4]>> {
        if self.kind != 20 {
            return None;
        }
        if self.columns == 2 {
            return Some(
                self.values
                    .iter()
                    .map(|v| {
                        let bits = v.to_bits();
                        let x = (bits & 0x7FF) as f32 / 1023.0 - 1.0;
                        let y = ((bits >> 11) & 0x7FF) as f32 / 1023.0 - 1.0;
                        let z = (bits >> 22) as f32 / 511.0 - 1.0;
                        let w2 = 1.0 - x * x - y * y - z * z;
                        [if w2 > 0.0 { w2.sqrt() } else { 0.0 }, x, y, z]
                    })
                    .collect(),
            );
        }
        // Stored x, y, z, w in animation keys.
        (self.columns == 4).then(|| {
            self.values
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| [c[3], c[0], c[1], c[2]])
                .collect()
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MeshFace {
    pub indices: [u16; 3],
    pub material: u32,
    pub normal: [f32; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Skin {
    /// Four bone weights per vertex.
    pub weights: Vec<[f32; 4]>,
    /// Four bone slots per vertex, indices into `bone_map`.
    pub bones: Vec<[f32; 4]>,
    /// Maps skin bone slot to node index (by name-table order); `-1`-like gaps are `0xFFFF_FFFF`.
    pub bone_map: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Mesh {
    pub texture: Option<String>,
    pub lightmap: Option<String>,
    pub diffuse: [f32; 3],
    pub ambient: [f32; 3],
    pub render: bool,
    pub shadow: bool,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub uvs2: Vec<[f32; 2]>,
    pub faces: Vec<MeshFace>,
    pub skin: Option<Skin>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub name: String,
    /// Node type bit flags (`NODE_*`).
    pub flags: u16,
    pub position: [f32; 3],
    /// Quaternion as `[w, x, y, z]`.
    pub orientation: [f32; 4],
    pub controllers: Vec<Controller>,
    pub mesh: Option<Mesh>,
    pub children: Vec<Node>,
}

impl Node {
    pub fn walk<'a>(&'a self, f: &mut impl FnMut(&'a Node)) {
        f(self);
        for c in &self.children {
            c.walk(f);
        }
    }

    pub fn find(&self, name: &str) -> Option<&Node> {
        if self.name.eq_ignore_ascii_case(name) {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.find(name))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnimEvent {
    pub time: f32,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    pub name: String,
    pub length: f32,
    pub transition: f32,
    pub root_name: String,
    pub events: Vec<AnimEvent>,
    pub root: Node,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub name: String,
    /// 0 other, 1 effect, 2 tile, 4 character, 8 door, 0x10 saber, 0x20 placeable.
    pub classification: u8,
    pub supermodel: Option<String>,
    pub bbox: [[f32; 3]; 2],
    pub radius: f32,
    pub anim_scale: f32,
    pub node_count: u32,
    pub root: Node,
    pub animations: Vec<Animation>,
}
