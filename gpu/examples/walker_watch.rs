//! Watches the people who walk routes in a module with no window, and says who gets stuck or jitters, and by what.
//!
//!   cargo run --release --example walker_watch -- tar_m02aa 600
//!
//! The second argument is game seconds (default 300). Every second the position of each walker (a creature, not the player or the
//! party, whose first action is a walk) is noted. Over each 6 s window a walker is
//! - STUCK when it moved under 0.3 m while it had a walk to do the whole window;
//! - JITTER when its steps (10 a second) reversed direction 4 times or more (it went back and forth).
//!
//! Each incident prints the module, the tag, where, and the placeables within 3 m of there.
use kotor_archive::Install;
use kotor_game::{Action, Game, Kind, World};

const STEPS_A_SECOND: usize = 60;
/// Samples a second.
const SAMPLES: usize = 10;
const WINDOW: usize = 6;

fn open(module: &str) -> Option<Game> {
    let dir = kotor_core::install_dir()?;
    let mut inst = Install::open(&dir).ok()?;
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).unwrap();
    w.interactive_dialogue = true;
    w.load_module(module).unwrap();
    let spec = w.recommended_character("Player", 0, 1).unwrap();
    w.create_character(&spec).unwrap();
    w.enter_area();
    Some(Game::new(w))
}

fn apart(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn main() {
    let module = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "tar_m02aa".into());
    let seconds: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(300);
    let Some(mut g) = open(&module) else {
        println!("no install");
        return;
    };
    // Where each walker was each second, and whether it had a walk to do.
    let mut trail: std::collections::BTreeMap<u32, Vec<([f32; 3], bool, String)>> =
        Default::default();
    let mut incidents = 0;
    for sec in 0..seconds {
        for _ in 0..SAMPLES {
            for _ in 0..STEPS_A_SECOND / SAMPLES {
                g.step_once();
            }
            let pc = g.world.pc;
            let party = g.world.party.clone();
            for o in g.world.objects.values() {
                if o.kind != Kind::Creature || o.dead || o.id == pc || party.contains(&o.id) {
                    continue;
                }
                let walking = matches!(o.actions.front(), Some(Action::MoveTo { .. }));
                let note = match o.actions.front() {
                    Some(Action::MoveTo { target, .. }) => format!(
                        "to {}",
                        target
                            .and_then(|t| g.world.object(t))
                            .map(|t| t.tag.clone())
                            .unwrap_or_default()
                    ),
                    Some(_) => "other".to_string(),
                    None => "none".to_string(),
                };
                trail
                    .entry(o.id)
                    .or_default()
                    .push((o.position, walking, note));
            }
        }
        if sec % WINDOW != WINDOW - 1 {
            continue;
        }
        for (id, t) in trail.iter_mut() {
            let n = WINDOW * SAMPLES;
            if t.len() < n + 1 {
                continue;
            }
            let w = &t[t.len() - n - 1..];
            let (first, last) = (w[0].0, w[n].0);
            let ground: f32 = w.windows(2).map(|p| apart(p[0].0, p[1].0)).sum();
            let always = w.iter().all(|p| p.1);
            let net = apart(first, last);
            // Direction reversals between steps that each moved: a walker that goes back and forth.
            let steps: Vec<[f32; 2]> = w
                .windows(2)
                .map(|p| [p[1].0[0] - p[0].0[0], p[1].0[1] - p[0].0[1]])
                .filter(|v| v[0].hypot(v[1]) > 0.01)
                .collect();
            let flips = steps
                .windows(2)
                .filter(|p| p[0][0] * p[1][0] + p[0][1] * p[1][1] < 0.0)
                .count();
            let kind = if always && ground < 0.3 {
                "STUCK"
            } else if flips >= 4 {
                "JITTER"
            } else {
                continue;
            };
            let Some(o) = g.world.object(*id) else {
                continue;
            };
            incidents += 1;
            let near: Vec<String> = g
                .world
                .objects
                .values()
                .filter(|p| {
                    (matches!(p.kind, Kind::Placeable | Kind::Door)
                        || (p.kind == Kind::Creature && p.id != *id && !p.dead))
                        && apart(p.position, last) < 3.0
                })
                .map(|p| {
                    format!(
                        "{}({})@{:.1},{:.1}",
                        p.tag, p.template, p.position[0], p.position[1]
                    )
                })
                .collect();
            if std::env::var_os("TRACE").is_some() {
                for (i, p) in w.iter().enumerate().step_by(3) {
                    println!(
                        "    {:.1}s ({:.2},{:.2}) {}",
                        (sec + 1) as f32 - WINDOW as f32 + i as f32 / SAMPLES as f32,
                        p.0[0],
                        p.0[1],
                        p.2
                    );
                }
            }
            println!(
                "{kind} t={}s {module} {} at ({:.1},{:.1},{:.1}) ground {ground:.1} m net {net:.1} m flips {flips}; near: {}",
                sec + 1,
                o.tag,
                last[0],
                last[1],
                last[2],
                near.join(" ")
            );
        }
    }
    println!("{module}: {incidents} incidents in {seconds} s");
}
