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

## Crates

The main workspace uses only `std`.

The standalone package `gpu/` (kotor-gpu) is approved by the owner (via Top Dog, 6 Nov) to use these crates.io crates only, no git or
path dependencies for them, exact versions fixed by `gpu/Cargo.lock`:

| Crate | Requested | Use |
|---|---|---|
| wgpu | 24 | GPU rendering |
| winit | 0.30 | window and input (not used by code yet) |
| pollster | 0.4 | block on wgpu setup |
| glam | 0.29 | matrices |
| bytemuck | 1 (derive) | upload vertex data |
| log | 0.4 | logging |
| env_logger | 0.11 | log output |

The exact versions that Cargo resolves, with their transitive dependencies, are to be listed here after the first
`cargo fetch --manifest-path gpu/Cargo.toml` (not yet run: the lockfile does not exist).
