//! Play from the start with no window, and print everything that stalled.
//! Usage: autopilot [module] [seconds] [real seconds]
//!
//! It runs as fast as the CPU allows, with no window, sound or particles: see `docs/FASTTEST.md`.
//! `AUTOPILOT_UNTIL` names a module to stop at, `AUTOPILOT_DIGEST` a file for the state digests,
//! `AUTOPILOT_READ_LINES` holds lines as long as a person would read them.
//! `AUTOPILOT_TRACE` adds the last lines said, everyone alive, every door and a map of where the player can walk.
//! `AUTOPILOT_EVENTS` names a file for the whole event log.

use kotor_archive::Install;
use kotor_game::{Autopilot, Game, Kind, World};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let module = a.get(1).map_or("end_m01aa", String::as_str);
    let seconds: f32 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(3600.0);
    // Real seconds, so that a runaway script shows up as a result and not as a run that never ends.
    let limit: f32 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(120.0);
    let mut inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).unwrap();
    w.interactive_dialogue = true;
    w.load_module(module).unwrap();
    let spec = w.recommended_character("Player", 0, 1).unwrap();
    w.create_character(&spec).unwrap();
    w.enter_area();
    let mut g = Game::new(w);
    let mut p = Autopilot::new();
    let started = std::time::Instant::now();
    p.read_lines = std::env::var_os("AUTOPILOT_READ_LINES").is_some();
    let until = std::env::var("AUTOPILOT_UNTIL").unwrap_or_default();
    while p.time < seconds
        && !p.finished
        && started.elapsed().as_secs_f32() < limit
        && g.world.module != until
    {
        p.run_steps(&mut g, 60);
    }
    let wall = started.elapsed().as_secs_f32();
    println!(
        "--- {:.1} game seconds in {wall:.2} wall seconds: {:.0} game seconds per wall second",
        p.time,
        p.time / wall
    );
    if let Some(path) = std::env::var_os("AUTOPILOT_DIGEST") {
        let lines: Vec<String> = p
            .digests
            .iter()
            .map(|(s, d)| format!("{s} {d:016x}"))
            .collect();
        std::fs::write(path, lines.join("\n") + "\n").unwrap();
    }
    for l in &p.log {
        println!("{l}");
    }
    println!("--- {}", p.summary(&g.world));
    if let Some(path) = std::env::var_os("AUTOPILOT_EVENTS") {
        std::fs::write(path, g.world.events.join("\n")).unwrap();
    }
    if std::env::var_os("AUTOPILOT_TRACE").is_some() {
        trace(&g.world);
    }
    println!("--- real time {:.1} s", started.elapsed().as_secs_f32());
    println!(
        "--- experience {}",
        g.world.object(g.world.pc).map_or(0, |o| o.xp)
    );
    println!(
        "--- routines that only answered nothing: {:?}",
        g.world.stubbed
    );
    println!(
        "--- routines answering a fixed value: {:?}",
        g.world.constant_defaults
    );
    let pc = g.world.object(g.world.pc).unwrap();
    println!(
        "--- player {:?} level {} hp {} of {}, {} carried, wearing {:?}",
        pc.position,
        pc.level,
        pc.hp,
        pc.max_hp,
        pc.inventory.len(),
        pc.equipped.keys().collect::<Vec<_>>()
    );
}

fn trace(w: &World) {
    for l in w.transcript.iter().rev().take(14).rev() {
        println!(
            "said [{}/{}] {}: {}",
            l.dialogue,
            l.node,
            l.speaker,
            l.text.chars().take(90).collect::<String>()
        );
    }
    let at = w.object(w.pc).unwrap().position;
    for o in w.objects.values() {
        if o.kind == Kind::Creature && !o.dead {
            println!(
                "alive {} at {:?} hp {}/{} faction {} target {:?}",
                o.tag,
                o.position,
                o.hp,
                o.max_hp,
                o.faction,
                w.combat.target.get(&o.id)
            );
        }
        if o.kind == Kind::Placeable {
            let route = w
                .nav
                .as_ref()
                .and_then(|n| n.find_path(at, o.position))
                .is_some();
            println!(
                "thing {} at [{:.1},{:.1}] route {route} conv {:?}",
                o.tag, o.position[0], o.position[1], o.conversation
            );
        }
        if o.kind == Kind::Door {
            let route = w
                .nav
                .as_ref()
                .and_then(|n| n.find_path(at, o.position))
                .is_some();
            println!(
                "door {} at [{:.1},{:.1}] locked {} open {} route {route} link {:?} fail {:?} conv {:?}",
                o.tag, o.position[0], o.position[1], o.locked, o.open, o.link_module, o.scripts.get("OnFailToOpen"), o.conversation
            );
        }
    }
    println!("globals {:?} {:?}", w.global_numbers, w.global_booleans);
    // '.' can be walked to from here, '#' is floor that cannot, 'D' a door, '@' the player.
    let Some(nav) = w.nav.as_ref() else {
        return;
    };
    for row in (0..90).rev() {
        let y = row as f32 * 2.0;
        let mut line = format!("{y:4.0} ");
        for col in 0..60 {
            let x = col as f32 * 2.0;
            let near = |p: [f32; 3]| (p[0] - x).abs() <= 1.0 && (p[1] - y).abs() <= 1.0;
            let door = w
                .objects
                .values()
                .any(|o| o.kind == Kind::Door && near(o.position));
            line.push(if near(at) {
                '@'
            } else if door {
                'D'
            } else if let Some(z) = nav.walkable_at(x, y, 50.0) {
                if nav.find_path(at, [x, y, z]).is_some() {
                    '.'
                } else {
                    '#'
                }
            } else {
                ' '
            });
        }
        println!("{line}");
    }
}
