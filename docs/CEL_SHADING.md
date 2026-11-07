# Cel shading

Options, Feedback, last row: **Cel shading: Off / Subtle / Strong**. Default Subtle. The **F7** key turns it off and brings back the level it had, for a before and after. Kept in the settings file as `cel_shading=0|1|2`.

## What it does

- **Light in steps.** In the fragment shader (`gpu/src/lib.rs`, `cel_steps`) the brightness of the light is cut into steps and the texture is left whole: textures and detail stay readable. A body lit by the one lamp runs from 0.55 to 1 in brightness, so its steps divide that; a level's lightmap runs from 0. Subtle is four steps with a soft edge and 70 per cent of the way to the step. Strong is three, a sharper edge, all the way.
- **A thin dark outline** round characters, doors and props (the meshes drawn each frame, not the rooms): the mesh again, fatter along its normals by a width in pixels, with only its faces turned away drawn (`vs_outline`). Subtle is 1.4 pixels at 1080 lines, strong 2.2; the width goes with the picture's height, so it is as thin at 2160 lines, and a little thinner far off. Alpha-cut textures (hair, grilles) cut the outline too.
- **Left alone:** the HUD and text (drawn over the picture), particles, the sky.

## Cost

`cel_shots` times a frame to the GPU finishing it, no read-back, the three levels round after round. Quiet machine, median of 80:

| Scene | Off | Subtle | Strong |
|---|---|---|---|
| Endar Spire, 720p | 2.37 ms | 2.46 | 2.43 |
| Endar Spire, 2160p | 3.76 ms | 3.85 | 3.91 |
| Endar Spire close-up, 2160p | 4.09 ms | 4.30 | 4.23 |
| Taris, 720p | 4.51 ms | 4.50 | 4.55 |
| Taris, 2160p | 3.32 ms | 3.42 | 3.37 |

About 0.1 to 0.2 ms at 2160p. On a machine busy with other work the same runs scatter (5 to 22 ms for one scene) and Subtle read 1 to 2 ms over Off at 2160p in Taris: the outline is one more draw of every moving mesh.

## Tests and frames

- `gpu/tests/cel.rs`: a ball in front of empty space is changed by Subtle and more by Strong, the space round it is not, and a dark ring lies outside it.
- `gpu/tests/menus.rs`: the row goes round and the file keeps it; the F7 toggle.
- `optionpages.rs`: the level goes through the settings file, and a level out of range is brought in.
- `cargo run --release --example cel_shots -- <module> <out_dir> [width] [height] [frames]` writes the three levels of a view and of a close-up and prints the times.

## Not done

- The outline is not drawn on what is seen through a doorway into a joined area.
- Hard edges of a body whose vertices are split there (the normal differs on each side) can show a hairline gap in the outline.
