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

#[test]
fn every_streamed_file_decodes_and_lasts_as_long_as_its_header_says() {
    let (mut mp3, mut other, mut worst) = (0, 0, 0.0f64);
    for sub in ["streamwaves", "streammusic", "streamsounds"] {
        for p in files(sub) {
            let d = std::fs::read(&p).unwrap();
            let want = kotor_audio::probe(&d).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            let a = decode_any(&d).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            assert!(a.frames() > 0, "{}", p.display());
            assert_eq!(a.channels, want.channels, "{}", p.display());
            assert_eq!(a.sample_rate, want.sample_rate, "{}", p.display());
            if a.codec == Codec::Mp3 {
                mp3 += 1;
                let rel =
                    (a.duration_secs() - want.duration_secs).abs() / want.duration_secs.max(0.01);
                worst = worst.max(rel);
                assert!(
                    rel < 0.05,
                    "{}: {} s decoded, {} s probed",
                    p.display(),
                    a.duration_secs(),
                    want.duration_secs
                );
            } else {
                other += 1;
            }
        }
    }
    println!(
        "{mp3} MP3 files decoded, {other} others; worst length error {:.2}%",
        worst * 100.0
    );
}
