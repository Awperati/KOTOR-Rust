//! What stands on open floor stops those on foot by its real outline: in Upper City North a round fixture in the middle of the
//! walkway has floor under it in the walkmesh, so the classic walker went through it while the modern body did not. Routes
//! and steps now keep to its outline (`NavMesh::crosses_fixture`, from `Barriers::outline`). Skips without the game.
use kotor_gpu::stage::Stage;

#[test]
fn a_fixture_standing_on_open_floor_is_walked_round_not_through() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let mut s = Stage::open("tar_m02ab", true).unwrap();
    s.enable_tutorials(false);
    s.update(1.0 / 60.0);
    let w = &s.game.world;
    assert!(w.fixtures.len() > 500, "{} outlines", w.fixtures.len());
    let nav = w.nav.as_ref().unwrap();
    let z = nav.height_at(120.26, 64.5, 5.0).expect("floor");
    let (from, to) = ([120.6, 64.5, z], [120.6, 62.0, z]);
    assert!(
        nav.crosses_fixture(from, to),
        "straight through the round fixture"
    );
    // A click on the far side: the walker's step along the straight way is turned along the outline.
    let mut p = from;
    let mut moved_round = false;
    for _ in 0..400 {
        let (dx, dy) = (to[0] - p[0], to[1] - p[1]);
        let l = dx.hypot(dy);
        if l < 0.05 {
            break;
        }
        let q = [p[0] + dx / l * 0.03, p[1] + dy / l * 0.03, z];
        let r = nav.slide_fixture(p, q);
        assert!(!nav.crosses_fixture(p, r), "never through");
        moved_round |= (r[0] - 120.6).abs() > 0.02;
        if std::env::var_os("FXDBG").is_some() {
            println!("{p:?} -> {r:?}");
        }
        p = r;
    }
    assert!(
        moved_round,
        "it went along the outline and past it: ended at {p:?}"
    );
    assert!((p[1] - 62.0).abs() < 0.2, "and reached the far side: {p:?}");
}
