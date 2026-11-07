//! Play a module in a window. It only ever opens on the monitor named DISPLAY2 and never takes focus by itself:
//! click the window to give it the keyboard.
//!
//!   cargo run --offline --release --manifest-path gpu\Cargo.toml --example viewer -- [options]
//!     --size WxH      window and picture size (default 1280x720; anything up to 3840x2160 and past it)
//!     --fullscreen    a borderless window over the whole monitor, which does not take the focus
//!     --no-hd         do not use upscaled interface pictures
//!     --module M      where New Game starts (default end_m01aa)
//!     --skip-menu     start straight in the game, not on the main menu
//!     --installed-mod use a .mod file that a mod left beside the module (default: the original .rim pair)
//!     --orbit D       turn the camera round the player by D degrees a second while idle
//!     --volume V      0 to 1 (default 0.8); --mute opens no sound device
//!     --frames N      close after N frames             --hidden     no visible window
//!     --state-log     print the game's state every few frames, for a test driver
//!     --near TAG      start beside the object with this tag, for a test scene, when a game starts
//!     --foe M         an enemy stands this many metres ahead of the player, when a game starts, for a test driver
//!     --loot T*N,...  the object of --near holds these too, this many of each template, for a test driver
//!     --place X,Y,Z,H start at this spot facing this many radians, with the camera behind, when a game starts, for a test driver
//!     --no-chargen    New Game starts at once, with the preset character
//!     --no-early-force New games play by the game's own rules, whatever Settings says
//!     --early-force   With --autopilot, new games have the early Force rule too, for a test driver
//!     --power N       give the player this power of `spells.2da` and the Force for it when a game starts, for a test driver
//!     --give NAMES    give the player these items, split by commas, when a game starts, for a test driver. One that ends
//!                     in ! is put on
//!     --gold N        give the player these credits when a game starts, for a test driver
//!     --store TAG     open this store when a game starts, as its script would, for a test driver
//!     --global NAME=N set these global numbers, split by commas, when a game starts, for a test driver
//!     --pazaak MAX    start a game of Pazaak with this most to wager when a game starts, for a test driver
//!     --alignment N   the player's alignment when a game starts, for a test driver
//!     --auto-aim      the turret points itself at the next fighter, for a test driver
//!     --grenadier     when the first fight starts, the nearest enemy has grenades and throws them, for a test driver
//!     --xp N          give the player this much experience when a game starts, for a test driver
//!     --appearance N  the player has this row of `appearance.2da` when a game starts, for a test driver
//!     --underwear ID  the player wears this underwear variant (a folder under the mods folder), for a test driver
//!     --original-underwear  the game's own underwear style, whatever Settings says, for a test driver
//!     --pilot-until W with --autopilot: hand over to the player when an enemy is seen (`fight`), or one whose death
//!                     gives a level (`level`), for a test driver
//!     --monitor NAME  suffix of the monitor name to use (default DISPLAY2; DISPLAY1 is refused)
//!     --frame-log F   write where each frame's time went to this file, as comma-separated lines
//!
//! W A S D: run, relative to the camera (Shift walks). Left Ctrl: roll, in Modern movement. Right button drag or the arrow keys: turn the camera.
//! Left click: walk, use what is under the cursor, or pick a reply. E: use the nearest thing. 1-9: take a reply.
//! Tab: the next party member takes over. Space: jump, end a line, or close a container. X: pause. Wheel: zoom. Esc: back out of a screen,
//! or pause. I: inventory. U: equipment. C: character.
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

/// Degrees a second. The reference video's camera turns about 85 degrees in 0.7 s, see `docs/REFERENCE.md`.
const TURN_SPEED: f32 = 120.0;

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
            Cue::Effect(bytes) => {
                if let (Some(o), Ok(a)) = (&mut self.out, decode_any(&bytes)) {
                    if let Err(e) = o.play_effect(&a) {
                        eprintln!("sound: {e}");
                    }
                }
            }
            Cue::FaintEffect(bytes, gain) => {
                if let (Some(o), Ok(mut a)) = (&mut self.out, decode_any(&bytes)) {
                    for sample in &mut a.samples {
                        *sample = (*sample as f32 * gain) as i16;
                    }
                    if let Err(e) = o.play_effect(&a) {
                        eprintln!("sound: {e}");
                    }
                }
            }
            Cue::Levels(music, voice, effects) => {
                if let Some(o) = &mut self.out {
                    o.set_levels(music, voice, effects);
                }
            }
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

/// Force points `--power` gives, enough for any power.
const DRIVER_FORCE: i32 = 100;

struct App {
    stage: Stage,
    sound: Sound,
    monitor: String,
    size: (u32, u32),
    fullscreen: bool,
    hidden: bool,
    max_frames: Option<u32>,
    frame_log: Option<std::io::BufWriter<std::fs::File>>,
    state_log: bool,
    pilot_seen: usize,
    /// Draw only every so many frames, so that more of the time goes to the game.
    render_every: u32,
    /// Stop when this many fixed steps have run.
    max_steps: Option<u64>,
    /// What a test driver asked for once a game has started, from the menu or at once.
    near: Option<String>,
    loot: Option<String>,
    foe: Option<f32>,
    place: Option<[f32; 4]>,
    xp: Option<i32>,
    appearance: Option<i32>,
    underwear: Option<String>,
    power: Option<i32>,
    give: Option<String>,
    gold: Option<i32>,
    store: Option<String>,
    global: Option<String>,
    pazaak: Option<i32>,
    alignment: Option<i32>,
    grenadier: bool,
    turret_cursor: bool,
    /// The turret has seen where the cursor is: till then, how far it has come means nothing.
    turret_cursor_known: bool,
    pilot_until: Option<String>,
    auto_pause: bool,
    frames: u32,
    started: std::time::Instant,
    last: std::time::Instant,
    cursor: (f32, f32),
    turning: f32,
    keys: [bool; 5],
    looking: bool,
    state: Option<State>,
    failed: Option<String>,
}

struct State {
    window: Arc<Window>,
    gpu: GpuRenderer,
    presenter: Presenter,
    static_seen: u64,
    portal_seen: u64,
    overlay_seen: u64,
}

impl App {
    fn start(&mut self, el: &ActiveEventLoop) -> Result<State, String> {
        let monitors: Vec<_> = el.available_monitors().collect();
        let mut attrs = Window::default_attributes()
            .with_title("kotor-rs")
            .with_inner_size(PhysicalSize::new(self.size.0, self.size.1))
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
            if self.fullscreen {
                // A borderless window the size of the monitor. Fullscreen proper would take the focus.
                attrs = attrs
                    .with_decorations(false)
                    .with_position(p)
                    .with_inner_size(m.size());
            }
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
        let gpu = GpuRenderer::with_instance(
            &instance,
            Some(&surface),
            size.width.max(1),
            size.height.max(1),
        )?;
        let presenter = Presenter::new(&gpu, surface, &formats, (size.width, size.height));
        self.stage.set_view((size.width, size.height));
        Ok(State {
            window,
            gpu,
            presenter,
            static_seen: 0,
            portal_seen: 0,
            overlay_seen: 0,
        })
    }

    fn frame(&mut self, el: &ActiveEventLoop) {
        let Some(st) = &mut self.state else { return };
        let now = std::time::Instant::now();
        let dt = (now - self.last).as_secs_f32();
        self.last = now;
        let ms = |from: std::time::Instant| from.elapsed().as_secs_f32() * 1000.0;
        if self.stage.in_game() {
            if let Some(tag) = self.near.take() {
                if !self.stage.stand_near(&tag) {
                    eprintln!("no floor beside {tag}");
                }
                if let Some(loot) = self.loot.take() {
                    let w = &mut self.stage.game.world;
                    let holder = w.by_tag(&tag).first().copied();
                    for (n, entry) in loot.split(',').enumerate() {
                        let (template, count) = entry.split_once('*').unwrap_or((entry, "1"));
                        let item = w.spawn_item(template);
                        if let Some(o) = w.object_mut(item) {
                            o.stack = count.parse().unwrap_or(1);
                        }
                        if let Some(o) = holder.and_then(|h| w.object_mut(h)) {
                            o.inventory.insert(n, item);
                        }
                    }
                }
            }
            if let Some([x, y, z, heading]) = self.place.take() {
                self.stage.place_player([x, y, z], heading);
            }
            if let Some(gold) = self.gold.take() {
                self.stage.game.world.gold += gold;
            }
            for global in self.global.take().unwrap_or_default().split(',') {
                if let Some((name, value)) = global.split_once('=') {
                    if let Ok(value) = value.parse() {
                        self.stage
                            .game
                            .world
                            .global_numbers
                            .insert(name.to_ascii_uppercase(), value);
                    }
                }
            }
            // The turret's sky asks for its number as the area is entered.
            if self.stage.game.world.minigame.is_some() {
                self.stage.game.world.enter_area();
            }
            if let Some(alignment) = self.alignment.take() {
                let w = &mut self.stage.game.world;
                let pc = w.pc;
                if let Some(o) = w.object_mut(pc) {
                    o.alignment = alignment;
                }
            }
            if let Some(max) = self.pazaak.take() {
                self.stage.game.world.start_pazaak(1, "", max, None);
            }
            if let Some(metres) = self.foe.take() {
                let w = &mut self.stage.game.world;
                let (at, heading) = (w.pc_mover.pos, w.pc_mover.heading);
                let spot = [
                    at[0] + heading.cos() * metres,
                    at[1] + heading.sin() * metres,
                    at[2],
                ];
                let foe = w.spawn_creature("n_calonord", spot);
                if let Some(o) = w.object_mut(foe) {
                    o.name = "Test Soldier".to_string();
                    o.faction = 1;
                    (o.hp, o.max_hp) = (FOE_HIT_POINTS, FOE_HIT_POINTS);
                }
                // The driver is looking at the slots, and neither is to die while it does.
                let pc = w.pc;
                if let Some(o) = w.object_mut(pc) {
                    (o.hp, o.max_hp) = (FOE_HIT_POINTS, FOE_HIT_POINTS);
                }
            }
            if let Some(tag) = self.store.take() {
                let w = &mut self.stage.game.world;
                if let Some(store) = w.by_tag(&tag).first().copied() {
                    w.open_store(store, 0, 0);
                }
            }
            if let Some(templates) = self.give.take() {
                let w = &mut self.stage.game.world;
                for template in templates.split(',') {
                    let (pc, item) = (w.pc, w.spawn_item(template.trim_end_matches('!')));
                    w.carry(pc, item);
                    if template.ends_with('!') {
                        let _ = w.equip(pc, item, None);
                    }
                }
            }
            if let Some(power) = self.power.take() {
                let w = &mut self.stage.game.world;
                let pc = w.pc;
                if let Some(o) = w.object_mut(pc) {
                    o.spells.insert(power);
                    o.max_force = o.max_force.max(DRIVER_FORCE);
                    o.force = o.max_force;
                }
            }
            if let Some(id) = self.underwear.take() {
                self.stage.game.world.set_player_underwear(&id);
            }
            if let Some(row) = self.appearance.take() {
                let w = &mut self.stage.game.world;
                let pc = w.pc;
                if let Some(o) = w.object_mut(pc) {
                    o.appearance = row;
                }
            }
            if let Some(xp) = self.xp.take() {
                let w = &mut self.stage.game.world;
                let pc = w.pc;
                w.grant_xp(pc, xp);
            }
        }
        if let Some(until) = &self.pilot_until {
            if self.stage.pilot_arrived() {
                self.stage.take_over_from_pilot(self.auto_pause);
                self.stage.keep_player_standing(until == "level");
                if self.grenadier {
                    self.grenadier = false;
                    self.stage.arm_nearest_enemy();
                }
                if until == "level" {
                    self.stage.one_kill_short_of_a_level();
                }
                self.pilot_until = None;
            }
        }
        self.stage.turn = self.turning;
        self.stage.set_looking(self.looking);
        if self.turret_cursor != self.stage.turret_on() {
            self.turret_cursor = !self.turret_cursor;
            self.turret_cursor_known = false;
            st.window.set_cursor_visible(!self.turret_cursor);
        }
        let axis = |a: bool, b: bool| a as i32 as f32 - b as i32 as f32;
        self.stage.steer = (
            axis(self.keys[0], self.keys[1]),
            axis(self.keys[3], self.keys[2]),
            self.keys[4],
        );
        let updating = std::time::Instant::now();
        let cues = self.stage.update(dt);
        let update_ms = ms(updating);
        for c in cues {
            // Sound runs on the wall clock and cannot keep up with a faster game.
            if self.stage.speed <= 1.0 {
                self.sound.cue(c);
            }
        }
        if let Some(p) = self.stage.pilot.as_ref() {
            for l in &p.log[self.pilot_seen..] {
                println!("pilot: {l}");
            }
            self.pilot_seen = p.log.len();
        }
        self.sound.pump();
        if let Some(on) = self.stage.take_wanted_vsync() {
            st.presenter.set_vsync(&st.gpu, on);
        }
        if let Some((w, h)) = self.stage.take_wanted_size() {
            let _ = st.window.request_inner_size(PhysicalSize::new(w, h));
        }
        if let Some(s) = self.stage.status() {
            println!("{s}");
        }
        let stage = &mut self.stage;
        let uploading = std::time::Instant::now();
        // What has joined has its textures put on the GPU a few a frame, so that it goes into the picture without a stall.
        let warm = stage.take_warm(3);
        if !warm.is_empty() {
            st.gpu.warm_textures(&warm, &mut |n| stage.texture_soon(n));
        }
        if st.static_seen != stage.static_version {
            // What the launcher has to say, then what the viewer always says.
            let note = std::env::var("KOTOR_BUILD_NOTE").unwrap_or_default();
            let label = stage.label();
            st.window.set_title(&if note.is_empty() {
                label
            } else {
                format!("{note} | {label}")
            });
            st.static_seen = stage.static_version;
            let scene = stage.static_scene().clone();
            for name in stage.textures_changed(true) {
                st.gpu.forget_texture(&name);
            }
            st.gpu.set_static_scene(&scene, &mut |n| stage.texture(n));
        }
        for name in stage.textures_changed(false) {
            st.gpu.forget_texture(&name);
        }
        if st.portal_seen != stage.portal_version() {
            st.portal_seen = stage.portal_version();
            for name in stage.textures_changed(true) {
                st.gpu.forget_texture(&name);
            }
            let other = stage.portal().map(|(sc, a)| (sc.clone(), a));
            st.gpu
                .set_portal(other.as_ref().map(|(sc, a)| (sc, *a)), &mut |n| {
                    stage.texture(n)
                });
        }
        if st.overlay_seen != stage.overlay_version {
            let change = stage.overlay_change_since(st.overlay_seen);
            st.overlay_seen = stage.overlay_version;
            match (change, stage.overlay()) {
                (Some(change), Some(picture)) => {
                    st.presenter.set_overlay_box(&st.gpu, picture, change)
                }
                (_, picture) => st.presenter.set_overlay(&st.gpu, picture),
            }
        }
        let upload_ms = ms(uploading);
        let building = std::time::Instant::now();
        let mut scene_ms = 0.0;
        if self.frames.is_multiple_of(self.render_every) {
            let (scene, cam, figure) = stage.frame_scene();
            st.gpu.set_figure_only(figure);
            // Idle bodies seen through a doorway move at 20 frames a second: nobody can tell, and each costs a rebuild.
            if !figure && self.frames.is_multiple_of(3) && stage.portal().is_some() {
                let moving = stage.portal_figures();
                st.gpu.set_portal_moving(&moving, &mut |n| stage.texture(n));
            }
            // A turret's sky has its ships and planets far off in it, each too wide to be anything else's sky.
            st.gpu
                .set_sky_at_infinity(stage.game.world.minigame.is_none());
            st.gpu.set_particles(stage.frame_particles());
            st.gpu.set_cel_shading(stage.cel_shading());
            scene_ms = ms(building);
            if let Err(e) =
                st.gpu
                    .render_to(&st.presenter, &scene, &mut |n| stage.texture_soon(n), &cam)
            {
                self.failed = Some(e);
                el.exit();
                return;
            }
        }
        self.frames += 1;
        if let Some(log) = &mut self.frame_log {
            use std::io::Write;
            let t = st.gpu.frame_times;
            let fight = self
                .stage
                .game
                .world
                .combat
                .in_combat(self.stage.game.world.pc);
            let _ = writeln!(
                log,
                "{},{:.3},{update_ms:.3},{:.3},{},{upload_ms:.3},{scene_ms:.3},{:.3},{:.3},{},{:.3},{},{},{}",
                self.frames,
                dt * 1000.0,
                self.stage.overlay_ms,
                self.stage.hud_drawn,
                t.draw_ms,
                t.textures_ms,
                t.new_textures,
                t.show_ms,
                t.meshes,
                u8::from(fight),
                self.stage.game.world.module,
            );
        }
        if self.state_log && self.frames.is_multiple_of(6) {
            println!("{}", self.stage.state_line());
        }
        if self.stage.wants_exit()
            || self.max_frames.is_some_and(|m| self.frames >= m)
            || self.max_steps.is_some_and(|m| self.stage.game.steps >= m)
        {
            el.exit();
        }
    }

    /// The buttons, as the Mouse Settings page has them.
    fn look_button(&self) -> MouseButton {
        if self.stage.settings().reverse_buttons {
            MouseButton::Left
        } else {
            MouseButton::Right
        }
    }

    fn click_button(&self) -> MouseButton {
        if self.stage.settings().reverse_buttons {
            MouseButton::Right
        } else {
            MouseButton::Left
        }
    }

    fn key(&mut self, code: KeyCode, pressed: bool) {
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
            KeyCode::ControlLeft | KeyCode::ControlRight => self.stage.set_ctrl(pressed),
            KeyCode::ShiftLeft | KeyCode::ShiftRight => self.stage.set_shift(pressed),
            _ => {}
        }
        // The Key Mapping page takes a key first, then what it says each key does.
        let name = format!("{code:?}");
        if pressed && self.stage.capture_key(&name) {
            return;
        }
        if code == KeyCode::F6 {
            if pressed {
                self.stage.toggle_nameplates();
            }
            return;
        }
        if code == KeyCode::F7 {
            if pressed {
                self.stage.toggle_cel_shading();
            }
            return;
        }
        match self.stage.action_for_key(&name) {
            Some("Jump") if self.stage.turret_on() => return self.stage.turret_fire(pressed),
            Some("MoveForward") => return self.keys[0] = pressed,
            Some("MoveBack") => return self.keys[1] = pressed,
            Some("MoveLeft") => return self.keys[2] = pressed,
            Some("MoveRight") => return self.keys[3] = pressed,
            Some("TurnLeft") => return self.turning = if pressed { TURN_SPEED } else { 0.0 },
            Some("TurnRight") => return self.turning = if pressed { -TURN_SPEED } else { 0.0 },
            Some(action) => {
                if pressed {
                    match action {
                        "Use" => self.stage.interact(),
                        "ChangeLeader" => {
                            self.stage.next_leader();
                        }
                        "Equip" => self.stage.open_equip(),
                        "Inventory" => self.stage.open_inventory(),
                        "Character" => self.stage.open_character(),
                        "Abilities" => self.stage.open_abilities(),
                        "Messages" => self.stage.open_messages(),
                        "Quests" => self.stage.open_journal(),
                        "Map" => self.stage.open_map(),
                        "Options" => self.stage.escape(),
                        "Jump" => self.stage.jump_key(),
                        "Pause" => self.stage.pause_key(),
                        "CancelCombat" => self.stage.hud_disengage(),
                        "Pickup" => self.stage.pickup(),
                        "SoloMode" => self.stage.solo_key(),
                        "Roll" => self.stage.roll_key(),
                        _ => {}
                    }
                }
                return;
            }
            None => {}
        }
        match code {
            KeyCode::ShiftLeft | KeyCode::ShiftRight => self.keys[4] = pressed,
            KeyCode::Escape if pressed => self.stage.escape(),
            KeyCode::Enter if pressed => self.stage.continue_line(),
            KeyCode::F8 if pressed => self.stage.reload_poses(),
            _ if pressed && digit > 0 => self.stage.number_pressed(digit),
            _ => {}
        }
    }
}

/// What the player and the enemy of `--foe` have.
const FOE_HIT_POINTS: i32 = 5000;

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
                if let Some(st) = &mut self.state {
                    st.gpu.resize(s.width, s.height);
                    st.presenter.resize(&st.gpu, (s.width, s.height));
                }
                self.stage.set_view((s.width, s.height));
            }
            WindowEvent::CursorMoved { position, .. } => {
                let now = (position.x as f32, position.y as f32);
                if self.stage.turret_on() {
                    if self.turret_cursor_known {
                        self.stage
                            .turret_look(now.0 - self.cursor.0, now.1 - self.cursor.1);
                    }
                    self.turret_cursor_known = true;
                    if let Some(middle) = self.state.as_ref().and_then(|st| back_to_middle(st, now))
                    {
                        self.cursor = middle;
                        return;
                    }
                } else if self.looking {
                    self.stage
                        .look(now.0 - self.cursor.0, now.1 - self.cursor.1);
                } else if let Some(st) = &self.state {
                    let z = st.window.inner_size();
                    self.stage.hover(now, (z.width, z.height));
                }
                self.cursor = now;
            }
            WindowEvent::MouseInput { state, button, .. } if button == self.look_button() => {
                self.looking = state == ElementState::Pressed
            }
            WindowEvent::Focused(false) => {
                self.stage.set_ctrl(false);
                self.stage.set_shift(false);
                self.keys = [false; 5];
                self.looking = false;
                self.turning = 0.0;
            }
            WindowEvent::MouseInput { state, button, .. }
                if button == self.click_button() && self.stage.turret_on() =>
            {
                self.stage.turret_fire(state == ElementState::Pressed);
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button,
                ..
            } if button == self.click_button() => {
                if let Some(st) = &self.state {
                    let z = st.window.inner_size();
                    self.stage.click(self.cursor, (z.width, z.height));
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let d = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
                if self.stage.menu_open() {
                    self.stage.menu_wheel(d);
                } else {
                    self.stage.distance = (self.stage.distance - d * 0.4).clamp(1.5, 12.0);
                }
            }
            WindowEvent::KeyboardInput { event, .. } if self.stage.typing() => {
                if event.state != ElementState::Pressed {
                    return;
                }
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Escape) => self.stage.escape(),
                    PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter) => {
                        self.stage.continue_line()
                    }
                    PhysicalKey::Code(KeyCode::Backspace) => self.stage.backspace(),
                    _ => {
                        if let Some(t) = &event.text {
                            self.stage.type_text(t);
                        }
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.key(code, event.state == ElementState::Pressed);
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

/// A cursor at the window's edge stops giving movement, so it goes back to the middle.
/// Only with the window in use: a driver's has no focus.
fn back_to_middle(st: &State, now: (f32, f32)) -> Option<(f32, f32)> {
    let z = st.window.inner_size();
    let middle = (z.width as f32 / 2.0, z.height as f32 / 2.0);
    let off = (now.0 - middle.0).abs().max((now.1 - middle.1).abs());
    if !st.window.has_focus() || off <= middle.1 / 2.0 {
        return None;
    }
    let spot = winit::dpi::PhysicalPosition::new(middle.0, middle.1);
    st.window
        .set_cursor_position(spot)
        .is_ok()
        .then_some(middle)
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
    let size = opt("--size")
        .and_then(|s| {
            let (w, h) = s.split_once('x')?;
            Some((w.parse::<u32>().ok()?, h.parse::<u32>().ok()?))
        })
        .unwrap_or((1280, 720));
    let original = !flag("--installed-mod");
    let mut stage = if flag("--skip-menu") || flag("--autopilot") {
        Stage::open(&module, original)?
    } else {
        Stage::menu(&module, original)?
    };
    if flag("--classic-movement") {
        stage.set_movement_modern(false);
    }
    if let Some(folder) = opt("--pose-folder") {
        stage.use_pose_folder(folder.into());
    }
    stage.orbit = opt("--orbit").and_then(|v| v.parse().ok()).unwrap_or(0.0);
    if flag("--no-hd") {
        stage.no_hd();
    }
    stage.enable_tutorials(!flag("--no-tutorials") && !flag("--autopilot"));
    stage.enable_auto_pause(!flag("--no-auto-pause") && !flag("--autopilot"));
    stage.enable_chargen(!flag("--no-chargen"));
    stage.enable_early_force(!flag("--no-early-force") && !flag("--autopilot"));
    stage.turret_assist = flag("--auto-aim");
    stage.combat_autosave = !flag("--state-log") && !flag("--autopilot");
    stage.pace_loading = true;
    if flag("--original-underwear") {
        stage.set_conventional_underwear(false);
    }
    if flag("--early-force") {
        stage.restart_with_early_force()?;
    }
    if let Some(file) = opt("--settings") {
        stage.use_settings_file(file.into());
    } else if !flag("--skip-menu") && !flag("--autopilot") {
        stage.use_saved_settings();
    }
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
    // A scripted player has no need of real time unless someone means to watch it: `--speed 1`.
    let speed: f32 = opt("--speed")
        .and_then(|n| n.parse().ok())
        .unwrap_or(if flag("--autopilot") { 16.0 } else { 1.0 });
    stage.speed = speed;
    if flag("--autopilot") {
        let mut p = kotor_game::Autopilot::new();
        // Lines are held to be read only when someone can read them.
        p.read_lines = speed <= 1.0 && !flag("--fast-lines");
        stage.pilot = Some(p);
    }
    let digest_file = opt("--digest");
    let mut app = App {
        stage,
        sound: Sound {
            out,
            tx,
            rx,
            newest: [0; 2],
        },
        monitor,
        size,
        fullscreen: flag("--fullscreen"),
        hidden: flag("--hidden"),
        max_frames: opt("--frames").and_then(|n| n.parse().ok()),
        frame_log: opt("--frame-log").and_then(|path| {
            use std::io::Write;
            let mut f = std::io::BufWriter::new(std::fs::File::create(path).ok()?);
            writeln!(
                f,
                "frame,dt,update,overlay,hud,upload,scene,draw,textures,new_textures,show,meshes,fight,module"
            )
            .ok()?;
            Some(f)
        }),
        state_log: flag("--state-log"),
        pilot_seen: 0,
        render_every: opt("--render-every")
            .and_then(|n| n.parse().ok())
            .unwrap_or((speed / 16.0) as u32)
            .max(1),
        max_steps: opt("--game-seconds")
            .and_then(|n| n.parse::<u64>().ok())
            .map(|s| s * 60),
        near: opt("--near"),
        loot: opt("--loot"),
        foe: opt("--foe").and_then(|m| m.parse().ok()),
        place: opt("--place").and_then(|p| {
            let n: Vec<f32> = p.split(',').filter_map(|v| v.parse().ok()).collect();
            <[f32; 4]>::try_from(n).ok()
        }),
        xp: opt("--xp").and_then(|n| n.parse().ok()),
        appearance: opt("--appearance").and_then(|n| n.parse().ok()),
        underwear: opt("--underwear"),
        power: opt("--power").and_then(|n| n.parse().ok()),
        give: opt("--give"),
        gold: opt("--gold").and_then(|n| n.parse().ok()),
        store: opt("--store"),
        global: opt("--global"),
        pazaak: opt("--pazaak").and_then(|n| n.parse().ok()),
        alignment: opt("--alignment").and_then(|n| n.parse().ok()),
        grenadier: flag("--grenadier"),
        turret_cursor: false,
        turret_cursor_known: false,
        pilot_until: opt("--pilot-until"),
        auto_pause: !flag("--no-auto-pause"),
        frames: 0,
        started: std::time::Instant::now(),
        last: std::time::Instant::now(),
        cursor: (0.0, 0.0),
        turning: 0.0,
        keys: [false; 5],
        looking: false,
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
    let played = app.stage.game.steps as f32 / 60.0;
    println!(
        "{played:.1} game seconds, {:.1} game seconds per wall second",
        played / secs
    );
    if let (Some(path), Some(p)) = (digest_file, app.stage.pilot.as_ref()) {
        let lines: Vec<String> = p
            .digests
            .iter()
            .map(|(s, d)| format!("{s} {d:016x}"))
            .collect();
        std::fs::write(path, lines.join("\n") + "\n")?;
    }
    Ok(())
}
