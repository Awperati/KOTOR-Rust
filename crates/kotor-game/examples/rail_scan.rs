//! Finds the edges of the floor in a module that a pushed creature could go over: no wall of the walkmesh in the way, and
//! either a floor far below or nothing. Prints how many of each, the longest unbroken runs of open edge over nothing, and the
//! things called "Remains" with the nearest such run: a body beside a rail is a clue to where one is.
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
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for o in w.objects.values() {
            lo = [lo[0].min(o.position[0]), lo[1].min(o.position[1])];
            hi = [hi[0].max(o.position[0]), hi[1].max(o.position[1])];
        }
        let (x0, y0, x1, y1) = (lo[0] - 60.0, lo[1] - 60.0, hi[0] + 60.0, hi[1] + 60.0);
        // Every open edge over nothing, as the place on the floor and the way to the edge.
        let mut open: Vec<[f32; 5]> = Vec::new();
        let (mut over_floor, mut over_void, mut walled) = (0, 0, 0);
        let mut x = x0;
        while x < x1 {
            let mut y = y0;
            while y < y1 {
                for z in [10.0_f32, 4.0, 0.0, -5.0, -20.0] {
                    let Some(fz) = nav.walkable_at(x, y, z) else {
                        continue;
                    };
                    for (dx, dy) in [(1.0_f32, 0.0_f32), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                        let (nx, ny) = (x + dx, y + dy);
                        let found = nav.height_at(nx, ny, fz + 0.4);
                        if found.is_some_and(|h| h >= fz - 1.0) {
                            continue;
                        }
                        if nav.sight_blocked([x, y, fz + 0.6], [nx, ny, fz + 0.6]) {
                            walled += 1;
                            continue;
                        }
                        if nav.locate(nx, ny, fz - 1.0).is_some() {
                            over_floor += 1;
                        } else {
                            over_void += 1;
                            open.push([x, y, fz, dx, dy]);
                        }
                    }
                    break;
                }
                y += 1.0;
            }
            x += 1.0;
        }
        println!(
            "{module}: lowest floor {:?}; edges over a lower floor {over_floor}, over nothing {over_void}, walled {walled}",
            nav.lowest_floor()
        );
        // Runs: edge cells that touch, in the same direction.
        let mut used = vec![false; open.len()];
        let mut runs: Vec<(usize, [f32; 3], [f32; 2])> = Vec::new();
        for i in 0..open.len() {
            if used[i] {
                continue;
            }
            let mut stack = vec![i];
            used[i] = true;
            let (mut n, mut sx, mut sy, mut sz) = (0, 0.0, 0.0, 0.0);
            while let Some(k) = stack.pop() {
                n += 1;
                sx += open[k][0];
                sy += open[k][1];
                sz += open[k][2];
                for j in 0..open.len() {
                    if !used[j]
                        && open[j][3] == open[k][3]
                        && open[j][4] == open[k][4]
                        && (open[j][0] - open[k][0]).hypot(open[j][1] - open[k][1]) < 1.5
                        && (open[j][2] - open[k][2]).abs() < 0.5
                    {
                        used[j] = true;
                        stack.push(j);
                    }
                }
            }
            let f = n as f32;
            runs.push((n, [sx / f, sy / f, sz / f], [open[i][3], open[i][4]]));
        }
        runs.sort_by_key(|r| std::cmp::Reverse(r.0));
        for (n, c, d) in runs.iter().take(6) {
            println!(
                "  run of {n} m round {:.1} {:.1} at height {:.1}, edge toward {} {}",
                c[0], c[1], c[2], d[0], d[1]
            );
        }
        // What stands within 12 m of the longest runs: a body beside a rail is a clue.
        for (n, c, _) in runs.iter().take(3) {
            let near: Vec<String> = w
                .objects
                .values()
                .filter(|o| (o.position[0] - c[0]).hypot(o.position[1] - c[1]) < 12.0)
                .map(|o| format!("{}@{:.0},{:.0}", o.tag, o.position[0], o.position[1]))
                .take(14)
                .collect();
            println!("  near the run of {n}: {}", near.join(" "));
        }
    }
}
