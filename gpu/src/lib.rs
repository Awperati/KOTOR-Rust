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
};
@group(0) @binding(0) var<uniform> g: Globals;
@group(1) @binding(0) var diffuse_tex: texture_2d<f32>;
@group(1) @binding(1) var lightmap_tex: texture_2d<f32>;
@group(1) @binding(2) var samp: sampler;
@group(1) @binding(3) var<uniform> params: vec4<f32>; // x = 1 with a lightmap, y = alpha below this is cut, z = 1 to blend, w = shade or 0

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
};

@vertex
fn vs_main(v: VsIn) -> VsOut {
    var o: VsOut;
    o.clip = g.view_proj * vec4<f32>(v.pos, 1.0);
    o.normal = v.normal;
    // The game's v axis points up; image rows start at the top.
    o.uv = vec2<f32>(v.uv.x, 1.0 - v.uv.y);
    o.uv2 = vec2<f32>(v.uv2.x, 1.0 - v.uv2.y);
    return o;
}

@fragment
fn fs_main(i: VsOut) -> @location(0) vec4<f32> {
    let tex = textureSample(diffuse_tex, samp, i.uv);
    if (tex.a < params.y) {
        discard;
    }
    let base = tex.rgb;
    var lit = vec3<f32>(0.55 + 0.45 * abs(dot(normalize(i.normal + vec3<f32>(0.0, 0.0, 1e-4)), g.light.xyz)));
    if (params.x > 0.5) {
        lit = textureSample(lightmap_tex, samp, i.uv2).rgb;
    }
    if (params.w > 0.0) {
        lit = lit * params.w;
    }
    return vec4<f32>(base * lit, mix(1.0, tex.a, params.z));
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

/// **Assumption:** the shares that tell these apart are chosen by eye, not read from the game.
fn alpha_look(img: &Image) -> AlphaLook {
    let (mut gone, mut between) = (0usize, 0usize);
    for p in img.rgba.chunks(4) {
        match p[3] {
            0..=19 => gone += 1,
            20..=234 => between += 1,
            _ => {}
        }
    }
    let n = (img.rgba.len() / 4).max(1);
    if (gone + between) * 200 < n {
        AlphaLook::Opaque
    } else if between * 3 > gone + between {
        AlphaLook::Blend
    } else {
        AlphaLook::Cutout
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
    smoke_pipeline: wgpu::RenderPipeline,
    particles: Vec<effects::ParticleBatch>,
    blend_pipeline: wgpu::RenderPipeline,
    transparency: bool,
    globals_layout: wgpu::BindGroupLayout,
    mesh_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    width: u32,
    height: u32,
    color: wgpu::Texture,
    depth: wgpu::TextureView,
    white: Tex,
    tex_cache: HashMap<String, Option<Tex>>,
    static_draws: Vec<Draw>,
    /// Draw only what each frame passes in: a figure on a screen.
    figure_only: bool,
}

struct Draw {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    count: u32,
    bind: wgpu::BindGroup,
    blended: bool,
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
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .map_err(|e| format!("request_device: {e}"))?;

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
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("kotor"),
            bind_group_layouts: &[&globals_layout, &mesh_layout],
            push_constant_ranges: &[],
        });
        let attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x2];
        let make =
            |depth_write: bool, compare: wgpu::CompareFunction, blend: Option<wgpu::BlendState>| {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("kotor"),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: &module,
                        entry_point: Some("vs_main"),
                        compilation_options: Default::default(),
                        buffers: &[wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<Vertex>() as u64,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &attrs,
                        }],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &module,
                        entry_point: Some("fs_main"),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba8Unorm,
                            blend,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState {
                        cull_mode: None,
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: wgpu::TextureFormat::Depth32Float,
                        depth_write_enabled: depth_write,
                        depth_compare: compare,
                        stencil: Default::default(),
                        bias: Default::default(),
                    }),
                    multisample: Default::default(),
                    multiview: None,
                    cache: None,
                })
            };
        let pipeline = make(true, wgpu::CompareFunction::Less, None);
        let sky_pipeline = make(false, wgpu::CompareFunction::Always, None);
        let blend_pipeline = make(
            false,
            wgpu::CompareFunction::Less,
            Some(wgpu::BlendState::ALPHA_BLENDING),
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let (color, depth) = make_targets(&device, width, height);
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
        let particle_pipes = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("particle"),
            bind_group_layouts: &[&globals_layout, &particle_layout],
            push_constant_ranges: &[],
        });
        let particle_attrs =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x4];
        let particle_pipeline = |blend: wgpu::BlendState| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("particle"),
                layout: Some(&particle_pipes),
                vertex: wgpu::VertexState {
                    module: &particle_module,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<effects::ParticleVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &particle_attrs,
                    }],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &particle_module,
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
                multisample: Default::default(),
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
            smoke_pipeline,
            particles: Vec::new(),
            sky_pipeline,
            sky_at_infinity: true,
            blend_pipeline,
            transparency: true,
            white,
            tex_cache: HashMap::new(),
            static_draws: Vec::new(),
            figure_only: false,
            device,
            queue,
            pipeline,
            globals_layout,
            mesh_layout,
            sampler,
            width,
            height,
            color,
            depth,
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
        (self.color, self.depth) = make_targets(&self.device, width, height);
        (self.width, self.height) = (width, height);
    }

    /// By default a sky is drawn first, around the camera, with no depth, so it is never in front of anything and
    /// the camera can never leave it. Off, it is an ordinary mesh, as in the software renderer.
    pub fn set_sky_at_infinity(&mut self, on: bool) {
        self.sky_at_infinity = on;
    }

    /// By default alpha cuts holes or blends. Off, everything is solid, as in the software renderer.
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
    }

    /// The view of a named texture, decoding and uploading it the first time.
    fn texture_view(
        &mut self,
        name: &Option<String>,
        textures: &mut dyn FnMut(&str) -> Option<Image>,
    ) -> Option<wgpu::TextureView> {
        let key = name.as_ref()?.to_ascii_lowercase();
        if !self.tex_cache.contains_key(&key) {
            let t = textures(&key).map(|img| upload_image(&self.device, &self.queue, &img));
            self.tex_cache.insert(key.clone(), t);
        }
        self.tex_cache[&key].as_ref().map(|t| t.view.clone())
    }

    fn build_draws(
        &mut self,
        scene: &Scene,
        textures: &mut dyn FnMut(&str) -> Option<Image>,
    ) -> Vec<Draw> {
        let mut draws = Vec::new();
        for m in &scene.meshes {
            if m.positions.is_empty() || m.triangles.is_empty() {
                continue;
            }
            let verts: Vec<Vertex> = (0..m.positions.len())
                .map(|i| Vertex {
                    pos: m.positions[i],
                    normal: m.normals.get(i).copied().unwrap_or([0.0, 0.0, 1.0]),
                    uv: m.uvs.get(i).copied().unwrap_or([0.0; 2]),
                    uv2: m.uvs2.get(i).copied().unwrap_or([0.0; 2]),
                })
                .collect();
            let indices: Vec<u32> = m.triangles.iter().flatten().copied().collect();
            let diffuse = self.texture_view(&m.texture, textures);
            let lightmap = self.texture_view(&m.lightmap, textures);
            let look = match m.texture.as_ref().map(|n| n.to_ascii_lowercase()) {
                Some(n) if self.transparency => self.tex_cache[&n]
                    .as_ref()
                    .map_or(AlphaLook::Opaque, |t| t.look),
                _ => AlphaLook::Opaque,
            };
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
            ];
            let params_buf = self
                .device
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
            draws.push(Draw {
                vertices: self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: None,
                        contents: bytemuck::cast_slice(&verts),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
                indices: self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: None,
                        contents: bytemuck::cast_slice(&indices),
                        usage: wgpu::BufferUsages::INDEX,
                    }),
                count: indices.len() as u32,
                bind,
                blended: look == AlphaLook::Blend,
                centre: mesh_centre(m),
                sky: sky_centre(m),
            });
        }
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
        let globals = Globals {
            view_proj: (proj * view).to_cols_array_2d(),
            light: [0.3, -0.5, 0.8, 0.0],
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
        let draws = self.build_draws(scene, textures);
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
        let color_view = self.color.create_view(&Default::default());
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    resolve_target: None,
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
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
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
                pass.set_vertex_buffer(0, d.vertices.slice(..));
                pass.set_index_buffer(d.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..d.count, 0, 0..1);
            }
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &globals_bind, &[]);
            for d in self
                .statics()
                .iter()
                .chain(&draws)
                .filter(|d| !d.blended && (!self.sky_at_infinity || d.sky.is_none()))
            {
                pass.set_bind_group(1, &d.bind, &[]);
                pass.set_vertex_buffer(0, d.vertices.slice(..));
                pass.set_index_buffer(d.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..d.count, 0, 0..1);
            }
            let mut see_through: Vec<&Draw> = self
                .statics()
                .iter()
                .chain(&draws)
                .filter(|d| d.blended && (!self.sky_at_infinity || d.sky.is_none()))
                .collect();
            see_through.sort_by(|a, b| {
                let far = |d: &Draw| Vec3::from(d.centre).distance_squared(eye);
                far(b).total_cmp(&far(a))
            });
            pass.set_pipeline(&self.blend_pipeline);
            for d in see_through {
                pass.set_bind_group(1, &d.bind, &[]);
                pass.set_vertex_buffer(0, d.vertices.slice(..));
                pass.set_index_buffer(d.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..d.count, 0, 0..1);
            }
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
        self.queue.submit([enc.finish()]);
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
        self.draw(scene, textures, cam);
        presenter.show(self)
    }
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

fn make_targets(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let color = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("color"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let depth = device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default());
    (color, depth)
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
    overlay: std::cell::RefCell<Option<wgpu::BindGroup>>,
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
    pub fn set_overlay(&self, gpu: &GpuRenderer, img: Option<&Image>) {
        let bind = img.map(|i| {
            let t = upload_image(&gpu.device, &gpu.queue, i);
            self.bind(gpu, &t.view)
        });
        *self.overlay.borrow_mut() = bind;
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
            if let Some(o) = self.overlay.borrow().as_ref() {
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
