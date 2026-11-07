//! Prints, ten times a second, where one walker is, what it is walking to, and who is within 2 m of it.
//!
//!   cargo run --release --example walker_trace -- tar_m02ac Tar02_Woman 150 160
use kotor_archive::Install;
use kotor_game::{Action, Game, Kind, World};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (module, tag) = (&a[0], &a[1]);
    let (from, to): (usize, usize) = (a[2].parse().unwrap(), a[3].parse().unwrap());
    let dir = kotor_core::install_dir().expect("install");
    let mut inst = Install::open(&dir).unwrap();
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).unwrap();
    w.interactive_dialogue = true;
    w.load_module(module).unwrap();
    let spec = w.recommended_character("Player", 0, 1).unwrap();
    w.create_character(&spec).unwrap();
    w.enter_area();
    let mut g = Game::new(w);
    for step in 0..to * 60 {
        g.step_once();
        if step < from * 60 || step % 6 != 0 {
            continue;
        }
        let id = g.world.by_tag(tag)[0];
        let o = g.world.object(id).unwrap();
        let head = match o.actions.front() {
            Some(Action::MoveTo {
                target,
                to,
                range,
                walker,
                ..
            }) => format!(
                "MoveTo target={:?}({}) to=({:.1},{:.1}) range={range} walker={}",
                target,
                target
                    .and_then(|t| g.world.object(t))
                    .map(|t| t.tag.clone())
                    .unwrap_or_default(),
                to[0],
                to[1],
                walker.is_some()
            ),
            Some(other) => format!("{:?}", std::mem::discriminant(other)),
            None => "none".into(),
        };
        let near: Vec<String> = g
            .world
            .objects
            .values()
            .filter(|p| p.id != id && matches!(p.kind, Kind::Creature | Kind::Placeable) && !p.dead)
            .filter(|p| (p.position[0] - o.position[0]).hypot(p.position[1] - o.position[1]) < 2.0)
            .map(|p| format!("{}@{:.1},{:.1}", p.tag, p.position[0], p.position[1]))
            .collect();
        println!(
            "{:.1}s ({:.2},{:.2},{:.2}) face {:.0} | {head} | near {}",
            step as f32 / 60.0,
            o.position[0],
            o.position[1],
            o.position[2],
            o.facing,
            near.join(" ")
        );
    }
}
