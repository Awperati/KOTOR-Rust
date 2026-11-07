//! Heartbeats: every creature beats every six seconds, each at its own moment, not all in one frame (a stutter: the opening
//! module's two dozen scripts took 11 ms together).
use kotor_archive::Install;
use kotor_game::{Kind, World};

#[test]
fn heartbeats_are_spread_over_the_six_seconds() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut w = World::new(Install::open(dir).unwrap()).unwrap();
    w.load_module("end_m01aa").unwrap();
    w.enter_area();
    let creatures = w
        .objects
        .values()
        .filter(|o| o.kind == Kind::Creature && !o.dead && o.id != w.pc)
        .count();
    assert!(creatures >= 10, "{creatures}");
    let (mut most, mut frames_with_any) = (0u64, 0);
    let start = w.combat.heartbeats_run;
    let mut last = start;
    for _ in 0..(13.0 * 60.0) as usize {
        w.step(1.0 / 60.0);
        let n = w.combat.heartbeats_run - last;
        last = w.combat.heartbeats_run;
        most = most.max(n);
        frames_with_any += (n > 0) as u32;
    }
    let total = last - start;
    // Two rounds of everyone, and never more than a few in one frame.
    assert!(total >= 2 * creatures as u64, "{total} for {creatures}");
    assert!(most <= 4, "{most} in one frame");
    assert!(frames_with_any >= 20, "{frames_with_any} frames had one");
}
