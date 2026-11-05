# kotor-rs plan

Goal from the owner: rewrite KotOR 1 in Rust so the game plays like the original, and then build new content
(more lore) on top of it. Clean-room: the engine reads the owner's installed files at runtime and contains no game
assets, BioWare code or decompiled exe code.

This plan is honest about size. A full-parity rewrite is a multi-month project even with agent help.

## Decision needed first: what does "build on an existing engine" mean?

The owner asked whether an engine already exists to build on. Yes, several exist (licences from memory, unchecked):

| Option | Language / licence | What it gives | Cost |
|---|---|---|---|
| reone | C++, GPL-3.0 | Plays KotOR 1 and 2 to a large degree: renderer, scripts, dialogue, party, UI | Not Rust. GPL forces any derived work to be GPL. Gameplay is known to be incomplete in places. |
| xoreos | C++, GPL-3.0+ | Broad engine reimplementation, good format docs. KotOR support is partial. | Same licence issue. |
| KotOR.js | TypeScript, licence unchecked | Playable in a browser | Slower, and also incomplete. |
| Modding the original | Tools (TSLPatcher, KotOR Tool, Holocron) | New lore in the real game today: new modules, dialogue, quests, items | Limited by what the exe can do. No new engine features. |
| This repo (clean room) | Rust, MIT/Apache | Full control, own licence, one language | Slowest. Combat and many routines must be rebuilt from behaviour. |

My recommendation, for the owner to confirm through Top Dog:

1. **Keep this repo as the long-term engine**, because it is the only option that is Rust and not GPL-bound.
2. **Use reone and xoreos as readable references, not as a base.** Reading them for format and behaviour questions is allowed
   by the brief. Copying code is not. Each consultation gets logged in `docs/SOURCES.md`.
3. **Make a content toolchain the first product**, because the owner's real goal is new lore. The Phase 1 readers plus
   writers for GFF, ERF/MOD, TLK and 2DA let us author a new module, dialogue and quest and run it in the *original* game
   (already at 1440p) long before the Rust engine can play a full game. The same data then runs on the new engine.

If the owner wants to build on reone instead (C++/GPL), that is a different project and should be a separate decision.

## Scope to a playable game

| Subsystem | What it needs | Status |
|---|---|---|
| Archives, resources | KEY/BIF, ERF/MOD/SAV, RIM, module layering | done (read) |
| GFF, 2DA, TLK | templates, tables, strings | done (GFF also writes) |
| Textures | TGA, TPC (DXT) | done (decode) |
| Models | MDL/MDX meshes, skins, controllers, animations | read done; animation playback, emitters, lights not done |
| Walkmesh | BWM, LYT, VIS | read done; collision, pathing, AABB queries not done |
| Scripts | NCS decode done; VM and ~760 engine routines | disassembler only |
| Renderer | wgpu, materials, lightmaps, skinning, fog, effects | not started (Phase 2) |
| Input, window, camera | winit, KotOR follow camera, click-to-move | not started |
| Audio | WAV (SFX/VO files carry a header the stock decoders may reject), MP3 music, 3D positional | not started |
| Area/module loading | IFO, ARE, GIT, spawning creatures/doors/placeables from UTC/UTD/UTP | not started |
| Dialogue | DLG, conversation camera, lip sync (LIP), VO | not started |
| Combat, Force, inventory, party | d20-style rules from 2DAs plus rules that live only in the exe | not started; largest unknown |
| UI | GUI GFFs, fonts (TXI + textures), menus, HUD | not started |
| Save/load | SAV ERF, GFF party/global state | not started |
| Video | BIK cutscenes | no pure-Rust decoder; needs a dependency decision |

## Phases

- **Phase 0 (done):** this plan, workspace, CI-style checks.
- **Phase 1 (mostly done):** format readers with tests on real files, inspection tool.
- **Phase 2: rendering slice.** window (not on the owner's main display), wgpu pipeline, load a room model with textures
  and lightmaps, free camera. Needs Top Dog's go-ahead and crate approval.
- **Phase 3: first playable slice.** Endar Spire (m01aa): load the area from LYT/ARE/GIT, spawn the player model (body
  and head parts), walk on the walkmesh with the follow camera, doors and placeables, no scripts. About 8-12k more lines.
- **Phase 4: scripting and dialogue.** NCS VM, the first 100 routines the Endar Spire needs, DLG conversations, journal.
- **Phase 5: rules.** Stats, inventory, combat rounds, Force powers, party, levelling, from 2DAs and behaviour.
- **Phase 6: UI, audio, save/load.**
- **Phase 7: full game pass.** Every module, every routine, regression-play both through.
- **Parallel track: content toolchain** (writers, module builder, dialogue and script authoring) as described above.

## Crates

Phase 1 uses only `std`. Proposed for later phases (all normal crates.io, none downloaded yet, approval needed):

| Crate | Use |
|---|---|
| wgpu | GPU rendering (Vulkan/DX12) |
| winit | window and input |
| pollster | block on wgpu setup |
| glam | maths (vectors, matrices, quaternions) |
| bytemuck | upload vertex data |
| rodio (cpal) or kira | audio output |
| symphonia | MP3 / PCM / ADPCM decode if rodio is not enough |
| log, env_logger | logging |
| egui (optional) | debug tooling only, not the game UI |

## Size estimate (rough, my guess)

- Phase 1 as it stands: about 6k lines of Rust, 61 tests.
- First playable slice: roughly 10k more lines.
- Full parity: I would guess 80-150k lines of Rust and several months of steady agent work. I do not know this number.
  The big cost is behaviour, not formats: every engine routine and every hard-coded rule has to be matched by observation.

## Biggest risks

1. **Hidden rules in the exe.** Combat maths, AI, perception, Force-power effects and many routines are not in the data.
   Matching them "to the exact gameplay" needs side-by-side play against the original. Decompiling the exe is off the table
   under the brief, so this is observation only.
2. **Licence contamination.** Reading GPL engines is fine, copying is not. Logged in `docs/SOURCES.md`.
3. **Animation and skinning correctness.** The packed quaternion decode and supermodel/animation sharing are unverified.
4. **Video (BIK) and audio quirks.** No pure-Rust BIK decoder that I know of.
5. **Scale of ~760 script routines.** Each needs defined behaviour and tests.
6. **Visual fidelity.** The original look (lightmaps, fixed-function era shaders, particle effects) has to be reproduced by eye.
7. **Mods and the owner's Override folder.** The game folder holds the owner's 1440p setup. The engine must ignore or
   handle Override explicitly and never write there.

## What I do not know

- Whether the owner wants a new engine, content for the original game, or both. This decides Phase 2 onward.
- Exact licences of the reference projects (from memory only).
- Several MDL fields (light/emitter headers, texture animation, vertex colours) and the 2-column packed quaternion.
- How much of KotOR combat is data-driven. I expect a lot is hard-coded.
- Whether any engine crates will be allowed beyond the list above.

## Safety rules honoured

Read-only on `E:\Games\Star Wars - KotOR`. Tests only read it and skip if absent. No window is opened in Phase 1; the
renderer in `kotor-tool` is software and writes PNG files. No pushes, no remote, local commits only.
