use kotor_core::{Error, Reader, Result};

/// Surface material ids with `walk = 1` in `surfacemat.2da` (ids 1,3-6,9-14,18,30).
/// Verified on all 1202 area walkmeshes: faces of these materials are exactly
/// the faces that carry adjacency records, and they come first in the face list.
pub const WALKABLE_SURFACES: &[u32] = &[1, 3, 4, 5, 6, 9, 10, 11, 12, 13, 14, 18, 30];

#[derive(Clone, Debug, PartialEq)]
pub struct Face {
    pub vertices: [u32; 3],
    pub material: u32,
    pub normal: [f32; 3],
    pub plane_distance: f32,
    /// Neighbour per edge as `face * 3 + edge`, or -1. Only walkable faces have this.
    pub adjacent: Option<[i32; 3]>,
}

impl Face {
    pub fn walkable(&self) -> bool {
        WALKABLE_SURFACES.contains(&self.material)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Aabb {
    pub min: [f32; 3],
    pub max: [f32; 3],
    /// Face index for leaves, -1 for inner nodes.
    pub face: i32,
    pub plane: u32,
    pub left: u32,
    pub right: u32,
}

#[derive(Clone, Debug)]
pub struct Bwm {
    /// 0 = placeable/door walkmesh, 1 = area walkmesh.
    pub kind: u32,
    pub position: [f32; 3],
    /// Door/placeable use positions (relative then absolute), 2 each.
    pub use_positions: [[f32; 3]; 4],
    pub vertices: Vec<[f32; 3]>,
    pub faces: Vec<Face>,
    pub aabbs: Vec<Aabb>,
    /// `(edge index, transition)` pairs on the outer border.
    pub edges: Vec<(i32, i32)>,
    pub perimeters: Vec<u32>,
    /// True if the stored AABB boxes did not match their triangles (by more than 1 mm) and were recomputed on load.
    pub tree_refit: bool,
}

fn count_bytes(n: u32, each: usize) -> Result<usize> {
    (n as usize)
        .checked_mul(each)
        .filter(|&b| b < (1 << 30))
        .ok_or_else(|| Error::format("bwm table too large"))
}

pub fn parse_bwm(data: &[u8]) -> Result<Bwm> {
    let mut r = Reader::new(data);
    let tag = r.tag()?;
    let ver = r.tag()?;
    if &tag != b"BWM " || &ver != b"V1.0" {
        return Err(Error::format(format!(
            "not a BWM V1.0 file: {tag:?} {ver:?}"
        )));
    }
    let kind = r.u32()?;
    let mut use_positions = [[0.0; 3]; 4];
    for p in &mut use_positions {
        *p = r.vec3()?;
    }
    let position = r.vec3()?;
    let vert_n = r.u32()?;
    let vert_off = r.u32()? as usize;
    let face_n = r.u32()?;
    let idx_off = r.u32()? as usize;
    let mat_off = r.u32()? as usize;
    let norm_off = r.u32()? as usize;
    let dist_off = r.u32()? as usize;
    let aabb_n = r.u32()?;
    let aabb_off = r.u32()? as usize;
    let _unknown = r.u32()?;
    let adj_n = r.u32()?;
    let adj_off = r.u32()? as usize;
    let edge_n = r.u32()?;
    let edge_off = r.u32()? as usize;
    let perim_n = r.u32()?;
    let perim_off = r.u32()? as usize;

    count_bytes(vert_n, 12)?;
    let mut vr = Reader::at(data, vert_off);
    let mut vertices = Vec::with_capacity(vert_n as usize);
    for _ in 0..vert_n {
        vertices.push(vr.vec3()?);
    }

    count_bytes(face_n, 12)?;
    let mut ir = Reader::at(data, idx_off);
    let mut mr = Reader::at(data, mat_off);
    let mut nr = Reader::at(data, norm_off);
    let mut dr = Reader::at(data, dist_off);
    let mut faces = Vec::with_capacity(face_n as usize);
    for i in 0..face_n {
        let v = [ir.u32()?, ir.u32()?, ir.u32()?];
        if v.iter().any(|&x| x >= vert_n) {
            return Err(Error::format(format!(
                "face {i} uses a vertex out of range"
            )));
        }
        faces.push(Face {
            vertices: v,
            material: mr.u32()?,
            normal: nr.vec3()?,
            plane_distance: dr.f32()?,
            adjacent: None,
        });
    }

    // Adjacency records belong to the walkable faces, in face order.
    count_bytes(adj_n, 12)?;
    let mut ar = Reader::at(data, adj_off);
    let walkable: Vec<usize> = (0..faces.len()).filter(|&i| faces[i].walkable()).collect();
    // Door and placeable walkmeshes carry no adjacency table at all.
    if adj_n != 0 && walkable.len() != adj_n as usize {
        return Err(Error::format(format!(
            "{} walkable faces but {} adjacency records",
            walkable.len(),
            adj_n
        )));
    }
    for &fi in walkable.iter().take(adj_n as usize) {
        faces[fi].adjacent = Some([ar.i32()?, ar.i32()?, ar.i32()?]);
    }

    count_bytes(aabb_n, 44)?;
    let mut br = Reader::at(data, aabb_off);
    let mut aabbs = Vec::with_capacity(aabb_n as usize);
    for _ in 0..aabb_n {
        let min = br.vec3()?;
        let max = br.vec3()?;
        let face = br.i32()?;
        let _unk = br.u32()?;
        let plane = br.u32()?;
        let left = br.u32()?;
        let right = br.u32()?;
        aabbs.push(Aabb {
            min,
            max,
            face,
            plane,
            left,
            right,
        });
    }

    count_bytes(edge_n, 8)?;
    let mut er = Reader::at(data, edge_off);
    let mut edges = Vec::with_capacity(edge_n as usize);
    for _ in 0..edge_n {
        edges.push((er.i32()?, er.i32()?));
    }
    count_bytes(perim_n, 4)?;
    let mut pr = Reader::at(data, perim_off);
    let mut perimeters = Vec::with_capacity(perim_n as usize);
    for _ in 0..perim_n {
        perimeters.push(pr.u32()?);
    }

    let mut bwm = Bwm {
        kind,
        position,
        use_positions,
        vertices,
        faces,
        aabbs,
        edges,
        perimeters,
        tree_refit: false,
    };
    bwm.tree_refit = bwm.refit_tree();
    Ok(bwm)
}

impl Bwm {
    /// Height of the walkable surface under `(x, y)`, searching all walkable faces.
    /// Linear scan; the AABB tree is for the engine's real queries later.
    pub fn height_at(&self, x: f32, y: f32) -> Option<f32> {
        self.faces
            .iter()
            .filter(|f| f.walkable())
            .filter_map(|f| {
                let [a, b, c] = f.vertices.map(|i| self.vertices[i as usize]);
                let d = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
                if d.abs() < 1e-9 {
                    return None;
                }
                let l1 = ((b[1] - c[1]) * (x - c[0]) + (c[0] - b[0]) * (y - c[1])) / d;
                let l2 = ((c[1] - a[1]) * (x - c[0]) + (a[0] - c[0]) * (y - c[1])) / d;
                let l3 = 1.0 - l1 - l2;
                let eps = -1e-4;
                (l1 >= eps && l2 >= eps && l3 >= eps).then(|| l1 * a[2] + l2 * b[2] + l3 * c[2])
            })
            .reduce(f32::max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_garbage() {
        assert!(parse_bwm(&[0; 200]).is_err());
        assert!(parse_bwm(b"BWM V1.0").is_err());
    }
}
