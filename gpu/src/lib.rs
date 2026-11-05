//! GPU renderer for kotor-render scenes (wgpu), rendering offscreen so no window is needed.
//!
//! Checked against the software renderer by `examples/compare.rs` (same camera, same scene). `Presenter` shows
//! the picture on a window surface for `examples/viewer.rs`.

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
@group(1) @binding(3) var<uniform> params: vec4<f32>; // x = 1 when the mesh has a lightmap

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
    let base = textureSample(diffuse_tex, samp, i.uv).rgb;
    var lit = vec3<f32>(0.55 + 0.45 * abs(dot(normalize(i.normal + vec3<f32>(0.0, 0.0, 1e-4)), g.light.xyz)));
    if (params.x > 0.5) {
        lit = textureSample(lightmap_tex, samp, i.uv2).rgb;
    }
    return vec4<f32>(base * lit, 1.0);
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
}

pub struct GpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    globals_layout: wgpu::BindGroupLayout,
    mesh_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    width: u32,
    height: u32,
    color: wgpu::Texture,
    depth: wgpu::TextureView,
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
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
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
        Ok(GpuRenderer {
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

    fn upload(&self, img: &Image) -> Tex {
        let size = wgpu::Extent3d {
            width: img.width,
            height: img.height,
            depth_or_array_layers: 1,
        };
        let t = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label: None,
                size,
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
        }
    }

    /// Render `scene` from `cam`. `textures` maps a texture name to a decoded image.
    pub fn render(
        &mut self,
        scene: &Scene,
        textures: &mut dyn FnMut(&str) -> Option<Image>,
        cam: &Camera,
    ) -> Result<Image, String> {
        self.draw(scene, textures, cam);
        self.read_back()
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

        let white = self.upload(&Image {
            width: 1,
            height: 1,
            rgba: vec![255, 255, 255, 255],
        });
        let mut cache: HashMap<String, Option<Tex>> = HashMap::new();
        struct Draw {
            vertices: wgpu::Buffer,
            indices: wgpu::Buffer,
            count: u32,
            bind: wgpu::BindGroup,
        }
        let mut draws: Vec<Draw> = Vec::new();
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
            let mut get = |name: &Option<String>| -> bool {
                let Some(n) = name else { return false };
                let key = n.to_ascii_lowercase();
                if !cache.contains_key(&key) {
                    let t = textures(&key).map(|img| self.upload(&img));
                    cache.insert(key.clone(), t);
                }
                cache[&key].is_some()
            };
            let (has_d, has_l) = (get(&m.texture), get(&m.lightmap));
            let pick = |name: &Option<String>, ok: bool| -> wgpu::TextureView {
                match (ok, name) {
                    (true, Some(n)) => cache[&n.to_ascii_lowercase()]
                        .as_ref()
                        .map(|t| t.view.clone())
                        .unwrap_or_else(|| white.view.clone()),
                    _ => white.view.clone(),
                }
            };
            let params = [if has_l { 1.0f32 } else { 0.0 }, 0.0, 0.0, 0.0];
            let params_buf = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::bytes_of(&params),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
            let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.mesh_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&pick(&m.texture, has_d)),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&pick(&m.lightmap, has_l)),
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
            });
        }

        let color_view = self.color.create_view(&Default::default());
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 18.0 / 255.0,
                            g: 20.0 / 255.0,
                            b: 28.0 / 255.0,
                            a: 1.0,
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
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &globals_bind, &[]);
            for d in &draws {
                pass.set_bind_group(1, &d.bind, &[]);
                pass.set_vertex_buffer(0, d.vertices.slice(..));
                pass.set_index_buffer(d.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..d.count, 0, 0..1);
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
        self.device.poll(wgpu::Maintain::Wait);
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
    layout: wgpu::BindGroupLayout,
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
        let pipeline = gpu
            .device
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
                    targets: &[Some(format.into())],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview: None,
                cache: None,
            });
        Presenter {
            surface,
            config: std::cell::RefCell::new(config),
            pipeline,
            layout,
        }
    }

    /// The window changed size.
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
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&src),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&gpu.sampler),
                },
            ],
        });
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
        }
        gpu.queue.submit([enc.finish()]);
        frame.present();
        Ok(())
    }
}
