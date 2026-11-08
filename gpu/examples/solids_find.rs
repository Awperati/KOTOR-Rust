//! Lists the solid things that stand on open floor (a light bar, a crate): connected patches where something solid is between
//! 0.25 m and 1.2 m over a floor the walkmesh has. `solids_find <module> [step]`
use kotor_gpu::stage::Stage;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let module = a.get(1).cloned().unwrap_or_else(|| "tar_m02ac".into());
    let step: f32 = a.get(2).and_then(|v| v.parse().ok()).unwrap_or(0.1);
    let mut s = Stage::open(&module, true).expect("module");
    s.enable_tutorials(false);
    s.update(1.0 / 60.0);
    let barriers = s.game.world.barriers.clone().expect("barriers");
    let nav = s.game.world.nav.as_ref().expect("nav");
    let z0 = s.game.world.object(s.game.world.pc).unwrap().position[2];
    let (w, h) = ((300.0 / step) as usize, (300.0 / step) as usize);
    let mut hit = vec![false; w * h];
    for iy in 0..h {
        for ix in 0..w {
            let (x, y) = (ix as f32 * step, iy as f32 * step);
            if let Some(f) = nav.height_at(x, y, z0 + 3.0) {
                hit[iy * w + ix] = barriers.across([x - 0.04, y], [x + 0.04, y], f + 0.25, f + 1.2)
                    || barriers.across([x, y - 0.04], [x, y + 0.04], f + 0.25, f + 1.2);
            }
        }
    }
    let mut seen = vec![false; w * h];
    for start in 0..w * h {
        if !hit[start] || seen[start] {
            continue;
        }
        let mut stack = vec![start];
        seen[start] = true;
        let (mut lo, mut hi, mut n) = ([usize::MAX; 2], [0usize; 2], 0usize);
        while let Some(i) = stack.pop() {
            let (ix, iy) = (i % w, i / w);
            n += 1;
            lo = [lo[0].min(ix), lo[1].min(iy)];
            hi = [hi[0].max(ix), hi[1].max(iy)];
            for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (ix as i32 + dx, iy as i32 + dy);
                if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                    continue;
                }
                let j = ny as usize * w + nx as usize;
                if hit[j] && !seen[j] {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        let (bw, bh) = (
            (hi[0] - lo[0] + 1) as f32 * step,
            (hi[1] - lo[1] + 1) as f32 * step,
        );
        let area = n as f32 * step * step;
        if area > 0.05 && area < 60.0 {
            println!(
                "patch at ({:.1},{:.1}) size {:.1} x {:.1} m, solid area {:.2} m2",
                lo[0] as f32 * step,
                lo[1] as f32 * step,
                bw,
                bh,
                area
            );
        }
    }
}
