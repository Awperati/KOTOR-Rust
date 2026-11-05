# Audio decoding (kotor-audio)

Audio files to 16-bit PCM in memory, std only, no output device. `kotor_audio::decode(bytes)` -> `Audio { sample_rate, channels, samples, codec }`;
`probe(bytes)` measures without decoding; `mp3_payload(bytes)` hands back the MP3 bytes behind any wrapper.

## What the install ships under `.wav` (counted over the owner's install)
| Kind | Where | Count | Handling |
|---|---|---|---|
| Plain RIFF PCM, 16-bit (and one 8-bit), mono/stereo, 11025-44100 Hz | BIFs (`sounds.bif` etc.) | 1924 | decoded |
| RIFF with IMA ADPCM (tag 0x11, 4-bit, 20-byte fmt) | BIFs | 4 | decoded (block header per channel, 4-byte nibble groups alternating channels) |
| **SFX wrapper**: a 470-byte stub of MP3 frames (a LAME tag and two or three empty frames) with a complete RIFF/WAVE file behind it | `streamsounds` | 1021 of 1026 stream files that decode | found by scanning the first 4 KB for `RIFF....WAVE` after an MP3 sync; decoded |
| **VO wrapper**: a RIFF header with an 18-byte `fmt` (8-bit, 22050 Hz, which describe nothing real), a `fact` chunk and a `data` chunk of size 0 (58 bytes in all), then **MP3** (often an `ID3` tag first) | `streamwaves` (all voice-over), most of `streammusic` | 558 files are MP3 including these and raw MP3 | recognised and measured (rate, channels, frames, duration), **not decoded** |
| Raw MP3 | `streamsounds`, `streammusic` | (in the 558) | recognised and measured, not decoded |

Over all files: 1026 stream files decode to PCM (1021 of them behind the MP3 stub), 558 are MP3; every one of the 1928 BIF sounds and every stream file
is either decoded or recognised (tests walk them all).

## MP3 is the open item
`decode` returns `AudioError::Mp3(Mp3Info)` (sample rate, channels, bit rate, frame count, duration, offset of the first frame) for MP3. A Layer III decoder
needs the Huffman code tables and the filter bank; I did not write one from memory, and the audio crate approved for "when the project reaches audio"
(symphonia) is the way to get it. Everything needed to hand it the right bytes is here: `mp3_payload`.

## Details that bit
- A few BIF files begin with bytes like an MP3 sync because their first 16-bit samples are -1 or -32 (`ff ff`, `e0 ff`). MP3 is only accepted behind a RIFF
  header when real frames chain from the first byte (three in a row, or the whole stream), so those stay PCM (tested).
- A RIFF `data` size of 0 or past the end of the file means "the rest of the file".
- Frame lengths use the standard formulas (Layer III: 144 x bit rate / sample rate for MPEG-1, 72 for MPEG-2 and 2.5, plus the padding bit).
