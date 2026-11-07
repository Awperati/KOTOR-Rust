//! Binary MDL/MDX models: node tree, meshes, skins, controllers and animations.
//!
//! Layout knowledge here was worked out against the installed game's own files
//! and is recorded in `docs/FORMATS.md`. Light, emitter, camera and reference
//! nodes keep only their common header (transform, controllers, children).

mod obj;
mod parse;
mod pose;

pub use obj::to_obj;
pub use parse::parse_model;
pub use pose::{blend, blend_by, unpack_quat, JointFrame, NodePose, Pose};

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
            return Some(self.values.iter().map(|v| unpack_quat(*v).0).collect());
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

/// Light node data (K1 layout, 92 bytes after the node header). Colour, radius and multiplier are
/// controllers on the node, not fields here.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Light {
    pub flare_radius: f32,
    pub priority: i32,
    pub ambient_only: bool,
    pub dynamic_type: u32,
    pub affect_dynamic: bool,
    pub shadow: bool,
    pub flare: bool,
    pub fading: bool,
    pub flare_sizes: Vec<f32>,
    pub flare_positions: Vec<f32>,
    pub flare_color_shifts: Vec<[f32; 3]>,
    pub flare_textures: Vec<String>,
}

/// Particle emitter node data (K1 layout). Names are the strings the game's particle code switches on,
/// e.g. update `Fountain`, render `Normal`, blend `Lighten`.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Emitter {
    pub dead_space: f32,
    pub blast_radius: f32,
    pub blast_length: f32,
    pub branch_count: u32,
    pub control_point_smoothing: u32,
    pub grid: [u32; 2],
    pub spawn_type: u32,
    pub update: String,
    pub render: String,
    pub blend: String,
    pub texture: String,
    pub chunk_name: String,
    pub two_sided: bool,
    pub looping: bool,
    pub render_order: u16,
    pub frame_blending: bool,
    pub depth_texture: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub name: String,
    /// Position of the node's name in the model's name table; skin bone maps refer to nodes by this number.
    pub index: u16,
    /// The node number stored in the node header (second u16).
    pub number: u16,
    /// Node type bit flags (`NODE_*`).
    pub flags: u16,
    pub position: [f32; 3],
    /// Quaternion as `[w, x, y, z]`.
    pub orientation: [f32; 4],
    pub controllers: Vec<Controller>,
    pub mesh: Option<Mesh>,
    pub light: Option<Light>,
    pub emitter: Option<Emitter>,
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
