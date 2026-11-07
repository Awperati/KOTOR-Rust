//! Times what a module change costs the frame that does it, for a few kinds of transition.
//!
//!   cargo run --release --example load_times -- <from:to> [<from:to> ...]
//!
//! Opens `from`, lets it settle, then asks the world to go to `to` and times the `Stage::update` that builds it (the frame
//! the player sees freeze), then the static scene and the first texture loads that the GPU upload needs. Twice for each
//! pair: the first time is cold (the files are read), the second goes back and forth so the files are in the OS cache.
use kotor_gpu::stage::Stage;

const DT: f32 = 1.0 / 60.0;

fn ms(t: std::time::Instant) -> f32 {
    t.elapsed().as_secs_f32() * 1000.0
}

fn main() {
    for pair in std::env::args().skip(1) {
        let Some((from, to)) = pair.split_once(':') else {
            continue;
        };
        let began = std::time::Instant::now();
        let Ok(mut s) = Stage::open(from, true) else {
            println!("{pair}: cannot open {from}");
            continue;
        };
        let open_ms = ms(began);
        s.enable_tutorials(false);
        s.enable_auto_pause(false);
        for _ in 0..120 {
            s.update(DT);
        }
        // PRELOAD=1: stand 10 m from an exit to `to` and wait for the module to be loaded ahead, as a player walking up does.
        let mut waited = String::new();
        if std::env::var_os("PRELOAD").is_some() {
            let w = &mut s.game.world;
            let exit = w
                .objects
                .values()
                .find(|o| o.link_module.eq_ignore_ascii_case(to))
                .map(|o| o.position);
            if let Some(p) = exit {
                let at = [p[0] + 10.0, p[1], p[2]];
                w.pc_mover = kotor_walkmesh::Mover::new(at, 0.0);
                let t = std::time::Instant::now();
                while s.preloaded().is_none() && t.elapsed().as_secs() < 30 {
                    s.update(DT);
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                waited = format!(
                    " (loaded ahead after {:.1} s: {})",
                    t.elapsed().as_secs_f32(),
                    s.preloaded().is_some()
                );
            }
        }
        let mut row = format!("{from} -> {to}: open {open_ms:.0} ms{waited}");
        for round in 0..2 {
            let (a, b) = (to, from);
            let t = std::time::Instant::now();
            if s.game.world.transition(a, "").is_err() {
                row += &format!(" | transition to {a} failed");
                break;
            }
            let logic = ms(t);
            let t = std::time::Instant::now();
            s.update(DT);
            let build = ms(t);
            let t = std::time::Instant::now();
            let scene = s.static_scene().clone();
            let mut seen = std::collections::HashSet::new();
            let mut slow: Vec<(f32, String)> = Vec::new();
            for m in &scene.meshes {
                if let Some(n) = &m.texture {
                    if seen.insert(n.clone()) {
                        let t1 = std::time::Instant::now();
                        let _ = s.texture(n);
                        slow.push((ms(t1), n.clone()));
                    }
                }
            }
            let tex = ms(t);
            slow.sort_by(|a, b| b.0.total_cmp(&a.0));
            if std::env::var_os("SLOW").is_some() {
                println!("  slowest textures: {:?}", &slow[..slow.len().min(6)]);
            }
            row += &format!(
                " | {} #{round}: world {logic:.0} + build {build:.0} + {} textures {tex:.0} = {:.0} ms",
                if round == 0 { "cold" } else { "again" },
                seen.len(),
                logic + build + tex
            );
            let _ = s.game.world.transition(b, "");
            s.update(DT);
        }
        println!("{row}");
    }
}
