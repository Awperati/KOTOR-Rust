use kotor_audio::Codec;
use kotor_audio_out::{decode_any, decode_mp3};

fn files(sub: &str) -> Vec<std::path::PathBuf> {
    let Some(dir) = kotor_core::install_dir() else {
        return vec![];
    };
    std::fs::read_dir(dir.join(sub))
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn garbage_is_an_error_not_a_panic() {
    assert!(decode_mp3(&[0u8; 100]).is_err());
    assert!(decode_any(b"not audio at all").is_err());
}

/// The checks on one file: whether it is an MP3, and how far its length is from what its header says.
fn check(p: &std::path::Path) -> (bool, f64) {
    let d = std::fs::read(p).unwrap();
    let want = kotor_audio::probe(&d).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    let a = decode_any(&d).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    assert!(a.frames() > 0, "{}", p.display());
    assert_eq!(a.channels, want.channels, "{}", p.display());
    assert_eq!(a.sample_rate, want.sample_rate, "{}", p.display());
    if a.codec != Codec::Mp3 {
        return (false, 0.0);
    }
    let rel = (a.duration_secs() - want.duration_secs).abs() / want.duration_secs.max(0.01);
    assert!(
        rel < 0.05,
        "{}: {} s decoded, {} s probed",
        p.display(),
        a.duration_secs(),
        want.duration_secs
    );
    (true, rel)
}

#[test]
fn every_streamed_file_decodes_and_lasts_as_long_as_its_header_says() {
    let all: Vec<std::path::PathBuf> = ["streamwaves", "streammusic", "streamsounds"]
        .iter()
        .flat_map(|sub| files(sub))
        .collect();
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get());
    let share = (all.len() / workers).max(1);
    let done: Vec<(bool, f64)> = std::thread::scope(|s| {
        let threads: Vec<_> = all
            .chunks(share)
            .map(|part| s.spawn(move || part.iter().map(|p| check(p)).collect::<Vec<_>>()))
            .collect();
        threads
            .into_iter()
            .flat_map(|t| t.join().expect("a file failed its checks"))
            .collect()
    });
    assert_eq!(done.len(), all.len());
    let mp3 = done.iter().filter(|d| d.0).count();
    let worst = done.iter().map(|d| d.1).fold(0.0, f64::max);
    println!(
        "{mp3} MP3 files decoded, {} others; worst length error {:.2}%",
        done.len() - mp3,
        worst * 100.0
    );
}
