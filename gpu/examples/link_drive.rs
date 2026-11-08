//! Drives every door and trigger link of the install with no window and times what the player would see freeze.
//!
//!   cargo run --release --example link_drive                      (every module that has a link, from census.tsv or the modules folder)
//!   cargo run --release --example link_drive -- tar_m10aa tar_m02aa
//!
//! For each link: opens the module fresh, stands the player 10 m from the door or trigger, waits up to `WAIT` seconds (default 8)
//! for the neighbour to be joined or loaded ahead, then asks for the crossing. A joined link is not timed: it needs no load
//! (see `seamless_drive`). Any other is timed as `load_times` does: the world change, the stage build, and the first texture
//! loads that the upload needs. One row per link goes to stdout and `drive.tsv`:
//! module, object tag, kind, goes to, mode (joined / ahead / cold / error), seconds waited, world ms, build ms, texture ms, total ms.
use kotor_game::Kind;
use kotor_gpu::stage::Stage;
use std::io::Write;

const DT: f32 = 1.0 / 60.0;

fn ms(t: std::time::Instant) -> f32 {
    t.elapsed().as_secs_f32() * 1000.0
}

fn modules_with_links() -> Vec<String> {
    let mut names = std::collections::BTreeSet::new();
    if let Ok(text) = std::fs::read_to_string("census.tsv") {
        for line in text.lines().skip(1) {
            let c: Vec<&str> = line.split('\t').collect();
            if c.len() > 4 && c[1].ends_with("_link") {
                names.insert(c[0].to_string());
            }
        }
    }
    names.into_iter().collect()
}

fn main() {
    let mut modules: Vec<String> = std::env::args().skip(1).collect();
    if modules.is_empty() {
        modules = modules_with_links();
    }
    let wait: u64 = std::env::var("WAIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8);
    let mut out = std::fs::File::create("drive.tsv").unwrap();
    for module in &modules {
        // The links of the module, listed once from a first opening.
        let Ok(mut first) = Stage::open(module, true) else {
            println!("{module}\t-\t-\t-\tcannot open");
            continue;
        };
        first.enable_tutorials(false);
        first.enable_auto_pause(false);
        for _ in 0..30 {
            first.update(DT);
        }
        let links: Vec<(String, Kind, String, [f32; 3])> = first
            .game
            .world
            .objects
            .values()
            .filter(|o| {
                matches!(o.kind, Kind::Door | Kind::Trigger)
                    && !o.link_module.is_empty()
                    && !o.link_module.eq_ignore_ascii_case(module)
            })
            .map(|o| (o.tag.clone(), o.kind, o.link_module.clone(), o.position))
            .collect();
        drop(first);
        for (tag, kind, to, at) in links {
            let mut row = format!(
                "{module}\t{tag}\t{}\t{to}",
                if kind == Kind::Door {
                    "door"
                } else {
                    "trigger"
                }
            );
            let Ok(mut s) = Stage::open(module, true) else {
                continue;
            };
            s.enable_tutorials(false);
            s.enable_auto_pause(false);
            for _ in 0..120 {
                s.update(DT);
            }
            // The same object again, by tag and place (ids differ between openings).
            let Some(id) = s
                .game
                .world
                .objects
                .values()
                .find(|o| {
                    o.tag == tag
                        && o.link_module.eq_ignore_ascii_case(&to)
                        && (o.position[0] - at[0]).abs() < 0.01
                        && (o.position[1] - at[1]).abs() < 0.01
                })
                .map(|o| o.id)
            else {
                println!("{row}\terror\tobject not found");
                continue;
            };
            s.game.world.pc_mover = kotor_walkmesh::Mover::new([at[0] + 10.0, at[1], at[2]], 0.0);
            let began = std::time::Instant::now();
            let mut mode = "cold";
            while began.elapsed().as_secs() < wait {
                s.update(DT);
                std::thread::sleep(std::time::Duration::from_millis(20));
                if s.game.world.seamless.attached.contains_key(&id) {
                    mode = "joined";
                    break;
                }
                if s.preloaded().is_some_and(|m| m.eq_ignore_ascii_case(&to)) {
                    mode = "ahead";
                    break;
                }
            }
            let waited = began.elapsed().as_secs_f32();
            if mode == "joined" {
                let line = format!("{row}\tjoined\t{waited:.1}\t0\t0\t0\t0");
                println!("{line}");
                writeln!(out, "{line}").unwrap();
                continue;
            }
            let w = &mut s.game.world;
            let t = std::time::Instant::now();
            if kind == Kind::Door {
                w.use_door(id);
            } else {
                let (m, l) = w
                    .objects
                    .get(&id)
                    .map(|o| (o.link_module.clone(), o.link_to.clone()))
                    .unwrap();
                w.transition_request = Some((m, l));
            }
            // The game step that takes the request does the world change.
            let step = t.elapsed();
            // A lift rides first: the frames of the ride are not a freeze, the ones after its end are.
            for _ in 0..3000 {
                if s.game.world.lift_ride().is_none_or(|r| r.progress() > 0.97) {
                    break;
                }
                s.update(DT);
            }
            let t = std::time::Instant::now();
            for _ in 0..40 {
                s.update(DT);
                if s.game.world.module.eq_ignore_ascii_case(&to) {
                    break;
                }
            }
            let build = ms(t);
            let changed = s.game.world.module.eq_ignore_ascii_case(&to);
            let t = std::time::Instant::now();
            let scene = s.static_scene().clone();
            let mut seen = std::collections::HashSet::new();
            for m in &scene.meshes {
                if let Some(n) = &m.texture {
                    if seen.insert(n.clone()) {
                        let _ = s.texture(n);
                    }
                }
            }
            let tex = ms(t);
            let world = step.as_secs_f32() * 1000.0;
            row += &format!(
                "\t{}\t{waited:.1}\t{world:.0}\t{build:.0}\t{tex:.0}\t{:.0}",
                if changed { mode } else { "error" },
                world + build + tex
            );
            if !changed {
                row += &format!("\tstayed in {}", s.game.world.module);
            }
            println!("{row}");
            writeln!(out, "{row}").unwrap();
            out.flush().ok();
        }
    }
}
