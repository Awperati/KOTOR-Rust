//! Finds the edges of the floor in a module that a pushed creature could go over: no wall of the walkmesh in the way, and
//! either a floor far below or nothing. Prints how many of each, and a few places.
//!
//!   cargo run --release -p kotor-game --example rail_scan -- tar_m02aa [more modules]
use kotor_archive::Install;
use kotor_game::World;

fn main() {
    for module in std::env::args().skip(1) {
        let inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
        let mut w = World::new(inst).unwrap();
        if w.load_module(&module).is_err() {
            println!("{module}: cannot load");
            continue;
        }
        let nav = w.nav.as_ref().unwrap();
        println!("{module}: lowest floor {:?}", nav.lowest_floor());
        let (mut over_floor, mut over_void, mut walled) = (0, 0, 0);
        let mut shown = 0;
        let mut samples = Vec::new();
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for o in w.objects.values() {
            lo = [lo[0].min(o.position[0]), lo[1].min(o.position[1])];
            hi = [hi[0].max(o.position[0]), hi[1].max(o.position[1])];
        }
        let (x0, y0, x1, y1) = (lo[0] - 60.0, lo[1] - 60.0, hi[0] + 60.0, hi[1] + 60.0);
        let mut x = x0;
        while x < x1 {
            let mut y = y0;
            while y < y1 {
                for z in [10.0_f32, 4.0, 0.0, -5.0, -20.0] {
                    if let Some(fz) = nav.walkable_at(x, y, z) {
                        samples.push([x, y, fz]);
                        break;
                    }
                }
                y += 1.0;
            }
            x += 1.0;
        }
        for p in samples {
            for (dx, dy) in [(1.0_f32, 0.0_f32), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                let (nx, ny) = (p[0] + dx, p[1] + dy);
                let found = nav.height_at(nx, ny, p[2] + 0.4);
                if found.is_some_and(|z| z >= p[2] - 1.0) {
                    continue;
                }
                let blocked = nav.sight_blocked([p[0], p[1], p[2] + 0.6], [nx, ny, p[2] + 0.6]);
                if blocked {
                    walled += 1;
                    continue;
                }
                let below = nav.locate(nx + dx * 3.0, ny + dy * 3.0, p[2] - 0.3);
                if let Some((_, bz)) = below {
                    over_floor += 1;
                    if over_floor % 10 == 1 {
                        println!(
                            "  ledge at {:.1} {:.1} {:.1} toward {dx} {dy}: floor {:.1} below",
                            p[0],
                            p[1],
                            p[2],
                            p[2] - bz
                        );
                    }
                } else {
                    over_void += 1;
                }
                if shown < 6 && below.is_none() && p[2] - nav.lowest_floor().unwrap_or(0.0) > 3.0 {
                    shown += 1;
                    println!(
                        "  void edge at {:.1} {:.1} {:.1} toward {dx} {dy}",
                        p[0], p[1], p[2]
                    );
                }
            }
        }
        println!("{module}: open edges over a lower floor {over_floor}, over nothing {over_void}, walled {walled}");
    }
}
