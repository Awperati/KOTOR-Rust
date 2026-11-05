//! A window that slowly orbits an area. It only ever opens on the monitor named DISPLAY2, never takes focus,
//! and needs no input.
//!
//!   cargo run --offline --release --manifest-path gpu\Cargo.toml --example viewer -- [options]
//!     --area A        area name (default m01aa)        --module M   module to load (default end_m01aa)
//!     --frames N      close after N frames             --hidden     no visible window (renders and presents anyway)
//!     --monitor NAME  suffix of the monitor name to use (default DISPLAY2; DISPLAY1 is refused)
use kotor_archive::Install;
use kotor_gpu::{GpuRenderer, Presenter};
use kotor_image::Image;
use kotor_render::{Camera, Scene};
use std::collections::HashMap;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

struct App {
    inst: Install,
    scene: Scene,
    textures: HashMap<String, Option<Image>>,
    monitor: String,
    hidden: bool,
    max_frames: Option<u32>,
    frames: u32,
    started: std::time::Instant,
    state: Option<State>,
    failed: Option<String>,
}

struct State {
    window: Arc<Window>,
    gpu: GpuRenderer,
    presenter: Presenter,
}

impl App {
    fn start(&mut self, el: &ActiveEventLoop) -> Result<State, String> {
        let monitors: Vec<_> = el.available_monitors().collect();
        let names: Vec<String> = monitors
            .iter()
            .map(|m| m.name().unwrap_or_default())
            .collect();
        println!("monitors: {names:?}");
        let mut attrs = Window::default_attributes()
            .with_title("kotor-rs viewer")
            .with_inner_size(PhysicalSize::new(960, 540))
            .with_active(false)
            .with_visible(!self.hidden);
        if !self.hidden {
            let m = monitors
                .iter()
                .find(|m| {
                    m.name()
                        .is_some_and(|n| n.to_ascii_uppercase().ends_with(&self.monitor))
                })
                .ok_or_else(|| {
                    format!("no monitor named *{}: not opening a window", self.monitor)
                })?;
            let p = m.position();
            attrs = attrs.with_position(PhysicalPosition::new(p.x + 60, p.y + 60));
        }
        let window = Arc::new(el.create_window(attrs).map_err(|e| e.to_string())?);
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| e.to_string())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .ok_or("no adapter for the window")?;
        let formats = surface.get_capabilities(&adapter).formats;
        let size = window.inner_size();
        let gpu = GpuRenderer::with_instance(&instance, Some(&surface), 960, 540)?;
        let presenter = Presenter::new(&gpu, surface, &formats, (size.width, size.height));
        Ok(State {
            window,
            gpu,
            presenter,
        })
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        match self.start(el) {
            Ok(s) => self.state = Some(s),
            Err(e) => {
                self.failed = Some(e);
                el.exit();
            }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(s) => {
                if let Some(st) = &self.state {
                    st.presenter.resize(&st.gpu, (s.width, s.height));
                }
            }
            WindowEvent::RedrawRequested => self.frame(el),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if self.hidden {
            // A hidden window gets no redraw events, so draw from here.
            self.frame(el);
        } else if let Some(st) = &self.state {
            st.window.request_redraw();
        }
    }
}

impl App {
    fn frame(&mut self, el: &ActiveEventLoop) {
        {
            {
                let Some(st) = &mut self.state else { return };
                let t = self.started.elapsed().as_secs_f32() * 0.25;
                let mid = [0, 1, 2].map(|k| (self.scene.min[k] + self.scene.max[k]) / 2.0);
                let r = (self.scene.max[0] - self.scene.min[0])
                    .max(self.scene.max[1] - self.scene.min[1])
                    * 0.45;
                let cam = Camera {
                    eye: [mid[0] + r * t.cos(), mid[1] + r * t.sin(), mid[2] + r * 0.5],
                    target: mid,
                    fov: 70.0,
                };
                let (inst, cache) = (&mut self.inst, &mut self.textures);
                let mut tex = |n: &str| {
                    cache
                        .entry(n.to_string())
                        .or_insert_with(|| kotor_render::load_texture(inst, n))
                        .clone()
                };
                if let Err(e) = st.gpu.render_to(&st.presenter, &self.scene, &mut tex, &cam) {
                    self.failed = Some(e);
                    el.exit();
                    return;
                }
                self.frames += 1;
                if self.frames == 1 {
                    println!(
                        "first frame after {:.2}s",
                        self.started.elapsed().as_secs_f32()
                    );
                }
                if self.max_frames.is_some_and(|m| self.frames >= m) {
                    el.exit();
                }
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let args: Vec<String> = std::env::args().collect();
    let opt = |n: &str| {
        args.iter()
            .position(|a| a == n)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let monitor = opt("--monitor")
        .unwrap_or_else(|| "DISPLAY2".into())
        .to_ascii_uppercase();
    if monitor.ends_with("DISPLAY1") {
        return Err("DISPLAY1 is off limits".into());
    }
    let area = opt("--area").unwrap_or_else(|| "m01aa".into());
    let module = opt("--module").unwrap_or_else(|| "end_m01aa".into());
    let dir = kotor_core::install_dir().ok_or("no KotOR install found")?;
    let mut inst = Install::open(dir)?;
    inst.load_module(&module)?;
    inst.load_texture_packs()?;
    let mut scene = kotor_render::load_area(&mut inst, &area)?;
    kotor_render::add_area_objects(&mut scene, &mut inst, &area)?;
    println!(
        "{} meshes, {} triangles",
        scene.meshes.len(),
        scene.triangle_count()
    );
    let mut app = App {
        inst,
        scene,
        textures: HashMap::new(),
        monitor,
        hidden: args.iter().any(|a| a == "--hidden"),
        max_frames: opt("--frames").and_then(|n| n.parse().ok()),
        frames: 0,
        started: std::time::Instant::now(),
        state: None,
        failed: None,
    };
    let el = EventLoop::new()?;
    el.set_control_flow(ControlFlow::Poll);
    el.run_app(&mut app)?;
    if let Some(e) = app.failed {
        return Err(e.into());
    }
    println!(
        "{} frames in {:.1}s",
        app.frames,
        app.started.elapsed().as_secs_f32()
    );
    Ok(())
}
