//! Short obstacles do not end sight; walls and closed doors do (bug 25).

use kotor_archive::Install;
use kotor_game::{Kind, World};

#[test]
fn sight_looks_over_gaps_in_the_floor_but_not_through_walls() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut base = Install::open(dir).unwrap();
    base.set_prefer_rim(true);
    let mut w = World::new(base).unwrap();
    w.load_module("end_m01aa").unwrap();
    let nav = w.nav.as_ref().unwrap();
    assert!(nav.wall_count() > 100, "no walls kept for sight");
    let pos: Vec<[f32; 3]> = w
        .objects
        .values()
        .filter(|o| o.kind == Kind::Creature)
        .map(|o| o.position)
        .collect();
    let (mut newly_seen, mut lost) = (0, 0);
    for i in 0..pos.len() {
        for j in i + 1..pos.len() {
            let (a, b) = (pos[i], pos[j]);
            let len = (a[0] - b[0]).hypot(a[1] - b[1]);
            if len > 20.0 {
                continue;
            }
            // What sight was: floor under every 0.75 m of the line, and no door across it.
            let steps = (len / 0.75).ceil().max(1.0) as usize;
            let top = a[2].max(b[2]) + 1.5;
            let old = !nav.crosses_block(a, b)
                && (1..steps).all(|k| {
                    let t = k as f32 / steps as f32;
                    nav.walkable_at(a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, top)
                        .is_some()
                });
            let new = w.can_see_along(a, b);
            let walled = nav.sight_blocked([a[0], a[1], a[2] + 1.5], [b[0], b[1], b[2] + 1.5]);
            if new && !old {
                newly_seen += 1;
            }
            if old && !new {
                lost += 1;
                assert!(walled, "sight lost with no wall: {a:?} {b:?}");
            }
            assert!(!(new && walled), "seen through a wall: {a:?} {b:?}");
        }
    }
    assert!(newly_seen > 0, "nothing can be seen over an obstacle");
    let _ = lost;
}
