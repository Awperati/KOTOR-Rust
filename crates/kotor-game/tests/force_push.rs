//! Force Push throws its target back and leaves it down for a few seconds.

use kotor_archive::Install;
use kotor_game::{EffectRec, World, DURATION_TEMPORARY};

#[test]
fn a_pushed_creature_slides_away_and_cannot_act_for_a_while() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut base = Install::open(dir).unwrap();
    base.set_prefer_rim(true);
    let mut w = World::new(base).unwrap();
    w.load_module("end_m01aa").unwrap();
    let spec = w.recommended_character("Player", 0, 1).unwrap();
    w.create_character(&spec).unwrap();
    let pc = w.pc;
    let p = w.object(pc).unwrap().position;
    let foe = w.spawn_creature("n_calonord", [p[0] + 0.8, p[1], p[2]]);
    let start = w.object(foe).unwrap().position;
    w.effects.push(EffectRec::new("EffectForcePushed"));
    let handle = w.effects.len() as u32;
    w.apply_effect(pc, foe, handle, DURATION_TEMPORARY, 0.1);
    assert!(w.is_disabled(foe));
    for _ in 0..40 {
        w.step(0.05);
    }
    let end = w.object(foe).unwrap().position;
    let moved = (end[0] - start[0]).hypot(end[1] - start[1]);
    assert!(moved > 1.0, "moved {moved} m");
    // Still down after 2 s, up after 3.5 s.
    assert!(w.is_disabled(foe));
    for _ in 0..30 {
        w.step(0.05);
    }
    assert!(!w.is_disabled(foe));
}
