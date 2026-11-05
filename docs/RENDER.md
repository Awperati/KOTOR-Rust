# Rendering status (Phase 2)

Reference renders are written to `docs/shots/` (git-ignored: they contain the game's own textures, so they are for local
viewing and are not committed).

Everything here is **software rendering with `std` only**. No window is opened, no GPU is used, and no screen is captured:
frames go to memory and are written as PNG. The wgpu/winit renderer approved for Phase 2 is **not started**, because the
permission layer refused `cargo fetch` for the new crates (see below). The software renderer stays useful after that as a
reference image to compare the GPU output against.

## What exists

| Piece | Where | Notes |
|---|---|---|
| Area scene loader | `kotor-render::load_area` | LYT rooms, each room model's render meshes placed by the layout, world-space vertices, texture and lightmap names |
| Software renderer | `kotor-render::render` | perspective camera (Z up), near-plane clipping, z-buffer, perspective-correct UVs, bilinear textures, diffuse x lightmap |
| Doors and placeables | `kotor-render::add_area_objects` | GIT `Door List` / `Placeable List`; UTD `GenericType` -> `genericdoors.2da`, UTP `Appearance` -> `placeables.2da`; placed by X, Y, Z and Bearing |
| Creature by appearance | `kotor-render::load_creature` | `appearance.2da` row + texture variation: body = `race`, body texture = `texX` + 2-digit variation, head from `heads.2da` |
| Character assembly | `kotor-render::load_character` | body model posed by a supermodel animation, head model attached at `head_g` |
| Texture lookup | `kotor-render::load_texture` | TPC from the texture packs, else TGA from the BIFs |
| CLI | `kotor-tool view`, `kotor-tool character` | write a PNG |

```powershell
target\debug\kotor-tool.exe view m01aa out\spire.png --module end_m01aa --eye 15.4 20.1 0.6 --target 25 20.5 0.2 --size 960x540
target\debug\kotor-tool.exe creature 6 out\carth.png --anim pause1      # appearance.2da row 6, or a creature template name
target\debug\kotor-tool.exe character p_bastilabb out\bastila.png --head p_bastilah --anim pause1 --time 0
```

## Checked

- Endar Spire (`m01aa`): 502 drawable meshes, 78,346 triangles, 125 distinct texture names of which 1 has no file in the install
  (`m01aa_04a_a0002t`, 58 triangles). A frame renders in about 0.5 s at 960x540. Looked at the output: the spawn corridor is
  recognisable, textured and lit by its lightmaps.
- AABB (walkmesh) nodes are not drawn; they showed up as grey boxes before they were excluded.
- Tests: scene size, texture resolution, lightmap UVs, determinism, coverage of the frame, character height 1.5-2.0 m, feet near
  the floor, idle pose differs from rest.

## Not done / wrong on purpose (be honest about these)

- **Creatures in an area** are not placed (GIT `Creature List`); doors and placeables are, in the rest pose (closed doors).
- **Skinned meshes** (heads, some bodies) are drawn rigidly at their node transform; no vertex blending.
- **Character textures** now follow the appearance row (Carth is orange trousers and a dark jacket, as expected), but only for model
  type `B` bodies in their default outfit. Armour and equipped items are not handled, and non-`B` models fall back to a
  "most used texture" heuristic. Bastila's torso texture still looks odd and I could not tell whether it is wrong or just her outfit.
- **Neck**: the body's `neck_g`, `necklwr_g` and `head_g` meshes are `render = false` placeholders and the head mesh ends at the
  chin, so no neck geometry is drawn. Carth's head sits cleanly on his collar; Bastila's looks detached in the idle pose. Whether the
  original blends the skinned head down onto the neck is unknown.
- The idle pose for Bastila leans her head away from the torso by about 0.1 m; I did not verify it against the game.
- No alpha, transparency, envmaps, bump maps, fog, particle emitters, light flares, dynamic lights, or shadows.
- Triangles are drawn double sided; face winding is unverified.
- No window, no input, no free-camera interaction; the "free camera" is the `--eye/--target` options.

## The wgpu step (blocked)

Approved by the owner via Top Dog: wgpu, winit, pollster, glam, bytemuck, log/env_logger. `cargo fetch` was refused with
"Untrusted Code Integration". Nothing was downloaded. The crate list, versions and `Cargo.lock` entries will be recorded in
`docs/SOURCES.md` when it goes through.
