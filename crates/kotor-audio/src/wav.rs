//! RIFF/WAVE files: PCM and IMA ADPCM to 16-bit samples, and the wrapper quirks.

use crate::mp3::{probe_mp3, probe_mp3_at_start};
use crate::AudioError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    Pcm8,
    Pcm16,
    ImaAdpcm,
    Mp3,
}

/// Decoded audio: interleaved signed 16-bit samples.
#[derive(Clone, Debug, PartialEq)]
pub struct Audio {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<i16>,
    pub codec: Codec,
}

impl Audio {
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1) as usize
    }
    pub fn duration_secs(&self) -> f64 {
        self.frames() as f64 / self.sample_rate.max(1) as f64
    }
}

/// What a file holds, found without decoding it.
#[derive(Clone, Debug, PartialEq)]
pub struct Info {
    pub codec: Codec,
    pub sample_rate: u32,
    pub channels: u16,
    /// Samples per channel.
    pub frames: u64,
    pub duration_secs: f64,
}

struct Fmt<'a> {
    tag: u16,
    channels: u16,
    rate: u32,
    align: u16,
    bits: u16,
    extra: &'a [u8],
}

struct Riff<'a> {
    fmt: Fmt<'a>,
    body: &'a [u8],
}

fn u16le(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(at..at + 2)?.try_into().ok()?))
}
fn u32le(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

/// Walk the chunks. A `data` chunk whose size is 0 or runs past the end takes the rest of the file (the game's
/// wrappers write 0).
fn riff(d: &[u8]) -> Option<Riff<'_>> {
    if d.len() < 12 || &d[..4] != b"RIFF" || &d[8..12] != b"WAVE" {
        return None;
    }
    let (mut p, mut fmt) = (12, None);
    while p + 8 <= d.len() {
        let id = &d[p..p + 4];
        let size = u32le(d, p + 4)? as usize;
        let body = p + 8;
        if id == b"fmt " {
            if size < 16 || body + 16 > d.len() {
                return None;
            }
            fmt = Some(Fmt {
                tag: u16le(d, body)?,
                channels: u16le(d, body + 2)?,
                rate: u32le(d, body + 4)?,
                align: u16le(d, body + 12)?,
                bits: u16le(d, body + 14)?,
                extra: d.get(body + 16..(body + size).min(d.len()))?,
            });
        } else if id == b"data" {
            let end = if size == 0 || body + size > d.len() {
                d.len()
            } else {
                body + size
            };
            return Some(Riff {
                fmt: fmt?,
                body: &d[body..end],
            });
        }
        p = body + size + (size & 1);
    }
    None
}

/// The bytes of the `data` chunk of a RIFF/WAVE file (to the end of the file if the size is 0 or too big).
pub(crate) fn data_chunk_body(d: &[u8]) -> Option<&[u8]> {
    riff(d).map(|r| r.body)
}

const STEP: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449,
    494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272,
    2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493,
    10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
];
const INDEX: [i32; 8] = [-1, -1, -1, -1, 2, 4, 6, 8];

#[derive(Clone, Copy)]
struct Ima {
    pred: i32,
    index: i32,
}

impl Ima {
    fn nibble(&mut self, n: u8) -> i16 {
        let step = STEP[self.index as usize];
        let mut diff = step >> 3;
        if n & 1 != 0 {
            diff += step >> 2;
        }
        if n & 2 != 0 {
            diff += step >> 1;
        }
        if n & 4 != 0 {
            diff += step;
        }
        if n & 8 != 0 {
            diff = -diff;
        }
        self.pred = (self.pred + diff).clamp(-32768, 32767);
        self.index = (self.index + INDEX[(n & 7) as usize]).clamp(0, 88);
        self.pred as i16
    }
}

/// Decode IMA ADPCM blocks (`align` bytes each, a 4-byte header per channel, then 4-byte groups of nibbles that
/// alternate between channels).
fn ima_decode(body: &[u8], channels: usize, align: usize) -> Result<Vec<i16>, AudioError> {
    if channels == 0 || align < 4 * channels {
        return Err(AudioError::Truncated("ADPCM block size"));
    }
    let mut out: Vec<i16> = Vec::new();
    for block in body.chunks(align) {
        if block.len() < 4 * channels {
            break;
        }
        let mut st: Vec<Ima> = (0..channels)
            .map(|c| Ima {
                pred: i16::from_le_bytes([block[c * 4], block[c * 4 + 1]]) as i32,
                index: (block[c * 4 + 2] as i32).min(88),
            })
            .collect();
        let groups = (block.len() - 4 * channels) / (4 * channels);
        let mut per_channel: Vec<Vec<i16>> = st.iter().map(|s| vec![s.pred as i16]).collect();
        for g in 0..groups {
            for (c, state) in st.iter_mut().enumerate() {
                let at = 4 * channels + (g * channels + c) * 4;
                for b in &block[at..at + 4] {
                    per_channel[c].push(state.nibble(b & 15));
                    per_channel[c].push(state.nibble(b >> 4));
                }
            }
        }
        for i in 0..per_channel[0].len() {
            for ch in &per_channel {
                out.push(ch[i]);
            }
        }
    }
    Ok(out)
}

/// The game's sound-effect wrapper: a short stub of MP3 frames (470 bytes in the files seen, a LAME tag and two or three
/// empty frames) with a complete RIFF/WAVE file behind it. Returns the bytes from the `RIFF` on, or the input unchanged.
pub(crate) fn unwrap_sfx(data: &[u8]) -> &[u8] {
    let mp3_start = data.len() > 4 && data[0] == 0xff && data[1] & 0xe0 == 0xe0;
    if mp3_start {
        let window = &data[..data.len().min(4096)];
        if let Some(p) = window
            .windows(12)
            .position(|w| &w[..4] == b"RIFF" && &w[8..12] == b"WAVE")
        {
            return &data[p..];
        }
    }
    data
}

/// What a file is, without decoding it.
pub fn probe(data: &[u8]) -> Result<Info, AudioError> {
    let data = unwrap_sfx(data);
    match riff(data) {
        Some(r) => {
            if let Some(m) = probe_mp3_at_start(r.body) {
                return Ok(Info {
                    codec: Codec::Mp3,
                    sample_rate: m.sample_rate,
                    channels: m.channels,
                    frames: m.samples_per_channel,
                    duration_secs: m.duration_secs,
                });
            }
            let (codec, frames) = match (r.fmt.tag, r.fmt.bits) {
                (1, 8) => (
                    Codec::Pcm8,
                    r.body.len() as u64 / r.fmt.channels.max(1) as u64,
                ),
                (1, 16) => (
                    Codec::Pcm16,
                    r.body.len() as u64 / 2 / r.fmt.channels.max(1) as u64,
                ),
                (0x11, 4) => {
                    let ch = r.fmt.channels.max(1) as u64;
                    let align = r.fmt.align as u64;
                    let per_block = if align >= 4 * ch {
                        (align - 4 * ch) * 2 / ch + 1
                    } else {
                        0
                    };
                    let whole = r.body.len() as u64 / align.max(1);
                    (Codec::ImaAdpcm, whole * per_block)
                }
                (tag, bits) => return Err(AudioError::Unsupported { tag, bits }),
            };
            Ok(Info {
                codec,
                sample_rate: r.fmt.rate,
                channels: r.fmt.channels,
                frames,
                duration_secs: frames as f64 / r.fmt.rate.max(1) as f64,
            })
        }
        None => match probe_mp3(data) {
            Some(m) => Ok(Info {
                codec: Codec::Mp3,
                sample_rate: m.sample_rate,
                channels: m.channels,
                frames: m.samples_per_channel,
                duration_secs: m.duration_secs,
            }),
            None => Err(AudioError::NotAudio),
        },
    }
}

/// Decode a file to PCM. MP3 data (raw, or behind the game's fake RIFF header) returns [`AudioError::Mp3`].
pub fn decode(data: &[u8]) -> Result<Audio, AudioError> {
    let whole = data;
    let data = unwrap_sfx(data);
    let skipped = whole.len() - data.len();
    let Some(r) = riff(data) else {
        return match probe_mp3(data) {
            Some(m) => Err(AudioError::Mp3(m)),
            None => Err(AudioError::NotAudio),
        };
    };
    if let Some(mut m) = probe_mp3_at_start(r.body) {
        // Report the offset in the file given, not in the data chunk.
        m.offset += skipped + data.len() - r.body.len();
        return Err(AudioError::Mp3(m));
    }
    let f = &r.fmt;
    let (samples, codec) = match (f.tag, f.bits) {
        (1, 8) => (
            r.body.iter().map(|b| ((*b as i16) - 128) << 8).collect(),
            Codec::Pcm8,
        ),
        (1, 16) => (
            r.body
                .as_chunks::<2>()
                .0
                .iter()
                .map(|p| i16::from_le_bytes(*p))
                .collect(),
            Codec::Pcm16,
        ),
        (0x11, 4) => (
            ima_decode(r.body, f.channels as usize, f.align as usize)?,
            Codec::ImaAdpcm,
        ),
        (tag, bits) => return Err(AudioError::Unsupported { tag, bits }),
    };
    let _ = f.extra;
    Ok(Audio {
        sample_rate: f.rate,
        channels: f.channels,
        samples,
        codec,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn wav(
        tag: u16,
        ch: u16,
        rate: u32,
        align: u16,
        bits: u16,
        extra: &[u8],
        body: &[u8],
    ) -> Vec<u8> {
        let mut fmt = Vec::new();
        fmt.extend(tag.to_le_bytes());
        fmt.extend(ch.to_le_bytes());
        fmt.extend(rate.to_le_bytes());
        fmt.extend((rate * align as u32).to_le_bytes());
        fmt.extend(align.to_le_bytes());
        fmt.extend(bits.to_le_bytes());
        fmt.extend(extra);
        let mut v = b"RIFF".to_vec();
        v.extend(((4 + 8 + fmt.len() + 8 + body.len()) as u32).to_le_bytes());
        v.extend(b"WAVEfmt ");
        v.extend((fmt.len() as u32).to_le_bytes());
        v.extend(fmt);
        v.extend(b"data");
        v.extend((body.len() as u32).to_le_bytes());
        v.extend(body);
        v
    }

    #[test]
    fn pcm16_and_pcm8_decode() {
        let a = decode(&wav(
            1,
            1,
            22050,
            2,
            16,
            &[],
            &[0x01, 0x00, 0xff, 0x7f, 0x00, 0x80],
        ))
        .unwrap();
        assert_eq!(
            (a.sample_rate, a.channels, a.codec),
            (22050, 1, Codec::Pcm16)
        );
        assert_eq!(a.samples, vec![1, 32767, -32768]);
        let b = decode(&wav(1, 2, 11025, 2, 8, &[], &[128, 128, 255, 0])).unwrap();
        assert_eq!(b.samples, vec![0, 0, 127 << 8, -128 << 8]);
        assert_eq!(b.frames(), 2);
    }

    #[test]
    fn a_data_size_of_zero_or_too_big_takes_the_rest_of_the_file() {
        let mut w = wav(1, 1, 8000, 2, 16, &[], &[1, 0, 2, 0, 3, 0]);
        let at = w.windows(4).position(|x| x == b"data").unwrap() + 4;
        w[at..at + 4].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(decode(&w).unwrap().samples, vec![1, 2, 3]);
        w[at..at + 4].copy_from_slice(&1_000_000u32.to_le_bytes());
        assert_eq!(decode(&w).unwrap().samples, vec![1, 2, 3]);
    }

    #[test]
    fn unknown_and_truncated_input_is_an_error() {
        assert_eq!(decode(b"hello world, not audio"), Err(AudioError::NotAudio));
        assert_eq!(
            decode(&wav(3, 1, 8000, 4, 32, &[], &[0; 8])),
            Err(AudioError::Unsupported { tag: 3, bits: 32 })
        );
        assert_eq!(decode(&[]), Err(AudioError::NotAudio));
        assert_eq!(decode(b"RIFF\0\0\0\0WAVEfmt "), Err(AudioError::NotAudio));
    }

    #[test]
    fn the_ima_step_table_has_89_entries_rising_to_32767() {
        assert_eq!(STEP.len(), 89);
        assert!(STEP.windows(2).all(|w| w[0] < w[1]));
        assert_eq!((STEP[0], STEP[88]), (7, 32767));
    }

    /// A simple IMA encoder, to check the decoder against.
    fn ima_encode(samples: &[i16], per_block: usize) -> Vec<u8> {
        let mut out = Vec::new();
        for chunk in samples.chunks(per_block) {
            let mut st = Ima {
                pred: chunk[0] as i32,
                index: 0,
            };
            out.extend((chunk[0]).to_le_bytes());
            out.push(0);
            out.push(0);
            let mut nibbles = Vec::new();
            for &s in &chunk[1..] {
                let step = STEP[st.index as usize];
                let mut diff = s as i32 - st.pred;
                let mut n = 0u8;
                if diff < 0 {
                    n = 8;
                    diff = -diff;
                }
                let mut t = step;
                if diff >= t {
                    n |= 4;
                    diff -= t;
                }
                t >>= 1;
                if diff >= t {
                    n |= 2;
                    diff -= t;
                }
                t >>= 1;
                if diff >= t {
                    n |= 1;
                }
                st.nibble(n);
                nibbles.push(n);
            }
            while nibbles.len() % 8 != 0 {
                nibbles.push(0);
            }
            for p in nibbles.chunks(2) {
                out.push(p[0] | (p[1] << 4));
            }
        }
        out
    }

    #[test]
    fn ima_adpcm_decodes_what_an_encoder_wrote() {
        let n = 8 * 16 + 1; // 17 samples... a block is 1 + 8k samples
        let per_block = 1 + 8 * 8;
        let signal: Vec<i16> = (0..per_block * 3)
            .map(|i| (((i as f32) * 0.03).sin() * 4000.0) as i16)
            .collect();
        let _ = n;
        let body = ima_encode(&signal, per_block);
        let align = (4 + 4 * 8) as u16;
        assert_eq!(body.len(), align as usize * 3);
        let a = decode(&wav(
            0x11,
            1,
            22050,
            align,
            4,
            &[2, 0, per_block as u8, 0],
            &body,
        ))
        .unwrap();
        assert_eq!(a.codec, Codec::ImaAdpcm);
        assert_eq!(a.samples.len(), signal.len());
        assert_eq!(
            a.samples[0], signal[0],
            "a block starts with its predictor exactly"
        );
        let worst = a
            .samples
            .iter()
            .zip(&signal)
            .map(|(a, b)| (*a as i32 - *b as i32).abs())
            .max()
            .unwrap();
        assert!(worst < 400, "ADPCM error {worst}");
        let info = probe(&wav(
            0x11,
            1,
            22050,
            align,
            4,
            &[2, 0, per_block as u8, 0],
            &body,
        ))
        .unwrap();
        assert_eq!(
            (info.codec, info.frames),
            (Codec::ImaAdpcm, signal.len() as u64)
        );
    }

    #[test]
    fn stereo_ima_blocks_alternate_channels_in_groups_of_four_bytes() {
        // Two channels, one block: header per channel then one 4-byte group each.
        let mut body = Vec::new();
        body.extend(100i16.to_le_bytes());
        body.extend([0, 0]);
        body.extend((-100i16).to_le_bytes());
        body.extend([0, 0]);
        body.extend([0x00; 4]); // left: eight zero nibbles
        body.extend([0x88; 4]); // right: eight nibbles of 8 (negative)
        let a = decode(&wav(0x11, 2, 44100, 16, 4, &[2, 0, 9, 0], &body)).unwrap();
        assert_eq!(a.samples.len(), 18);
        assert_eq!((a.samples[0], a.samples[1]), (100, -100));
        let left: Vec<i16> = a.samples.iter().step_by(2).copied().collect();
        let right: Vec<i16> = a.samples.iter().skip(1).step_by(2).copied().collect();
        assert!(
            left.windows(2).all(|w| w[1] >= w[0]),
            "zero nibbles only add: {left:?}"
        );
        assert!(
            right.windows(2).all(|w| w[1] <= w[0]),
            "sign-bit nibbles subtract: {right:?}"
        );
    }
}
