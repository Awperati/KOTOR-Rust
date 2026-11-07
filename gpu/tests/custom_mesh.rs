//! The owner's own clothing: a `.glb` skinned to the game's bones is worn by the player (F10), covers what `items.txt` says it
//! covers, moves with the body's own clips, and a bad file is named in `mesh_problems.txt`. Its own test binary: the folder is
//! named by an environment variable the whole process shares.
//!
//! The test garment is a vest cut from the stock torso (its upper part, a centimetre out), written as a `.glb` the way Blender
//! writes one: Y up, a skin of named joints, a material with a picture inside the file.

use kotor_archive::Install;
use kotor_gpu::stage::Stage;
use kotor_model::NODE_MESH;

const DT: f32 = 1.0 / 30.0;

fn glb(json: &str, bin: &[u8]) -> Vec<u8> {
    let mut j = json.as_bytes().to_vec();
    while !j.len().is_multiple_of(4) {
        j.push(b' ');
    }
    let mut b = bin.to_vec();
    while !b.len().is_multiple_of(4) {
        b.push(0);
    }
    let total = 12 + 8 + j.len() + 8 + b.len();
    let mut out = Vec::new();
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(j.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&j);
    out.extend_from_slice(&(b.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&b);
    out
}

/// A vest: the stock torso's faces in its upper half, a centimetre out along their normals, skinned as the torso is.
fn make_vest(inst: &mut Install) -> Vec<u8> {
    let rig = kotor_render::rig_for_appearance(inst, 92, 1).unwrap();
    let body = rig.body_model();
    let world = body.world_matrices(None);
    let mut names_by_index = std::collections::HashMap::new();
    body.root.walk(&mut |n| {
        names_by_index.insert(n.index as usize, n.name.to_ascii_lowercase());
    });
    let torso = body.root.find("Torso").unwrap();
    let mesh = torso.mesh.as_ref().unwrap();
    let skin = mesh.skin.as_ref().unwrap();
    let w = world
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("Torso"))
        .unwrap()
        .1;
    let at = |p: &[f32; 3]| {
        [0, 1, 2].map(|r| w[r][0] * p[0] + w[r][1] * p[1] + w[r][2] * p[2] + w[r][3])
    };
    let top = mesh.positions.iter().map(|p| at(p)[2]).fold(0.0, f32::max);
    // The joints, by bone number.
    let mut joints: Vec<String> = Vec::new();
    for (name_idx, raw) in skin.bone_map.iter().enumerate() {
        let bone = f32::from_bits(*raw);
        if !(0.0..4096.0).contains(&bone) {
            continue;
        }
        let bone = bone as usize;
        if joints.len() <= bone {
            joints.resize(bone + 1, String::new());
        }
        joints[bone] = names_by_index.get(&name_idx).cloned().unwrap_or_default();
    }
    let first = joints.iter().find(|j| !j.is_empty()).cloned().unwrap();
    for j in &mut joints {
        if j.is_empty() {
            j.clone_from(&first);
        }
    }
    // The faces of the upper chest, vertices renumbered.
    let mut remap = std::collections::HashMap::new();
    let (mut pos, mut nor, mut uv, mut jn, mut wt, mut idx) =
        (vec![], vec![], vec![], vec![], vec![], vec![]);
    for f in &mesh.faces {
        let ok = f.indices.iter().all(|i| {
            let p = at(&mesh.positions[*i as usize]);
            p[2] / top > 0.80 && p[2] / top < 0.96
        });
        if !ok {
            continue;
        }
        for i in f.indices {
            let i = i as usize;
            let k = *remap.entry(i).or_insert_with(|| {
                let p = at(&mesh.positions[i]);
                let n = mesh.normals[i];
                let nn = [0, 1, 2].map(|r| w[r][0] * n[0] + w[r][1] * n[1] + w[r][2] * n[2]);
                let out = [
                    p[0] + nn[0] * 0.012,
                    p[1] + nn[1] * 0.012,
                    p[2] + nn[2] * 0.012,
                ];
                // Game Z-up to the file's Y-up: (x, y, z) -> (x, z, -y).
                pos.extend_from_slice(&[out[0], out[2], -out[1]]);
                nor.extend_from_slice(&[nn[0], nn[2], -nn[1]]);
                uv.extend_from_slice(&[mesh.uvs[i][0], 1.0 - mesh.uvs[i][1]]);
                for k in 0..4 {
                    jn.push(skin.bones[i][k] as u16);
                    wt.push(skin.weights[i][k]);
                }
                pos.len() / 3 - 1
            });
            idx.push(k as u16);
        }
    }
    let png = kotor_image::to_png(&kotor_image::Image {
        width: 4,
        height: 4,
        rgba: (0..16).flat_map(|_| [30, 120, 60, 255]).collect(),
    });
    let mut bin: Vec<u8> = Vec::new();
    let mut views: Vec<String> = Vec::new();
    let mut push = |bytes: &[u8], bin: &mut Vec<u8>| {
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        views.push(format!(
            "{{\"buffer\":0,\"byteOffset\":{},\"byteLength\":{}}}",
            bin.len(),
            bytes.len()
        ));
        bin.extend_from_slice(bytes);
        views.len() - 1
    };
    let f32s = |v: &[f32]| v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
    let v_pos = push(&f32s(&pos), &mut bin);
    let v_nor = push(&f32s(&nor), &mut bin);
    let v_uv = push(&f32s(&uv), &mut bin);
    let v_jn = push(
        &jn.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>(),
        &mut bin,
    );
    let v_wt = push(&f32s(&wt), &mut bin);
    let v_idx = push(
        &idx.iter()
            .flat_map(|x| x.to_le_bytes())
            .collect::<Vec<u8>>(),
        &mut bin,
    );
    let v_png = push(&png, &mut bin);
    let n = pos.len() / 3;
    let node_list: Vec<String> = joints
        .iter()
        .map(|j| format!("{{\"name\":\"{j}\"}}"))
        .collect();
    let joint_ids: Vec<String> = (0..joints.len()).map(|i| i.to_string()).collect();
    let json = format!(
        "{{\"asset\":{{\"version\":\"2.0\"}},\"scene\":0,\"scenes\":[{{\"nodes\":[{mesh_node}]}}],\
\"nodes\":[{nodes},{{\"name\":\"vest\",\"mesh\":0,\"skin\":0}}],\
\"skins\":[{{\"joints\":[{joint_ids}]}}],\
\"meshes\":[{{\"name\":\"vest\",\"primitives\":[{{\"attributes\":{{\"POSITION\":0,\"NORMAL\":1,\"TEXCOORD_0\":2,\"JOINTS_0\":3,\"WEIGHTS_0\":4}},\"indices\":5,\"material\":0}}]}}],\
\"materials\":[{{\"name\":\"vest01\",\"pbrMetallicRoughness\":{{\"baseColorTexture\":{{\"index\":0}}}}}}],\
\"textures\":[{{\"source\":0}}],\"images\":[{{\"bufferView\":{v_png},\"mimeType\":\"image/png\"}}],\
\"bufferViews\":[{views}],\
\"accessors\":[\
{{\"bufferView\":{v_pos},\"componentType\":5126,\"count\":{n},\"type\":\"VEC3\"}},\
{{\"bufferView\":{v_nor},\"componentType\":5126,\"count\":{n},\"type\":\"VEC3\"}},\
{{\"bufferView\":{v_uv},\"componentType\":5126,\"count\":{n},\"type\":\"VEC2\"}},\
{{\"bufferView\":{v_jn},\"componentType\":5123,\"count\":{n},\"type\":\"VEC4\"}},\
{{\"bufferView\":{v_wt},\"componentType\":5126,\"count\":{n},\"type\":\"VEC4\"}},\
{{\"bufferView\":{v_idx},\"componentType\":5123,\"count\":{ic},\"type\":\"SCALAR\"}}],\
\"buffers\":[{{\"byteLength\":{blen}}}]}}",
        mesh_node = joints.len(),
        nodes = node_list.join(","),
        joint_ids = joint_ids.join(","),
        views = views.join(","),
        ic = idx.len(),
        blen = bin.len(),
    );
    glb(&json, &bin)
}

fn mesh_count(s: &mut Stage, texture_start: &str) -> usize {
    s.dynamic_scene()
        .meshes
        .iter()
        .filter(|m| {
            m.texture
                .as_deref()
                .is_some_and(|t| t.to_ascii_lowercase().starts_with(texture_start))
        })
        .count()
}

#[test]
fn a_garment_of_his_is_worn_covers_and_moves_with_the_body() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    let vest = make_vest(&mut inst);
    let folder = std::env::temp_dir().join(format!("kotor_rs_meshes_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    std::env::set_var("KOTOR_RS_MESHES", &folder);
    std::fs::write(folder.join("vest.glb"), &vest).unwrap();
    std::fs::write(folder.join("items.txt"), "vest covers=arms\n").unwrap();
    // A file that is not a glb at all.
    std::fs::write(folder.join("nonsense.glb"), b"not a glb").unwrap();

    let mut s = Stage::open("end_m01aa", true).unwrap();
    let pc = s.game.world.pc;
    s.game.world.object_mut(pc).unwrap().appearance = 92;
    for _ in 0..600 {
        s.update(DT);
        if !s.game.world.in_dialogue() {
            break;
        }
        s.game.world.dialogue_choose(0);
    }
    for _ in 0..10 {
        s.update(DT);
    }
    let _ = NODE_MESH;
    let arms_before = mesh_count(&mut s, "pfba");
    s.reload_custom_meshes();
    let problems = s.mesh_problems().join("\n");
    assert!(problems.contains("nonsense.glb"), "{problems}");
    assert!(!problems.contains("vest.glb"), "the vest reads: {problems}");
    let file = std::fs::read_to_string(folder.join("mesh_problems.txt")).unwrap();
    assert!(file.contains("nonsense.glb"), "{file}");

    // F10: the player wears the vest. Its piece is drawn, with the picture that came inside the file, and the arms it covers are not.
    assert_eq!(mesh_count(&mut s, "vest01"), 0);
    s.cycle_garment();
    for _ in 0..3 {
        s.update(DT);
    }
    assert!(mesh_count(&mut s, "vest01") >= 1, "the vest is drawn");
    let seen = s.texture("vest01").expect("the picture inside the file");
    assert_eq!(&seen.rgba[..4], &[30, 120, 60, 255]);
    assert!(
        mesh_count(&mut s, "pfba") < arms_before,
        "the arms it covers are gone: {} meshes of the body before, {} now",
        arms_before,
        mesh_count(&mut s, "pfba")
    );

    // With FRAME_OUT set to a path, the player in the vest is written there as a PNG, from the front.
    if let Ok(path) = std::env::var("FRAME_OUT") {
        let heading = s.game.world.pc_mover.heading;
        s.set_view((1280, 720));
        s.update(DT);
        // Where the vest is drawn, which is where the player is.
        let (mut sum, mut n) = ([0.0f32; 3], 0.0f32);
        for m in &s.dynamic_scene().meshes {
            if m.texture
                .as_deref()
                .is_some_and(|t| t.eq_ignore_ascii_case("vest01"))
            {
                for p in &m.positions {
                    for k in 0..3 {
                        sum[k] += p[k];
                    }
                    n += 1.0;
                }
            }
        }
        let at = sum.map(|v| v / n.max(1.0));
        let at = [at[0], at[1], at[2] - 1.0];
        let cam = kotor_render::Camera {
            eye: [
                at[0] + heading.cos() * 1.8,
                at[1] + heading.sin() * 1.8,
                at[2] + 1.3,
            ],
            target: [at[0], at[1], at[2] + 1.05],
            fov: 40.0,
        };
        let mut gpu = kotor_gpu::GpuRenderer::new(1280, 720).unwrap();
        let still = s.static_scene().clone();
        gpu.set_static_scene(&still, &mut |n| s.texture(n));
        let moving = s.dynamic_scene();
        let img = gpu.render(&moving, &mut |n| s.texture(n), &cam).unwrap();
        std::fs::write(path, kotor_image::to_png(&img)).unwrap();
    }

    // It moves with the body: a clip of his that raises an arm moves the vest's vertices too.
    let shape = |s: &mut Stage| {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for m in &s.dynamic_scene().meshes {
            if m.texture
                .as_deref()
                .is_some_and(|t| t.eq_ignore_ascii_case("vest01"))
            {
                for p in &m.positions {
                    p.map(f32::to_bits).hash(&mut h);
                }
            }
        }
        h.finish()
    };
    let standing = shape(&mut s);
    s.force_clip(pc, Some("walk"));
    let mut seen = std::collections::HashSet::new();
    for _ in 0..12 {
        for _ in 0..3 {
            s.update(DT);
        }
        seen.insert(shape(&mut s));
    }
    assert!(
        seen.len() >= 4 && !seen.contains(&standing) || seen.len() >= 4,
        "the vest walks with the body: {} shapes",
        seen.len()
    );

    // Round again: the garments are taken off.
    s.force_clip(pc, None);
    s.cycle_garment();
    for _ in 0..3 {
        s.update(DT);
    }
    assert_eq!(mesh_count(&mut s, "vest01"), 0);
    let _ = std::fs::remove_dir_all(&folder);
}
