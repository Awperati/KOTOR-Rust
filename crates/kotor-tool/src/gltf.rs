//! `kotor-tool blender-export`: a character as a glTF 2.0 file for Blender, with its skeleton (nodes named as the game names
//! them, in lower case), skin weights, textures as PNG under the game's own names, and every animation of its clip set sampled
//! 30 times a second. Read-only from the install; the folder it writes to is the caller's.
//!
//! Space: the game is Z-up, glTF Y-up. The whole character hangs under one node, `kotor_root`, turned to convert, and
//! everything below is in the game's own space, so a bone's numbers are the game's. Blender undoes the turn when it imports.

use kotor_archive::Install;
use kotor_model::{Model, Node, Pose};
use kotor_render::Rig;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;

type Mat = [[f32; 4]; 4];

/// The clips written when none are asked for: stand, walk, run, listen, talk, the fist stance and its blow, dying.
pub const DEFAULT_CLIPS: [&str; 9] = [
    "pause1", "pause2", "walk", "run", "listen", "tlknorm", "g8r1", "g8a1", "die",
];

/// Frames a second of an exported clip: the game's own.
pub const FPS: f32 = 30.0;

struct Buffer {
    data: Vec<u8>,
    views: Vec<String>,
    accessors: Vec<String>,
}

impl Buffer {
    fn new() -> Self {
        Buffer {
            data: Vec::new(),
            views: Vec::new(),
            accessors: Vec::new(),
        }
    }

    /// Add `bytes` as a buffer view and an accessor over it; returns the accessor's number.
    fn add(
        &mut self,
        bytes: &[u8],
        component: u32,
        count: usize,
        kind: &str,
        bounds: Option<(Vec<f32>, Vec<f32>)>,
    ) -> usize {
        while !self.data.len().is_multiple_of(4) {
            self.data.push(0);
        }
        let offset = self.data.len();
        self.data.extend_from_slice(bytes);
        let view = self.views.len();
        self.views.push(format!(
            "{{\"buffer\":0,\"byteOffset\":{offset},\"byteLength\":{}}}",
            bytes.len()
        ));
        let mut acc = format!(
            "{{\"bufferView\":{view},\"componentType\":{component},\"count\":{count},\"type\":\"{kind}\""
        );
        if let Some((lo, hi)) = bounds {
            let list = |v: &[f32]| {
                v.iter()
                    .map(|x| format!("{x}"))
                    .collect::<Vec<_>>()
                    .join(",")
            };
            let _ = write!(acc, ",\"min\":[{}],\"max\":[{}]", list(&lo), list(&hi));
        }
        acc.push('}');
        self.accessors.push(acc);
        self.accessors.len() - 1
    }

    fn floats(
        &mut self,
        v: &[f32],
        count: usize,
        kind: &str,
        bounds: Option<(Vec<f32>, Vec<f32>)>,
    ) -> usize {
        let bytes: Vec<u8> = v.iter().flat_map(|f| f.to_le_bytes()).collect();
        self.add(&bytes, 5126, count, kind, bounds)
    }
}

/// The inverse of a transform that is a turn and a shift.
fn rigid_inverse(m: &Mat) -> Mat {
    let mut out = [[0.0; 4]; 4];
    for (r, row) in out.iter_mut().enumerate().take(3) {
        for (c, v) in row.iter_mut().enumerate().take(3) {
            *v = m[c][r];
        }
    }
    for row in out.iter_mut().take(3) {
        row[3] = -(0..3).map(|k| row[k] * m[k][3]).sum::<f32>();
    }
    out[3][3] = 1.0;
    out
}

fn world_of(model: &Model, name: &str) -> Mat {
    model
        .world_matrices(None)
        .into_iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map_or(
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            |x| x.1,
        )
}

/// glTF is column-major: a matrix as 16 numbers.
fn column_major(m: &Mat) -> [f32; 16] {
    let mut out = [0.0; 16];
    for r in 0..4 {
        for c in 0..4 {
            out[c * 4 + r] = m[r][c];
        }
    }
    out
}

fn quat_xyzw(q: [f32; 4]) -> [f32; 4] {
    [q[1], q[2], q[3], q[0]]
}

/// One node of the skeleton in the glTF's list.
struct Slot {
    name: String,
    parent: Option<usize>,
    children: Vec<usize>,
    translation: [f32; 3],
    rotation: [f32; 4],
    mesh: Option<usize>,
}

/// Everything the file will hold, gathered before it is written.
struct Out {
    buf: Buffer,
    nodes: Vec<Slot>,
    meshes: Vec<String>,
    skins: Vec<String>,
    materials: Vec<String>,
    textures: Vec<String>,
    images: Vec<String>,
    material_of: HashMap<String, usize>,
    pngs: Vec<(String, Vec<u8>)>,
    animations: Vec<String>,
    skinned: Vec<(usize, usize)>,
}

fn add_nodes(
    node: &Node,
    parent: Option<usize>,
    prefix: &str,
    out: &mut Out,
    map: &mut HashMap<String, usize>,
) {
    let id = out.nodes.len();
    let name = format!("{prefix}{}", node.name.to_ascii_lowercase());
    out.nodes.push(Slot {
        name: name.clone(),
        parent,
        children: Vec::new(),
        translation: node.position,
        rotation: quat_xyzw(node.orientation),
        mesh: None,
    });
    if let Some(p) = parent {
        out.nodes[p].children.push(id);
    }
    map.insert(node.name.to_ascii_lowercase(), id);
    for c in &node.children {
        add_nodes(c, Some(id), prefix, out, map);
    }
}

/// The material of a texture named `name`: its picture written as PNG, and a material that shows it.
fn material(out: &mut Out, inst: &mut Install, name: &str) -> Option<usize> {
    let key = name.to_ascii_lowercase();
    if let Some(m) = out.material_of.get(&key) {
        return Some(*m);
    }
    let img = kotor_render::load_texture(inst, name)?;
    out.pngs.push((key.clone(), kotor_image::to_png(&img)));
    let image = out.images.len();
    out.images.push(format!(
        "{{\"uri\":\"textures/{key}.png\",\"name\":\"{key}\"}}"
    ));
    let texture = out.textures.len();
    out.textures
        .push(format!("{{\"source\":{image},\"sampler\":0}}"));
    let m = out.materials.len();
    out.materials.push(format!(
        "{{\"name\":\"{key}\",\"doubleSided\":true,\"alphaMode\":\"MASK\",\"alphaCutoff\":0.5,\"pbrMetallicRoughness\":{{\"baseColorTexture\":{{\"index\":{texture}}},\"metallicFactor\":0.0,\"roughnessFactor\":1.0}}}}"
    ));
    out.material_of.insert(key, m);
    Some(m)
}

/// One mesh of a model as a glTF mesh; vertices are put in the model's rest world space when `world` is set (a skinned mesh).
fn add_mesh(
    out: &mut Out,
    inst: &mut Install,
    node: &Node,
    world: Option<&Mat>,
    texture: Option<&str>,
    joint_of: &dyn Fn(u32) -> Option<usize>,
) -> Option<usize> {
    let mesh = node
        .mesh
        .as_ref()
        .filter(|m| m.render && !m.positions.is_empty() && !m.faces.is_empty())?;
    let tex = mesh
        .texture
        .as_deref()
        .filter(|t| !t.eq_ignore_ascii_case("null"))
        .or(texture)?;
    let material = material(out, inst, tex);
    let n = mesh.positions.len();
    let mut pos = Vec::with_capacity(n * 3);
    let mut nor = Vec::with_capacity(n * 3);
    let (mut lo, mut hi) = (vec![f32::MAX; 3], vec![f32::MIN; 3]);
    for (i, p) in mesh.positions.iter().enumerate() {
        let (q, nn) = match world {
            Some(w) => {
                let nrm = mesh.normals.get(i).copied().unwrap_or([0.0, 0.0, 1.0]);
                (
                    [0, 1, 2].map(|r| w[r][0] * p[0] + w[r][1] * p[1] + w[r][2] * p[2] + w[r][3]),
                    [0, 1, 2].map(|r| w[r][0] * nrm[0] + w[r][1] * nrm[1] + w[r][2] * nrm[2]),
                )
            }
            None => (*p, mesh.normals.get(i).copied().unwrap_or([0.0, 0.0, 1.0])),
        };
        for k in 0..3 {
            lo[k] = lo[k].min(q[k]);
            hi[k] = hi[k].max(q[k]);
        }
        pos.extend_from_slice(&q);
        nor.extend_from_slice(&nn);
    }
    let a_pos = out.buf.floats(&pos, n, "VEC3", Some((lo, hi)));
    let a_nor = out.buf.floats(&nor, n, "VEC3", None);
    let mut attrs = format!("\"POSITION\":{a_pos},\"NORMAL\":{a_nor}");
    if mesh.uvs.len() == n {
        let uv: Vec<f32> = mesh.uvs.iter().flat_map(|t| [t[0], 1.0 - t[1]]).collect();
        let a = out.buf.floats(&uv, n, "VEC2", None);
        let _ = write!(attrs, ",\"TEXCOORD_0\":{a}");
    }
    if let Some(skin) = mesh
        .skin
        .as_ref()
        .filter(|s| s.weights.len() == n && s.bones.len() == n)
    {
        let mut joints = Vec::with_capacity(n * 4);
        let mut weights = Vec::with_capacity(n * 4);
        for i in 0..n {
            for k in 0..4 {
                // Vertices hold bone numbers, which are the positions in the mesh's own list of joints.
                let bone = skin.bones[i][k];
                let w = skin.weights[i][k];
                match (joint_of(bone as u32), w > 0.0) {
                    (Some(j), true) => {
                        joints.push(j as u16);
                        weights.push(w);
                    }
                    _ => {
                        joints.push(0);
                        weights.push(0.0);
                    }
                }
            }
        }
        let bytes: Vec<u8> = joints.iter().flat_map(|j| j.to_le_bytes()).collect();
        let a = out.buf.add(&bytes, 5123, n, "VEC4", None);
        let _ = write!(attrs, ",\"JOINTS_0\":{a}");
        let a = out.buf.floats(&weights, n, "VEC4", None);
        let _ = write!(attrs, ",\"WEIGHTS_0\":{a}");
    }
    let mut idx: Vec<u8> = Vec::with_capacity(mesh.faces.len() * 6);
    for f in &mesh.faces {
        for i in f.indices {
            idx.extend_from_slice(&i.to_le_bytes());
        }
    }
    let a_idx = out
        .buf
        .add(&idx, 5123, mesh.faces.len() * 3, "SCALAR", None);
    let mat = material.map_or(String::new(), |m| format!(",\"material\":{m}"));
    out.meshes.push(format!(
        "{{\"name\":\"{}\",\"primitives\":[{{\"attributes\":{{{attrs}}},\"indices\":{a_idx}{mat}}}]}}",
        node.name.to_ascii_lowercase()
    ));
    Some(out.meshes.len() - 1)
}

/// The file's text for node `i` and what hangs under it.
fn node_json(out: &Out, i: usize) -> String {
    let s = &out.nodes[i];
    let mut j = format!("{{\"name\":\"{}\"", s.name);
    if !s.children.is_empty() {
        let c: Vec<String> = s.children.iter().map(usize::to_string).collect();
        let _ = write!(j, ",\"children\":[{}]", c.join(","));
    }
    let _ = write!(
        j,
        ",\"translation\":[{},{},{}],\"rotation\":[{},{},{},{}]",
        s.translation[0],
        s.translation[1],
        s.translation[2],
        s.rotation[0],
        s.rotation[1],
        s.rotation[2],
        s.rotation[3]
    );
    if let Some(m) = s.mesh {
        let _ = write!(j, ",\"mesh\":{m}");
        if let Some((_, skin)) = out.skinned.iter().find(|(n, _)| *n == i) {
            let _ = write!(j, ",\"skin\":{skin}");
        }
    }
    j.push('}');
    j
}

/// Write the character `rig` (its model names `body` and `head`) into `dir`: `<name>.gltf`, `<name>.bin`, `textures/*.png`
/// and `bones.txt`. Returns a summary line.
pub fn export(
    inst: &mut Install,
    rig: &Rig,
    name: &str,
    dir: &Path,
    clips: Option<&[String]>,
) -> std::io::Result<String> {
    std::fs::create_dir_all(dir.join("textures"))?;
    let mut out = Out {
        buf: Buffer::new(),
        nodes: Vec::new(),
        meshes: Vec::new(),
        skins: Vec::new(),
        materials: Vec::new(),
        textures: Vec::new(),
        images: Vec::new(),
        material_of: HashMap::new(),
        pngs: Vec::new(),
        animations: Vec::new(),
        skinned: Vec::new(),
    };
    // The root that turns the game's Z up into glTF's Y up.
    let half = std::f32::consts::FRAC_1_SQRT_2;
    out.nodes.push(Slot {
        name: "kotor_root".into(),
        parent: None,
        children: Vec::new(),
        translation: [0.0; 3],
        rotation: [-half, 0.0, 0.0, half],
        mesh: None,
    });
    let body = rig.body_model();
    let mut by_name: HashMap<String, usize> = HashMap::new();
    add_nodes(&body.root, Some(0), "", &mut out, &mut by_name);
    let body_texture = rig.body_texture().map(str::to_string);
    // The nodes of the body by the number the skins give them.
    let mut by_index: HashMap<u32, usize> = HashMap::new();
    body.root.walk(&mut |n| {
        if let Some(id) = by_name.get(&n.name.to_ascii_lowercase()) {
            by_index.insert(u32::from(n.index), *id);
        }
    });
    let world = body.world_matrices(None);
    let mut meshes_to_add: Vec<&Node> = Vec::new();
    body.root.walk(&mut |n| {
        if n.mesh.is_some() {
            meshes_to_add.push(n);
        }
    });
    let mut joint_lists: Vec<Vec<usize>> = Vec::new();
    for n in meshes_to_add {
        let skinned = n.mesh.as_ref().is_some_and(|m| m.skin.is_some());
        let w = world
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(&n.name))
            .map(|x| x.1);
        // `bone_map` is indexed by a node's place in the name table and holds its bone number (not a bone: all ones, or a
        // float outside 0 to 4096). The joints are the nodes by bone number; a number no node has stands for the first.
        let mut joints_here: Vec<usize> = Vec::new();
        if let Some(sk) = n.mesh.as_ref().and_then(|m| m.skin.as_ref()) {
            for (name_idx, raw) in sk.bone_map.iter().enumerate() {
                let bone = f32::from_bits(*raw);
                if !(0.0..4096.0).contains(&bone) {
                    continue;
                }
                let Some(node_id) = by_index.get(&(name_idx as u32)).copied() else {
                    continue;
                };
                let bone = bone as usize;
                if joints_here.len() <= bone {
                    joints_here.resize(bone + 1, usize::MAX);
                }
                joints_here[bone] = node_id;
            }
        }
        let fallback = joints_here.iter().copied().find(|j| *j != usize::MAX);
        for j in &mut joints_here {
            if *j == usize::MAX {
                *j = fallback.unwrap_or(0);
            }
        }
        let pos_in_list = |bone: u32| (bone as usize) < joints_here.len();
        let pos_in_list = |bone: u32| pos_in_list(bone).then_some(bone as usize);
        let Some(mesh_id) = add_mesh(
            &mut out,
            inst,
            n,
            if skinned { w.as_ref() } else { None },
            body_texture.as_deref(),
            &pos_in_list,
        ) else {
            continue;
        };
        if skinned {
            // A skinned mesh is carried by a node of its own under the root, since a skin ignores its node's place.
            let carrier = out.nodes.len();
            out.nodes.push(Slot {
                name: format!("{}.mesh", n.name.to_ascii_lowercase()),
                parent: Some(0),
                children: Vec::new(),
                translation: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
                mesh: Some(mesh_id),
            });
            out.nodes[0].children.push(carrier);
            // Inverse bind matrices: each bone's rest place undone.
            let mut ibm = Vec::new();
            for j in &joints_here {
                let bone_name = out.nodes[*j].name.clone();
                ibm.extend_from_slice(&column_major(&rigid_inverse(&world_of(body, &bone_name))));
            }
            let a = out.buf.floats(&ibm, joints_here.len(), "MAT4", None);
            let list: Vec<String> = joints_here.iter().map(usize::to_string).collect();
            out.skins.push(format!(
                "{{\"name\":\"{}\",\"inverseBindMatrices\":{a},\"joints\":[{}]}}",
                n.name.to_ascii_lowercase(),
                list.join(",")
            ));
            out.skinned.push((carrier, out.skins.len() - 1));
            joint_lists.push(joints_here);
        } else if let Some(id) = by_name.get(&n.name.to_ascii_lowercase()) {
            out.nodes[*id].mesh = Some(mesh_id);
        }
    }
    // The head hangs rigidly under the body's head_g, its own head_g laid on it; its names carry `head.` so none repeats.
    if let Some(head) = rig.head_model() {
        // A creature's body has a `head_g` for the head's own to lie on; a player's has only a `headhook`, with the head's
        // origin at the neck riding it as it is.
        let on_head_g = by_name.contains_key("head_g");
        let hook_name = if on_head_g { "head_g" } else { "headhook" };
        if let Some(&hook) = by_name.get(hook_name) {
            let mut head_names = HashMap::new();
            let first = out.nodes.len();
            add_nodes(&head.root, Some(hook), "head.", &mut out, &mut head_names);
            let identity: Mat = [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ];
            let local = if on_head_g {
                rigid_inverse(&world_of(head, "head_g"))
            } else {
                identity
            };
            let q = {
                // Rotation of the matrix as a quaternion, xyzw.
                let m = local;
                let t = m[0][0] + m[1][1] + m[2][2];
                if t > 0.0 {
                    let s = (t + 1.0).sqrt() * 2.0;
                    [
                        (m[2][1] - m[1][2]) / s,
                        (m[0][2] - m[2][0]) / s,
                        (m[1][0] - m[0][1]) / s,
                        0.25 * s,
                    ]
                } else {
                    [0.0, 0.0, 0.0, 1.0]
                }
            };
            out.nodes[first].translation = [local[0][3], local[1][3], local[2][3]];
            out.nodes[first].rotation = q;
            let head_texture = rig.head_texture().map(str::to_string);
            let mut head_meshes: Vec<&Node> = Vec::new();
            head.root.walk(&mut |n| {
                if n.mesh.is_some() {
                    head_meshes.push(n);
                }
            });
            for n in head_meshes {
                if let Some(mesh_id) =
                    add_mesh(&mut out, inst, n, None, head_texture.as_deref(), &|_| None)
                {
                    if let Some(id) = head_names.get(&n.name.to_ascii_lowercase()) {
                        out.nodes[*id].mesh = Some(mesh_id);
                    }
                }
            }
        }
    }
    // Animations: every clip of the rig, 30 frames a second, as translation and rotation of each node it moves.
    let mut clip_count = 0;
    // A body has some 270 clips, which Blender takes many minutes to read; the ones people start from come unless asked.
    // `clips` of `Some(empty)` is every clip.
    let wanted: Vec<String> = match clips {
        Some(list) => list.to_vec(),
        None => DEFAULT_CLIPS.iter().map(|c| c.to_string()).collect(),
    };
    let chosen: Vec<String> = rig
        .clip_names()
        .into_iter()
        .filter(|c| wanted.is_empty() || wanted.iter().any(|w| w.eq_ignore_ascii_case(c)))
        .collect();
    for clip in chosen {
        let Some(length) = rig.animation_length(&clip).filter(|l| *l > 0.0) else {
            continue;
        };
        let frames = (length * FPS).ceil() as usize + 1;
        let poses: Vec<Pose> = (0..frames)
            .filter_map(|f| rig.sample(&clip, (f as f32 / FPS).min(length)))
            .collect();
        if poses.len() != frames {
            continue;
        }
        let times: Vec<f32> = (0..frames).map(|f| f as f32 / FPS).collect();
        let a_time = out.buf.floats(
            &times,
            frames,
            "SCALAR",
            Some((vec![0.0], vec![times[frames - 1]])),
        );
        let mut samplers = Vec::new();
        let mut channels = Vec::new();
        let mut moved: Vec<&String> = poses[0]
            .keys()
            .filter(|k| by_name.contains_key(*k))
            .collect();
        moved.sort();
        for node in moved {
            let target = by_name[node];
            let trans: Vec<f32> = poses
                .iter()
                .flat_map(|p| p.get(node).map_or([0.0; 3], |n| n.position))
                .collect();
            let rot: Vec<f32> = poses
                .iter()
                .flat_map(|p| {
                    p.get(node)
                        .map_or([0.0, 0.0, 0.0, 1.0], |n| quat_xyzw(n.orientation))
                })
                .collect();
            let a_t = out.buf.floats(&trans, frames, "VEC3", None);
            let a_r = out.buf.floats(&rot, frames, "VEC4", None);
            for (path, acc) in [("translation", a_t), ("rotation", a_r)] {
                samplers.push(format!(
                    "{{\"input\":{a_time},\"output\":{acc},\"interpolation\":\"LINEAR\"}}"
                ));
                channels.push(format!(
                    "{{\"sampler\":{},\"target\":{{\"node\":{target},\"path\":\"{path}\"}}}}",
                    samplers.len() - 1
                ));
            }
        }
        out.animations.push(format!(
            "{{\"name\":\"{clip}\",\"samplers\":[{}],\"channels\":[{}]}}",
            samplers.join(","),
            channels.join(",")
        ));
        clip_count += 1;
    }
    // Write it all.
    let mut json = String::from(
        "{\"asset\":{\"version\":\"2.0\",\"generator\":\"kotor-tool blender-export\"}",
    );
    json.push_str(",\"scene\":0,\"scenes\":[{\"nodes\":[0]}]");
    let nodes: Vec<String> = (0..out.nodes.len()).map(|i| node_json(&out, i)).collect();
    let _ = write!(json, ",\"nodes\":[{}]", nodes.join(","));
    let _ = write!(json, ",\"meshes\":[{}]", out.meshes.join(","));
    if !out.skins.is_empty() {
        let _ = write!(json, ",\"skins\":[{}]", out.skins.join(","));
    }
    if !out.materials.is_empty() {
        let _ = write!(json, ",\"materials\":[{}]", out.materials.join(","));
        let _ = write!(json, ",\"textures\":[{}]", out.textures.join(","));
        let _ = write!(json, ",\"images\":[{}]", out.images.join(","));
        json.push_str(",\"samplers\":[{\"magFilter\":9729,\"minFilter\":9987,\"wrapS\":10497,\"wrapT\":10497}]");
    }
    if !out.animations.is_empty() {
        let _ = write!(json, ",\"animations\":[{}]", out.animations.join(","));
    }
    let _ = write!(
        json,
        ",\"buffers\":[{{\"uri\":\"{name}.bin\",\"byteLength\":{}}}]",
        out.buf.data.len()
    );
    let _ = write!(json, ",\"bufferViews\":[{}]", out.buf.views.join(","));
    let _ = write!(json, ",\"accessors\":[{}]", out.buf.accessors.join(","));
    json.push('}');
    std::fs::write(dir.join(format!("{name}.gltf")), json)?;
    std::fs::write(dir.join(format!("{name}.bin")), &out.buf.data)?;
    for (key, png) in &out.pngs {
        std::fs::write(dir.join("textures").join(format!("{key}.png")), png)?;
    }
    // The skeleton the add-on builds its own armature from, so that a bone's frame in Blender is the game's: name, parent,
    // then the bone's place and turn (w x y z) relative to its parent at rest. The root has `-` for a parent.
    let mut bones = String::from(
        "# Bones as the game names them (lower case), parents first: name, parent, place x y z, turn w x y z, at rest.\n",
    );
    for s in out.nodes.iter().skip(1) {
        if s.name.ends_with(".mesh") || s.name.starts_with("head.") {
            continue;
        }
        let parent = s
            .parent
            .filter(|p| *p != 0)
            .map_or("-", |p| out.nodes[p].name.as_str());
        let _ = writeln!(
            bones,
            "{}\t{parent}\t{} {} {}\t{} {} {} {}",
            s.name,
            s.translation[0],
            s.translation[1],
            s.translation[2],
            s.rotation[3],
            s.rotation[0],
            s.rotation[1],
            s.rotation[2]
        );
    }
    std::fs::write(dir.join("bones.txt"), bones)?;
    Ok(format!(
        "{} nodes, {} meshes, {} skins, {} textures, {} clips of {FPS} frames a second, into {}",
        out.nodes.len(),
        out.meshes.len(),
        out.skins.len(),
        out.pngs.len(),
        clip_count,
        dir.display()
    ))
}
