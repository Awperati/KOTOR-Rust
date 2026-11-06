//! Watch for reads of the game's files that fail: open and read every resource of a module's archives, over and over,
//! with no retry, and log the path, the time and the operating system's error for each failure.
//!
//!   cargo run --release -p kotor-archive --example read_watch -- <seconds> <log file>
use kotor_archive::{Erf, Source};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};

fn stamp() -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:03}", t.as_secs(), t.subsec_millis())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seconds: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(60);
    let mut log = File::create(args.get(2).map_or("read_watch.log", String::as_str)).unwrap();
    let dir = kotor_core::install_dir()
        .expect("no install")
        .join("modules");
    let mut files = Vec::new();
    for name in ["end_m01aa.mod", "end_m01aa.rim", "end_m01aa_s.rim"] {
        let path = dir.join(name);
        let erf = Erf::open(Source::file(&path)).unwrap();
        let spots: Vec<(u64, usize)> = erf
            .entries
            .iter()
            .map(|e| (e.offset as u64, e.size as usize))
            .collect();
        files.push((path, spots));
    }
    let started = std::time::Instant::now();
    let (mut reads, mut failures) = (0u64, 0u64);
    while started.elapsed().as_secs() < seconds {
        for (path, spots) in &files {
            for (offset, size) in spots {
                reads += 1;
                let result = File::open(path).and_then(|mut f| {
                    f.seek(SeekFrom::Start(*offset))?;
                    let mut buf = vec![0u8; *size];
                    f.read_exact(&mut buf)
                });
                if let Err(e) = result {
                    failures += 1;
                    writeln!(
                        log,
                        "{} {} at {offset}: {e} (os error {:?})",
                        stamp(),
                        path.display(),
                        e.raw_os_error()
                    )
                    .unwrap();
                }
            }
        }
    }
    writeln!(log, "{} done: {reads} reads, {failures} failures", stamp()).unwrap();
}
