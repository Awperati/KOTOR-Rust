# Sources and licences

Rule for this repo: no game assets, no BioWare code, no decompiled exe code. The engine reads the owner's
installed game at runtime. Nothing from the game folder is copied into the repo.

## How the format knowledge was obtained

1. **The owner's own files.** Every layout in `FORMATS.md` was checked by parsing the installed game
   (GOG v1.03) and asserting structural invariants over every file of that type. Where a layout was
   unknown, it was found by dumping words from real files and matching them against counts, sizes and
   `surfacemat.2da` content. These checks are the tests in `crates/*/tests/real.rs`.
2. **General familiarity with public format documentation and open-source engines.** I wrote the layouts
   from memory of what the community has documented, then corrected them against the data. I did **not**
   open, download or copy any source file from these projects in this session, and no code was ported.

## Projects the format knowledge is generally informed by (licences from memory, not re-checked)

| Project | What it is | Licence (believed) |
|---|---|---|
| reone | C++ engine that plays KotOR 1 and 2 | GPL-3.0 |
| xoreos | C++ reimplementation of BioWare engines, with format docs | GPL-3.0+ |
| xoreos-docs | Format notes (incl. Torlack's MDL notes) | not checked |
| KotOR.js | TypeScript/WebGL KotOR engine and tools | not checked |
| PyKotor / Holocron Toolset | Python format libraries and editors | not checked |
| KotOR modding wiki (deadlystream and others) | Community format notes | not checked |

Because several of these are GPL, nothing may be pasted from them into this MIT/Apache-licensed repo.
If a future session reads their source for a hard format, record the file and licence here first.

## The owner's own pictures

`title-card.png`, the main menu's title card: third-party artwork the owner supplied, for local use only. It is not in this repository and no licence is known. The engine reads it at run time from `%LOCALAPPDATA%\kotor-rs\assets`, or from `KOTOR_ASSETS_DIR`, and draws the game's own logo when it is not there. `scripts/upscale_title.ps1` makes the upscaled copy in `%LOCALAPPDATA%\kotor-rs\hd`.

## Crates

The main workspace uses only `std`.

The standalone package `gpu/` (kotor-gpu) is approved by the owner (via Top Dog, 6 Nov) to use these crates.io crates only, no git or
path dependencies for them, exact versions fixed by `gpu/Cargo.lock`:

| Crate | Requested | Use |
|---|---|---|
| wgpu | 24 | GPU rendering |
| winit | 0.30 | the viewer window (`gpu/examples/viewer.rs`); no input handled yet |
| pollster | 0.4 | block on wgpu setup |
| glam | 0.29 | matrices |
| bytemuck | 1 (derive) | upload vertex data |
| log | 0.4 | logging |
| env_logger | 0.11 | log output |
| symphonia | =0.5.4, only the `mp3` feature | decode the MP3 inside voice-over and music (`gpu/audio`); pinned exactly |
| rodio | =0.20.1, no default features | play decoded samples on the default output device (`gpu/audio`); pinned exactly |

Resolved by the owner's `cargo fetch` (6 Nov 2026): `gpu/Cargo.lock` has 324 packages, 313 from crates.io, none from git, the rest are this
repo's path crates. Direct dependencies as locked: wgpu 24.0.5, winit 0.30.13, pollster 0.4.0, glam 0.29.3, bytemuck 1.25.2, log 0.4.35,
env_logger 0.11.11, symphonia 0.5.4 (its `symphonia-core`, `-metadata` and `-bundle-mp3` are 0.5.5), rodio 0.20.1 (brings cpal 0.15.3).
`cargo tree --manifest-path gpu/Cargo.toml --workspace` lists the rest. `gpu/` is a workspace of two packages
(`kotor-gpu` and `gpu/audio` = `kotor-audio-out`), so one fetch covers everything; builds run with `--offline`.

**Why rodio and not kira:** the std-only `kotor-audio` crate already decodes everything except MP3 to PCM in memory, so what is missing is a
sink that plays a buffer of samples: rodio's `SamplesBuffer` and `Sink` do exactly that on top of cpal with a few lines, and it is the older, smaller
API. kira is a fuller game audio engine (mixer tracks, tweens, clocks) that this project does not need yet; it can replace rodio later behind
`play_blocking` without touching the decoders. rodio is used with `default-features = false`, so it brings no decoders of its own and symphonia
(MP3 only) is the single decoder crate.

**Built and checked:** `gpu/src/lib.rs` and `gpu/audio/src/lib.rs` compiled on the first try after the fetch (no source changes were needed).
Clippy `-D warnings` and fmt pass on the whole `gpu/` workspace.

## Nameplates

How the nameplates and object labels behave comes from the README and settings file of the K1 Nameplates mod by phalladar
(MIT, https://github.com/phalladar/k1-nameplates). None of its code or files are used: its DLL patches the original game and
was never run. See `gpu/src/nameplates.rs` for what was kept and what was assumed.

Override content is the owner's own install, read at run time from its `Override` folder and never copied into the repository, a build or the launcher's folder. Third-party mods there stay read only and are not distributed.
