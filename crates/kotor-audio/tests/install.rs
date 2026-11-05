use kotor_archive::Install;
use kotor_audio::{decode, probe, AudioError, Codec};
use kotor_core::ResType;

fn install() -> Option<Install> {
    Some(Install::open(kotor_core::install_dir()?).unwrap())
}

#[test]
fn every_wav_in_the_bifs_is_pcm_adpcm_or_recognised_mp3() {
    let Some(mut inst) = install() else { return };
    let (mut pcm, mut adpcm, mut mp3) = (0, 0, 0);
    for name in inst.list(ResType(4)) {
        let d = inst.read(&name, ResType(4)).unwrap();
        let info = probe(&d).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(info.duration_secs > 0.0, "{name}: {info:?}");
        assert!(matches!(info.channels, 1 | 2), "{name}: {info:?}");
        match decode(&d) {
            Ok(a) => {
                assert_eq!(
                    (a.sample_rate, a.channels),
                    (info.sample_rate, info.channels),
                    "{name}"
                );
                assert_eq!(
                    a.frames() as u64,
                    info.frames,
                    "{name}: decoded length matches the probe"
                );
                match a.codec {
                    Codec::ImaAdpcm => adpcm += 1,
                    _ => pcm += 1,
                }
            }
            Err(AudioError::Mp3(m)) => {
                assert_eq!(m.sample_rate, info.sample_rate, "{name}");
                mp3 += 1;
            }
            Err(e) => panic!("{name}: {e}"),
        }
    }
    println!("pcm {pcm}, adpcm {adpcm}, mp3 {mp3}");
    // Every BIF sound is PCM or ADPCM: the few that begin with bytes like an MP3 sync are plain 16-bit samples.
    assert!(pcm > 1900 && adpcm == 4 && mp3 == 0, "{pcm} {adpcm} {mp3}");
}

#[test]
fn the_streamed_files_are_recognised_through_their_headers() {
    let Some(inst) = install() else { return };
    let root = inst.dir().to_path_buf();
    let mut seen = 0;
    for sub in ["streamwaves", "streamsounds", "streammusic"] {
        let Ok(rd) = std::fs::read_dir(root.join(sub)) else {
            continue;
        };
        for e in rd.flatten().take(150) {
            let d = std::fs::read(e.path()).unwrap();
            let info = probe(&d).unwrap_or_else(|er| panic!("{}: {er}", e.path().display()));
            assert!(info.duration_secs > 0.1, "{}: {info:?}", e.path().display());
            assert!(matches!(info.channels, 1 | 2));
            if let Err(AudioError::Mp3(m)) = decode(&d) {
                if d.starts_with(b"RIFF") {
                    // Behind the game's fake header: 58 bytes (18-byte fmt) or 44 (16-byte fmt), plus any ID3 tag.
                    assert!(
                        m.offset >= 44,
                        "{}: first frame at {}",
                        e.path().display(),
                        m.offset
                    );
                }
                seen += 1;
            }
        }
    }
    assert!(seen > 100, "{seen} MP3 files seen");
}

#[test]
fn every_streamed_file_is_decoded_or_recognised_as_mp3() {
    let Some(inst) = install() else { return };
    let root = inst.dir().to_path_buf();
    let (mut pcm, mut mp3, mut wrapped_pcm) = (0, 0, 0);
    for sub in ["streamwaves", "streamsounds", "streammusic"] {
        let Ok(rd) = std::fs::read_dir(root.join(sub)) else {
            continue;
        };
        for e in rd.flatten() {
            let Ok(d) = std::fs::read(e.path()) else {
                continue; // a sub-folder
            };
            match decode(&d) {
                Ok(a) => {
                    assert!(a.frames() > 0, "{}", e.path().display());
                    pcm += 1;
                    if !d.starts_with(b"RIFF") {
                        // A sound-effect stub of MP3 frames with a real WAVE behind it.
                        wrapped_pcm += 1;
                    }
                }
                Err(AudioError::Mp3(_)) => mp3 += 1,
                Err(er) => panic!("{}: {er}", e.path().display()),
            }
        }
    }
    println!("stream files: {pcm} decode to PCM ({wrapped_pcm} behind an MP3 stub), {mp3} are MP3");
    assert!(wrapped_pcm > 100 && mp3 > 100, "{wrapped_pcm} {mp3}");
}
