//! Finds the grenade throws of the Endar Spire: who throws, at what, when, and what the burst is. For the cinematic's burst.
//!
//!   cargo run --release -p kotor-game --example grenade_cinematic
use kotor_archive::Install;
use kotor_game::{Autopilot, Game, World};

fn main() {
    let mut inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).unwrap();
    w.interactive_dialogue = true;
    w.load_module("end_m01aa").unwrap();
    let spec = w.recommended_character("Player", 0, 1).unwrap();
    w.create_character(&spec).unwrap();
    w.enter_area();
    let mut g = Game::new(w);
    let mut p = Autopilot::new();
    let mut seen = 0;
    while p.time < 1500.0 && !p.finished && g.world.module != "tar_m02af" {
        p.run_steps(&mut g, 30);
        let ev = &g.world.events;
        for e in ev.iter().skip(seen) {
            if e.contains("power 87") || e.contains("power 88") || e.contains("grenade") {
                println!("{:.1}s {}: {e}", g.world.time, g.world.module);
            }
        }
        seen = ev.len();
        for (h, at) in g.world.effects_at.drain(..).collect::<Vec<_>>() {
            let rec = h
                .checked_sub(1)
                .and_then(|i| g.world.effects.get(i as usize));
            println!("{:.1}s burst at {at:?}: {rec:?}", g.world.time);
        }
    }
}
