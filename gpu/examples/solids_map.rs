//! A map from above of what stops people: grey is floor, red is something solid between 0.25 m and 1.2 m over the floor
//! (the room shell the player's modern movement and walkers use), blue is no floor and nothing solid (a hole in the walkmesh).
//!
//!   cargo run --release --example solids_map -- tar_m02ac 135 90 12 out.png
use kotor_gpu::stage::Stage;
use kotor_image::Image;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let module = a.get(1).cloned().unwrap_or_else(|| "tar_m02ac".into());
    let f = |i: usize, d: f32| a.get(i).and_then(|s| s.parse().ok()).unwrap_or(d);
    let (cx, cy, half) = (f(2, 135.0), f(3, 90.0), f(4, 12.0));
    let out = a.get(5).cloned().unwrap_or_else(|| "solids.png".into());
    let mut s = Stage::open(&module, true).expect("module");
    s.enable_tutorials(false);
    s.update(1.0 / 60.0);
    let barriers = s.game.world.barriers.clone().expect("barriers");
    let nav = s.game.world.nav.as_ref().expect("nav");
    let z = s.game.world.object(s.game.world.pc).unwrap().position[2];
    for o in s.game.world.objects.values() {
        if (o.position[0] - cx).abs() < half && (o.position[1] - cy).abs() < half {
            println!("{:?} {} {:?}", o.kind, o.tag, o.position);
        }
    }
    let step = f(6, 0.04);
    let n = (2.0 * half / step) as u32;
    let mut img = Image {
        width: n,
        height: n,
        rgba: vec![255; (n * n * 4) as usize],
    };
    for py in 0..n {
        for px in 0..n {
            let x = cx - half + px as f32 * step;
            let y = cy + half - py as f32 * step;
            let floor = nav.height_at(x, y, z + 2.0);
            let zz = floor.unwrap_or(z);
            let solid = barriers.across([x - 0.03, y], [x + 0.03, y], zz + 0.25, zz + 1.2)
                || barriers.across([x, y - 0.03], [x, y + 0.03], zz + 0.25, zz + 1.2);
            let c = match (solid, floor.is_some()) {
                (true, _) => [220, 40, 40],
                (false, true) => [170, 170, 170],
                (false, false) => [60, 90, 200],
            };
            let i = ((py * n + px) * 4) as usize;
            img.rgba[i..i + 3].copy_from_slice(&c);
        }
    }
    std::fs::write(&out, kotor_image::to_png(&img)).unwrap();
    println!("{out}");
}
