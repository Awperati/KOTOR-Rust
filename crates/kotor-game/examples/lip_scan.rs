//! What the install has for a talking face: the `.lip` files (viseme keyframes of a voice clip) in `lips/*.mod`, how many each
//! holds, and one parsed.
//!
//!   cargo run --release -p kotor-game --example lip_scan
use kotor_archive::{Erf, Source};
use kotor_core::ResType;

fn main() {
    let dir = kotor_core::install_dir().expect("no install");
    let mut files: Vec<_> = std::fs::read_dir(dir.join("lips"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    files.sort();
    let (mut total, mut sample) = (0, None);
    for f in &files {
        let Ok(erf) = Erf::open(Source::file(f.clone())) else {
            continue;
        };
        let lips: Vec<_> = erf
            .entries
            .iter()
            .filter(|e| e.restype == ResType(3004))
            .collect();
        total += lips.len();
        println!(
            "{}: {} entries, {} lips",
            f.file_name().unwrap().to_string_lossy(),
            erf.entries.len(),
            lips.len()
        );
        if sample.is_none() && f.file_name().is_some_and(|n| n == "end_m01aa_loc.mod") {
            sample = lips
                .first()
                .and_then(|e| Some((e.resref.as_str().to_string(), erf.read(e).ok()?)));
        }
    }
    println!("total lips: {total}");
    if let Some((name, b)) = sample {
        println!(
            "{name}: {} bytes, header {:?}",
            b.len(),
            String::from_utf8_lossy(&b[..8])
        );
        let len = f32::from_le_bytes([b[8], b[9], b[10], b[11]]);
        let n = u32::from_le_bytes([b[12], b[13], b[14], b[15]]);
        println!("length {len} s, {n} keys");
        for k in 0..(n as usize).min(16) {
            let o = 16 + k * 5;
            let t = f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
            println!("  {t:.3} s shape {}", b[o + 4]);
        }
    }
}
