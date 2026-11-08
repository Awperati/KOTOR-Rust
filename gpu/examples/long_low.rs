//! Meshes of a module that are long and low, like a light bar: `long_low <module>`.
use kotor_gpu::stage::Stage;

fn main() {
    let module = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "tar_m02ac".into());
    let mut s = Stage::open(&module, true).expect("module");
    s.update(1.0 / 60.0);
    for m in &s.static_scene().meshes {
        let lo = |k: usize| m.positions.iter().map(|p| p[k]).fold(f32::MAX, f32::min);
        let hi = |k: usize| m.positions.iter().map(|p| p[k]).fold(f32::MIN, f32::max);
        let (dx, dy, dz) = (hi(0) - lo(0), hi(1) - lo(1), hi(2) - lo(2));
        let (long, short) = (dx.max(dy), dx.min(dy));
        if long > 4.0 && long < 14.0 && short < 2.0 && dz < 1.3 && dz > 0.2 {
            println!(
                "{} {:?} {:.1}x{:.1}x{:.2} at ({:.1},{:.1},{:.2}) tris {}",
                m.name,
                m.texture,
                dx,
                dy,
                dz,
                lo(0),
                lo(1),
                lo(2),
                m.triangles.len()
            );
        }
    }
}
