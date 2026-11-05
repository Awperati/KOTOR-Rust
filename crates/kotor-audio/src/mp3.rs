//! MP3 frame headers: enough to recognise MP3 data and measure it (not to decode it).

/// What walking the frames of an MP3 stream found.
#[derive(Clone, Debug, PartialEq)]
pub struct Mp3Info {
    /// Byte offset of the first frame in the data given (after any ID3 tag).
    pub offset: usize,
    /// 1, 2 or 25 for MPEG-1, MPEG-2 and MPEG-2.5.
    pub version: u8,
    /// 1, 2 or 3.
    pub layer: u8,
    pub sample_rate: u32,
    pub channels: u16,
    /// Bit rate of the first frame in kilobits per second.
    pub bitrate_kbps: u32,
    pub frames: usize,
    pub samples_per_channel: u64,
    pub duration_secs: f64,
}

struct Frame {
    len: usize,
    samples: u32,
    sample_rate: u32,
    channels: u16,
    version: u8,
    layer: u8,
    bitrate_kbps: u32,
}

const BITRATE_V1_L1: [u32; 15] = [
    0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448,
];
const BITRATE_V1_L2: [u32; 15] = [
    0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384,
];
const BITRATE_V1_L3: [u32; 15] = [
    0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
];
const BITRATE_V2_L1: [u32; 15] = [
    0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256,
];
const BITRATE_V2_L23: [u32; 15] = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160];

fn frame_at(d: &[u8], at: usize) -> Option<Frame> {
    let h = d.get(at..at + 4)?;
    if h[0] != 0xff || h[1] & 0xe0 != 0xe0 {
        return None;
    }
    let version = match (h[1] >> 3) & 3 {
        3 => 1,
        2 => 2,
        0 => 25,
        _ => return None,
    };
    let layer = match (h[1] >> 1) & 3 {
        3 => 1,
        2 => 2,
        1 => 3,
        _ => return None,
    };
    let bi = (h[2] >> 4) as usize;
    let si = ((h[2] >> 2) & 3) as usize;
    if bi == 0 || bi == 15 || si == 3 {
        return None;
    }
    let table = match (version, layer) {
        (1, 1) => &BITRATE_V1_L1,
        (1, 2) => &BITRATE_V1_L2,
        (1, _) => &BITRATE_V1_L3,
        (_, 1) => &BITRATE_V2_L1,
        _ => &BITRATE_V2_L23,
    };
    let bitrate_kbps = table[bi];
    let sample_rate = match version {
        1 => [44100, 48000, 32000][si],
        2 => [22050, 24000, 16000][si],
        _ => [11025, 12000, 8000][si],
    };
    let pad = ((h[2] >> 1) & 1) as u32;
    let (len, samples) = match layer {
        1 => ((12 * bitrate_kbps * 1000 / sample_rate + pad) * 4, 384),
        2 => (144 * bitrate_kbps * 1000 / sample_rate + pad, 1152),
        _ if version == 1 => (144 * bitrate_kbps * 1000 / sample_rate + pad, 1152),
        _ => (72 * bitrate_kbps * 1000 / sample_rate + pad, 576),
    };
    Some(Frame {
        len: len as usize,
        samples,
        sample_rate,
        channels: if h[3] >> 6 == 3 { 1 } else { 2 },
        version,
        layer,
        bitrate_kbps,
    })
}

/// Skip an ID3v2 tag at the start, returning the offset after it.
fn after_id3(d: &[u8]) -> usize {
    if d.len() >= 10 && &d[..3] == b"ID3" {
        let size = d[6..10]
            .iter()
            .fold(0usize, |a, b| (a << 7) | (*b as usize & 0x7f));
        let footer = if d[5] & 0x10 != 0 { 10 } else { 0 };
        return (10 + size + footer).min(d.len());
    }
    0
}

/// Measure MP3 data that starts with its frames (after an optional ID3 tag): at least three frames in a row
/// must chain with matching sample rate, or the whole stream if it is shorter. `None` if it does not.
/// `strict_start` demands the first frame right after the tag; otherwise leading junk is skipped.
fn probe(d: &[u8], strict_start: bool) -> Option<Mp3Info> {
    let start = after_id3(d);
    let limit = if strict_start {
        start + 1
    } else {
        d.len().min(start + 4096)
    };
    for first in start..limit {
        let Some(f0) = frame_at(d, first) else {
            continue;
        };
        let (mut at, mut frames, mut samples) = (first, 0usize, 0u64);
        while let Some(f) = frame_at(d, at) {
            if f.sample_rate != f0.sample_rate || f.layer != f0.layer || f.version != f0.version {
                break;
            }
            if at + f.len > d.len() {
                // A last frame cut short by the end of the data counts only after two whole ones.
                if frames >= 2 {
                    frames += 1;
                    samples += f.samples as u64;
                    at = d.len();
                }
                break;
            }
            frames += 1;
            samples += f.samples as u64;
            at += f.len;
        }
        // Three frames in a row, or a shorter stream that is exactly its frames.
        if frames >= 3 || (frames >= 1 && at == d.len()) {
            return Some(Mp3Info {
                offset: first,
                version: f0.version,
                layer: f0.layer,
                sample_rate: f0.sample_rate,
                channels: f0.channels,
                bitrate_kbps: f0.bitrate_kbps,
                frames,
                samples_per_channel: samples,
                duration_secs: samples as f64 / f0.sample_rate as f64,
            });
        }
        if strict_start {
            return None;
        }
    }
    None
}

/// Measure raw MP3 data (skipping an ID3 tag and a little junk before the first frame).
pub fn probe_mp3(data: &[u8]) -> Option<Mp3Info> {
    probe(data, false)
}

/// Like [`probe_mp3`] but the first frame must follow the tag at once: used on what follows a RIFF header, where a
/// stray sync pattern in PCM samples must not be taken for MP3.
pub(crate) fn probe_mp3_at_start(data: &[u8]) -> Option<Mp3Info> {
    probe(data, true)
}

/// The MP3 bytes of a file, whichever way it is wrapped: the file itself if it is raw MP3, or what follows the
/// `data` chunk header of a RIFF wrapper (the fake 36 or 58 byte header). `None` if it holds no MP3.
pub fn mp3_payload(data: &[u8]) -> Option<&[u8]> {
    let data = crate::wav::unwrap_sfx(data);
    if let Some(body) = crate::wav::data_chunk_body(data) {
        return probe_mp3_at_start(body).map(|_| body);
    }
    probe_mp3(data).map(|_| data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode, probe, AudioError, Codec};

    /// `n` MPEG-1 Layer III frames at 128 kbit/s and 44100 Hz (417 bytes each, joint stereo), padding bytes zero.
    fn frames(n: usize) -> Vec<u8> {
        let mut v = Vec::new();
        for _ in 0..n {
            v.extend([0xff, 0xfb, 0x90, 0x44]);
            v.extend(std::iter::repeat_n(0u8, 417 - 4));
        }
        v
    }

    #[test]
    fn frames_are_walked_and_measured() {
        let m = probe_mp3(&frames(10)).unwrap();
        assert_eq!(
            (
                m.version,
                m.layer,
                m.sample_rate,
                m.channels,
                m.bitrate_kbps
            ),
            (1, 3, 44100, 2, 128)
        );
        assert_eq!((m.frames, m.offset), (10, 0));
        assert_eq!(m.samples_per_channel, 10 * 1152);
        assert!((m.duration_secs - 10.0 * 1152.0 / 44100.0).abs() < 1e-9);
    }

    #[test]
    fn an_id3_tag_and_leading_junk_are_skipped() {
        let mut d = b"ID3\x03\x00\x00\x00\x00\x00\x05hello".to_vec(); // 5-byte tag body
        d.extend(frames(4));
        assert_eq!(probe_mp3(&d).unwrap().offset, 15);
        let mut j = vec![0u8; 7];
        j.extend(frames(4));
        assert_eq!(probe_mp3(&j).unwrap().offset, 7);
    }

    #[test]
    fn a_frame_cut_short_at_the_end_still_counts_and_noise_is_not_mp3() {
        let mut d = frames(5);
        d.truncate(d.len() - 100);
        assert_eq!(probe_mp3(&d).unwrap().frames, 5);
        assert_eq!(
            probe_mp3(&[0xff; 64]),
            None,
            "all-ones is no frame header (bitrate index 15)"
        );
        assert_eq!(probe_mp3(&[0u8; 2000]), None);
    }

    #[test]
    fn mpeg2_layer3_frames_are_half_as_long() {
        // MPEG-2 (version bits 10), layer III, 64 kbit/s, 22050 Hz: 72 * 64000 / 22050 = 208 bytes, 576 samples.
        let mut v = Vec::new();
        for _ in 0..6 {
            v.extend([0xff, 0xf3, 0x80, 0xc4]);
            v.extend(std::iter::repeat_n(0u8, 208 - 4));
        }
        let m = probe_mp3(&v).unwrap();
        assert_eq!(
            (m.version, m.sample_rate, m.channels, m.frames),
            (2, 22050, 1, 6)
        );
        assert_eq!(m.samples_per_channel, 6 * 576);
    }

    /// The game's wrapper: a RIFF header with an 18-byte fmt, a fact chunk and a data chunk of size 0, then MP3.
    fn fake_header() -> Vec<u8> {
        let mut h = b"RIFF\x32\0\0\0WAVEfmt \x12\0\0\0".to_vec();
        h.extend([
            1, 0, 1, 0, 0x22, 0x56, 0, 0, 0x22, 0x56, 0, 0, 1, 0, 8, 0, 0, 0,
        ]);
        h.extend(b"fact\x04\0\0\0\0\0\0\0data\0\0\0\0");
        h
    }

    #[test]
    fn the_58_byte_fake_riff_header_in_front_of_mp3_is_seen_through() {
        let mut d = fake_header();
        assert_eq!(d.len(), 58);
        d.extend(frames(8));
        match decode(&d) {
            Err(AudioError::Mp3(m)) => assert_eq!((m.offset, m.frames), (58, 8)),
            other => panic!("{other:?}"),
        }
        let info = probe(&d).unwrap();
        assert_eq!(
            (info.codec, info.sample_rate, info.channels),
            (Codec::Mp3, 44100, 2)
        );
        assert_eq!(mp3_payload(&d).unwrap().len(), 8 * 417);
        assert_eq!(
            mp3_payload(&frames(3)).unwrap().len(),
            3 * 417,
            "raw MP3 is its own payload"
        );
    }

    #[test]
    fn pcm_that_happens_to_look_like_a_sync_word_is_still_pcm() {
        // 16-bit samples -1 (ff ff) and -32 (e0 ff) begin with bytes that look like an MP3 sync.
        let body: Vec<u8> = [0xff, 0xff, 0xe0, 0xff, 0xff, 0xff].to_vec();
        let mut w = b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0".to_vec();
        w.extend([1, 0, 1, 0, 0x22, 0x56, 0, 0, 0x44, 0xac, 0, 0, 2, 0, 16, 0]);
        w.extend(b"data");
        w.extend((body.len() as u32).to_le_bytes());
        w.extend(&body);
        let a = decode(&w).unwrap();
        assert_eq!(a.samples, vec![-1, -32, -1]);
    }
}
