//! The owner's own meshes: clothing, made in Blender and saved as `.glb` (or `.gltf` with its `.bin`), worn over the game's body.
//!
//! A garment is skinned to the game's bones, named as the game names them (lower case): the rig `tools/blender/kotor_anim.py`
//! builds. It is in the body's rest pose, as the export gives the body, and the game draws it with the body's own skeleton, so
//! it moves with every clip. Its picture is named by its material (lower case) and comes from the owner's texture folder
//! (`docs/TEXTURES.md`), or from the picture inside the file.
//!
//! What a garment `covers` of the stock body is not drawn while it is worn: `body` is the stock body's torso piece, which in
//! the game is the trunk, the pelvis, the legs and the feet in one mesh, and `arms` the two arms with their hands.

use crate::json::{self, Json};
use kotor_image::Image;
use kotor_model::{Mesh, MeshFace, Model, Node, Skin};
use std::collections::HashMap;
use std::sync::RwLock;

/// More triangles than this in one garment are refused: the body itself is 1500 to 1800.
pub const MAX_TRIANGLES: usize = 8000;

/// One piece of a garment, in the body's rest pose, in the game's Z-up space.
#[derive(Clone, Debug, PartialEq)]
pub struct GarmentPart {
    pub name: String,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub faces: Vec<[u16; 3]>,
    /// Per vertex, four joints as places in `joints`, and their weights.
    pub bones: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
    /// The game's bones this piece is skinned to, by name.
    pub joints: Vec<String>,
    /// The picture's name, lower case.
    pub texture: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Garment {
    pub name: String,
    pub parts: Vec<GarmentPart>,
    /// What of the stock body it takes the place of: `body`, `arms`.
    pub covers: Vec<String>,
}

static EMBEDDED: RwLock<Option<HashMap<String, Image>>> = RwLock::new(None);

/// A picture that came inside a garment's file, by its material's name (lower case).
pub fn embedded_texture(name: &str) -> Option<Image> {
    EMBEDDED
        .read()
        .ok()?
        .as_ref()?
        .get(&name.to_ascii_lowercase())
        .cloned()
}

fn keep_embedded(name: &str, img: Image) {
    if let Ok(mut m) = EMBEDDED.write() {
        m.get_or_insert_with(HashMap::new)
            .insert(name.to_ascii_lowercase(), img);
    }
}

/// Forget the pictures that came inside files, before they are read again.
pub fn clear_embedded() {
    if let Ok(mut m) = EMBEDDED.write() {
        *m = None;
    }
}

struct File {
    json: Json,
    bin: Vec<u8>,
}

fn split_glb(bytes: &[u8]) -> Result<File, String> {
    if bytes.len() < 20 || &bytes[0..4] != b"glTF" {
        return Err("not a glTF binary (.glb) file".to_string());
    }
    let u32at = |i: usize| {
        u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]) as usize
    };
    let (mut at, mut json, mut bin) = (12, None, Vec::new());
    while at + 8 <= bytes.len() {
        let len = u32at(at);
        let kind = &bytes[at + 4..at + 8];
        let body = bytes
            .get(at + 8..at + 8 + len)
            .ok_or("a chunk of the file is cut short")?;
        if kind == b"JSON" {
            json = Some(json::parse(&String::from_utf8_lossy(body))?);
        } else if kind == b"BIN\0" {
            bin = body.to_vec();
        }
        at += 8 + len.div_ceil(4) * 4;
    }
    Ok(File {
        json: json.ok_or("the file has no JSON chunk")?,
        bin,
    })
}

struct Reader<'a> {
    file: &'a File,
}

impl Reader<'_> {
    /// The numbers of accessor `acc` as `n` floats each: (values, count). Integers are read as they are, or normalised when the
    /// accessor says so.
    fn floats(&self, acc: usize, n: usize) -> Result<(Vec<f32>, usize), String> {
        let a = self
            .file
            .json
            .get("accessors")
            .and_then(|x| x.at(acc))
            .ok_or("an accessor is missing")?;
        let count = a
            .get("count")
            .and_then(Json::usize)
            .ok_or("an accessor has no count")?;
        let comp = a
            .get("componentType")
            .and_then(Json::usize)
            .ok_or("an accessor has no type")?;
        let normalized = matches!(a.get("normalized"), Some(Json::Bool(true)));
        let per = match a.get("type").and_then(Json::str) {
            Some("SCALAR") => 1,
            Some("VEC2") => 2,
            Some("VEC3") => 3,
            Some("VEC4") => 4,
            _ => return Err("an accessor has a type that is not read".to_string()),
        };
        if per != n {
            return Err(format!(
                "an accessor holds {per} numbers a vertex, {n} wanted"
            ));
        }
        let view = a
            .get("bufferView")
            .and_then(Json::usize)
            .and_then(|v| self.file.json.get("bufferViews")?.at(v))
            .ok_or("an accessor has no buffer view")?;
        let start = view.get("byteOffset").and_then(Json::usize).unwrap_or(0)
            + a.get("byteOffset").and_then(Json::usize).unwrap_or(0);
        let stride = view.get("byteStride").and_then(Json::usize);
        let size = match comp {
            5120 | 5121 => 1,
            5122 | 5123 => 2,
            5125 | 5126 => 4,
            _ => return Err("an accessor has a component type that is not read".to_string()),
        };
        let step = stride.unwrap_or(size * per);
        let mut out = Vec::with_capacity(count * per);
        for i in 0..count {
            for k in 0..per {
                let at = start + i * step + k * size;
                let b = self
                    .file
                    .bin
                    .get(at..at + size)
                    .ok_or("an accessor reaches past the data")?;
                let v = match comp {
                    5126 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                    5125 => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32,
                    5123 => {
                        let v = u16::from_le_bytes([b[0], b[1]]) as f32;
                        if normalized {
                            v / 65535.0
                        } else {
                            v
                        }
                    }
                    5122 => i16::from_le_bytes([b[0], b[1]]) as f32,
                    5121 => {
                        let v = b[0] as f32;
                        if normalized {
                            v / 255.0
                        } else {
                            v
                        }
                    }
                    _ => b[0] as i8 as f32,
                };
                out.push(v);
            }
        }
        Ok((out, count))
    }
}

/// Read a garment from the text and data of a glTF file: `bytes` is a `.glb`, or the `.gltf` text with `bin` beside it.
/// `bones` are the names of the game's bones. `Err` holds every mistake found.
pub fn parse(
    name: &str,
    bytes: &[u8],
    bin: Option<&[u8]>,
    bones: &[String],
) -> Result<Garment, Vec<String>> {
    let file = if bytes.starts_with(b"glTF") {
        split_glb(bytes)
    } else {
        json::parse(&String::from_utf8_lossy(bytes)).map(|json| File {
            json,
            bin: bin.map(<[u8]>::to_vec).unwrap_or_default(),
        })
    }
    .map_err(|e| vec![e])?;
    let (mut errors, mut parts) = (Vec::new(), Vec::new());
    let r = Reader { file: &file };
    let j = &file.json;
    let skins = j.get("skins").map(Json::items).unwrap_or(&[]);
    let nodes = j.get("nodes").map(Json::items).unwrap_or(&[]);
    let mut triangles = 0;
    for node in nodes {
        let Some(mesh_id) = node.get("mesh").and_then(Json::usize) else {
            continue;
        };
        let Some(mesh) = j.get("meshes").and_then(|m| m.at(mesh_id)) else {
            continue;
        };
        let mesh_name = mesh
            .get("name")
            .and_then(Json::str)
            .or_else(|| node.get("name").and_then(Json::str))
            .unwrap_or("part")
            .to_ascii_lowercase();
        let Some(skin) = node
            .get("skin")
            .and_then(Json::usize)
            .and_then(|s| skins.get(s))
        else {
            errors.push(format!("{mesh_name}: the mesh has no skin; weight it to the game's bones (Ctrl+P > With Automatic Weights, or paint)"));
            continue;
        };
        let joints: Vec<String> = skin
            .get("joints")
            .map(Json::items)
            .unwrap_or(&[])
            .iter()
            .map(|n| {
                n.usize()
                    .and_then(|n| nodes.get(n))
                    .and_then(|n| n.get("name"))
                    .and_then(Json::str)
                    .unwrap_or("")
                    .to_ascii_lowercase()
            })
            .collect();
        let unknown: Vec<&String> = joints
            .iter()
            .filter(|j| !bones.iter().any(|b| b == *j))
            .collect();
        if !unknown.is_empty() {
            errors.push(format!(
                "{mesh_name}: bones the game does not have: {}",
                unknown
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            continue;
        }
        for prim in mesh.get("primitives").map(Json::items).unwrap_or(&[]) {
            let attr = |k: &str| {
                prim.get("attributes")
                    .and_then(|a| a.get(k))
                    .and_then(Json::usize)
            };
            let (Some(p), Some(jn), Some(wt)) =
                (attr("POSITION"), attr("JOINTS_0"), attr("WEIGHTS_0"))
            else {
                errors.push(format!("{mesh_name}: a piece needs positions, JOINTS_0 and WEIGHTS_0 (each vertex weighted to 1 to 4 bones)"));
                continue;
            };
            let Some(uv) = attr("TEXCOORD_0") else {
                errors.push(format!("{mesh_name}: a piece needs a UV map (TEXCOORD_0)"));
                continue;
            };
            let read = || -> Result<GarmentPart, String> {
                let (pos, n) = r.floats(p, 3)?;
                let (jv, _) = r.floats(jn, 4)?;
                let (wv, _) = r.floats(wt, 4)?;
                let (uvs, _) = r.floats(uv, 2)?;
                let normals = match attr("NORMAL") {
                    Some(a) => r.floats(a, 3)?.0,
                    None => [0.0, 0.0, 1.0].repeat(n),
                };
                let (idx, _) = match prim.get("indices").and_then(Json::usize) {
                    Some(i) => r.floats(i, 1)?,
                    None => ((0..n as u32).map(|v| v as f32).collect(), n),
                };
                // Y-up of the file to the game's Z-up: (x, y, z) -> (x, -z, y).
                let up = |v: &[f32; 3]| [v[0], -v[2], v[1]];
                let max_joint = joints.len() as f32;
                let mut bones_v = Vec::with_capacity(n);
                let mut weights_v = Vec::with_capacity(n);
                for i in 0..n {
                    let mut b = [0u16; 4];
                    let mut w = [0.0f32; 4];
                    for k in 0..4 {
                        let (jj, ww) = (jv[i * 4 + k], wv[i * 4 + k]);
                        if ww > 0.0 && jj < max_joint {
                            b[k] = jj as u16;
                            w[k] = ww;
                        }
                    }
                    let total: f32 = w.iter().sum();
                    if total <= 1e-6 {
                        return Err(format!("vertex {i} has no weight on any bone"));
                    }
                    weights_v.push(w.map(|x| x / total));
                    bones_v.push(b);
                }
                let faces: Vec<[u16; 3]> = idx
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .map(|t| [t[0] as u16, t[1] as u16, t[2] as u16])
                    .collect();
                if n > 65535 {
                    return Err("more than 65535 vertices in one piece".to_string());
                }
                // The picture is named by the material.
                let texture = prim
                    .get("material")
                    .and_then(Json::usize)
                    .and_then(|m| j.get("materials")?.at(m))
                    .and_then(|m| m.get("name").and_then(Json::str).map(str::to_string))
                    .unwrap_or_else(|| mesh_name.clone())
                    .to_ascii_lowercase();
                Ok(GarmentPart {
                    name: mesh_name.clone(),
                    positions: pos.as_chunks::<3>().0.iter().map(up).collect(),
                    normals: normals.as_chunks::<3>().0.iter().map(up).collect(),
                    uvs: uvs
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|t| [t[0], 1.0 - t[1]])
                        .collect(),
                    faces,
                    bones: bones_v,
                    weights: weights_v,
                    joints: joints.clone(),
                    texture,
                })
            };
            match read() {
                Ok(part) => {
                    triangles += part.faces.len();
                    parts.push(part);
                }
                Err(e) => errors.push(format!("{mesh_name}: {e}")),
            }
        }
    }
    if triangles > MAX_TRIANGLES {
        errors.push(format!(
            "{triangles} triangles; the most the game takes is {MAX_TRIANGLES}"
        ));
    }
    if parts.is_empty() && errors.is_empty() {
        errors.push("the file has no skinned mesh".to_string());
    }
    // Pictures inside the file: kept by material name, for a part that has no picture of the owner's in his folder.
    if errors.is_empty() {
        for (i, img) in j
            .get("images")
            .map(Json::items)
            .unwrap_or(&[])
            .iter()
            .enumerate()
        {
            let Some(view) = img
                .get("bufferView")
                .and_then(Json::usize)
                .and_then(|v| j.get("bufferViews")?.at(v))
            else {
                continue;
            };
            let off = view.get("byteOffset").and_then(Json::usize).unwrap_or(0);
            let len = view.get("byteLength").and_then(Json::usize).unwrap_or(0);
            let Some(data) = file.bin.get(off..off + len) else {
                continue;
            };
            let Some(picture) = kotor_image::parse_png(data) else {
                continue;
            };
            // Which material shows image `i`?
            for m in j.get("materials").map(Json::items).unwrap_or(&[]) {
                let tex = m
                    .get("pbrMetallicRoughness")
                    .and_then(|p| p.get("baseColorTexture"))
                    .and_then(|t| t.get("index"))
                    .and_then(Json::usize)
                    .and_then(|t| j.get("textures")?.at(t))
                    .and_then(|t| t.get("source"))
                    .and_then(Json::usize);
                if tex == Some(i) {
                    if let Some(n) = m.get("name").and_then(Json::str) {
                        keep_embedded(n, picture.clone());
                    }
                }
            }
        }
    }
    if errors.is_empty() {
        Ok(Garment {
            name: name.to_ascii_lowercase(),
            parts,
            covers: Vec::new(),
        })
    } else {
        Err(errors)
    }
}

impl Garment {
    /// The body with this garment on: what it covers is gone, and its pieces hang under the root, skinned to the body's bones.
    pub fn dress(&self, body: &Model) -> Model {
        let mut m = body.clone();
        let hide: Vec<&str> = self
            .covers
            .iter()
            .flat_map(|c| match c.as_str() {
                "body" | "torso" => ["torso", ""],
                "arms" => ["rarm", "larm"],
                _ => ["", ""],
            })
            .filter(|s| !s.is_empty())
            .collect();
        fn clear(n: &mut Node, hide: &[&str]) {
            if hide.iter().any(|h| n.name.eq_ignore_ascii_case(h)) {
                n.mesh = None;
            }
            for c in &mut n.children {
                clear(c, hide);
            }
        }
        clear(&mut m.root, &hide);
        // The body's nodes by name, with the numbers skins know them by.
        let mut index_of: HashMap<String, u16> = HashMap::new();
        let mut next = 0u16;
        m.root.walk(&mut |n| {
            index_of.insert(n.name.to_ascii_lowercase(), n.index);
            next = next.max(n.index + 1);
        });
        for (k, part) in self.parts.iter().enumerate() {
            // Bone numbers in this piece's own list, and the map from a node's place in the name table to that number.
            let mut bone_map = vec![f32::to_bits(-1.0); usize::from(next)];
            for (number, joint) in part.joints.iter().enumerate() {
                if let Some(i) = index_of.get(joint) {
                    bone_map[usize::from(*i)] = f32::to_bits(number as f32);
                }
            }
            let n = part.positions.len();
            let mesh = Mesh {
                texture: Some(part.texture.clone()),
                lightmap: None,
                diffuse: [1.0; 3],
                ambient: [1.0; 3],
                render: true,
                shadow: false,
                positions: part.positions.clone(),
                normals: part.normals.clone(),
                uvs: part.uvs.clone(),
                uvs2: Vec::new(),
                faces: part
                    .faces
                    .iter()
                    .map(|f| MeshFace {
                        indices: *f,
                        material: 0,
                        normal: [0.0, 0.0, 1.0],
                    })
                    .collect(),
                skin: Some(Skin {
                    weights: part.weights.clone(),
                    bones: part.bones.iter().map(|b| b.map(f32::from)).collect(),
                    bone_map,
                }),
            };
            debug_assert_eq!(mesh.uvs.len(), n);
            m.root.children.push(Node {
                name: format!("garment_{}_{k}", self.name),
                index: next + k as u16,
                number: next + k as u16,
                flags: kotor_model::NODE_HEADER | kotor_model::NODE_MESH,
                position: [0.0; 3],
                orientation: [1.0, 0.0, 0.0, 0.0],
                controllers: Vec::new(),
                mesh: Some(mesh),
                light: None,
                emitter: None,
                children: Vec::new(),
            });
        }
        m
    }
}
