//! Frames and timings for `docs/BUG_AUDIT.md`: the owner's list checked on the current build. Offscreen: no window, no screen
//! capture, no saves.
//!
//!   cargo run --release --example audit_shots -- [out_dir] [width] [height]
use kotor_game::Kind;
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};
use kotor_render::Camera;
use std::time::Instant;

const DT: f32 = 1.0 / 30.0;

struct Rig {
    s: Stage,
    gpu: GpuRenderer,
    out: std::path::PathBuf,
    size: (u32, u32),
}

impl Rig {
    fn open(
        module: &str,
        out: &std::path::Path,
        size: (u32, u32),
    ) -> Result<Rig, Box<dyn std::error::Error>> {
        let mut s = Stage::open(module, true)?;
        s.enable_tutorials(false);
        s.enable_auto_pause(false);
        s.set_view(size);
        let mut gpu = GpuRenderer::new(size.0, size.1)?;
        let scene = s.static_scene().clone();
        gpu.set_static_scene(&scene, &mut |n| s.texture(n));
        Ok(Rig {
            s,
            gpu,
            out: out.to_path_buf(),
            size,
        })
    }

    fn shot(&mut self, name: &str) {
        let scene = self.s.dynamic_scene();
        let cam = self.s.camera();
        self.gpu.set_particles(self.s.particles());
        let mut img = self
            .gpu
            .render(&scene, &mut |n| self.s.texture(n), &cam)
            .unwrap();
        if let Some(o) = self.s.overlay() {
            composite(&mut img, o);
        }
        let p = self.out.join(format!("audit_{name}_{}p.png", self.size.1));
        std::fs::write(&p, kotor_image::to_png(&img)).unwrap();
        println!("wrote {}", p.display());
    }

    fn run(&mut self, seconds: f32) {
        for _ in 0..(seconds / DT) as usize {
            self.s.update(DT);
            if self.s.game.world.in_dialogue() {
                self.s.number_key(1);
                self.s.continue_line();
            }
        }
    }

    fn carrying(&mut self, template: &str) -> u32 {
        let w = &mut self.s.game.world;
        let pc = w.pc;
        let item = w.spawn_item(template);
        w.object_mut(pc).unwrap().inventory.push(item);
        item
    }

    /// Milliseconds a thing takes, with the update that shows it.
    fn time(&mut self, what: &str, f: impl FnOnce(&mut Stage)) {
        let (t, seen) = (Instant::now(), self.s.overlay_version);
        f(&mut self.s);
        self.s.update(DT);
        println!(
            "{what}: {:.1} ms, overlay {:.1}, box {:?}",
            t.elapsed().as_secs_f32() * 1000.0,
            self.s.overlay_ms,
            self.s.overlay_change_since(seen),
        );
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).map_or("audit", String::as_str));
    let width: u32 = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(1280);
    let height: u32 = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(720);
    let size = (width, height);
    std::fs::create_dir_all(&out)?;

    // Bug 29: a medpac in the quick slots.
    let mut r = Rig::open("end_m01aa", &out, size)?;
    r.run(50.0);
    let pc = r.s.game.world.pc;
    let medpac = r.carrying("g_i_medeqpmnt01");
    r.carrying("g_i_adrnaline003");
    r.s.game.world.object_mut(pc).unwrap().hp = 1;
    r.s.update(DT);
    r.shot("29_quick_slots");
    if let Some(slot) = r.s.hud_pixel("BTN_ACTION1", size) {
        r.s.click(slot, size);
        r.run(1.0);
        println!(
            "bug 29: hp 1 -> {}, medpac used up {}",
            r.s.game.world.object(pc).unwrap().hp,
            r.s.game.world.object(medpac).is_none()
        );
    }

    // Bugs 1 and 7: how long a panel and the cursor take, the first time and after.
    for (name, open) in [
        ("character", Stage::open_character as fn(&mut Stage)),
        ("inventory", Stage::open_inventory),
        ("equip", Stage::open_equip),
    ] {
        r.time(&format!("bug 7: {name}, first time"), open);
        r.s.escape();
        r.s.update(DT);
        r.time(&format!("bug 7: {name}, again"), open);
        r.time(&format!("bug 1: {name}, the cursor moved"), |s| {
            s.hover((width as f32 * 0.4, height as f32 * 0.4), size);
        });
        r.time(&format!("bug 1: {name}, the cursor moved again"), |s| {
            s.hover((width as f32 * 0.6, height as f32 * 0.6), size);
        });
        // What it has drawn once is kept.
        for k in [0.4, 0.6, 0.4, 0.6] {
            r.time(&format!("bug 1: {name}, back and forth"), |s| {
                s.hover((width as f32 * k, height as f32 * k), size);
            });
        }
        r.shot(&format!("7_{name}"));
        r.s.escape();
        r.s.update(DT);
    }

    // Bug 16: a lock, with the skill and a spike.
    let w = &mut r.s.game.world;
    let doors: Vec<u32> = w
        .objects
        .values()
        .filter(|o| o.kind == Kind::Door && o.link_module.is_empty())
        .map(|o| o.id)
        .collect();
    if let Some(door) = doors.into_iter().find(|d| w.lock_can_be_picked(*d)) {
        let at = w.object(w.pc).unwrap().position;
        for o in w.objects.values_mut() {
            o.useable = false;
        }
        let o = w.object_mut(door).unwrap();
        o.locked = true;
        o.open = false;
        o.useable = true;
        o.conversation.clear();
        o.scripts.clear();
        o.position = [at[0] - 0.8, at[1], at[2]];
        r.carrying("g_i_secspike02");
        r.s.game.world.object_mut(pc).unwrap().skills = vec![4; 8];
        r.s.yaw = std::f32::consts::PI;
        r.run(0.4);
        r.s.yaw = std::f32::consts::PI;
        r.s.update(DT);
        r.shot("16_security");
        println!(
            "bug 16: {}",
            r.s.state_line()
                .split(' ')
                .filter(|p| p.starts_with("slot0=") || p.starts_with("target="))
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
    drop(r);

    // Bug 12: a treasure footlocker of the game's own.
    let mut r = Rig::open("tar_m10aa", &out, size)?;
    r.run(7.0);
    let w = &mut r.s.game.world;
    let chest = w
        .objects
        .values()
        .find(|o| {
            o.scripts
                .get("OnHeartbeat")
                .is_some_and(|s| s.starts_with("k_plc_tres"))
        })
        .map(|o| (o.id, o.tag.clone(), o.position));
    if let Some((chest, tag, at)) = chest {
        let pc = w.pc;
        w.object_mut(chest).unwrap().locked = false;
        w.teleport(pc, [at[0] + 0.5, at[1], at[2]]);
        w.pc_mover.pos = [at[0] + 0.5, at[1], at[2]];
        w.interactive_dialogue = true;
        w.click(chest);
        println!(
            "bug 12: {tag} opened {:?}, holds {:?}",
            w.opened == Some(chest),
            w.container_items()
                .iter()
                .map(|i| i.1.clone())
                .collect::<Vec<_>>()
        );
        r.run(0.3);
        r.shot("12_footlocker");
    }
    drop(r);

    // The later ones: the cantina's arena monitors.
    let mut r = Rig::open("tar_m02ae", &out, size)?;
    r.run(1.0);
    let w = &r.s.game.world;
    let screens: Vec<_> = w
        .objects
        .values()
        .filter(|o| {
            o.kind == Kind::Placeable
                && (o.tag.to_ascii_lowercase().contains("mon")
                    || o.name.to_ascii_lowercase().contains("monitor"))
        })
        .map(|o| (o.tag.clone(), o.name.clone(), o.position))
        .collect();
    println!("bug 42: {screens:?}");
    if let Some((_, _, at)) = screens.first().cloned() {
        // From each side in turn, with no one in the way: the screen faces one of them.
        for n in 0..4 {
            let (s, c) = (n as f32 * std::f32::consts::FRAC_PI_2).sin_cos();
            let cam = Camera {
                eye: [at[0] + c * 1.6, at[1] + s * 1.6, at[2] + 1.4],
                target: [at[0], at[1], at[2] + 1.2],
                fov: 55.0,
            };
            r.gpu.set_particles(Vec::new());
            let scene = r.s.dynamic_scene();
            let img = r.gpu.render(&scene, &mut |n| r.s.texture(n), &cam).unwrap();
            let p = r
                .out
                .join(format!("audit_42_arena_monitor{n}_{}p.png", r.size.1));
            std::fs::write(&p, kotor_image::to_png(&img)).unwrap();
        }
    }
    drop(r);

    // Bug 3: the figures on the class screen move.
    let mut s = Stage::menu("end_m01aa", true)?;
    s.set_view(size);
    s.enable_chargen(true);
    let new_game = s.menu_pixel("BTN_NEWGAME", None, size).unwrap();
    let load = s.menu_pixel("BTN_LOADGAME", None, size).unwrap();
    s.update(DT);
    for (what, at) in [
        ("New Game", new_game),
        ("Load", load),
        ("New Game", new_game),
    ]
    .into_iter()
    .cycle()
    .take(9)
    {
        let (t, seen) = (Instant::now(), s.overlay_version);
        s.hover(at, size);
        let hovered = t.elapsed().as_secs_f32() * 1000.0;
        s.update(DT);
        println!(
            "bug 1: main menu, the cursor on {what}: {:.1} ms, hover {hovered:.1}, overlay {:.1}, box {:?}",
            t.elapsed().as_secs_f32() * 1000.0,
            s.overlay_ms,
            s.overlay_change_since(seen),
        );
    }
    s.hover(new_game, size);
    s.click(new_game, size);
    println!("bug 3: on {}", s.menu_name());
    let pose = |s: &mut Stage| {
        let (scene, _, _) = s.frame_scene();
        scene
            .meshes
            .iter()
            .flat_map(|m| m.positions.iter())
            .map(|p| p[0] + p[1] + p[2])
            .sum::<f32>()
    };
    s.update(DT);
    let a = pose(&mut s);
    for _ in 0..20 {
        s.update(DT);
    }
    let b = pose(&mut s);
    println!("bug 3: the figures' vertices moved by {:.3}", (a - b).abs());
    Ok(())
}
