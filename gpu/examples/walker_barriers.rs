//! Counts the steps in which a walker goes through something solid that the player is stopped by (a light, a rail, a crate:
//! what `World::barriers` holds), in a module with the stage's own room shell, no window.
//!
//!   cargo run --release --example walker_barriers -- tar_m02aa 300
//!
//! A step counts when the sheet from where the walker was to where it is, between 0.25 m and 1.2 m over its feet (the player's
//! own test, `movement.rs`), crosses a surface. Prints each place it happens, with the placeables within 2 m.
use kotor_game::{Action, Kind};
use kotor_gpu::stage::Stage;

const DT: f32 = 1.0 / 60.0;

fn main() {
    let module = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "tar_m02aa".into());
    let seconds: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(300);
    let mut s = Stage::open(&module, true).expect("module");
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let Some(barriers) = s.game.world.barriers.clone() else {
        println!("{module}: no barriers");
        return;
    };
    // NOBARRIER=1: the walkers are not given the barriers, to see how they walked before they were.
    if std::env::var_os("NOBARRIER").is_some() {
        s.game.world.barriers = None;
    }
    let (pc, party) = (s.game.world.pc, s.game.world.party.clone());
    let mut last: std::collections::BTreeMap<u32, [f32; 3]> = Default::default();
    let mut hits: std::collections::BTreeMap<(i32, i32), (usize, String)> = Default::default();
    let mut steps = 0usize;
    // Where each walker was six seconds ago, and whether it had a walk to do ever since: held up, if it has not moved.
    let mut ago: std::collections::BTreeMap<u32, ([f32; 3], bool)> = Default::default();
    let mut stuck = 0usize;
    for tick in 0..seconds * 60 {
        s.update(DT);
        if tick % 360 == 359 {
            for o in s.game.world.objects.values() {
                if o.kind != Kind::Creature || o.dead || o.id == pc || party.contains(&o.id) {
                    continue;
                }
                let walking = matches!(o.actions.front(), Some(Action::MoveTo { .. }));
                if let Some((p, was)) = ago.insert(o.id, (o.position, walking)) {
                    if was && walking && (o.position[0] - p[0]).hypot(o.position[1] - p[1]) < 0.3 {
                        stuck += 1;
                        println!(
                            "STUCK t={}s {} at ({:.1},{:.1})",
                            tick / 60 + 1,
                            o.tag,
                            o.position[0],
                            o.position[1]
                        );
                    }
                }
            }
        }
        let w = &s.game.world;
        for o in w.objects.values() {
            if o.kind != Kind::Creature || o.dead || o.id == pc || party.contains(&o.id) {
                continue;
            }
            if let Some(p) = last.insert(o.id, o.position) {
                let d = (o.position[0] - p[0]).hypot(o.position[1] - p[1]);
                // A teleport is not a step.
                if d > 0.0005
                    && d < 0.5
                    && barriers.across(
                        [p[0], p[1]],
                        [o.position[0], o.position[1]],
                        p[2] + 0.25,
                        p[2] + 1.2,
                    )
                {
                    steps += 1;
                    let key = ((o.position[0] * 2.0) as i32, (o.position[1] * 2.0) as i32);
                    let near: Vec<String> = w
                        .objects
                        .values()
                        .filter(|q| {
                            matches!(q.kind, Kind::Placeable)
                                && (q.position[0] - o.position[0])
                                    .hypot(q.position[1] - o.position[1])
                                    < 2.0
                        })
                        .map(|q| format!("{}({})", q.tag, q.template))
                        .collect();
                    let e = hits.entry(key).or_insert((0, String::new()));
                    e.0 += 1;
                    e.1 = format!(
                        "{} at ({:.1},{:.1},{:.1}); near: {}",
                        o.tag,
                        o.position[0],
                        o.position[1],
                        o.position[2],
                        near.join(" ")
                    );
                }
            }
        }
    }
    let mut v: Vec<_> = hits.values().collect();
    v.sort_by_key(|a| std::cmp::Reverse(a.0));
    for (n, text) in v.iter().take(12) {
        println!("{n:5} steps  {text}");
    }
    println!("{module}: {steps} steps through something solid, {stuck} walkers held up for 6 s, in {seconds} s");
}
