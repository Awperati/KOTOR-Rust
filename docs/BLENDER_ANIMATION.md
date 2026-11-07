# Making your own animations in Blender

You pose the game's own characters in Blender, save a clip, and the game plays it. Nothing of the game's files changes.

What is here:

| Piece | Where | What it does |
| --- | --- | --- |
| `blender-export` | `kotor-tool` | Writes a character (meshes, the game's bones, skin weights, textures, clips) for Blender |
| The add-on | `tools/blender/kotor_anim.py` | One file. Loads that export, checks your clip, writes `<name>.anim.txt` |
| The engine | the viewer | Plays `%LOCALAPPDATA%\kotor-rs\anims\*.anim.txt`; F8 reads them, F9 plays them |

## 1. Export a character

From the project folder (a terminal):

```
target\release\kotor-tool.exe blender-export x C:\Users\User\Projects\blender-work\hero --appearance 92 --name hero
```

`--appearance 92` is the middle woman; 91 small, 93 large; 137 a man. The folder is made and must be outside the repository. It gets `hero.gltf`, `hero.bin`, `bones.txt` and `textures\*.png` (the game's textures under the game's own names, ready to paint over: see `docs/TEXTURES.md`).

`--clips pause1,g8r1` picks which of the game's clips come along to look at (the default is pause1 pause2 walk run listen tlknorm g8r1 g8a1 die; `--clips all` writes all 270, which Blender takes minutes to read).

The game's own files are only read.

## 2. Put the add-on in Blender (once)

1. Close Blender.
2. Copy `tools\blender\kotor_anim.py` to the add-ons folder. For Blender 5.2 that is  
   `%APPDATA%\Blender Foundation\Blender\5.2\scripts\addons\` (make the `addons` folder if it is not there). For another version, change `5.2`.
3. Open Blender, **Edit > Preferences > Add-ons**, search `KotOR`, tick **KotOR-RS animation**.
4. In the 3D view press **N**; a **KotOR** tab is on the right.

## 3. Make a clip

1. **Load export**, pick `hero.gltf`. You get the character, and a rig called *KotOR Rig* with 64 bones named as the game names them (`rbicep_g`, `rforearm_g`, `torso_g`, ...). Keep the names and do not add bones: the game plays only its own.
2. The scene runs at 30 frames a second, the game's (the panel says so and has a button if it does not).
3. Select the rig, go to **Pose Mode** (Ctrl+Tab), turn a bone with **R**, press **I** to key it. Move to another frame, turn it again, key it. Set the **start and end frame** of the clip (the panel has them).
4. **Preview** plays it.
5. Type a **Clip name** (letters, digits and `_`). **Validate for KotOR-RS** says what is wrong: a bone the game does not have is an error; bones you did not key are noted as staying at rest.
6. **Export clip** writes `<name>.anim.txt` in the folder shown (default `%LOCALAPPDATA%\kotor-rs\anims`).

### Rules

- A bone you do not key stays as the body rests.
- Root motion: the game moves the body, so the sideways and forward movement of `rootdummy` is not played; its height is. Walk in place.
- Only places and turns are played, not scale.
- A clip named like one of the game's (`pause1` the standing idle, `g8r1` the fists-up stance, `walk`, `run`, `listen`, `tlknorm`, ...) **replaces** it for every character that has the clip. Any other name is a clip of its own, to play with F9. (The roll and the landing are made from `roll_pose.txt` and `landing_pose.txt`, see `docs/MOVEMENT_POSE.md`, not from clips.)

## 4. Play it in the game

1. In the viewer press **F8**: it reads your animation files (and pose and texture files). If a file has a mistake the game keeps its own and the reason is in `anim_problems.txt` in the same folder.
2. Press **F9**: the player plays your next clip; press again for the next; after the last it goes back to the game's own.
3. Change the clip in Blender, export again, press F8, F9.

## Worked example: a wave of 11 frames

The file `docs/examples/wave.anim.txt` is one made this way: the right arm goes up in three frames and the forearm sways over the 11. To try it without Blender:

1. Make the folder `%LOCALAPPDATA%\kotor-rs\anims` and copy `docs\examples\wave.anim.txt` into it.
2. Start the viewer, press F8, then F9. The player waves; F9 again puts it away.

To make it yourself: load the export, key `rbicep_g` at frame 0 (rest) and frame 3 (turned about its own Y axis by -100 degrees, which raises the arm), key `rforearm_g` swaying about X between -40 and 40 degrees over frames 0 to 10, set the range 0 to 10, name it `wave`, Export clip.

## The file

```
version = 2
name = wave
fps = 30
loop = yes
key rbicep_g 0 -0.0098 -0.0146 0.1158  -0.00046 0.97236 0.00132 0.23348
```

A `key` line is: bone, frame, the bone's place relative to its parent (x y z, metres) and turn (w x y z). The add-on writes exactly what the game's own skeleton has, plus your turn. Any text editor will do for a small change.

## Checks

- The add-on's checks (in Blender): unknown bones, nothing moving, an empty range, a turn that is not a unit quaternion, scene rate.
- The game's checks (on F8): wrong version, missing name, a line it cannot read, a bone the body does not have (that key is ignored), a time before the start.
- Tests: `what_the_blender_add_on_writes_the_engine_reads` (the example file, written by the add-on's own code, parses and plays), `a_clip_of_his_is_played_replaces_a_stock_clip_and_a_bad_file_is_named` in `gpu/tests/custom_anim.rs` (a file plays on the hero, replaces a stock clip, and bad files are named), and `crates/kotor-tool/tests/blender_export.rs` (the export).

## Not tried

The add-on has not been run inside Blender in the build environment (Blender could not be started there), only its file writing and checks. If a button misbehaves, send the message at the bottom of Blender's window and the **Window > Toggle System Console** text.
