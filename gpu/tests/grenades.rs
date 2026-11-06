//! What a grenade shows on the screen.

use kotor_gpu::stage::Stage;

const DT: f32 = 1.0 / 30.0;

#[test]
fn a_grenade_bursts_where_it_lands() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let mut s = Stage::open("end_m01aa", true).unwrap();
    s.update(DT);
    let pc = s.game.world.pc;
    let vertices =
        |s: &mut Stage| -> usize { s.frame_particles().iter().map(|b| b.vertices.len()).sum() };
    let before = vertices(&mut s);
    let w = &mut s.game.world;
    let grenade = w.spawn_item("g_w_fraggren01");
    w.carry(pc, grenade);
    w.use_item_on(pc, grenade, pc).unwrap();
    assert!(!s.game.world.effects_at.is_empty());
    let mut most = 0;
    for _ in 0..30 {
        s.update(DT);
        most = most.max(vertices(&mut s));
    }
    assert!(s.game.world.effects_at.is_empty(), "taken");
    assert!(most > before, "{before} then {most}");
}
