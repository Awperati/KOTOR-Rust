//! How many fixture outlines a module has and how long they take to work out: `fixtures_cost tar_m02aa tar_m02ab ...`
use kotor_gpu::stage::Stage;

fn main() {
    for m in std::env::args().skip(1) {
        let mut s = Stage::open(&m, true).expect("module");
        s.update(1.0 / 60.0);
        let t = std::time::Instant::now();
        s.game.world.refresh_fixtures();
        println!(
            "{m}: {} outlines in {:.1} ms",
            s.game.world.fixtures.len(),
            t.elapsed().as_secs_f64() * 1000.0
        );
        let w = &s.game.world;
        let nav = w.nav.as_ref().unwrap();
        for o in w.objects.values() {
            if o.kind != kotor_game::Kind::Door || o.link_module.is_empty() {
                continue;
            }
            let n = [-o.door_axis[1], o.door_axis[0]];
            let (a, b) = (
                [
                    o.position[0] - n[0] * 2.0,
                    o.position[1] - n[1] * 2.0,
                    o.position[2],
                ],
                [
                    o.position[0] + n[0] * 2.0,
                    o.position[1] + n[1] * 2.0,
                    o.position[2],
                ],
            );
            if nav.crosses_fixture(a, b) {
                println!(
                    "  fixture across the way through door {} at {:?}",
                    o.tag, o.position
                );
            }
        }
    }
}
