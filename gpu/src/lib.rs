//! GPU renderer for kotor-render scenes (wgpu), rendering offscreen so no window is needed.
//!
//! Checked against the software renderer by `examples/compare.rs` (same camera, same scene). `Presenter` shows
//! the picture on a window surface for `examples/viewer.rs`.

/// Which build this is, for Settings. CI names the commit and the day; a build on a developer's machine has neither.
pub fn build_line() -> String {
    match (
        option_env!("KOTOR_BUILD_SHA"),
        option_env!("KOTOR_BUILD_DATE"),
    ) {
        (Some(sha), Some(date)) => format!("Build {sha}, {date}"),
        _ => "Build: local".to_string(),
    }
}

pub mod effects;
pub mod stage;
pub mod texture_override;
mod watch;
pub use watch::{watch, Watch};

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use kotor_image::Image;
use kotor_render::{Camera, Scene};
use std::collections::HashMap;
use wgpu::util::DeviceExt;

const SHADER: &str = r#"
struct Globals {
    view_proj: mat4x4<f32>,
    light: vec4<f32>,
    // A doorway seen into another area: five planes whose inside is the view through it, and mode.x = 1 to cut that
    // view out of what is drawn, 2 to draw nothing but that view, 0 for everything.
    planes: array<vec4<f32>, 5>,
    // x: the doorway mode above. y: cel shading, 0 off, 1 subtle, 2 strong. z: metres across one pixel a metre away,
    // and w: the outline's width in pixels, for the outline pass.
    mode: vec4<f32>,
};
@group(0) @binding(0) var<uniform> g: Globals;
@group(1) @binding(0) var diffuse_tex: texture_2d<f32>;
@group(1) @binding(1) var lightmap_tex: texture_2d<f32>;
@group(1) @binding(2) var samp: sampler;
struct MeshParams {
    a: vec4<f32>, // x = 1 with a lightmap, y = alpha below this is cut, z = 1 to blend, w = shade or 0
    fade: vec4<f32>, // x = how far faded out, 0 to 1
};
@group(1) @binding(3) var<uniform> params: MeshParams;

// A body fading out, pixel by pixel in a fixed 4 by 4 order: it needs no sorting, and it keeps its own depth, so no limb
// shows through another as it would when blended.
fn faded_out(pos: vec4<f32>) -> bool {
    if (params.fade.x <= 0.0) {
        return false;
    }
    let x = u32(pos.x) % 4u;
    let y = u32(pos.y) % 4u;
    let n = ((x ^ y) & 1u) * 8u + (y & 1u) * 4u + (((x ^ y) >> 1u) & 1u) * 2u + ((y >> 1u) & 1u);
    return (f32(n) + 0.5) / 16.0 < params.fade.x;
}

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) uv2: vec2<f32>,
};
struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) uv2: vec2<f32>,
    @location(3) world: vec3<f32>,
};

@vertex
fn vs_main(v: VsIn) -> VsOut {
    var o: VsOut;
    o.clip = g.view_proj * vec4<f32>(v.pos, 1.0);
    o.normal = v.normal;
    o.world = v.pos;
    // The game's v axis points up; image rows start at the top.
    o.uv = vec2<f32>(v.uv.x, 1.0 - v.uv.y);
    o.uv2 = vec2<f32>(v.uv2.x, 1.0 - v.uv2.y);
    return o;
}

// Light cut into steps with a soft riser, so that it stays subtle: `x` runs from `lo` to 1. Subtle is four steps and
// most of the way there, strong is three and all the way.
fn cel_steps(x: f32, lo: f32) -> f32 {
    let strong = g.mode.y > 1.5;
    let n = select(4.0, 3.0, strong);
    let soft = select(0.3, 0.08, strong);
    let amount = select(0.7, 1.0, strong);
    let u = clamp((x - lo) / (1.0 - lo), 0.0, 1.0) * n;
    let f = min(floor(u), n - 1.0);
    let r = smoothstep(0.5 - soft, 0.5 + soft, u - f);
    let q = lo + (f + r) / n * (1.0 - lo);
    return mix(x, select(q, x, x > 1.0), amount);
}

@vertex
fn vs_outline(v: VsIn) -> VsOut {
    var o: VsOut;
    let c = g.view_proj * vec4<f32>(v.pos, 1.0);
    // As wide in pixels at any distance, a little thinner far off.
    let px = g.mode.w * clamp(14.0 / max(c.w, 0.1), 0.5, 1.0);
    let wide = px * g.mode.z * c.w;
    let n = normalize(v.normal + vec3<f32>(0.0, 0.0, 1e-4));
    o.clip = g.view_proj * vec4<f32>(v.pos + n * wide, 1.0);
    o.normal = v.normal;
    o.world = v.pos;
    o.uv = vec2<f32>(v.uv.x, 1.0 - v.uv.y);
    o.uv2 = vec2<f32>(v.uv2.x, 1.0 - v.uv2.y);
    return o;
}

@fragment
fn fs_outline(i: VsOut) -> @location(0) vec4<f32> {
    // The same cut as the body's own: a figure seen through a doorway has its outline only inside the doorway's view.
    if (g.mode.x > 0.5) {
        var inside = true;
        for (var k = 0; k < 5; k = k + 1) {
            if (dot(g.planes[k].xyz, i.world) + g.planes[k].w < 0.0) {
                inside = false;
            }
        }
        if (inside == (g.mode.x < 1.5)) {
            discard;
        }
    }
    if (faded_out(i.clip)) {
        discard;
    }
    let tex = textureSample(diffuse_tex, samp, i.uv);
    if (tex.a < max(params.a.y, 0.02)) {
        discard;
    }
    return vec4<f32>(0.02, 0.015, 0.03, 1.0);
}

@fragment
fn fs_main(i: VsOut) -> @location(0) vec4<f32> {
    if (g.mode.x > 0.5) {
        var inside = true;
        for (var k = 0; k < 5; k = k + 1) {
            if (dot(g.planes[k].xyz, i.world) + g.planes[k].w < 0.0) {
                inside = false;
            }
        }
        if (inside == (g.mode.x < 1.5)) {
            discard;
        }
    }
    if (faded_out(i.clip)) {
        discard;
    }
    let tex = textureSample(diffuse_tex, samp, i.uv);
    if (tex.a < params.a.y) {
        discard;
    }
    let base = tex.rgb;
    var lit = vec3<f32>(0.55 + 0.45 * abs(dot(normalize(i.normal + vec3<f32>(0.0, 0.0, 1e-4)), g.light.xyz)));
    if (params.a.x > 0.5) {
        lit = textureSample(lightmap_tex, samp, i.uv2).rgb;
    }
    if (params.a.w > 0.0) {
        lit = lit * params.a.w;
    }
    if (g.mode.y > 0.5) {
        // The step is cut from the light's brightness; the texture keeps all its detail. A body lit by the one lamp
        // runs from 0.55 to 1, so its steps divide that, and a level's lightmap runs from 0.
        let lum = max(dot(lit, vec3<f32>(0.299, 0.587, 0.114)), 1e-3);
        let lo = select(0.55, 0.0, params.a.x > 0.5);
        lit = lit * (cel_steps(lum, lo) / lum);
    }
    return vec4<f32>(base * lit, mix(1.0, tex.a, params.a.z));
}
"#;

const PARTICLE_SHADER: &str = r#"
struct Globals {
    view_proj: mat4x4<f32>,
    light: vec4<f32>,
};
@group(0) @binding(0) var<uniform> g: Globals;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;
struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};
struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};
@vertex
fn vs(v: VsIn) -> VsOut {
    var o: VsOut;
    o.clip = g.view_proj * vec4<f32>(v.pos, 1.0);
    o.uv = v.uv;
    o.color = v.color;
    return o;
}
@fragment
fn fs(i: VsOut) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, i.uv) * i.color;
}
"#;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    pos: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
    uv2: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    light: [f32; 4],
    planes: [[f32; 4]; 5],
    mode: [f32; 4],
}

/// A doorway through which another area is seen, in the world space: where it is, the way through it, half its width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aperture {
    pub at: [f32; 3],
    pub out: f32,
    pub half: f32,
}

/// How high a doorway is drawn, in metres.
const APERTURE_HEIGHT: f32 = 3.6;
/// How far past a doorway the player can still be on the way through it, in metres. The world takes them over at 0.4 m.
const APERTURE_WAY_THROUGH: f32 = 1.0;
/// How far beside the opening still counts as going through it, in metres, as the crossing has it.
const APERTURE_EDGE: f32 = 0.5;

/// A lift ride's picture: the area the party leaves slides one way and the one it arrives in comes in from the other, the
/// camera held. The doorway's portal holds the arrival area; this cuts both at one height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LiftView {
    /// How far each picture has slid up, in metres: the one left (the world's), and the one arriving (the portal's).
    pub left_up: f32,
    pub arriving_up: f32,
    /// The height, in the camera's own frame, below which the arriving area is drawn and above which the left one is.
    pub boundary: f32,
    /// The arriving area is above the boundary, not below it: a ride up.
    pub arriving_above: bool,
}

struct Portal {
    draws: Vec<Draw>,
    /// What moves in it, drawn afresh each frame.
    moving: Vec<Draw>,
    aperture: Aperture,
}

impl Aperture {
    /// Whether the other area has any part in the picture from `eye`, which looks at `target`, the player as a rule. The
    /// door's plane runs on for ever and the doorway does not. An eye past that plane by any other way than the opening is
    /// still in this area, however far from the door: on Upper City South's plaza, 97 m along the plane of the door to the
    /// north, it took the floor and the player away and showed the area behind that door.
    fn in_the_picture(&self, eye: Vec3, target: Vec3) -> bool {
        let (s, c) = self.out.sin_cos();
        let into = Vec3::new(c, s, 0.0);
        let at = Vec3::from(self.at);
        let (past_eye, past_target) = ((eye - at).dot(into), (target - at).dot(into));
        if past_eye <= 0.0 {
            return true;
        }
        // Where the way from the target to the eye meets the door's plane, or the target itself if it is on its way through.
        let on = if past_target < 0.0 {
            target + (eye - target) * (-past_target / (past_eye - past_target))
        } else if past_target <= APERTURE_WAY_THROUGH {
            target
        } else {
            return false;
        };
        let across = (on - at).dot(Vec3::new(-s, c, 0.0));
        let up = on.z - at.z;
        across.abs() <= self.half + APERTURE_EDGE
            && (-APERTURE_EDGE..=APERTURE_HEIGHT + APERTURE_EDGE).contains(&up)
    }

    /// Whether `p` is seen through the doorway from `eye`: `eye` is on the world side of it and `p` inside the view that
    /// the doorway cuts out. What of this area stands there is not drawn, for the other area is.
    pub fn sees_through(&self, eye: [f32; 3], p: [f32; 3]) -> bool {
        let (planes, on_world_side) = self.planes(Vec3::from(eye));
        on_world_side
            && planes
                .iter()
                .all(|n| n[0] * p[0] + n[1] * p[1] + n[2] * p[2] + n[3] >= 0.0)
    }

    /// The five planes that bound the view through the doorway from `eye`, and whether `eye` is on the world side.
    /// Inside all five is the view through the doorway onto the far side.
    fn planes(&self, eye: Vec3) -> ([[f32; 4]; 5], bool) {
        let (s, c) = self.out.sin_cos();
        let into = Vec3::new(c, s, 0.0);
        let lat = Vec3::new(-s, c, 0.0);
        let at = Vec3::from(self.at);
        let side = (eye - at).dot(into);
        let low = at - Vec3::Z * 0.2;
        let corners = [
            low - lat * self.half,
            low + lat * self.half,
            low + lat * self.half + Vec3::Z * APERTURE_HEIGHT,
            low - lat * self.half + Vec3::Z * APERTURE_HEIGHT,
        ];
        let centre = low + Vec3::Z * (APERTURE_HEIGHT / 2.0);
        let mut out = [[0.0; 4]; 5];
        for k in 0..4 {
            let mut n = (corners[k] - eye).cross(corners[(k + 1) % 4] - eye);
            if n.dot(centre - eye) < 0.0 {
                n = -n;
            }
            let n = n.normalize_or_zero();
            out[k] = [n.x, n.y, n.z, -n.dot(eye)];
        }
        // The half of space beyond the doorway, away from the eye.
        let n = if side <= 0.0 { into } else { -into };
        out[4] = [n.x, n.y, n.z, -n.dot(at)];
        (out, side <= 0.0)
    }
}

struct Tex {
    view: wgpu::TextureView,
    look: AlphaLook,
}

/// What a picture's alpha channel does on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
enum AlphaLook {
    Opaque,
    /// Mostly fully there or fully gone: pixels are cut, and depth still works.
    Cutout,
    /// Many pixels in between: drawn after everything solid, far to near.
    Blend,
}

/// How bright a mark's pixels are on average, at most. **Measured:** 13 of the game's scorch and shadow textures are under
/// this, and its bushes and ferns are over it.
const SOFT_MARK_BRIGHTEST: u64 = 40;

/// **Assumption:** the shares that tell these apart are chosen by eye, not read from the game.
fn alpha_look(img: &Image) -> AlphaLook {
    let (mut gone, mut between, mut seen, mut bright) = (0usize, 0usize, 0u64, 0u64);
    for p in img.rgba.chunks(4) {
        match p[3] {
            0..=19 => gone += 1,
            20..=234 => between += 1,
            _ => {}
        }
        if p[3] > 19 {
            seen += 1;
            bright += p[0].max(p[1]).max(p[2]) as u64;
        }
    }
    let n = (img.rgba.len() / 4).max(1);
    // Scorch marks and painted shadows, as `LHR_blst02` on the Endar Spire's floors: nearly black with soft edges. Cut, they
    // are hard black splats. Plants have as much in between, and stay cut for their depth.
    let soft_and_dark = between * 8 > gone + between && bright < seen * SOFT_MARK_BRIGHTEST;
    if (gone + between) * 200 < n {
        AlphaLook::Opaque
    } else if between * 3 > gone + between || soft_and_dark {
        AlphaLook::Blend
    } else {
        AlphaLook::Cutout
    }
}

/// The pipelines that draw the scene, made for `count` samples a pixel: they change with the anti-aliasing level.
struct Pipes {
    pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,
    blend_pipeline: wgpu::RenderPipeline,
    adding_pipeline: wgpu::RenderPipeline,
    outline_pipeline: wgpu::RenderPipeline,
    additive_pipeline: wgpu::RenderPipeline,
    smoke_pipeline: wgpu::RenderPipeline,
}

fn make_pipelines(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    particle_module: &wgpu::ShaderModule,
    globals_layout: &wgpu::BindGroupLayout,
    mesh_layout: &wgpu::BindGroupLayout,
    particle_layout: &wgpu::BindGroupLayout,
    count: u32,
) -> Pipes {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("kotor"),
        bind_group_layouts: &[globals_layout, mesh_layout],
        push_constant_ranges: &[],
    });
    let attrs =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x2];
    let make = |depth_write: bool,
                compare: wgpu::CompareFunction,
                blend: Option<wgpu::BlendState>,
                (vs, fs, cull): (&str, &str, Option<wgpu::Face>)| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("kotor"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some(vs),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attrs,
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some(fs),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: cull,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: depth_write,
                depth_compare: compare,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count,
                ..Default::default()
            },
            multiview: None,
            cache: None,
        })
    };
    let plain = ("vs_main", "fs_main", None);
    let pipeline = make(true, wgpu::CompareFunction::Less, None, plain);
    let sky_pipeline = make(false, wgpu::CompareFunction::Always, None, plain);
    let blend_pipeline = make(
        false,
        wgpu::CompareFunction::Less,
        Some(wgpu::BlendState::ALPHA_BLENDING),
        plain,
    );
    // A picture that adds its light: what is behind stays, and this goes on top of it, as far as it is bright.
    let adding = wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    };
    let adding_pipeline = make(
        false,
        wgpu::CompareFunction::Less,
        Some(wgpu::BlendState {
            color: adding,
            alpha: wgpu::BlendComponent::OVER,
        }),
        plain,
    );
    // The outline is the body again, a little fatter, of which only the faces turned away show.
    let outline_pipeline = make(
        true,
        wgpu::CompareFunction::Less,
        None,
        ("vs_outline", "fs_outline", Some(wgpu::Face::Front)),
    );
    let particle_pipes = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("particle"),
        bind_group_layouts: &[globals_layout, particle_layout],
        push_constant_ranges: &[],
    });
    let particle_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x4];
    let particle_pipeline = |blend: wgpu::BlendState| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("particle"),
            layout: Some(&particle_pipes),
            vertex: wgpu::VertexState {
                module: particle_module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<effects::ParticleVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &particle_attrs,
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: particle_module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::COLOR,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count,
                ..Default::default()
            },
            multiview: None,
            cache: None,
        })
    };
    let additive_pipeline = particle_pipeline(wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent::OVER,
    });
    let smoke_pipeline = particle_pipeline(wgpu::BlendState::ALPHA_BLENDING);
    Pipes {
        pipeline,
        sky_pipeline,
        blend_pipeline,
        adding_pipeline,
        outline_pipeline,
        additive_pipeline,
        smoke_pipeline,
    }
}

/// After the scene: motion blur and the edge smoothing that is the top anti-aliasing tier. A full-screen triangle each.
/// `DEPTH_TY` is the depth texture's type, which is multisampled when the scene is.
const POST_SHADER: &str = r#"
struct Post {
    inv_vp: mat4x4<f32>,
    prev_vp: mat4x4<f32>,
    // x: how much of the last frame's movement to blur along (the exposure over the frame time), y: the longest blur in
    // picture widths, z and w: one pixel across and down.
    params: vec4<f32>,
};
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> p: Post;
@group(1) @binding(0) var depth: DEPTH_TY;

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> VOut {
    let x = f32((i << 1u) & 2u);
    let y = f32(i & 2u);
    var o: VOut;
    o.pos = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    o.uv = vec2<f32>(x, y);
    return o;
}

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.299, 0.587, 0.114));
}

// FXAA (Lottes), the short form: the edge's direction from the lumas of the four corners, then two or four taps along it.
@fragment
fn fs_fxaa(i: VOut) -> @location(0) vec4<f32> {
    let px = p.params.zw;
    let rgb_nw = textureSampleLevel(src, samp, i.uv + vec2<f32>(-1.0, -1.0) * px, 0.0).rgb;
    let rgb_ne = textureSampleLevel(src, samp, i.uv + vec2<f32>(1.0, -1.0) * px, 0.0).rgb;
    let rgb_sw = textureSampleLevel(src, samp, i.uv + vec2<f32>(-1.0, 1.0) * px, 0.0).rgb;
    let rgb_se = textureSampleLevel(src, samp, i.uv + vec2<f32>(1.0, 1.0) * px, 0.0).rgb;
    let rgb_m = textureSampleLevel(src, samp, i.uv, 0.0).rgb;
    let l_nw = luma(rgb_nw);
    let l_ne = luma(rgb_ne);
    let l_sw = luma(rgb_sw);
    let l_se = luma(rgb_se);
    let l_m = luma(rgb_m);
    let l_min = min(l_m, min(min(l_nw, l_ne), min(l_sw, l_se)));
    let l_max = max(l_m, max(max(l_nw, l_ne), max(l_sw, l_se)));
    var dir = vec2<f32>(-((l_nw + l_ne) - (l_sw + l_se)), (l_nw + l_sw) - (l_ne + l_se));
    let reduce = max((l_nw + l_ne + l_sw + l_se) * 0.25 * 0.125, 1.0 / 128.0);
    let rcp = 1.0 / (min(abs(dir.x), abs(dir.y)) + reduce);
    dir = clamp(dir * rcp, vec2<f32>(-8.0), vec2<f32>(8.0)) * px;
    let a = 0.5 * (textureSampleLevel(src, samp, i.uv + dir * (1.0 / 3.0 - 0.5), 0.0).rgb
        + textureSampleLevel(src, samp, i.uv + dir * (2.0 / 3.0 - 0.5), 0.0).rgb);
    let b = a * 0.5 + 0.25 * (textureSampleLevel(src, samp, i.uv + dir * -0.5, 0.0).rgb
        + textureSampleLevel(src, samp, i.uv + dir * 0.5, 0.0).rgb);
    let l_b = luma(b);
    if (l_b < l_min || l_b > l_max) {
        return vec4<f32>(a, 1.0);
    }
    return vec4<f32>(b, 1.0);
}

// Motion blur from the camera's own movement: where this pixel's point was on the last frame, from its depth and the
// last frame's view and projection, and the picture smeared along the way it came, no further than the longest allowed.
@fragment
fn fs_blur(i: VOut) -> @location(0) vec4<f32> {
    let xy = vec2<i32>(i.pos.xy);
    let d = textureLoad(depth, xy, 0);
    let ndc = vec4<f32>(i.uv.x * 2.0 - 1.0, 1.0 - i.uv.y * 2.0, d, 1.0);
    let wp = p.inv_vp * ndc;
    let world = wp / wp.w;
    let pc = p.prev_vp * world;
    var v = vec2<f32>(0.0);
    if (pc.w > 0.0001) {
        let before = vec2<f32>(pc.x / pc.w * 0.5 + 0.5, 0.5 - pc.y / pc.w * 0.5);
        v = (i.uv - before) * p.params.x;
    }
    let len = length(v);
    if (len > p.params.y) {
        v = v * (p.params.y / len);
    }
    var acc = vec3<f32>(0.0);
    let n = 12;
    for (var k = 0; k < n; k = k + 1) {
        let t = f32(k) / f32(n - 1) - 0.5;
        acc = acc + textureSampleLevel(src, samp, i.uv + v * t, 0.0).rgb;
    }
    return vec4<f32>(acc / f32(n), 1.0);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PostUniform {
    inv_vp: [[f32; 4]; 4],
    prev_vp: [[f32; 4]; 4],
    params: [f32; 4],
}

/// The full-screen passes after the scene.
struct Post {
    sampler: wgpu::Sampler,
    layout: wgpu::BindGroupLayout,
    depth_layout_1: wgpu::BindGroupLayout,
    depth_layout_ms: wgpu::BindGroupLayout,
    fxaa: wgpu::RenderPipeline,
    blur_1: wgpu::RenderPipeline,
    blur_ms: wgpu::RenderPipeline,
}

/// How long the shutter stays open for each motion blur level, in seconds, and the longest blur in picture widths: off, subtle,
/// moderate and intense. The owner: "Make the subtle motion blur more subtle. Make the current subtle the moderate and then make
/// the intense the moderate". **Assumed:** Subtle half of the old Subtle, Moderate the old Subtle, Intense the old Moderate.
const BLUR_EXPOSURE: [f32; 4] = [0.0, 0.005, 0.010, 0.020];
const BLUR_LONGEST: [f32; 4] = [0.0, 0.006, 0.012, 0.026];

fn make_post(device: &wgpu::Device) -> Post {
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("post"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });
    let depth_layout = |multisampled: bool| {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("post depth"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled,
                },
                count: None,
            }],
        })
    };
    let (depth_layout_1, depth_layout_ms) = (depth_layout(false), depth_layout(true));
    let module = |ty: &str| {
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("post"),
            source: wgpu::ShaderSource::Wgsl(POST_SHADER.replace("DEPTH_TY", ty).into()),
        })
    };
    let (m1, mms) = (
        module("texture_depth_2d"),
        module("texture_depth_multisampled_2d"),
    );
    let pipe = |module: &wgpu::ShaderModule, fs: &str, depth: Option<&wgpu::BindGroupLayout>| {
        let mut layouts = vec![&layout];
        layouts.extend(depth);
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("post"),
            bind_group_layouts: &layouts,
            push_constant_ranges: &[],
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("post"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some(fs),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        })
    };
    Post {
        fxaa: pipe(&m1, "fs_fxaa", None),
        blur_1: pipe(&m1, "fs_blur", Some(&depth_layout_1)),
        blur_ms: pipe(&mms, "fs_blur", Some(&depth_layout_ms)),
        sampler,
        layout,
        depth_layout_1,
        depth_layout_ms,
    }
}

/// What the scene is drawn into besides the finished picture: the multisampled colour and the depth, and the two images
/// the post passes pass between.
struct SceneTargets {
    depth: wgpu::TextureView,
    msaa: Option<wgpu::TextureView>,
    aux: Vec<wgpu::Texture>,
}

/// The samples a pixel gets at each anti-aliasing level when the adapter allows `max`: off, 4x, 8x, 16x. 16x is not a
/// sample count any adapter has: it is the 8x count (or the best there is) and the edge pass on top.
fn aa_samples(level: u8, max: u32) -> u32 {
    match level {
        0 => 1,
        1 => 4.min(max),
        _ => {
            if max >= 8 {
                8
            } else {
                4.min(max)
            }
        }
    }
}

pub struct GpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,
    sky_at_infinity: bool,
    particle_layout: wgpu::BindGroupLayout,
    additive_pipeline: wgpu::RenderPipeline,
    pub frame_times: FrameTimes,
    smoke_pipeline: wgpu::RenderPipeline,
    particles: Vec<effects::ParticleBatch>,
    blend_pipeline: wgpu::RenderPipeline,
    adding_pipeline: wgpu::RenderPipeline,
    transparency: bool,
    globals_layout: wgpu::BindGroupLayout,
    mesh_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    width: u32,
    height: u32,
    color: wgpu::Texture,
    white: Tex,
    /// Drawn in place of a picture that is not ready, for a frame or two.
    grey: Tex,
    tex_cache: HashMap<String, Option<Tex>>,
    /// Bind groups of meshes by what they show, so a scene built each frame makes none it made before. Cleared when a
    /// texture comes or goes.
    bind_cache: HashMap<BindKey, wgpu::BindGroup>,
    /// A picture was asked for that is not ready: the bind group made with its grey stand-in is not kept.
    stand_in_used: bool,
    static_draws: Vec<Draw>,
    /// Another area seen through a doorway.
    portal: Option<Portal>,
    /// A lift ride, drawn with the portal: see `LiftView`.
    lift_view: Option<LiftView>,
    /// Draw only what each frame passes in: a figure on a screen.
    figure_only: bool,
    /// Cel shading: 0 off, 1 subtle, 2 strong. The moving meshes get a thin dark outline from the second pipeline.
    cel: u8,
    outline_pipeline: wgpu::RenderPipeline,
    module: wgpu::ShaderModule,
    particle_module: wgpu::ShaderModule,
    /// Anti-aliasing: 0 off, 1 4x, 2 8x, 3 16x; the samples a pixel has, and the most the adapter has.
    aa: u8,
    samples: u32,
    max_samples: u32,
    /// Motion blur: 0 off, 1 subtle, 2 moderate, 3 intense.
    blur: u8,
    /// Not blurred while this is on: a loading screen, a paused game.
    blur_suspended: bool,
    /// The frame time the blur goes by, when the caller knows it, and else the time since the last frame.
    frame_dt: Option<f32>,
    last_draw: Option<std::time::Instant>,
    prev_vp: Option<Mat4>,
    scene_targets: SceneTargets,
    post: Post,
}

/// What a mesh's bind group depends on: its two pictures and its look.
#[derive(Clone, PartialEq, Eq, Hash)]
struct BindKey {
    diffuse: Option<String>,
    lightmap: Option<String>,
    params: [u32; 5],
}

struct Draw {
    /// The buffers of every mesh built together: one pair for a whole scene, and this mesh's bytes of each.
    vertices: wgpu::Buffer,
    vertex_bytes: std::ops::Range<u64>,
    indices: wgpu::Buffer,
    index_bytes: std::ops::Range<u64>,
    count: u32,
    bind: wgpu::BindGroup,
    blended: bool,
    /// It adds its light, and is drawn among the blended.
    adding: bool,
    centre: [f32; 3],
    /// A sky: the centre of its mesh, which is kept on the camera.
    sky: Option<[f32; 3]>,
}

impl GpuRenderer {
    /// Create a renderer drawing into an offscreen `width` x `height` image. Fails if no adapter exists.
    pub fn new(width: u32, height: u32) -> Result<GpuRenderer, String> {
        Self::with_instance(&wgpu::Instance::default(), None, width, height)
    }

    /// Like `new`, but on a caller's instance and an adapter that can present to `surface` (see `Presenter`).
    pub fn with_instance(
        instance: &wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
        width: u32,
        height: u32,
    ) -> Result<GpuRenderer, String> {
        let _wait = watch("making the GPU device and pipelines");
        pollster::block_on(Self::new_async(instance, surface, width, height))
    }

    async fn new_async(
        instance: &wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
        width: u32,
        height: u32,
    ) -> Result<GpuRenderer, String> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: surface,
                force_fallback_adapter: false,
            })
            .await
            .ok_or("no GPU adapter found")?;
        log::info!("adapter: {:?}", adapter.get_info());
        // The sample counts above 4 a format has are told by the adapter, and only once this feature is on.
        let wanted = wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES & adapter.features();
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: wanted,
                    ..Default::default()
                },
                None,
            )
            .await
            .map_err(|e| format!("request_device: {e}"))?;
        let can = |format: wgpu::TextureFormat, n: u32| {
            adapter
                .get_texture_format_features(format)
                .flags
                .sample_count_supported(n)
        };
        let max_samples = [8u32, 4, 2]
            .into_iter()
            .find(|n| {
                can(wgpu::TextureFormat::Rgba8Unorm, *n)
                    && can(wgpu::TextureFormat::Depth32Float, *n)
                    && (*n <= 4
                        || wanted
                            .contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES))
            })
            .unwrap_or(1);

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("kotor"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let uniform = |binding: u32, visibility| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let texture = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[uniform(0, wgpu::ShaderStages::VERTEX_FRAGMENT)],
        });
        let mesh_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mesh"),
            entries: &[
                texture(0),
                texture(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                uniform(3, wgpu::ShaderStages::FRAGMENT),
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let color = make_color(&device, width, height);
        let particle_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("particle"),
            entries: &[
                texture(0),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let particle_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("particle"),
            source: wgpu::ShaderSource::Wgsl(PARTICLE_SHADER.into()),
        });
        let Pipes {
            pipeline,
            sky_pipeline,
            blend_pipeline,
            adding_pipeline,
            outline_pipeline,
            additive_pipeline,
            smoke_pipeline,
        } = make_pipelines(
            &device,
            &module,
            &particle_module,
            &globals_layout,
            &mesh_layout,
            &particle_layout,
            1,
        );
        let scene_targets = make_scene_targets(&device, width, height, 1, false);
        let post = make_post(&device);
        let grey = upload_image(
            &device,
            &queue,
            &Image {
                width: 1,
                height: 1,
                rgba: vec![128, 128, 128, 255],
            },
        );
        let white = upload_image(
            &device,
            &queue,
            &Image {
                width: 1,
                height: 1,
                rgba: vec![255, 255, 255, 255],
            },
        );
        Ok(GpuRenderer {
            particle_layout,
            additive_pipeline,
            frame_times: FrameTimes::default(),
            smoke_pipeline,
            particles: Vec::new(),
            sky_pipeline,
            sky_at_infinity: true,
            blend_pipeline,
            adding_pipeline,
            transparency: true,
            white,
            grey,
            tex_cache: HashMap::new(),
            bind_cache: HashMap::new(),
            stand_in_used: false,
            static_draws: Vec::new(),
            portal: None,
            lift_view: None,
            figure_only: false,
            cel: 0,
            outline_pipeline,
            device,
            queue,
            pipeline,
            globals_layout,
            mesh_layout,
            sampler,
            width,
            height,
            color,
            module,
            particle_module,
            aa: 0,
            samples: 1,
            max_samples,
            blur: 0,
            blur_suspended: false,
            frame_dt: None,
            last_draw: None,
            prev_vp: None,
            scene_targets,
            post,
        })
    }

    /// Render `scene` from `cam`. `textures` maps a texture name to a decoded image.
    pub fn render(
        &mut self,
        scene: &Scene,
        textures: &mut dyn FnMut(&str) -> Option<Image>,
        cam: &Camera,
    ) -> Result<Image, String> {
        let _wait = watch("rendering a frame");
        self.draw(scene, textures, cam);
        self.read_back()
    }

    /// The particles of the next frame.
    pub fn set_particles(&mut self, batches: Vec<effects::ParticleBatch>) {
        self.particles = batches;
    }

    /// Draw at a new size from now on. A zero size is ignored (a minimised window).
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 || (width, height) == (self.width, self.height) {
            return;
        }
        self.color = make_color(&self.device, width, height);
        (self.width, self.height) = (width, height);
        self.rebuild_scene_targets();
        self.prev_vp = None;
    }

    /// The most samples a pixel can have on this adapter: 1, 2, 4 or 8.
    pub fn max_samples(&self) -> u32 {
        self.max_samples
    }

    /// The samples a pixel has now.
    pub fn samples(&self) -> u32 {
        self.samples
    }

    /// Anti-aliasing: 0 off, 1 4x, 2 8x, 3 16x. 4x and 8x are that many samples a pixel where the adapter has them (a
    /// lesser count where it does not). 16x is no sample count that adapters have: it is 8x and an edge-smoothing pass on
    /// the finished picture, which also catches the edges that samples cannot (cut-out leaves, hair).
    pub fn set_anti_aliasing(&mut self, level: u8) {
        let level = level.min(3);
        let samples = aa_samples(level, self.max_samples);
        let sample_change = samples != self.samples;
        let post_change = (level == 3) != (self.aa == 3);
        self.aa = level;
        if sample_change {
            self.samples = samples;
            let p = make_pipelines(
                &self.device,
                &self.module,
                &self.particle_module,
                &self.globals_layout,
                &self.mesh_layout,
                &self.particle_layout,
                samples,
            );
            self.pipeline = p.pipeline;
            self.sky_pipeline = p.sky_pipeline;
            self.blend_pipeline = p.blend_pipeline;
            self.adding_pipeline = p.adding_pipeline;
            self.outline_pipeline = p.outline_pipeline;
            self.additive_pipeline = p.additive_pipeline;
            self.smoke_pipeline = p.smoke_pipeline;
        }
        if sample_change || post_change {
            self.rebuild_scene_targets();
        }
    }

    /// Motion blur: 0 off, 1 subtle, 2 moderate, 3 intense.
    pub fn set_motion_blur(&mut self, level: u8) {
        let level = level.min(3);
        if level != self.blur {
            let had = self.post_wanted();
            self.blur = level;
            if had != self.post_wanted() {
                self.rebuild_scene_targets();
            }
        }
    }

    /// No blur while `on`: a loading screen, a paused game. The frame after it starts fresh.
    pub fn set_motion_blur_suspended(&mut self, on: bool) {
        if on {
            self.prev_vp = None;
        }
        self.blur_suspended = on;
    }

    /// The seconds a frame stands for, when the caller knows (a driver that steps by a fixed time): the blur goes by
    /// this and not by the clock.
    pub fn set_frame_time(&mut self, dt: Option<f32>) {
        self.frame_dt = dt;
    }

    /// Whether a post pass is wanted: the edge pass of 16x, or motion blur.
    fn post_wanted(&self) -> bool {
        self.aa == 3 || self.blur > 0
    }

    fn rebuild_scene_targets(&mut self) {
        let aux = self.post_wanted();
        self.scene_targets =
            make_scene_targets(&self.device, self.width, self.height, self.samples, aux);
    }

    /// By default a sky is drawn first, around the camera, with no depth, so it is never in front of anything and
    /// the camera can never leave it. Off, it is an ordinary mesh, as in the software renderer.
    pub fn set_sky_at_infinity(&mut self, on: bool) {
        self.sky_at_infinity = on;
    }

    /// By default alpha cuts holes or blends. Off, everything is solid, as in the software renderer.
    /// Cel shading: 0 off, 1 subtle, 2 strong. Characters and props get a thin dark outline and every lit surface its light
    /// cut into steps; the sky, the particles and anything drawn over the picture are left as they are.
    pub fn set_cel_shading(&mut self, level: u8) {
        self.cel = level.min(2);
    }

    pub fn set_transparency(&mut self, on: bool) {
        self.transparency = on;
    }

    /// Draw only what each frame passes in, leaving out the static scene, for a figure on a screen.
    pub fn set_figure_only(&mut self, on: bool) {
        self.figure_only = on;
    }

    fn statics(&self) -> &[Draw] {
        if self.figure_only {
            &[]
        } else {
            &self.static_draws
        }
    }

    /// Keep `scene` on the GPU and draw it in every later frame, behind whatever each frame passes in.
    pub fn set_static_scene(
        &mut self,
        scene: &Scene,
        textures: &mut dyn FnMut(&str) -> Option<Image>,
    ) {
        self.static_draws = self.build_draws(scene, textures);
        // A new area: the last frame's view says nothing of it.
        self.prev_vp = None;
    }

    /// Another area, drawn only where it is seen through `aperture`, or none. The area's own picture is cut out of that
    /// view, so the two never mix.
    pub fn set_portal(
        &mut self,
        other: Option<(&Scene, Aperture)>,
        textures: &mut dyn FnMut(&str) -> Option<Image>,
    ) {
        self.portal = other.map(|(scene, aperture)| Portal {
            draws: self.build_draws(scene, textures),
            moving: Vec::new(),
            aperture,
        });
    }

    /// Draw the portal's area as a lift ride's arrival, or go back to the doorway's view with `None`.
    pub fn set_lift_view(&mut self, view: Option<LiftView>) {
        self.lift_view = view;
    }

    /// What moves in the area seen through the doorway, for the frames from now on: the creatures standing in it.
    pub fn set_portal_moving(
        &mut self,
        scene: &Scene,
        textures: &mut dyn FnMut(&str) -> Option<Image>,
    ) {
        if self.portal.is_none() {
            return;
        }
        let draws = self.build_draws(scene, textures);
        if let Some(p) = self.portal.as_mut() {
            p.moving = draws;
        }
    }

    /// Decode and upload these textures now, so that a picture that uses them is later put up without waiting for them. The
    /// caller hands them over a few a frame.
    pub fn warm_textures(
        &mut self,
        names: &[String],
        textures: &mut dyn FnMut(&str) -> Option<Image>,
    ) {
        for n in names {
            let _ = self.texture_view(&Some(n.clone()), textures);
        }
    }

    /// Ask again for this picture the next time it is drawn: a better one has come.
    pub fn forget_texture(&mut self, name: &str) {
        self.bind_cache.clear();
        self.tex_cache.remove(&name.to_ascii_lowercase());
    }

    /// The view of a named texture, decoding and uploading it the first time. A picture with no pixels is not ready: it is
    /// grey for now, and asked for again next time.
    fn texture_view(
        &mut self,
        name: &Option<String>,
        textures: &mut dyn FnMut(&str) -> Option<Image>,
    ) -> Option<wgpu::TextureView> {
        let key = name.as_ref()?.to_ascii_lowercase();
        if !self.tex_cache.contains_key(&key) {
            let began = std::time::Instant::now();
            let img = textures(&key);
            if img.as_ref().is_some_and(|i| i.width == 0) {
                self.stand_in_used = true;
                return Some(self.grey.view.clone());
            }
            let t = img.map(|img| upload_image(&self.device, &self.queue, &img));
            self.bind_cache.clear();
            self.tex_cache.insert(key.clone(), t);
            self.frame_times.new_textures += 1;
            self.frame_times.textures_ms += began.elapsed().as_secs_f32() * 1000.0;
        }
        self.tex_cache[&key].as_ref().map(|t| t.view.clone())
    }

    fn build_draws(
        &mut self,
        scene: &Scene,
        textures: &mut dyn FnMut(&str) -> Option<Image>,
    ) -> Vec<Draw> {
        // Every mesh's vertices and indices go into one buffer each, made once, not a pair of buffers a mesh: a scene
        // built each frame made hundreds.
        let mut verts: Vec<Vertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        let vsize = std::mem::size_of::<Vertex>() as u64;
        struct Part {
            vertex_bytes: std::ops::Range<u64>,
            index_bytes: std::ops::Range<u64>,
            count: u32,
            bind: wgpu::BindGroup,
            blended: bool,
            adding: bool,
            centre: [f32; 3],
            sky: Option<[f32; 3]>,
        }
        let mut parts: Vec<Part> = Vec::new();
        for m in &scene.meshes {
            if m.positions.is_empty() || m.triangles.is_empty() {
                continue;
            }
            let v_start = verts.len() as u64 * vsize;
            verts.extend((0..m.positions.len()).map(|i| Vertex {
                pos: m.positions[i],
                normal: m.normals.get(i).copied().unwrap_or([0.0, 0.0, 1.0]),
                uv: m.uvs.get(i).copied().unwrap_or([0.0; 2]),
                uv2: m.uvs2.get(i).copied().unwrap_or([0.0; 2]),
            }));
            let v_end = verts.len() as u64 * vsize;
            let i_start = indices.len() as u64 * 4;
            indices.extend(m.triangles.iter().flatten().copied());
            let i_end = indices.len() as u64 * 4;
            self.stand_in_used = false;
            let diffuse = self.texture_view(&m.texture, textures);
            let lightmap = self.texture_view(&m.lightmap, textures);
            let stand_in = self.stand_in_used;
            let look = match m.texture.as_ref().map(|n| n.to_ascii_lowercase()) {
                Some(n) if self.transparency => self
                    .tex_cache
                    .get(&n)
                    .and_then(Option::as_ref)
                    .map_or(AlphaLook::Opaque, |t| t.look),
                _ => AlphaLook::Opaque,
            };
            let adding = self.transparency
                && m.texture
                    .as_ref()
                    .is_some_and(|n| kotor_render::is_additive(n));
            let look = if adding { AlphaLook::Blend } else { look };
            let (cut, blend) = match look {
                AlphaLook::Opaque => (0.0, 0.0),
                AlphaLook::Cutout => (0.5, 0.0),
                AlphaLook::Blend => (0.02, 1.0),
            };
            let params = [
                if lightmap.is_some() { 1.0f32 } else { 0.0 },
                cut,
                blend,
                m.shade,
                m.fade,
                0.0,
                0.0,
                0.0,
            ];
            let key = BindKey {
                diffuse: m.texture.as_ref().map(|n| n.to_ascii_lowercase()),
                lightmap: m.lightmap.as_ref().map(|n| n.to_ascii_lowercase()),
                params: [
                    params[0].to_bits(),
                    params[1].to_bits(),
                    params[2].to_bits(),
                    params[3].to_bits(),
                    params[4].to_bits(),
                ],
            };
            let bind = match self.bind_cache.get(&key) {
                Some(b) if !stand_in => b.clone(),
                _ => {
                    let params_buf =
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: None,
                                contents: bytemuck::bytes_of(&params),
                                usage: wgpu::BufferUsages::UNIFORM,
                            });
                    let white = self.white.view.clone();
                    let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: None,
                        layout: &self.mesh_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(
                                    diffuse.as_ref().unwrap_or(&white),
                                ),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::TextureView(
                                    lightmap.as_ref().unwrap_or(&white),
                                ),
                            },
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: wgpu::BindingResource::Sampler(&self.sampler),
                            },
                            wgpu::BindGroupEntry {
                                binding: 3,
                                resource: params_buf.as_entire_binding(),
                            },
                        ],
                    });
                    if !stand_in {
                        if self.bind_cache.len() > 4096 {
                            self.bind_cache.clear();
                        }
                        self.bind_cache.insert(key, bind.clone());
                    }
                    bind
                }
            };
            parts.push(Part {
                vertex_bytes: v_start..v_end,
                index_bytes: i_start..i_end,
                count: ((i_end - i_start) / 4) as u32,
                bind,
                blended: look == AlphaLook::Blend,
                adding,
                centre: mesh_centre(m),
                sky: sky_centre(m),
            });
        }
        if parts.is_empty() {
            return Vec::new();
        }
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&verts),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        let draws: Vec<Draw> = parts
            .into_iter()
            .map(|p| Draw {
                vertices: vertex_buffer.clone(),
                vertex_bytes: p.vertex_bytes,
                indices: index_buffer.clone(),
                index_bytes: p.index_bytes,
                count: p.count,
                bind: p.bind,
                blended: p.blended,
                adding: p.adding,
                centre: p.centre,
                sky: p.sky,
            })
            .collect();
        draws
    }

    fn draw(
        &mut self,
        scene: &Scene,
        textures: &mut dyn FnMut(&str) -> Option<Image>,
        cam: &Camera,
    ) {
        let eye = Vec3::from(cam.eye);
        let view = Mat4::look_at_rh(eye, Vec3::from(cam.target), Vec3::Z);
        let proj = Mat4::perspective_rh(
            cam.fov.to_radians(),
            self.width as f32 / self.height as f32,
            0.05,
            2000.0,
        );
        let this_vp = proj * view;
        let now = std::time::Instant::now();
        let dt = self
            .frame_dt
            .or_else(|| self.last_draw.map(|t| now.duration_since(t).as_secs_f32()));
        self.last_draw = Some(now);
        let blur_now = self.blur > 0
            && !self.blur_suspended
            && !self.figure_only
            && dt.is_some_and(|d| d > 0.0001 && d < 0.25)
            && self.prev_vp.is_some();
        let prev_vp = self.prev_vp.unwrap_or(this_vp);
        self.prev_vp = Some(this_vp);
        let globals = Globals {
            view_proj: this_vp.to_cols_array_2d(),
            light: [0.3, -0.5, 0.8, 0.0],
            planes: [[0.0; 4]; 5],
            // Cel shading: the level, how wide a pixel is a metre off, and the outline's width in pixels (a thin line at
            // 1080 lines, so as thin at every size).
            mode: [
                0.0,
                self.cel as f32,
                2.0 * (cam.fov.to_radians() / 2.0).tan() / self.height as f32,
                if self.cel == 0 {
                    0.0
                } else {
                    (if self.cel == 1 { 1.9 } else { 2.2 })
                        * (self.height as f32 / 1080.0).max(0.75)
                },
            ],
        };
        let globals_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("globals"),
                contents: bytemuck::bytes_of(&globals),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let globals_bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals"),
            layout: &self.globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buf.as_entire_binding(),
            }],
        });
        let building = std::time::Instant::now();
        let draws = self.build_draws(scene, textures);
        self.frame_times.build_ms = building.elapsed().as_secs_f32() * 1000.0;
        // Binds 1 and 2 cut a doorway view out of one picture and keep nothing else of the other.
        let lift = self
            .lift_view
            .filter(|_| self.portal.is_some() && !self.figure_only);
        let (portal_binds, on_world_side): (Option<[wgpu::BindGroup; 2]>, bool) =
            match (&self.portal, self.figure_only) {
                (Some(_), false) if lift.is_some() => {
                    let lv = lift.unwrap();
                    let make = |mode: f32, up: f32| {
                        let mut planes = [[0.0, 0.0, 0.0, 1.0]; 5];
                        planes[0] = if lv.arriving_above {
                            [0.0, 0.0, 1.0, up - lv.boundary]
                        } else {
                            [0.0, 0.0, -1.0, lv.boundary - up]
                        };
                        let g = Globals {
                            view_proj: (proj * view * Mat4::from_translation(Vec3::Z * up))
                                .to_cols_array_2d(),
                            planes,
                            mode: [mode, globals.mode[1], globals.mode[2], globals.mode[3]],
                            ..globals
                        };
                        let buf =
                            self.device
                                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                    label: Some("lift globals"),
                                    contents: bytemuck::bytes_of(&g),
                                    usage: wgpu::BufferUsages::UNIFORM,
                                });
                        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("lift globals"),
                            layout: &self.globals_layout,
                            entries: &[wgpu::BindGroupEntry {
                                binding: 0,
                                resource: buf.as_entire_binding(),
                            }],
                        })
                    };
                    (
                        Some([make(1.0, lv.left_up), make(2.0, lv.arriving_up)]),
                        true,
                    )
                }
                (Some(p), false) if p.aperture.in_the_picture(eye, Vec3::from(cam.target)) => {
                    let (planes, on_world_side) = p.aperture.planes(eye);
                    let make = |mode: f32| {
                        let g = Globals {
                            planes,
                            mode: [mode, globals.mode[1], globals.mode[2], globals.mode[3]],
                            ..globals
                        };
                        let buf =
                            self.device
                                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                    label: Some("portal globals"),
                                    contents: bytemuck::bytes_of(&g),
                                    usage: wgpu::BufferUsages::UNIFORM,
                                });
                        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("portal globals"),
                            layout: &self.globals_layout,
                            entries: &[wgpu::BindGroupEntry {
                                binding: 0,
                                resource: buf.as_entire_binding(),
                            }],
                        })
                    };
                    (Some([make(1.0), make(2.0)]), on_world_side)
                }
                _ => (None, true),
            };
        // Which of the three binds each part is drawn with: 0 plain, 1 with the doorway view cut out, 2 with only it.
        let (g_static, g_moving, g_portal) = match (&portal_binds, on_world_side) {
            (None, _) => (0, 0, 0),
            (Some(_), true) => (1, 0, 2),
            (Some(_), false) => (2, 2, 1),
        };
        let batches = std::mem::take(&mut self.particles);
        let particle_draws: Vec<(bool, u32, wgpu::Buffer, wgpu::BindGroup)> = batches
            .iter()
            .filter(|b| !b.vertices.is_empty())
            .filter_map(|b| {
                let view = self.texture_view(&Some(b.texture.clone()), textures)?;
                let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("particle"),
                    layout: &self.particle_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                    ],
                });
                let buffer = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("particle"),
                        contents: bytemuck::cast_slice(&b.vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                Some((b.additive, b.vertices.len() as u32, buffer, bind))
            })
            .collect();
        let sky_binds: Vec<(wgpu::BindGroup, &Draw)> = self
            .statics()
            .iter()
            .chain(&draws)
            .filter(|_| self.sky_at_infinity)
            .filter_map(|d| {
                let shift = Mat4::from_translation(eye - Vec3::from(d.sky?));
                let g = Globals {
                    view_proj: (proj * view * shift).to_cols_array_2d(),
                    light: globals.light,
                    planes: [[0.0; 4]; 5],
                    mode: [0.0; 4],
                };
                let buf = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("sky"),
                        contents: bytemuck::bytes_of(&g),
                        usage: wgpu::BufferUsages::UNIFORM,
                    });
                let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("sky"),
                    layout: &self.globals_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buf.as_entire_binding(),
                    }],
                });
                Some((bind, d))
            })
            .collect();
        // A sky drawn with the rest is not cut into steps.
        let plain_bind = (self.cel > 0).then(|| {
            let g = Globals {
                mode: [0.0; 4],
                ..globals
            };
            let buf = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("plain globals"),
                    contents: bytemuck::bytes_of(&g),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("plain globals"),
                layout: &self.globals_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf.as_entire_binding(),
                }],
            })
        });
        // The post passes: motion blur from how far the view moved since the last frame, then the edge pass of 16x.
        let fxaa_now = self.aa == 3;
        let post_now = (blur_now || fxaa_now) && self.scene_targets.aux.len() == 2;
        let (blur_now, fxaa_now) = (blur_now && post_now, fxaa_now && post_now);
        let final_view = self.color.create_view(&Default::default());
        let aux_views: Vec<wgpu::TextureView> = self
            .scene_targets
            .aux
            .iter()
            .map(|t| t.create_view(&Default::default()))
            .collect();
        let dest_view = if post_now { &aux_views[0] } else { &final_view };
        let (color_view, resolve) = match &self.scene_targets.msaa {
            Some(m) => (m, Some(dest_view)),
            None => (dest_view, None),
        };
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view,
                    resolve_target: resolve,
                    ops: wgpu::Operations {
                        // A figure's or a planet's screen has space behind it, and a world has its sky.
                        load: wgpu::LoadOp::Clear(if self.figure_only {
                            wgpu::Color::BLACK
                        } else {
                            wgpu::Color {
                                r: 18.0 / 255.0,
                                g: 20.0 / 255.0,
                                b: 28.0 / 255.0,
                                a: 1.0,
                            }
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.scene_targets.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: if blur_now {
                            wgpu::StoreOp::Store
                        } else {
                            wgpu::StoreOp::Discard
                        },
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.sky_pipeline);
            for (g, d) in &sky_binds {
                pass.set_bind_group(0, g, &[]);
                pass.set_bind_group(1, &d.bind, &[]);
                pass.set_vertex_buffer(0, d.vertices.slice(d.vertex_bytes.clone()));
                pass.set_index_buffer(
                    d.indices.slice(d.index_bytes.clone()),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..d.count, 0, 0..1);
            }
            let mut parts: Vec<(&Draw, usize)> = Vec::new();
            let sky_plain = |d: &Draw, k: usize| {
                if d.sky.is_some() && plain_bind.is_some() {
                    3
                } else {
                    k
                }
            };
            parts.extend(self.statics().iter().map(|d| (d, sky_plain(d, g_static))));
            parts.extend(draws.iter().map(|d| (d, sky_plain(d, g_moving))));
            if let (Some(p), Some(_)) = (&self.portal, &portal_binds) {
                parts.extend(
                    p.draws
                        .iter()
                        .chain(&p.moving)
                        .filter(|d| d.sky.is_none())
                        .map(|d| (d, g_portal)),
                );
            }
            parts.retain(|(d, _)| !self.sky_at_infinity || d.sky.is_none());
            let bind_of = |k: usize| -> &wgpu::BindGroup {
                match (k, &portal_binds) {
                    (3, _) => plain_bind.as_ref().unwrap_or(&globals_bind),
                    (1, Some(b)) => &b[0],
                    (2, Some(b)) => &b[1],
                    _ => &globals_bind,
                }
            };
            pass.set_pipeline(&self.pipeline);
            for (d, k) in parts.iter().filter(|(d, _)| !d.blended) {
                pass.set_bind_group(0, bind_of(*k), &[]);
                pass.set_bind_group(1, &d.bind, &[]);
                pass.set_vertex_buffer(0, d.vertices.slice(d.vertex_bytes.clone()));
                pass.set_index_buffer(
                    d.indices.slice(d.index_bytes.clone()),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..d.count, 0, 0..1);
            }
            if self.cel > 0 {
                // Bodies and props only: the level's rooms and the sky have no outline.
                pass.set_pipeline(&self.outline_pipeline);
                for d in draws.iter().filter(|d| !d.blended && d.sky.is_none()) {
                    pass.set_bind_group(0, bind_of(g_moving), &[]);
                    pass.set_bind_group(1, &d.bind, &[]);
                    pass.set_vertex_buffer(0, d.vertices.slice(d.vertex_bytes.clone()));
                    pass.set_index_buffer(
                        d.indices.slice(d.index_bytes.clone()),
                        wgpu::IndexFormat::Uint32,
                    );
                    pass.draw_indexed(0..d.count, 0, 0..1);
                }
                // The figures of the area seen through a doorway, in the doorway's view only.
                if let (Some(p), Some(_)) = (&self.portal, &portal_binds) {
                    for d in p.moving.iter().filter(|d| !d.blended && d.sky.is_none()) {
                        pass.set_bind_group(0, bind_of(g_portal), &[]);
                        pass.set_bind_group(1, &d.bind, &[]);
                        pass.set_vertex_buffer(0, d.vertices.slice(d.vertex_bytes.clone()));
                        pass.set_index_buffer(
                            d.indices.slice(d.index_bytes.clone()),
                            wgpu::IndexFormat::Uint32,
                        );
                        pass.draw_indexed(0..d.count, 0, 0..1);
                    }
                }
            }
            let mut see_through: Vec<(&Draw, usize)> =
                parts.iter().filter(|(d, _)| d.blended).copied().collect();
            see_through.sort_by(|a, b| {
                let far = |d: &Draw| Vec3::from(d.centre).distance_squared(eye);
                far(b.0).total_cmp(&far(a.0))
            });
            for (d, k) in see_through {
                pass.set_pipeline(if d.adding {
                    &self.adding_pipeline
                } else {
                    &self.blend_pipeline
                });
                pass.set_bind_group(0, bind_of(k), &[]);
                pass.set_bind_group(1, &d.bind, &[]);
                pass.set_vertex_buffer(0, d.vertices.slice(d.vertex_bytes.clone()));
                pass.set_index_buffer(
                    d.indices.slice(d.index_bytes.clone()),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..d.count, 0, 0..1);
            }
            pass.set_bind_group(0, &globals_bind, &[]);
            for (additive, count, buffer, bind) in &particle_draws {
                pass.set_pipeline(if *additive {
                    &self.additive_pipeline
                } else {
                    &self.smoke_pipeline
                });
                pass.set_bind_group(1, bind, &[]);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..*count, 0..1);
            }
        }
        if post_now {
            let px = [1.0 / self.width as f32, 1.0 / self.height as f32];
            let exposure = BLUR_EXPOSURE[self.blur as usize];
            let longest = BLUR_LONGEST[self.blur as usize];
            let uniform = PostUniform {
                inv_vp: this_vp.inverse().to_cols_array_2d(),
                prev_vp: prev_vp.to_cols_array_2d(),
                params: [
                    exposure / dt.unwrap_or(1.0).max(0.0001),
                    longest,
                    px[0],
                    px[1],
                ],
            };
            let buf = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("post"),
                    contents: bytemuck::bytes_of(&uniform),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
            let bind = |src: &wgpu::TextureView| {
                self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("post"),
                    layout: &self.post.layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(src),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.post.sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: buf.as_entire_binding(),
                        },
                    ],
                })
            };
            let run = |enc: &mut wgpu::CommandEncoder,
                       pipe: &wgpu::RenderPipeline,
                       src: &wgpu::TextureView,
                       depth: Option<&wgpu::BindGroup>,
                       to: &wgpu::TextureView| {
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("post"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: to,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                pass.set_pipeline(pipe);
                pass.set_bind_group(0, &bind(src), &[]);
                if let Some(d) = depth {
                    pass.set_bind_group(1, d, &[]);
                }
                pass.draw(0..3, 0..1);
            };
            let mut from = &aux_views[0];
            if blur_now {
                let ms = self.samples > 1;
                let depth_bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("post depth"),
                    layout: if ms {
                        &self.post.depth_layout_ms
                    } else {
                        &self.post.depth_layout_1
                    },
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&self.scene_targets.depth),
                    }],
                });
                let to = if fxaa_now { &aux_views[1] } else { &final_view };
                let pipe = if ms {
                    &self.post.blur_ms
                } else {
                    &self.post.blur_1
                };
                run(&mut enc, pipe, from, Some(&depth_bind), to);
                if fxaa_now {
                    from = &aux_views[1];
                }
            }
            if fxaa_now {
                run(&mut enc, &self.post.fxaa, from, None, &final_view);
            }
        }
        let submitting = std::time::Instant::now();
        self.queue.submit([enc.finish()]);
        self.frame_times.submit_ms = submitting.elapsed().as_secs_f32() * 1000.0;
    }

    /// Draw a frame without reading it back, and wait for the GPU to finish it: milliseconds, for measuring what a setting
    /// costs.
    pub fn draw_ms(
        &mut self,
        scene: &Scene,
        textures: &mut dyn FnMut(&str) -> Option<Image>,
        cam: &Camera,
    ) -> f32 {
        let began = std::time::Instant::now();
        self.draw(scene, textures, cam);
        self.device.poll(wgpu::Maintain::Wait);
        began.elapsed().as_secs_f32() * 1000.0
    }

    fn read_back(&mut self) -> Result<Image, String> {
        let mut enc = self.device.create_command_encoder(&Default::default());
        // Copy the picture to a buffer; rows are padded to 256 bytes.
        let padded = (self.width * 4).div_ceil(256) * 256;
        let out = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (padded * self.height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.color,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &out,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([enc.finish()]);
        let slice = out.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let polling = watch("device.poll, for a frame to read back");
        self.device.poll(wgpu::Maintain::Wait);
        drop(polling);
        let _mapped = watch("the frame's buffer to be mapped");
        rx.recv()
            .map_err(|e| e.to_string())?
            .map_err(|e| format!("map: {e}"))?;
        let data = slice.get_mapped_range();
        let mut rgba = Vec::with_capacity((self.width * self.height * 4) as usize);
        for row in data.chunks(padded as usize).take(self.height as usize) {
            rgba.extend_from_slice(&row[..(self.width * 4) as usize]);
        }
        drop(data);
        out.unmap();
        Ok(Image {
            width: self.width,
            height: self.height,
            rgba,
        })
    }

    /// Render `scene` and show it on `presenter`'s window without reading it back.
    pub fn render_to(
        &mut self,
        presenter: &Presenter,
        scene: &Scene,
        textures: &mut dyn FnMut(&str) -> Option<Image>,
        cam: &Camera,
    ) -> Result<(), String> {
        self.frame_times = FrameTimes::default();
        let began = std::time::Instant::now();
        self.draw(scene, textures, cam);
        let drawn = std::time::Instant::now();
        let shown = presenter.show(self);
        self.frame_times.draw_ms = (drawn - began).as_secs_f32() * 1000.0;
        self.frame_times.show_ms = drawn.elapsed().as_secs_f32() * 1000.0;
        self.frame_times.meshes = scene.meshes.len() as u32;
        shown
    }
}

/// Where the last frame's time went on this side of the GPU, for `--frame-log`.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameTimes {
    /// Everything that puts the scene on the GPU, the textures among it.
    pub draw_ms: f32,
    /// Reading and uploading textures not seen before.
    pub textures_ms: f32,
    pub new_textures: u32,
    /// Waiting for the window's next picture, and handing this one over.
    pub show_ms: f32,
    /// Meshes built anew this frame.
    pub meshes: u32,
    /// Of `draw_ms`: turning the frame's scene into buffers and bind groups (`build_draws`), the textures it asked for among them.
    pub build_ms: f32,
    /// Of `draw_ms`: handing the commands to the card (`queue.submit`).
    pub submit_ms: f32,
}

/// **Assumption:** a mesh wider than this with no lightmap is a sky (the Endar Spire's star sphere is 409 m across).
const SKY_WIDTH: f32 = 150.0;

fn mesh_centre(m: &kotor_render::SceneMesh) -> [f32; 3] {
    let n = m.positions.len().max(1) as f32;
    let mut c = [0.0; 3];
    for p in &m.positions {
        for (a, v) in c.iter_mut().zip(p) {
            *a += *v / n;
        }
    }
    c
}

/// The centre of a mesh that is a sky.
fn sky_centre(m: &kotor_render::SceneMesh) -> Option<[f32; 3]> {
    if m.lightmap.is_some() {
        return None;
    }
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for p in &m.positions {
        for (k, v) in p.iter().enumerate() {
            lo[k] = lo[k].min(*v);
            hi[k] = hi[k].max(*v);
        }
    }
    (hi[0] - lo[0] > SKY_WIDTH || hi[1] - lo[1] > SKY_WIDTH)
        .then(|| [0, 1, 2].map(|k| (lo[k] + hi[k]) / 2.0))
}

fn make_color(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("color"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}

fn make_scene_targets(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    samples: u32,
    aux: bool,
) -> SceneTargets {
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let depth_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth"),
        size,
        mip_level_count: 1,
        sample_count: samples,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let depth = depth_tex.create_view(&Default::default());
    let msaa = (samples > 1).then(|| {
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("color samples"),
                size,
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default())
    });
    let image = || {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("post"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    };
    SceneTargets {
        depth,
        msaa,
        aux: if aux {
            vec![image(), image()]
        } else {
            Vec::new()
        },
    }
}

fn upload_image(device: &wgpu::Device, queue: &wgpu::Queue, img: &Image) -> Tex {
    let t = device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: img.width,
                height: img.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        &img.rgba,
    );
    Tex {
        view: t.create_view(&Default::default()),
        look: alpha_look(img),
    }
}

/// Lay a premultiplied overlay over `scene` in software, the same way `Presenter` does on the GPU (for PNGs).
pub fn composite(scene: &mut Image, overlay: &Image) {
    for y in 0..scene.height {
        let sy = (y * overlay.height / scene.height) as usize;
        for x in 0..scene.width {
            let sx = (x * overlay.width / scene.width) as usize;
            let s = &overlay.rgba[(sy * overlay.width as usize + sx) * 4..][..4];
            let a = s[3] as u32;
            let i = ((y * scene.width + x) * 4) as usize;
            for (d, s) in scene.rgba[i..i + 4].iter_mut().zip(s) {
                *d = (*s as u32 + *d as u32 * (255 - a) / 255).min(255) as u8;
            }
        }
    }
}

const BLIT: &str = r#"
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
struct Out { @builtin(position) p: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex
fn vs(@builtin(vertex_index) i: u32) -> Out {
    var o: Out;
    let xy = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    o.p = vec4<f32>(xy * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2<f32>(xy.x, 1.0 - xy.y);
    return o;
}
@fragment
fn fs(i: Out) -> @location(0) vec4<f32> { return textureSample(src, samp, i.uv); }
"#;

/// Puts the renderer's picture on a window surface (a fullscreen blit, stretched to the window).
pub struct Presenter {
    surface: wgpu::Surface<'static>,
    config: std::cell::RefCell<wgpu::SurfaceConfiguration>,
    pipeline: wgpu::RenderPipeline,
    overlay_pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    overlay: std::cell::RefCell<Option<OverlayPicture>>,
}

/// The picture over the scene, kept on the GPU. It changes many times a second in a fight, and most of it stays as it was.
struct OverlayPicture {
    texture: wgpu::Texture,
    bind: wgpu::BindGroup,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

/// The smallest box, `[left, top, width, height]` in pixels, that holds every pixel that differs between two pictures of
/// `width` pixels across.
fn changed_box(old: &[u8], new: &[u8], width: u32) -> Option<[u32; 4]> {
    let row = width as usize * 4;
    let rows = old.chunks_exact(row).zip(new.chunks_exact(row)).enumerate();
    let (mut top, mut bottom, mut left, mut right) = (u32::MAX, 0, u32::MAX, 0);
    for (y, (a, b)) in rows {
        if a == b {
            continue;
        }
        let first = a.iter().zip(b).position(|(p, q)| p != q).unwrap_or(0);
        let last = a.iter().zip(b).rposition(|(p, q)| p != q).unwrap_or(0);
        top = top.min(y as u32);
        bottom = y as u32;
        left = left.min(first as u32 / 4);
        right = right.max(last as u32 / 4);
    }
    (top != u32::MAX).then(|| [left, top, right - left + 1, bottom - top + 1])
}

impl Presenter {
    pub fn new(
        gpu: &GpuRenderer,
        surface: wgpu::Surface<'static>,
        adapter_formats: &[wgpu::TextureFormat],
        size: (u32, u32),
    ) -> Presenter {
        // A non-sRGB format: the renderer's output is already display-encoded.
        let format = adapter_formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(adapter_formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.0.max(1),
            height: size.1.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };
        surface.configure(&gpu.device, &config);
        let module = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("blit"),
                source: wgpu::ShaderSource::Wgsl(BLIT.into()),
            });
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("blit"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let pl = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("blit"),
                bind_group_layouts: &[&layout],
                push_constant_ranges: &[],
            });
        let make = |blend: Option<wgpu::BlendState>| {
            gpu.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("blit"),
                    layout: Some(&pl),
                    vertex: wgpu::VertexState {
                        module: &module,
                        entry_point: Some("vs"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &module,
                        entry_point: Some("fs"),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format,
                            blend,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: Default::default(),
                    depth_stencil: None,
                    multisample: Default::default(),
                    multiview: None,
                    cache: None,
                })
        };
        let pipeline = make(None);
        let overlay_pipeline = make(Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING));
        Presenter {
            surface,
            config: std::cell::RefCell::new(config),
            pipeline,
            overlay_pipeline,
            layout,
            overlay: std::cell::RefCell::new(None),
        }
    }

    fn bind(&self, gpu: &GpuRenderer, view: &wgpu::TextureView) -> wgpu::BindGroup {
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&gpu.sampler),
                },
            ],
        })
    }

    /// Replace the premultiplied picture stretched over the whole scene, or remove it.
    /// As `set_overlay`, when all that has changed is inside a box: left, top, width, height.
    pub fn set_overlay_box(&self, gpu: &GpuRenderer, img: &Image, change: [u32; 4]) {
        let mut slot = self.overlay.borrow_mut();
        let Some(o) = slot
            .as_mut()
            .filter(|o| o.width == img.width && o.height == img.height)
        else {
            drop(slot);
            return self.set_overlay(gpu, Some(img));
        };
        let [left, top, width, height] = change;
        let row = img.width as usize * 4;
        let start = top as usize * row + left as usize * 4;
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &o.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: left,
                    y: top,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &img.rgba[start..],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(img.width * 4),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        for y in top..top + height {
            let from = y as usize * row + left as usize * 4;
            let to = from + width as usize * 4;
            o.pixels[from..to].copy_from_slice(&img.rgba[from..to]);
        }
    }

    pub fn set_overlay(&self, gpu: &GpuRenderer, img: Option<&Image>) {
        let mut slot = self.overlay.borrow_mut();
        let Some(img) = img else {
            *slot = None;
            return;
        };
        if let Some(o) = slot
            .as_mut()
            .filter(|o| o.width == img.width && o.height == img.height)
        {
            if let Some([left, top, width, height]) = changed_box(&o.pixels, &img.rgba, img.width) {
                let row = img.width as usize * 4;
                let start = top as usize * row + left as usize * 4;
                gpu.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &o.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d {
                            x: left,
                            y: top,
                            z: 0,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &img.rgba[start..],
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(img.width * 4),
                        rows_per_image: None,
                    },
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                );
                o.pixels.clone_from(&img.rgba);
            }
            return;
        }
        let texture = gpu.device.create_texture_with_data(
            &gpu.queue,
            &wgpu::TextureDescriptor {
                label: Some("overlay"),
                size: wgpu::Extent3d {
                    width: img.width,
                    height: img.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &img.rgba,
        );
        let bind = self.bind(gpu, &texture.create_view(&Default::default()));
        *slot = Some(OverlayPicture {
            texture,
            bind,
            width: img.width,
            height: img.height,
            pixels: img.rgba.clone(),
        });
    }

    /// The window changed size.
    /// Whether a frame waits for the screen.
    pub fn set_vsync(&self, gpu: &GpuRenderer, on: bool) {
        let mut c = self.config.borrow_mut();
        c.present_mode = if on {
            wgpu::PresentMode::Fifo
        } else {
            wgpu::PresentMode::AutoNoVsync
        };
        self.surface.configure(&gpu.device, &c);
    }

    pub fn resize(&self, gpu: &GpuRenderer, size: (u32, u32)) {
        if size.0 == 0 || size.1 == 0 {
            return;
        }
        let mut c = self.config.borrow_mut();
        c.width = size.0;
        c.height = size.1;
        self.surface.configure(&gpu.device, &c);
    }

    fn show(&self, gpu: &GpuRenderer) -> Result<(), String> {
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                self.surface.configure(&gpu.device, &self.config.borrow());
                return Ok(());
            }
            Err(e) => return Err(format!("surface: {e}")),
        };
        let view = frame.texture.create_view(&Default::default());
        let src = gpu.color.create_view(&Default::default());
        let bind = self.bind(gpu, &src);
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("present"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.draw(0..3, 0..1);
            if let Some(o) = self.overlay.borrow().as_ref().map(|o| &o.bind) {
                pass.set_pipeline(&self.overlay_pipeline);
                pass.set_bind_group(0, o, &[]);
                pass.draw(0..3, 0..1);
            }
        }
        gpu.queue.submit([enc.finish()]);
        frame.present();
        Ok(())
    }
}

#[cfg(test)]
mod aperture_tests {
    use super::{Aperture, Vec3};

    /// A door 8 m wide at the origin that leads along +y.
    fn door() -> Aperture {
        Aperture {
            at: [0.0, 0.0, 0.0],
            out: std::f32::consts::FRAC_PI_2,
            half: 4.0,
        }
    }

    #[test]
    fn an_eye_on_the_near_side_of_a_door_can_see_through_it() {
        let target = Vec3::new(0.0, -5.0, 1.2);
        assert!(door().in_the_picture(Vec3::new(0.0, -8.0, 3.0), target));
        assert!(door().in_the_picture(Vec3::new(97.0, -1.0, 3.0), target));
    }

    #[test]
    fn an_eye_that_went_through_the_opening_is_in_the_other_area() {
        let eye = Vec3::new(1.0, 2.0, 2.0);
        assert!(door().in_the_picture(eye, Vec3::new(0.0, -1.0, 1.2)));
        // The player between the door's plane and the crossing, on the way through it.
        assert!(door().in_the_picture(eye, Vec3::new(0.0, 0.3, 1.2)));
    }

    #[test]
    fn an_eye_past_the_plane_of_a_door_it_did_not_go_through_is_still_here() {
        // Far along the plane, as on Upper City South's plaza, with the player past it or not.
        let eye = Vec3::new(97.0, 6.0, 3.0);
        assert!(!door().in_the_picture(eye, Vec3::new(97.0, 4.0, 1.2)));
        assert!(!door().in_the_picture(eye, Vec3::new(97.0, -1.0, 1.2)));
        // Beside the opening and over the top of it.
        let target = Vec3::new(6.0, -1.0, 1.2);
        assert!(!door().in_the_picture(Vec3::new(6.0, 2.0, 2.0), target));
        let target = Vec3::new(0.0, -0.5, 1.2);
        assert!(!door().in_the_picture(Vec3::new(0.0, 0.5, 9.0), target));
        // Past the door by more than a crossing takes, in line with the opening.
        let target = Vec3::new(0.0, 3.0, 1.2);
        assert!(!door().in_the_picture(Vec3::new(0.0, 6.0, 3.0), target));
    }

    #[test]
    fn what_stands_in_the_view_through_a_doorway_is_seen_through_it_and_what_stands_beside_is_not()
    {
        let eye = [0.0, -4.0, 1.6];
        // Ahead of the doorway, inside the view: this area's own door there would show in the other area.
        assert!(door().sees_through(eye, [0.0, 10.0, 1.0]));
        // Beside the view, short of the doorway, over it: not in the view.
        assert!(!door().sees_through(eye, [30.0, 10.0, 1.0]));
        assert!(!door().sees_through(eye, [0.0, -2.0, 1.0]));
        assert!(!door().sees_through(eye, [0.0, 10.0, 40.0]));
        // From the other side nothing of this area is seen through it.
        assert!(!door().sees_through([0.0, 4.0, 1.6], [0.0, -10.0, 1.0]));
    }
}

#[cfg(test)]
mod overlay_tests {
    use super::changed_box;

    #[test]
    fn only_the_part_of_the_overlay_that_changed_goes_up() {
        let old = vec![0u8; 4 * 4 * 3];
        assert_eq!(changed_box(&old, &old, 4), None);
        let mut new = old.clone();
        // Row 1, pixel 2, its alpha.
        new[(4 + 2) * 4 + 3] = 9;
        assert_eq!(changed_box(&old, &new, 4), Some([2, 1, 1, 1]));
        // And row 2, pixel 0.
        new[8 * 4] = 7;
        assert_eq!(changed_box(&old, &new, 4), Some([0, 1, 3, 2]));
    }
}
