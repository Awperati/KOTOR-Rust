//! Walking a character along a path on the walkmesh, with a follow camera.

use crate::scene::Scene;
use crate::soft::{render, Camera};
use crate::Rig;
use kotor_archive::Install;
use kotor_core::{Error, ResType, Result};
use kotor_image::Image;
use kotor_walkmesh::{parse_bwm, parse_lyt, Bwm};

/// The walkable floor of an area: the walkmesh of every room that has one.
pub struct Floor {
    meshes: Vec<Bwm>,
}

impl Floor {
    pub fn load(inst: &mut Install, area: &str) -> Result<Floor> {
        let lyt = parse_lyt(&inst.read(area, ResType::LYT)?)?;
        let meshes: Vec<Bwm> = lyt
            .rooms
            .iter()
            .filter_map(|r| parse_bwm(&inst.read(&r.model, ResType::WOK).ok()?).ok())
            .collect();
        if meshes.is_empty() {
            return Err(Error::format(format!("area {area} has no room walkmeshes")));
        }
        Ok(Floor { meshes })
    }

    /// The walkable faces as a navigation mesh (doors count as open).
    pub fn navmesh(&self) -> kotor_walkmesh::NavMesh {
        kotor_walkmesh::NavMesh::from_meshes(&self.meshes)
    }

    /// How far a ray goes before it meets any walkmesh face, walls included.
    pub fn ray_distance(&self, origin: [f32; 3], dir: [f32; 3], max: f32) -> Option<f32> {
        self.meshes
            .iter()
            .filter_map(|m| m.raycast(origin, dir, max, false).map(|h| h.t))
            .reduce(f32::min)
    }

    /// The top of the walls around `(x, y)`, within `radius` metres: where the ceiling is, or would be. Only upright
    /// faces that are not floor count, and only those above `from_z`. `None` where no wall stands near.
    pub fn wall_top_near(&self, x: f32, y: f32, from_z: f32, radius: f32) -> Option<f32> {
        let mut top: Option<f32> = None;
        for m in &self.meshes {
            for f in m
                .faces
                .iter()
                .filter(|f| !f.walkable() && f.normal[2].abs() < 0.7)
            {
                for &i in &f.vertices {
                    let v = m.vertices[i as usize];
                    if (v[0] - x).hypot(v[1] - y) <= radius && v[2] > from_z {
                        top = Some(top.map_or(v[2], |t| t.max(v[2])));
                    }
                }
            }
        }
        top
    }

    /// Highest walkable floor under `(x, y)` that is at or below `max_z`.
    pub fn height(&self, x: f32, y: f32, max_z: f32) -> Option<f32> {
        self.meshes
            .iter()
            .filter_map(|m| m.floor_at(x, y, max_z).map(|(_, z)| z))
            .reduce(f32::max)
    }
}

/// One position along a walked path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    pub pos: [f32; 3],
    /// Direction of travel in the XY plane, radians (0 = +x).
    pub heading: f32,
    /// Distance walked from the start, metres.
    pub distance: f32,
}

/// Walk straight segments between `waypoints` in steps of `spacing` metres, standing on the floor at each
/// step (a step may climb or drop at most 0.4 m). Fails if any step has no floor under it, which means
/// the straight line leaves the walkable area.
pub fn walk_path(
    floor: &Floor,
    waypoints: &[[f32; 2]],
    start_z: f32,
    spacing: f32,
) -> Result<Vec<Step>> {
    if waypoints.len() < 2 || spacing <= 0.0 {
        return Err(Error::format(
            "a path needs two waypoints and a positive spacing",
        ));
    }
    let mut steps = Vec::new();
    let (mut z, mut distance) = (start_z, 0.0f32);
    for pair in waypoints.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-6 {
            continue;
        }
        let heading = dy.atan2(dx);
        let n = (len / spacing).ceil() as usize;
        for i in 0..=n {
            if i == 0 && !steps.is_empty() {
                continue;
            }
            let t = (i as f32 / n as f32).min(1.0);
            let (x, y) = (a[0] + dx * t, a[1] + dy * t);
            z = floor.height(x, y, z + 0.4).ok_or_else(|| {
                Error::format(format!("no floor at ({x:.2}, {y:.2}) on the path"))
            })?;
            steps.push(Step {
                pos: [x, y, z],
                heading,
                distance: distance + len * t,
            });
        }
        distance += len;
    }
    Ok(steps)
}

/// Settings for [`render_walk`].
pub struct WalkOptions<'a> {
    pub frames: usize,
    pub width: u32,
    pub height: u32,
    /// Animation name in the character's supermodel, for example `walk`.
    pub animation: &'a str,
    /// Metres covered by one cycle of the animation (`walkdist` / `rundist` in `appearance.2da`).
    pub metres_per_cycle: f32,
}

/// Render `frames` pictures of `rig` walking along `steps` through `area_scene`, spaced evenly by distance,
/// with a camera 3.3 m behind and 1.7 m above, looking at the chest. The animation time follows the
/// distance walked.
pub fn render_walk(
    area_scene: &Scene,
    rig: &Rig,
    steps: &[Step],
    texture: &mut dyn FnMut(&str) -> Option<Image>,
    opts: &WalkOptions,
) -> Result<Vec<Image>> {
    let (Some(first), Some(last)) = (steps.first(), steps.last()) else {
        return Err(Error::format("empty path"));
    };
    let cycle = rig.animation_length(opts.animation).filter(|l| *l > 0.0);
    let total = last.distance - first.distance;
    let mut out = Vec::with_capacity(opts.frames);
    for f in 0..opts.frames {
        let d = if opts.frames > 1 {
            total * f as f32 / (opts.frames - 1) as f32
        } else {
            0.0
        };
        let step = steps
            .iter()
            .min_by(|a, b| {
                (a.distance - first.distance - d)
                    .abs()
                    .partial_cmp(&(b.distance - first.distance - d).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .copied()
            .unwrap_or(*first);
        let t = cycle.map_or(0.0, |len| (d / opts.metres_per_cycle.max(0.1) * len) % len);
        let who = rig.pose(cycle.map(|_| (opts.animation, t)))?;
        // Models face +y at rest; turn so +y points along the heading.
        let (s, c) = (step.heading - std::f32::consts::FRAC_PI_2).sin_cos();
        let xf = [
            [c, -s, 0.0, step.pos[0]],
            [s, c, 0.0, step.pos[1]],
            [0.0, 0.0, 1.0, step.pos[2]],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let mut frame = area_scene.clone();
        frame.append_transformed(&who, &xf);
        let dir = [step.heading.cos(), step.heading.sin()];
        let cam = Camera {
            eye: [
                step.pos[0] - dir[0] * 3.3,
                step.pos[1] - dir[1] * 3.3,
                step.pos[2] + 1.7,
            ],
            target: [step.pos[0], step.pos[1], step.pos[2] + 1.2],
            fov: 60.0,
        };
        out.push(render(&frame, texture, &cam, opts.width, opts.height));
    }
    Ok(out)
}

/// Tile equally sized frames into a grid, row by row, `cols` per row (a final short row is left dark).
pub fn contact_sheet(frames: &[Image], cols: usize) -> Option<Image> {
    let first = frames.first()?;
    let (w, h) = (first.width as usize, first.height as usize);
    if cols == 0
        || frames
            .iter()
            .any(|f| f.width as usize != w || f.height as usize != h)
    {
        return None;
    }
    let rows = frames.len().div_ceil(cols);
    let (sw, sh) = (w * cols, h * rows);
    let mut rgba = vec![0u8; sw * sh * 4];
    for px in rgba.chunks_mut(4) {
        px[3] = 255;
    }
    for (i, f) in frames.iter().enumerate() {
        let (ox, oy) = ((i % cols) * w, (i / cols) * h);
        for y in 0..h {
            let dst = ((oy + y) * sw + ox) * 4;
            rgba[dst..dst + w * 4].copy_from_slice(&f.rgba[y * w * 4..(y + 1) * w * 4]);
        }
    }
    Some(Image {
        width: sw as u32,
        height: sh as u32,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(v: u8) -> Image {
        Image {
            width: 2,
            height: 1,
            rgba: vec![v, v, v, 255, v, v, v, 255],
        }
    }

    #[test]
    fn contact_sheet_tiles_row_by_row() {
        let s = contact_sheet(&[img(10), img(20), img(30)], 2).unwrap();
        assert_eq!((s.width, s.height), (4, 2));
        assert_eq!(s.rgba[0], 10);
        assert_eq!(s.rgba[8], 20);
        assert_eq!(s.rgba[16], 30); // second row, first tile
        assert_eq!(s.rgba[24], 0); // empty last tile
    }

    #[test]
    fn contact_sheet_rejects_mixed_sizes() {
        let other = Image {
            width: 1,
            height: 1,
            rgba: vec![0, 0, 0, 255],
        };
        assert!(contact_sheet(&[img(1), other], 2).is_none());
        assert!(contact_sheet(&[], 2).is_none());
    }
}
