use crate::*;
use kotor_core::{Error, Reader, Result};

/// Offsets inside an MDL are relative to byte 12 (after the file preamble).
const BASE: usize = 12;
const MAX_DEPTH: usize = 128;
const NONE: u32 = 0xFFFF_FFFF;

struct Ctx<'a> {
    /// MDL bytes after the 12-byte preamble.
    d: &'a [u8],
    mdx: &'a [u8],
    names: Vec<String>,
}

pub fn parse_model(mdl: &[u8], mdx: &[u8]) -> Result<Model> {
    if mdl.len() < BASE + 196 {
        return Err(Error::format("MDL too short"));
    }
    let mut pre = Reader::new(mdl);
    if pre.u32()? != 0 {
        return Err(Error::format(
            "MDL preamble is not zero (ascii or unknown format)",
        ));
    }
    let body = &mdl[BASE..];
    let mut r = Reader::new(body);

    // Geometry header, 80 bytes.
    r.skip(8);
    let name = r.fixed_str(32)?;
    let root_off = r.u32()? as usize;
    let node_count = r.u32()?;

    // Model header.
    r.seek(80);
    let classification = r.u8()?;
    r.seek(88);
    let anim_off = r.u32()? as usize;
    let anim_count = r.u32()? as usize;
    r.seek(104);
    let bbox = [r.vec3()?, r.vec3()?];
    let radius = r.f32()?;
    let anim_scale = r.f32()?;
    let supermodel = r.fixed_str(32)?;
    r.seek(184);
    let names_off = r.u32()? as usize;
    let names_n = r.u32()? as usize;

    let mut names = Vec::with_capacity(names_n.min(4096));
    let mut nr = Reader::at(body, names_off);
    for _ in 0..names_n {
        let o = nr.u32()? as usize;
        names.push(Reader::at(body, o).fixed_str(64.min(body.len().saturating_sub(o)))?);
    }
    let ctx = Ctx {
        d: body,
        mdx,
        names,
    };
    let root = ctx.node(root_off, 0, false)?;

    let mut animations = Vec::new();
    let mut ar = Reader::at(body, anim_off);
    for _ in 0..anim_count {
        let off = ar.u32()? as usize;
        animations.push(ctx.animation(off)?);
    }

    Ok(Model {
        name,
        classification,
        supermodel: (!supermodel.is_empty() && !supermodel.eq_ignore_ascii_case("null"))
            .then_some(supermodel),
        bbox,
        radius,
        anim_scale,
        node_count,
        root,
        animations,
    })
}

impl Ctx<'_> {
    fn animation(&self, off: usize) -> Result<Animation> {
        let mut r = Reader::at(self.d, off + 8);
        let name = r.fixed_str(32)?;
        let root_off = r.u32()? as usize;
        r.seek(off + 80);
        let length = r.f32()?;
        let transition = r.f32()?;
        let root_name = r.fixed_str(32)?;
        let ev_off = r.u32()? as usize;
        let ev_n = r.u32()? as usize;
        let mut events = Vec::new();
        let mut er = Reader::at(self.d, ev_off);
        for _ in 0..ev_n.min(10_000) {
            events.push(AnimEvent {
                time: er.f32()?,
                name: er.fixed_str(32)?,
            });
        }
        let root = self.node(root_off, 0, true)?;
        Ok(Animation {
            name,
            length,
            transition,
            root_name,
            events,
            root,
        })
    }

    fn node(&self, off: usize, depth: usize, anim: bool) -> Result<Node> {
        if depth > MAX_DEPTH {
            return Err(Error::format("model node tree too deep"));
        }
        let mut r = Reader::at(self.d, off);
        let flags = r.u16()?;
        let _supernode = r.u16()?;
        let name_idx = r.u16()? as usize;
        r.skip(2 + 4 + 4); // pad, root offset, parent offset
        let position = r.vec3()?;
        let orientation = [r.f32()?, r.f32()?, r.f32()?, r.f32()?];
        let child_off = r.u32()? as usize;
        let child_n = r.u32()? as usize;
        r.skip(4);
        let key_off = r.u32()? as usize;
        let key_n = r.u32()? as usize;
        r.skip(4);
        let data_off = r.u32()? as usize;
        let data_n = r.u32()? as usize;
        let name = self
            .names
            .get(name_idx)
            .cloned()
            .ok_or_else(|| Error::format(format!("node name index {name_idx} out of range")))?;

        let controllers = self.controllers(key_off, key_n, data_off, data_n)?;
        let mesh = if flags & NODE_MESH != 0 && !anim {
            Some(self.mesh(off, flags)?)
        } else {
            None
        };
        let (light, emitter) = if anim {
            (None, None)
        } else {
            (
                (flags & NODE_LIGHT != 0)
                    .then(|| self.light(off))
                    .transpose()?,
                (flags & NODE_EMITTER != 0)
                    .then(|| self.emitter(off))
                    .transpose()?,
            )
        };
        let mut children = Vec::with_capacity(child_n.min(1024));
        let mut cr = Reader::at(self.d, child_off);
        for _ in 0..child_n {
            children.push(self.node(cr.u32()? as usize, depth + 1, anim)?);
        }
        Ok(Node {
            name,
            flags,
            position,
            orientation,
            controllers,
            mesh,
            light,
            emitter,
            children,
        })
    }

    fn light(&self, node: usize) -> Result<Light> {
        let mut r = Reader::at(self.d, node + 80);
        let flare_radius = r.f32()?;
        r.skip(12);
        let array = |r: &mut Reader| -> Result<(usize, usize)> {
            let o = r.u32()? as usize;
            let n = r.u32()? as usize;
            r.skip(4);
            Ok((o, n.min(1024)))
        };
        let (sizes, positions, shifts, textures) = (
            array(&mut r)?,
            array(&mut r)?,
            array(&mut r)?,
            array(&mut r)?,
        );
        let priority = r.i32()?;
        let ambient_only = r.u32()? != 0;
        let dynamic_type = r.u32()?;
        let affect_dynamic = r.u32()? != 0;
        let shadow = r.u32()? != 0;
        let flare = r.u32()? != 0;
        let fading = r.u32()? != 0;
        let floats = |(o, n): (usize, usize)| -> Result<Vec<f32>> {
            let mut r = Reader::at(self.d, o);
            (0..n).map(|_| r.f32()).collect()
        };
        let mut light = Light {
            flare_radius,
            priority,
            ambient_only,
            dynamic_type,
            affect_dynamic,
            shadow,
            flare,
            fading,
            flare_sizes: floats(sizes)?,
            flare_positions: floats(positions)?,
            ..Light::default()
        };
        let mut sr = Reader::at(self.d, shifts.0);
        for _ in 0..shifts.1 {
            light.flare_color_shifts.push(sr.vec3()?);
        }
        let mut tr = Reader::at(self.d, textures.0);
        for _ in 0..textures.1 {
            let o = tr.u32()? as usize;
            let len = 64.min(self.d.len().saturating_sub(o));
            light
                .flare_textures
                .push(Reader::at(self.d, o).fixed_str(len)?);
        }
        Ok(light)
    }

    fn emitter(&self, node: usize) -> Result<Emitter> {
        let mut r = Reader::at(self.d, node + 80);
        let dead_space = r.f32()?;
        let blast_radius = r.f32()?;
        let blast_length = r.f32()?;
        let branch_count = r.u32()?;
        let control_point_smoothing = r.u32()?;
        let grid = [r.u32()?, r.u32()?];
        let spawn_type = r.u32()?;
        let update = r.fixed_str(32)?;
        let render = r.fixed_str(32)?;
        let blend = r.fixed_str(32)?;
        let texture = r.fixed_str(32)?;
        let chunk_name = r.fixed_str(32)?;
        let two_sided = r.u32()? != 0;
        let looping = r.u32()? != 0;
        let render_order = r.u16()?;
        let frame_blending = r.u8()? != 0;
        let depth_texture = r.fixed_str(16)?;
        Ok(Emitter {
            dead_space,
            blast_radius,
            blast_length,
            branch_count,
            control_point_smoothing,
            grid,
            spawn_type,
            update,
            render,
            blend,
            texture,
            chunk_name,
            two_sided,
            looping,
            render_order,
            frame_blending,
            depth_texture,
        })
    }

    fn controllers(
        &self,
        key_off: usize,
        key_n: usize,
        data_off: usize,
        data_n: usize,
    ) -> Result<Vec<Controller>> {
        if key_n == 0 {
            return Ok(Vec::new());
        }
        let mut dr = Reader::at(self.d, data_off);
        let mut data = Vec::with_capacity(data_n.min(1 << 20));
        for _ in 0..data_n {
            data.push(dr.f32()?);
        }
        let mut kr = Reader::at(self.d, key_off);
        let mut out = Vec::with_capacity(key_n.min(256));
        for _ in 0..key_n {
            let kind = kr.u32()?;
            let _unk = kr.u16()?;
            let rows = kr.u16()? as usize;
            let time_start = kr.u16()? as usize;
            let data_start = kr.u16()? as usize;
            let columns = kr.u8()?;
            kr.skip(3);
            let width = if kind == 20 && columns == 2 {
                1 // compressed quaternion: one packed word per key
            } else if columns & 0x10 != 0 {
                (columns & 0x0F) as usize * 3
            } else {
                columns as usize
            };
            let times = data
                .get(time_start..time_start + rows)
                .ok_or_else(|| Error::format("controller times out of range"))?
                .to_vec();
            if std::env::var_os("KOTOR_DEBUG").is_some() {
                eprintln!("ctrl kind {kind} rows {rows} t {time_start} d {data_start} cols {columns:#x} data_n {data_n}");
            }
            let values = data
                .get(data_start..data_start + rows * width)
                .ok_or_else(|| Error::format("controller values out of range"))?
                .to_vec();
            out.push(Controller {
                kind,
                columns,
                times,
                values,
            });
        }
        Ok(out)
    }

    fn mesh(&self, node: usize, flags: u16) -> Result<Mesh> {
        // Mesh header starts right after the 80-byte node header.
        let mut r = Reader::at(self.d, node + 88);
        let face_off = r.u32()? as usize;
        let face_n = r.u32()? as usize;
        r.seek(node + 152);
        let diffuse = r.vec3()?;
        let ambient = r.vec3()?;
        r.seek(node + 168);
        let tex = r.fixed_str(32)?;
        let lightmap = r.fixed_str(32)?;
        r.seek(node + 332);
        let stride = r.u32()? as usize;
        let _mdx_flags = r.u32()?;
        let pos_off = r.u32()?;
        let norm_off = r.u32()?;
        let _color_off = r.u32()?;
        let uv1_off = r.u32()?;
        let uv2_off = r.u32()?;
        r.seek(node + 384);
        let vert_n = r.u16()? as usize;
        let _channels = r.u16()?;
        r.seek(node + 388);
        let _lightmapped = r.u8()?;
        let _rotate = r.u8()?;
        let _bg = r.u8()?;
        let shadow = r.u8()? != 0;
        r.seek(node + 392);
        let _beaming = r.u8()?;
        let render = r.u8()? != 0;
        r.seek(node + 404);
        let mdx_start = r.u32()? as usize;

        let mut mesh = Mesh {
            texture: texture(tex),
            lightmap: texture(lightmap),
            diffuse,
            ambient,
            render,
            shadow,
            ..Mesh::default()
        };

        let mut fr = Reader::at(self.d, face_off);
        for _ in 0..face_n {
            let normal = fr.vec3()?;
            let _dist = fr.f32()?;
            let material = fr.u32()?;
            fr.skip(6); // adjacent faces
            let indices = [fr.u16()?, fr.u16()?, fr.u16()?];
            if indices.iter().any(|&i| i as usize >= vert_n) {
                return Err(Error::format("mesh face index out of range"));
            }
            mesh.faces.push(MeshFace {
                indices,
                material,
                normal,
            });
        }

        if vert_n > 0 && stride > 0 && mdx_start != NONE as usize {
            let need = mdx_start + vert_n * stride;
            if need > self.mdx.len() {
                return Err(Error::format(format!(
                    "mesh vertices run past MDX ({need} > {})",
                    self.mdx.len()
                )));
            }
            let row =
                |i: usize, off: u32| Reader::at(self.mdx, mdx_start + i * stride + off as usize);
            for i in 0..vert_n {
                if pos_off != NONE {
                    mesh.positions.push(row(i, pos_off).vec3()?);
                }
                if norm_off != NONE {
                    mesh.normals.push(row(i, norm_off).vec3()?);
                }
                if uv1_off != NONE {
                    let mut p = row(i, uv1_off);
                    mesh.uvs.push([p.f32()?, p.f32()?]);
                }
                if uv2_off != NONE {
                    let mut p = row(i, uv2_off);
                    mesh.uvs2.push([p.f32()?, p.f32()?]);
                }
            }
            if flags & NODE_SKIN != 0 {
                let mut sr = Reader::at(self.d, node + 424);
                let w_off = sr.u32()?;
                let b_off = sr.u32()?;
                let map_off = sr.u32()? as usize;
                let map_n = sr.u32()? as usize;
                let mut skin = Skin {
                    weights: Vec::new(),
                    bones: Vec::new(),
                    bone_map: Vec::new(),
                };
                for i in 0..vert_n {
                    let mut w = row(i, w_off);
                    skin.weights.push([w.f32()?, w.f32()?, w.f32()?, w.f32()?]);
                    let mut b = row(i, b_off);
                    skin.bones.push([b.f32()?, b.f32()?, b.f32()?, b.f32()?]);
                }
                let mut mr = Reader::at(self.d, map_off);
                for _ in 0..map_n {
                    skin.bone_map.push(mr.u32()?);
                }
                mesh.skin = Some(skin);
            }
        }
        Ok(mesh)
    }
}

fn texture(s: String) -> Option<String> {
    (!s.is_empty() && !s.eq_ignore_ascii_case("null")).then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_short_and_ascii() {
        assert!(parse_model(&[0; 20], &[]).is_err());
        let mut v = vec![0u8; 400];
        v[0] = b'#';
        assert!(parse_model(&v, &[]).is_err());
    }

    #[test]
    fn zeroed_body_does_not_panic() {
        let v = vec![0u8; 600];
        let _ = parse_model(&v, &[]);
    }
}
