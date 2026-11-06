//! Where the game's floors end over a lower floor, for the movement driver: a spot beside a drop a player could fall off.
//!
//! `cargo run --release -p kotor-render --example ledges [module ...]`, with KOTOR_DIR set. Prints each module's deepest
//! drops within reach of Modern movement: `module x y z heading drop`.

use kotor_archive::Install;

/// Drops a walk leaves as a slope or stair, and those it refuses as a wall (see `kotor_game::Tuning`).
const STEP_DOWN: f32 = 0.35;
const MAX_DROP: f32 = 6.0;
const STEP_UP: f32 = 0.6;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::var("KOTOR_DIR").unwrap_or_else(|_| r"E:\Games\Star Wars - KotOR".into());
    let mut names: Vec<String> = std::env::args().skip(1).collect();
    if names.is_empty() {
        for e in std::fs::read_dir(std::path::Path::new(&dir).join("modules"))? {
            let n = e?.file_name().to_string_lossy().to_ascii_lowercase();
            if let Some(m) = n.strip_suffix(".rim").filter(|m| !m.ends_with("_s")) {
                names.push(m.to_string());
            }
        }
    }
    names.sort();
    for module in names {
        let mut inst = Install::open(std::path::Path::new(&dir))?;
        if let Err(e) = inst.load_module(&module) {
            eprintln!("{module}: {e}");
            continue;
        }
        let Some(area) = kotor_gff::Gff::parse(&inst.read("module", kotor_core::ResType::IFO)?)?
            .root
            .str("Mod_Entry_Area")
            .map(str::to_string)
        else {
            continue;
        };
        let floor = match kotor_render::Floor::load(&mut inst, &area) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("{module}: {e}");
                continue;
            }
        };
        let nav = floor.navmesh();
        eprintln!("{module}: {} faces", nav.face_count());
        let mut found: Vec<(f32, [f32; 3], f32)> = Vec::new();
        for i in 0..nav.face_count() {
            let c = nav.face_centroid(i);
            for k in 0..16 {
                let a = k as f32 * std::f32::consts::TAU / 16.0;
                let (dx, dy) = (a.cos(), a.sin());
                let mut z = c[2];
                let mut d = 0.1;
                while d < 4.0 {
                    let (x, y) = (c[0] + dx * d, c[1] + dy * d);
                    let Some(f) = nav.height_at(x, y, z + STEP_UP) else {
                        // The floor ends: over a gap, as a cliff leaves, the body looks 1.2 m on for a lower one.
                        for k in 1..=4 {
                            let s = k as f32 * 0.3;
                            let (qx, qy) = (x + dx * s, y + dy * s);
                            if let Some(low) = nav.height_at(qx, qy, z - STEP_DOWN) {
                                let from = [x - dx * 0.4, y - dy * 0.4, z + 1.0];
                                let clear = !nav.sight_blocked(from, [qx, qy, z + 1.0]);
                                let back =
                                    nav.same_island([x - dx * 0.2, y - dy * 0.2, z], [qx, qy, low]);
                                if clear && back && z - low < MAX_DROP {
                                    found.push((z - low, [c[0], c[1], c[2]], a));
                                }
                                break;
                            }
                        }
                        break;
                    };
                    let drop = z - f;
                    if drop > STEP_DOWN {
                        if drop < MAX_DROP && nav.same_island([c[0], c[1], c[2]], [x, y, f]) {
                            // A run-up: back along the way it came, while the floor goes on.
                            let mut back = 0.0;
                            let mut spot = [c[0], c[1], c[2]];
                            while back < 4.0 {
                                let (x, y) = (c[0] - dx * (back + 0.1), c[1] - dy * (back + 0.1));
                                match nav.height_at(x, y, spot[2] + STEP_UP) {
                                    Some(h) if (h - spot[2]).abs() < STEP_DOWN => {
                                        spot = [x, y, h];
                                        back += 0.1;
                                    }
                                    _ => break,
                                }
                            }
                            found.push((drop, spot, a));
                        }
                        break;
                    }
                    z = f;
                    d += 0.1;
                }
            }
        }
        found.sort_by(|a, b| b.0.total_cmp(&a.0));
        eprintln!(
            "{module}: {} spots and headings with a drop over 0.5 m",
            found.iter().filter(|f| f.0 > 0.5).count()
        );
        for (drop, p, a) in found.iter().take(3) {
            println!(
                "{module} {:.2} {:.2} {:.2} heading {a:.3} drop {drop:.2}",
                p[0], p[1], p[2]
            );
        }
    }
    Ok(())
}
