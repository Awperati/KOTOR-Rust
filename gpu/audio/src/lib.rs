//! MP3 decoding (symphonia) and playback (rodio) for the audio the std-only `kotor-audio` crate cannot decode.
//!
//! `tests/mp3.rs` decodes all 558 MP3 files of the install. Playback (`play_blocking`) has not been run: it would make sound.

use kotor_audio::{Audio, AudioError, Codec};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Decode MP3 bytes (see `kotor_audio::mp3_payload`) to interleaved 16-bit samples.
pub fn decode_mp3(payload: &[u8]) -> Result<Audio, String> {
    let source = std::io::Cursor::new(payload.to_vec());
    let stream = MediaSourceStream::new(Box::new(source), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp3");
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| e.to_string())?;
    let mut format = probed.format;
    let track = format.default_track().ok_or("no audio track")?;
    let id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| e.to_string())?;
    let (mut samples, mut rate, mut channels) = (Vec::new(), 0u32, 0u16);
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(e) => return Err(e.to_string()),
        };
        if packet.track_id() != id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(buf) => {
                let spec = *buf.spec();
                let mut sb = SampleBuffer::<i16>::new(buf.capacity() as u64, spec);
                sb.copy_interleaved_ref(buf);
                samples.extend_from_slice(sb.samples());
                rate = spec.rate;
                channels = spec.channels.count() as u16;
            }
            // A damaged frame: skip it, as players do.
            Err(SymError::DecodeError(_)) => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    if samples.is_empty() {
        return Err("no samples decoded".into());
    }
    Ok(Audio {
        sample_rate: rate,
        channels,
        samples,
        codec: Codec::Mp3,
    })
}

/// Decode any file from the game: PCM and ADPCM by `kotor-audio`, MP3 by symphonia.
pub fn decode_any(bytes: &[u8]) -> Result<Audio, String> {
    match kotor_audio::decode(bytes) {
        Ok(a) => Ok(a),
        Err(AudioError::Mp3(_)) => {
            let payload = kotor_audio::mp3_payload(bytes).ok_or("MP3 payload not found")?;
            decode_mp3(payload)
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Play decoded audio on the default output device and wait for it to finish.
pub fn play_blocking(audio: &Audio) -> Result<(), String> {
    let (_stream, handle) = rodio::OutputStream::try_default().map_err(|e| e.to_string())?;
    let sink = rodio::Sink::try_new(&handle).map_err(|e| e.to_string())?;
    sink.append(rodio::buffer::SamplesBuffer::new(
        audio.channels,
        audio.sample_rate,
        audio.samples.clone(),
    ));
    sink.sleep_until_end();
    Ok(())
}

/// The output device with one voice channel and one looping music channel. Dropping it stops everything.
pub struct AudioOut {
    _stream: rodio::OutputStream,
    handle: rodio::OutputStreamHandle,
    voice: Option<rodio::Sink>,
    music: Option<rodio::Sink>,
    volume: f32,
}

impl AudioOut {
    /// Open the default output device. `volume` is 0 (silent) to 1.
    pub fn new(volume: f32) -> Result<AudioOut, String> {
        let (stream, handle) = rodio::OutputStream::try_default().map_err(|e| e.to_string())?;
        Ok(AudioOut {
            _stream: stream,
            handle,
            voice: None,
            music: None,
            volume,
        })
    }

    fn sink(&self) -> Result<rodio::Sink, String> {
        let s = rodio::Sink::try_new(&self.handle).map_err(|e| e.to_string())?;
        s.set_volume(self.volume);
        Ok(s)
    }

    /// Say a line, cutting off the one before.
    pub fn play_voice(&mut self, a: &Audio) -> Result<(), String> {
        let s = self.sink()?;
        s.append(rodio::buffer::SamplesBuffer::new(
            a.channels,
            a.sample_rate,
            a.samples.clone(),
        ));
        self.voice = Some(s);
        Ok(())
    }

    pub fn stop_voice(&mut self) {
        self.voice = None;
    }

    /// Replace the music with `a`, looping.
    pub fn play_music(&mut self, a: &Audio) -> Result<(), String> {
        use rodio::Source;
        let s = self.sink()?;
        s.set_volume(self.volume * 0.6);
        s.append(
            rodio::buffer::SamplesBuffer::new(a.channels, a.sample_rate, a.samples.clone())
                .repeat_infinite(),
        );
        self.music = Some(s);
        Ok(())
    }

    pub fn stop_music(&mut self) {
        self.music = None;
    }
}
