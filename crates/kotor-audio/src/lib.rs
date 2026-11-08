//! Audio files to PCM in memory, no output device. std only.
//!
//! What the installed game ships under the `.wav` name (counted over the owner's install: 1928 resources in the
//! BIFs and about 1500 files in `streamwaves`, `streamsounds` and `streammusic`):
//! - plain RIFF PCM, 16-bit (most sound effects) or 8-bit, mono or stereo: decoded here;
//! - RIFF with IMA ADPCM (format tag 0x11, 4 bits): decoded here;
//! - **the header quirk**: a RIFF/WAVE header (fmt chunk of 16 or 18 bytes, a `fact` chunk, a `data` chunk whose size is
//!   0 or wrong, 58 bytes in all for the 18-byte `fmt`) followed by **MP3 data** (often starting with an `ID3` tag).
//!   The header's own fields (8-bit PCM, 22050 Hz) describe nothing real. All voice-over and most music are like this;
//! - raw MP3 with no header at all (most of `streamsounds`).
//!
//! MP3 is recognised and measured (sample rate, channels, frame count, duration) but **not decoded**: Layer III needs
//! its Huffman tables and a filter bank, which is the job of the audio crate the owner approved for when the project
//! reaches audio. [`decode`] returns [`AudioError::Mp3`] with the measurements and [`mp3_payload`] gives the bytes
//! after the fake header.

mod mp3;
mod wav;

pub use mp3::{mp3_payload, probe_mp3, Mp3Info};
pub use wav::{decode, probe, Audio, Codec, Info};

#[derive(Clone, Debug, PartialEq)]
pub enum AudioError {
    /// Not a RIFF/WAVE file and no MP3 frames either.
    NotAudio,
    /// The header or a block runs past the end of the data.
    Truncated(&'static str),
    /// A RIFF format this decoder does not read (tag, bits per sample).
    Unsupported { tag: u16, bits: u16 },
    /// MP3 data: measured but not decoded.
    Mp3(Mp3Info),
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioError::NotAudio => write!(f, "not audio"),
            AudioError::Truncated(what) => write!(f, "truncated {what}"),
            AudioError::Unsupported { tag, bits } => {
                write!(f, "unsupported wave format tag {tag:#x}, {bits} bits")
            }
            AudioError::Mp3(i) => write!(
                f,
                "MP3 data ({} Hz, {} channel(s), {:.2} s): decoding needs an MP3 decoder",
                i.sample_rate, i.channels, i.duration_secs
            ),
        }
    }
}

impl std::error::Error for AudioError {}
