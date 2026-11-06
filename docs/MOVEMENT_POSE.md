# Editing the roll and the landing

The roll and the landing have no animation in the game's files, so they are built from angles in two plain text files. You can change them with Notepad. No code and no rebuild are needed.

## Where the files are

`%LOCALAPPDATA%\kotor-rs\roll_pose.txt` and `%LOCALAPPDATA%\kotor-rs\landing_pose.txt`, beside the settings file. The game writes them the first time it starts from its main menu. Paste the first path into the Windows Run box (Win+R) to open one.

## How to change a pose

1. Open the file in Notepad, change a number, and save.
2. In the game press **F8**. "Poses reloaded" rises over the player, unless floating numbers are off in the options.
3. Press the roll key (Left Ctrl unless you changed it), or fall off something for the landing.

Modern movement must be on: Options, Gameplay, "Movement: Modern". To see a move slowly, start the game with `--speed 0.1`. F8 is not on the keymap screen.

## What is in the file

**A key** is one moment of the move. It starts with `[key]`. The game moves smoothly from each key to the next.

| Line | What it does | Range |
|---|---|---|
| `at` | How far through the move this key is. 0 is the start and 1 is the end. Each key's must be higher than the last. | 0 to 1 |
| `ease` | How it moves into this key: `smooth`, `linear`, `in` (starts slow), `out` (ends slow). Leave it out for `smooth`. | |
| `tuck` | At 1 the running animation has gone. In a roll, the body's middle is also down at `centre_height`. | 0 to 1 |
| `lower` | How far the whole body sinks, in metres. A landing uses it to keep the feet on the floor as the knees bend. | 0 to 1 |
| a joint | Up to three angles in degrees: bend, out, twist. | -180 to 180 |

**The roll file has two more lines**, before the first key:

| Line | What it does | Range |
|---|---|---|
| `turns` | Whole somersaults. 0 stays upright, and a minus rolls backward. | -3 to 3, whole numbers |
| `centre_height` | How high the body's middle is over the floor when `tuck` is 1, in metres. Too low and the head and feet go through the floor. | 0.3 to 1.5 |

The roll's length of 0.55 seconds and its distance of 3.2 m are not in these files.

## The three angles

| Angle | A positive number | A negative number |
|---|---|---|
| **bend** | Curls the joint in: knee to the chest, heel to the bottom, chin to the chest, elbow closed | Opens it the other way |
| **out** | Moves the limb out to the side, away from the body | Brings it in, across the body |
| **twist** | Turns the limb about its own length, its front going outward | Turns it inward |

Left and right work the same way, so give both sides the same numbers to keep the body even. You can leave out numbers at the end: `lshin_g = 135` means bend 135, out 0, twist 0.

Every angle counts from the skeleton's own standing pose, with its arms held about 35 degrees out from the body. So arms usually need a minus in `out` to come in. A joint a key does not name is at 0 in that key.

## The joints

| Joint | What it is | Safe bend | Safe out | Safe twist |
|---|---|---|---|---|
| `lthigh_g` `rthigh_g` | Hip | -30 to 140 | -30 to 60 | -40 to 40 |
| `lshin_g` `rshin_g` | Knee | 0 to 150 | do not use | do not use |
| `lfoot_g` `rfoot_g` | Ankle. Positive bend lifts the toes. | -50 to 35 | -20 to 20 | -30 to 30 |
| `torso_g` `torsoupr_g` | Lower and upper back | -30 to 50 | -30 to 30 | -40 to 40 |
| `necklwr_g` `neck_g` `head_g` | Neck and head | -30 to 45 | -30 to 30 | -60 to 60 |
| `lcollar_g` `rcollar_g` | Shoulder blade | -20 to 20 | -20 to 20 | do not use |
| `lbicep_g` `rbicep_g` | Shoulder. Positive bend swings the arm forward. | -60 to 170 | -40 to 120 | -70 to 70 |
| `lforearm_g` `rforearm_g` | Elbow | 0 to 145 | do not use | -70 to 70 |
| `lhand_g` `rhand_g` | Wrist | -60 to 60 | -20 to 20 | do not use |
| `pelvis_g` | The hips. It moves the legs, not the back. | -30 to 30 | -15 to 15 | -30 to 30 |

The safe ranges are where a body can bend, and they are my estimates. The game refuses only an angle past 180 either way.

One thing that catches people out: a bone hangs from the one above it. Curling the back forward swings the hanging arms backward by the same amount. The default roll curls the back 60 degrees in all, which is why its shoulders are bent 48 to keep the hands on the shins.

## An example

Knees tighter to the chest in the roll. Change these lines in **both** of the tucked keys, at 0.18 and at 0.84:

```
lthigh_g   =   135    8    0
rthigh_g   =   135    8    0
```

## If something goes wrong

- **A mistake in a file:** F8 says "Pose file mistake". The game uses its own pose for that file, and writes the file and line into `pose_problems.txt` in the same folder. Your file is left alone, so you can fix the line and press F8 again.
- **Back to the start:** delete the file. The game writes a fresh one the next time it starts from its main menu. Until then it uses its own pose.
- **New versions of the game** do not change your files, so you keep your edits but do not get new default numbers. Delete a file to get them.

## Pictures without playing

This needs the source code. It writes four moments from the front and four from the side, as `roll_front.png`, `roll_side.png`, `landing_front.png` and `landing_side.png`:

```
cargo run --offline --release --manifest-path gpu\Cargo.toml --example pose_preview -- pose_preview
```

Add `--folder SOME_FOLDER` to read the files from another folder. Any mistake is printed.

## For a developer

`gpu/src/bodypose.rs` reads the files. `gpu/src/moving.rs` turns the joints. The defaults are `gpu/src/roll_pose.txt` and `gpu/src/landing_pose.txt`.

Each angle turns about a line in the body's frame, carried into the frame of the joint's parent in the standing pose. The bones' own axes cannot be used: `lcollar_g` and `rcollar_g` have an x axis about 60 degrees from the body's side to side line, and `rcollar_g`'s y axis is flipped. The clip gives way to the skeleton's own pose, not to `pause1`, because `pause1` has one foot forward.
