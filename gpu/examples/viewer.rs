//! Play a module in a window. It only ever opens on the monitor named DISPLAY2 and never takes focus by itself:
//! click the window to give it the keyboard.
//!
//!   cargo run --offline --release --manifest-path gpu\Cargo.toml --example viewer -- [options]
//!     --module M      module to load (default end_m01aa)
//!     --installed-mod use a .mod file that a mod left beside the module (default: the original .rim pair)
//!     --orbit D       turn the camera round the player by D degrees a second while idle
//!     --volume V      0 to 1 (default 0.8); --mute opens no sound device
//!     --frames N      close after N frames             --hidden     no visible window
//!     --monitor NAME  suffix of the monitor name to use (default DISPLAY2; DISPLAY1 is refused)
//!
//! Left click: walk, or use what is under the cursor; a click or Space ends a line. 1-9: take a reply.
//! Q/E or the arrow keys: turn the camera. Wheel: zoom. Esc: quit.
use kotor_audio::Audio;
use kotor_audio_out::{decode_any, AudioOut};
use kotor_gpu::stage::{Cue, Stage};
use kotor_gpu::{GpuRenderer, Presenter};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
const TURN_SPEED: f32 = 90.0;

#[derive(Clone, Copy, PartialEq)]
enum Channel {
    Voice,
    Music,
}

struct Sound {
    out: Option<AudioOut>,
    tx: Sender<(u64, Channel, Result<Audio, String>)>,
    rx: Receiver<(u64, Channel, Result<Audio, String>)>,
    /// Decodes finish late: only the newest request per channel plays.
    newest: [u64; 2],
}

impl Sound {
    fn request(&mut self, ch: Channel, path: PathBuf) {
        if self.out.is_none() {
            return;
        }
        let i = ch as usize;
        self.newest[i] += 1;
        let (id, tx) = (self.newest[i], self.tx.clone());
        std::thread::spawn(move || {
            let r = std::fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|d| decode_any(&d));
            let _ = tx.send((id, ch, r));
        });
    }

    fn cue(&mut self, c: Cue) {
        match c {
            Cue::Voice(p) => self.request(Channel::Voice, p),
            Cue::Music(p) => self.request(Channel::Music, p),
            Cue::StopVoice => {
                self.newest[Channel::Voice as usize] += 1;
                if let Some(o) = &mut self.out {
                    o.stop_voice();
                }
            }
            Cue::StopMusic => {
                self.newest[Channel::Music as usize] += 1;
                if let Some(o) = &mut self.out {
                    o.stop_music();
                }
            }
        }
    }

    fn pump(&mut self) {
        while let Ok((id, ch, r)) = self.rx.try_recv() {
            if id != self.newest[ch as usize] {
                continue;
            }
            let Some(o) = &mut self.out else { continue };
            let done = r.and_then(|a| match ch {
                Channel::Voice => o.play_voice(&a),
                Channel::Music => o.play_music(&a),
            });
            if let Err(e) = done {
                eprintln!("sound: {e}");
            }
        }
    }
}

struct App {
    stage: Stage,
    sound: Sound,
    monitor: String,
    hidden: bool,
    max_frames: Option<u32>,
    frames: u32,
    started: std::time::Instant,
    last: std::time::Instant,
    cursor: (f32, f32),
    turning: f32,
    state: Option<State>,
    failed: Option<String>,
}

struct State {
    window: Arc<Window>,
    gpu: GpuRenderer,
    presenter: Presenter,
    static_seen: u64,
    overlay_seen: u64,
}

impl App {
    fn start(&mut self, el: &ActiveEventLoop) -> Result<State, String> {
        let monitors: Vec<_> = el.available_monitors().collect();
        let mut attrs = Window::default_attributes()
            .with_title("kotor-rs")
            .with_inner_size(PhysicalSize::new(WIDTH, HEIGHT))
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
        let gpu = GpuRenderer::with_instance(&instance, Some(&surface), WIDTH, HEIGHT)?;
        let presenter = Presenter::new(&gpu, surface, &formats, (size.width, size.height));
        Ok(State {
            window,
            gpu,
            presenter,
            static_seen: 0,
            overlay_seen: 0,
        })
    }

    fn frame(&mut self, el: &ActiveEventLoop) {
        let Some(st) = &mut self.state else { return };
        let now = std::time::Instant::now();
        let dt = (now - self.last).as_secs_f32();
        self.last = now;
        self.stage.turn = self.turning;
        for c in self.stage.update(dt) {
            self.sound.cue(c);
        }
        self.sound.pump();
        if let Some(s) = self.stage.status() {
            println!("{s}");
        }
        let stage = &mut self.stage;
        if st.static_seen != stage.static_version {
            st.window.set_title(&stage.label());
            st.static_seen = stage.static_version;
            let scene = stage.static_scene().clone();
            st.gpu.set_static_scene(&scene, &mut |n| stage.texture(n));
        }
        if st.overlay_seen != stage.overlay_version {
            st.overlay_seen = stage.overlay_version;
            st.presenter.set_overlay(&st.gpu, stage.overlay());
        }
        let scene = stage.dynamic_scene();
        let cam = stage.camera();
        if let Err(e) = st
            .gpu
            .render_to(&st.presenter, &scene, &mut |n| stage.texture(n), &cam)
        {
            self.failed = Some(e);
            el.exit();
            return;
        }
        self.frames += 1;
        if self.max_frames.is_some_and(|m| self.frames >= m) {
            el.exit();
        }
    }

    fn key(&mut self, el: &ActiveEventLoop, code: KeyCode, pressed: bool) {
        let digit = match code {
            KeyCode::Digit1 | KeyCode::Numpad1 => 1,
            KeyCode::Digit2 | KeyCode::Numpad2 => 2,
            KeyCode::Digit3 | KeyCode::Numpad3 => 3,
            KeyCode::Digit4 | KeyCode::Numpad4 => 4,
            KeyCode::Digit5 | KeyCode::Numpad5 => 5,
            KeyCode::Digit6 | KeyCode::Numpad6 => 6,
            KeyCode::Digit7 | KeyCode::Numpad7 => 7,
            KeyCode::Digit8 | KeyCode::Numpad8 => 8,
            KeyCode::Digit9 | KeyCode::Numpad9 => 9,
            _ => 0,
        };
        match code {
            KeyCode::KeyQ | KeyCode::ArrowLeft => {
                self.turning = if pressed { TURN_SPEED } else { 0.0 };
            }
            KeyCode::KeyE | KeyCode::ArrowRight => {
                self.turning = if pressed { -TURN_SPEED } else { 0.0 };
            }
            KeyCode::Escape if pressed => el.exit(),
            KeyCode::Space | KeyCode::Enter if pressed => self.stage.continue_line(),
            _ if pressed && digit > 0 => self.stage.number_key(digit),
            _ => {}
        }
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
                self.stage.set_view((s.width, s.height));
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as f32, position.y as f32);
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(st) = &self.state {
                    let s = st.window.inner_size();
                    self.stage.click(self.cursor, (s.width, s.height));
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let d = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
                self.stage.distance = (self.stage.distance - d * 0.4).clamp(1.5, 12.0);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.key(el, code, event.state == ElementState::Pressed);
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let args: Vec<String> = std::env::args().collect();
    let opt = |n: &str| {
        args.iter()
            .position(|a| a == n)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let flag = |n: &str| args.iter().any(|a| a == n);
    let monitor = opt("--monitor")
        .unwrap_or_else(|| "DISPLAY2".into())
        .to_ascii_uppercase();
    if monitor.ends_with("DISPLAY1") {
        return Err("DISPLAY1 is off limits".into());
    }
    let module = opt("--module").unwrap_or_else(|| "end_m01aa".into());
    let volume = opt("--volume")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.8);
    let mut stage = Stage::open(&module, !flag("--installed-mod"))?;
    stage.orbit = opt("--orbit").and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let out = if flag("--mute") {
        None
    } else {
        match AudioOut::new(volume.clamp(0.0, 1.0)) {
            Ok(o) => Some(o),
            Err(e) => {
                eprintln!("no sound: {e}");
                None
            }
        }
    };
    let (tx, rx) = channel();
    let mut app = App {
        stage,
        sound: Sound {
            out,
            tx,
            rx,
            newest: [0; 2],
        },
        monitor,
        hidden: flag("--hidden"),
        max_frames: opt("--frames").and_then(|n| n.parse().ok()),
        frames: 0,
        started: std::time::Instant::now(),
        last: std::time::Instant::now(),
        cursor: (0.0, 0.0),
        turning: 0.0,
        state: None,
        failed: None,
    };
    let el = EventLoop::new()?;
    el.set_control_flow(ControlFlow::Poll);
    el.run_app(&mut app)?;
    if let Some(e) = app.failed {
        return Err(e.into());
    }
    let secs = app.started.elapsed().as_secs_f32();
    println!(
        "{} frames in {secs:.1}s ({:.0} fps), {} lines shown, {} world errors",
        app.frames,
        app.frames as f32 / secs,
        app.stage.lines_shown,
        app.stage.game.world.errors.len()
    );
    Ok(())
}
