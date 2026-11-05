//! Particles: the area models' own emitters, and blaster bolts, muzzle flashes and impact sparks. Everything here is CPU
//! work that ends in textured quads for the GPU.
//!
//! **Read from the models:** an emitter node's texture, grid, blend and update type, and its controllers (kind numbers
//! below). **Assumed:** a particle leaves along the node's +z inside a cone of `spread` radians; `Motion_Blur` stretches
//! a quad along its velocity; only `Fountain` emitters run by themselves. The game's other types (`Explosion`,
//! `Lightning`, `Single`) are not simulated.

use bytemuck::{Pod, Zeroable};
use kotor_archive::Install;
use kotor_core::ResType;
use kotor_model::{parse_model, Node};
use kotor_render::Camera;

type Mat = [[f32; 4]; 4];

const BIRTH_RATE: u32 = 88;
const RANDOM_BIRTH_RATE: u32 = 240;
const VELOCITY: u32 = 168;
const RANDOM_VELOCITY: u32 = 140;
const SPREAD: u32 = 160;
const LIFE: u32 = 120;
const GRAVITY: u32 = 116;
const MASS: u32 = 124;
const FPS: u32 = 104;
const FRAME_START: u32 = 112;
const FRAME_END: u32 = 108;
const COLOR_START: u32 = 392;
const COLOR_MID: u32 = 284;
const COLOR_END: u32 = 380;
const ALPHA_START: u32 = 84;
const ALPHA_MID: u32 = 216;
const ALPHA_END: u32 = 80;
const SIZE_START: u32 = 144;
const SIZE_MID: u32 = 232;
const SIZE_END: u32 = 148;
const PERCENT_MID: u32 = 224;

/// Emitters farther than this from the camera neither run nor draw (metres).
const RANGE: f32 = 40.0;
const MAX_PER_SOURCE: usize = 300;
const BOLT_SPEED: f32 = 45.0;
const BOLT_LENGTH: f32 = 1.4;
const BOLT_WIDTH: f32 = 0.14;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq)]
pub struct ParticleVertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

/// Quads sharing a texture and a way of blending.
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleBatch {
    pub texture: String,
    pub additive: bool,
    pub vertices: Vec<ParticleVertex>,
}

#[derive(Clone, Copy)]
struct Particle {
    pos: [f32; 3],
    vel: [f32; 3],
    age: f32,
    life: f32,
    frame: f32,
}

struct Source {
    at: Mat,
    texture: String,
    additive: bool,
    stretched: bool,
    grid: (u32, u32),
    rate: f32,
    random_rate: f32,
    speed: f32,
    random_speed: f32,
    spread: f32,
    life: f32,
    gravity: f32,
    fps: f32,
    frames: (f32, f32),
    colors: [[f32; 3]; 3],
    alphas: [f32; 3],
    sizes: [f32; 3],
    middle: f32,
    due: f32,
    particles: Vec<Particle>,
}

/// A bolt in flight.
struct Bolt {
    at: [f32; 3],
    to: [f32; 3],
    texture: String,
    color: [f32; 4],
}

/// A single flash that fades where it is.
struct Flash {
    at: [f32; 3],
    size: f32,
    age: f32,
    life: f32,
    texture: String,
}

#[derive(Default)]
pub struct Effects {
    sources: Vec<Source>,
    bolts: Vec<Bolt>,
    flashes: Vec<Flash>,
    sparks: Vec<Source>,
    seed: u32,
}

fn one(n: &Node, kind: u32, or: f32) -> f32 {
    n.controllers
        .iter()
        .find(|c| c.kind == kind)
        .and_then(|c| c.values.first().copied())
        .unwrap_or(or)
}

/// What a stock bolt model's first emitter draws: its texture and start colour.
pub fn bolt_look(model: &kotor_model::Model) -> Option<(String, [f32; 3])> {
    let mut found = None;
    model.root.walk(&mut |n| {
        if let Some(e) = &n.emitter {
            found.get_or_insert((e.texture.clone(), three(n, COLOR_START, [1.0; 3])));
        }
    });
    found
}

fn three(n: &Node, kind: u32, or: [f32; 3]) -> [f32; 3] {
    n.controllers
        .iter()
        .find(|c| c.kind == kind && c.values.len() >= 3)
        .map_or(or, |c| [c.values[0], c.values[1], c.values[2]])
}

fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

impl Effects {
    fn random(&mut self) -> f32 {
        let mut x = self.seed.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.seed = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Read the `Fountain` emitters of an area's room models. A model that cannot be read adds nothing.
    pub fn load_area(&mut self, inst: &mut Install, area: &str) {
        self.sources.clear();
        self.bolts.clear();
        self.flashes.clear();
        self.sparks.clear();
        let Some(lyt) = inst
            .read(area, ResType::LYT)
            .ok()
            .and_then(|b| kotor_walkmesh::parse_lyt(&b).ok())
        else {
            return;
        };
        for room in &lyt.rooms {
            let Some(model) = inst.read(&room.model, ResType::MDL).ok().and_then(|mdl| {
                let mdx = inst.read(&room.model, ResType::MDX).unwrap_or_default();
                parse_model(&mdl, &mdx).ok()
            }) else {
                continue;
            };
            let mats = model.world_matrices(None);
            let sources = &mut self.sources;
            model.root.walk(&mut |n| {
                let Some(e) = &n.emitter else { return };
                if !e.update.eq_ignore_ascii_case("fountain") || e.texture.is_empty() {
                    return;
                }
                let Some((_, m)) = mats.iter().find(|(name, _)| *name == n.name) else {
                    return;
                };
                let mut at = *m;
                for (row, v) in at.iter_mut().zip(room.position) {
                    row[3] += v;
                }
                sources.push(Source {
                    at,
                    texture: e.texture.to_ascii_lowercase(),
                    additive: e.blend.eq_ignore_ascii_case("lighten"),
                    stretched: e.render.eq_ignore_ascii_case("motion_blur"),
                    grid: (e.grid[0].max(1), e.grid[1].max(1)),
                    rate: one(n, BIRTH_RATE, 0.0),
                    random_rate: one(n, RANDOM_BIRTH_RATE, 0.0),
                    speed: one(n, VELOCITY, 0.0),
                    random_speed: one(n, RANDOM_VELOCITY, 0.0),
                    spread: one(n, SPREAD, 0.0),
                    life: one(n, LIFE, 1.0).max(0.05),
                    gravity: if one(n, MASS, 0.0) > 0.2 {
                        one(n, GRAVITY, 0.0).max(4.0)
                    } else {
                        one(n, GRAVITY, 0.0)
                    },
                    fps: one(n, FPS, 0.0),
                    frames: (one(n, FRAME_START, 0.0), one(n, FRAME_END, 0.0)),
                    colors: [
                        three(n, COLOR_START, [1.0; 3]),
                        three(n, COLOR_MID, [1.0; 3]),
                        three(n, COLOR_END, [1.0; 3]),
                    ],
                    alphas: [
                        one(n, ALPHA_START, 1.0),
                        one(n, ALPHA_MID, 1.0),
                        one(n, ALPHA_END, 0.0),
                    ],
                    sizes: [
                        one(n, SIZE_START, 0.2),
                        one(n, SIZE_MID, 0.2),
                        one(n, SIZE_END, 0.2),
                    ],
                    middle: one(n, PERCENT_MID, 0.5).clamp(0.01, 0.99),
                    due: 0.0,
                    particles: Vec::new(),
                });
            });
        }
    }

    /// How many emitters the area has.
    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    /// Where the area's emitters are, and their textures.
    pub fn source_places(&self) -> Vec<([f32; 3], String)> {
        self.sources
            .iter()
            .map(|s| ([s.at[0][3], s.at[1][3], s.at[2][3]], s.texture.clone()))
            .collect()
    }

    /// How many bolts are in flight.
    pub fn bolt_count(&self) -> usize {
        self.bolts.len()
    }

    /// A shot from `from` to `to`: a flash at the muzzle and a bolt on its way.
    pub fn fire(&mut self, from: [f32; 3], to: [f32; 3], bolt_texture: &str, color: [f32; 3]) {
        self.flashes.push(Flash {
            at: from,
            size: 0.45,
            age: 0.0,
            life: 0.07,
            texture: "fx_muzflash".to_string(),
        });
        self.bolts.push(Bolt {
            at: from,
            to,
            texture: bolt_texture.to_ascii_lowercase(),
            color: [color[0], color[1], color[2], 1.0],
        });
    }

    fn impact(&mut self, at: [f32; 3]) {
        self.flashes.push(Flash {
            at,
            size: 0.6,
            age: 0.0,
            life: 0.14,
            texture: "fx_flare01".to_string(),
        });
        let mut burst = Source {
            at: [
                [1.0, 0.0, 0.0, at[0]],
                [0.0, 1.0, 0.0, at[1]],
                [0.0, 0.0, 1.0, at[2]],
                [0.0, 0.0, 0.0, 1.0],
            ],
            texture: "fx_spark".to_string(),
            additive: true,
            stretched: true,
            grid: (2, 2),
            rate: 0.0,
            random_rate: 0.0,
            speed: 3.0,
            random_speed: 2.0,
            spread: 3.0,
            life: 0.35,
            gravity: 6.0,
            fps: 0.0,
            frames: (0.0, 0.0),
            colors: [[1.0, 0.85, 0.6]; 3],
            alphas: [1.0, 1.0, 0.0],
            sizes: [0.05, 0.04, 0.02],
            middle: 0.5,
            due: 0.0,
            particles: Vec::new(),
        };
        for _ in 0..10 {
            let p = self.spawn(&burst);
            burst.particles.push(p);
        }
        self.sparks.push(burst);
    }

    fn spawn(&mut self, s: &Source) -> Particle {
        let turn = self.random() * std::f32::consts::TAU;
        let tilt = (self.random() - 0.5) * s.spread;
        let local = [tilt.sin() * turn.cos(), tilt.sin() * turn.sin(), tilt.cos()];
        let d = [0, 1, 2].map(|r| (0..3).map(|c| s.at[r][c] * local[c]).sum::<f32>());
        let speed = s.speed + self.random() * s.random_speed;
        Particle {
            pos: [s.at[0][3], s.at[1][3], s.at[2][3]],
            vel: d.map(|v| v * speed),
            age: 0.0,
            life: s.life * (0.8 + 0.4 * self.random()),
            frame: self.random(),
        }
    }

    /// Advance everything by `dt` seconds; emitters only run near the camera.
    pub fn update(&mut self, dt: f32, eye: [f32; 3]) {
        let mut sources = std::mem::take(&mut self.sources);
        for s in &mut sources {
            let (dx, dy) = (s.at[0][3] - eye[0], s.at[1][3] - eye[1]);
            if dx.hypot(dy) > RANGE {
                s.particles.clear();
                continue;
            }
            s.due += (s.rate + self.random() * s.random_rate) * dt;
            while s.due >= 1.0 {
                s.due -= 1.0;
                if s.particles.len() < MAX_PER_SOURCE {
                    let p = self.spawn(s);
                    s.particles.push(p);
                }
            }
            advance(s, dt);
        }
        self.sources = sources;
        for s in &mut self.sparks {
            advance(s, dt);
        }
        self.sparks.retain(|s| !s.particles.is_empty());
        for f in &mut self.flashes {
            f.age += dt;
        }
        self.flashes.retain(|f| f.age < f.life);
        let mut hits = Vec::new();
        self.bolts.retain_mut(|b| {
            let d = [b.to[0] - b.at[0], b.to[1] - b.at[1], b.to[2] - b.at[2]];
            let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            let go = BOLT_SPEED * dt;
            if len <= go {
                hits.push(b.to);
                return false;
            }
            for (k, v) in d.iter().enumerate() {
                b.at[k] += v / len * go;
            }
            true
        });
        for h in hits {
            self.impact(h);
        }
    }

    /// The quads for this frame, facing the camera.
    pub fn batches(&self, cam: &Camera) -> Vec<ParticleBatch> {
        let mut out: Vec<ParticleBatch> = Vec::new();
        let (right, up) = camera_axes(cam);
        for s in self.sources.iter().chain(&self.sparks) {
            for p in &s.particles {
                let t = (p.age / p.life).clamp(0.0, 1.0);
                let curve = |a: f32, b: f32, c: f32| {
                    if t < s.middle {
                        mix(a, b, t / s.middle)
                    } else {
                        mix(b, c, (t - s.middle) / (1.0 - s.middle))
                    }
                };
                let color =
                    [0, 1, 2].map(|k| curve(s.colors[0][k], s.colors[1][k], s.colors[2][k]));
                let alpha = curve(s.alphas[0], s.alphas[1], s.alphas[2]).clamp(0.0, 1.0);
                let size = curve(s.sizes[0], s.sizes[1], s.sizes[2]).max(0.001);
                let rgba = [color[0], color[1], color[2], alpha];
                let cells = s.grid.0 * s.grid.1;
                let span = (s.frames.1 - s.frames.0).max(0.0);
                let cell = if s.fps > 0.0 {
                    (s.frames.0 + (p.age * s.fps) % (span + 1.0)) as u32
                } else {
                    (p.frame * cells as f32) as u32
                } % cells;
                let uv = cell_uv(cell, s.grid);
                let (a, b) = if s.stretched {
                    axial_quad(p.pos, p.vel, size * 3.0, size, cam)
                } else {
                    (scaled(right, size), scaled(up, size))
                };
                push_quad(&mut out, &s.texture, s.additive, p.pos, a, b, rgba, uv);
            }
        }
        for f in &self.flashes {
            let fade = 1.0 - f.age / f.life;
            let (a, b) = (scaled(right, f.size), scaled(up, f.size));
            let whole = [0.0, 0.0, 1.0, 1.0];
            push_quad(
                &mut out,
                &f.texture,
                true,
                f.at,
                a,
                b,
                [1.0, 1.0, 1.0, fade],
                whole,
            );
        }
        for b in &self.bolts {
            let d = [b.to[0] - b.at[0], b.to[1] - b.at[1], b.to[2] - b.at[2]];
            let (a, c) = axial_quad(b.at, d, BOLT_LENGTH, BOLT_WIDTH, cam);
            let whole = [0.0, 0.0, 1.0, 1.0];
            push_quad(&mut out, &b.texture, true, b.at, a, c, b.color, whole);
        }
        out
    }
}

fn advance(s: &mut Source, dt: f32) {
    for p in &mut s.particles {
        p.age += dt;
        p.vel[2] -= s.gravity * dt;
        for k in 0..3 {
            p.pos[k] += p.vel[k] * dt;
        }
    }
    s.particles.retain(|p| p.age < p.life);
}

fn scaled(v: [f32; 3], k: f32) -> [f32; 3] {
    v.map(|c| c * k / 2.0)
}

fn normal(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
    v.map(|c| c / l)
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn camera_axes(cam: &Camera) -> ([f32; 3], [f32; 3]) {
    let f = normal([0, 1, 2].map(|k| cam.target[k] - cam.eye[k]));
    let right = normal(cross(f, [0.0, 0.0, 1.0]));
    (right, cross(right, f))
}

/// Half-extents of a quad of this length along `along` and this width across it, turned to the camera.
fn axial_quad(
    at: [f32; 3],
    along: [f32; 3],
    length: f32,
    width: f32,
    cam: &Camera,
) -> ([f32; 3], [f32; 3]) {
    let a = normal(along);
    let to_eye = [0, 1, 2].map(|k| cam.eye[k] - at[k]);
    let side = normal(cross(a, to_eye));
    (scaled(side, width), scaled(a, length))
}

fn cell_uv(cell: u32, grid: (u32, u32)) -> [f32; 4] {
    let (w, h) = (1.0 / grid.0 as f32, 1.0 / grid.1 as f32);
    let (x, y) = (cell % grid.0, cell / grid.0);
    [x as f32 * w, y as f32 * h, w, h]
}

#[allow(clippy::too_many_arguments)]
fn push_quad(
    out: &mut Vec<ParticleBatch>,
    texture: &str,
    additive: bool,
    at: [f32; 3],
    a: [f32; 3],
    b: [f32; 3],
    color: [f32; 4],
    uv: [f32; 4],
) {
    let i = match out
        .iter()
        .position(|x| x.additive == additive && x.texture == texture)
    {
        Some(i) => i,
        None => {
            out.push(ParticleBatch {
                texture: texture.to_string(),
                additive,
                vertices: Vec::new(),
            });
            out.len() - 1
        }
    };
    let corner = |sa: f32, sb: f32, u: f32, v: f32| ParticleVertex {
        pos: [0, 1, 2].map(|k| at[k] + a[k] * sa + b[k] * sb),
        uv: [uv[0] + u * uv[2], uv[1] + v * uv[3]],
        color,
    };
    let (p, q, r, s) = (
        corner(-1.0, -1.0, 0.0, 1.0),
        corner(1.0, -1.0, 1.0, 1.0),
        corner(1.0, 1.0, 1.0, 0.0),
        corner(-1.0, 1.0, 0.0, 0.0),
    );
    out[i].vertices.extend([p, q, r, p, r, s]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn camera() -> Camera {
        Camera {
            eye: [0.0, -5.0, 1.0],
            target: [0.0, 0.0, 1.0],
            fov: 55.0,
        }
    }

    #[test]
    fn a_bolt_flies_then_bursts_where_it_was_aimed() {
        let mut e = Effects::default();
        e.fire(
            [0.0, 0.0, 1.0],
            [9.0, 0.0, 1.0],
            "fx_laser_01",
            [1.0, 0.2, 0.2],
        );
        assert_eq!(e.bolt_count(), 1);
        assert!(e
            .batches(&camera())
            .iter()
            .any(|b| b.texture == "fx_laser_01"));
        e.update(0.1, [0.0; 3]);
        assert_eq!(e.bolt_count(), 1, "4.5 m out of 9");
        e.update(0.2, [0.0; 3]);
        assert_eq!(e.bolt_count(), 0, "arrived");
        let b = e.batches(&camera());
        assert!(b.iter().any(|x| x.texture == "fx_spark"), "{b:?}");
        assert!(b.iter().any(|x| x.texture == "fx_flare01"));
        for _ in 0..30 {
            e.update(0.1, [0.0; 3]);
        }
        assert!(e.batches(&camera()).is_empty(), "everything fades out");
    }

    #[test]
    fn grid_cells_cover_their_part_of_the_picture() {
        assert_eq!(cell_uv(3, (2, 2)), [0.5, 0.5, 0.5, 0.5]);
        assert_eq!(cell_uv(0, (4, 4)), [0.0, 0.0, 0.25, 0.25]);
    }
}
