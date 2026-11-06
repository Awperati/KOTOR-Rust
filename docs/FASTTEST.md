# Running the game faster than real time for tests

The game runs in fixed steps of 1/60 s, and every game timer counts those steps. A faster run is the same steps with less waiting, so it plays the same game.

## How to run it

| Way | Command | Measured |
|---|---|---|
| No window | `cargo run --release -p kotor-game --example autopilot -- end_m01aa 900 120` | 606 game seconds per wall second |
| Window, 16x | `viewer --autopilot` (16x is the default with `--autopilot`) | 15.6 |
| Window, 64x | `viewer --autopilot --speed 64` | 58.5 |
| Window, 256x | `viewer --autopilot --speed 256` | 182 |
| Window, watching | `viewer --autopilot --speed 1` | 1, and lines stay up long enough to read |

Measured on 6 Nov 2026 over 900 game seconds, in a hidden 1280x720 window with `--mute`. With no window, the pilot leaves the Endar Spire at 503 game seconds, in about 0.8 wall seconds (worked out from the rate; the whole 900.8 game seconds took 1.5). That no-window figure was measured after the pilot began running and using the second module's real route, so it reaches busier places sooner. Blocking doors by faces had cost 440 against 606: every route for someone walking through doors worked out each door's faces twice. A door is now a line, which costs nothing to lift. The window figures are from before that and have not been measured again.

Other switches:
- `--speed N` works without `--autopilot` too, for playing by hand. I have measured it only with the pilot.
- `--game-seconds S` stops the window after that much game time.
- `--digest FILE` writes the state digests, and `AUTOPILOT_DIGEST` does the same with no window.
- `--render-every K` draws one frame in K. The default is speed / 16.
- `--fast-lines` keeps the pilot from holding lines at 1x, so that a 1x run can be compared with a fast one.
- `Game::steps_due` stops at 4096 steps per call, and time beyond what a call can catch up on is dropped. Dropping changes how fast a run goes, not what happens in it.

## Proof that it is the same game

`Autopilot::digests` holds a fingerprint every 600 steps, from `state_digest`. It covers the clock, the module, every object's position, hit points, doors, inventory and equipment, every line said, the fight log and the global numbers.

| Compared with no window | Game time | Digests | Different |
|---|---|---|---|
| Window at 1x | 120 s | 12 | 0 |
| Window at 16x | 120 s and 900 s | 12 and 90 | 0 |
| Window at 64x | 120 s and 900 s | 12 and 90 | 0 |
| Window at 256x | 900 s | 90 | 0 |

The 900 s runs cross six modules. `gpu/tests/fast.rs` repeats the check at 1x and 16x with uneven frames. `how_steps_are_grouped_into_frames_changes_nothing` also compares save files, with steps grouped in 1, 16 and 1024.

The first comparison failed. The window turned the speaker and the player to each other once per frame, and the player's heading steers its next walk. That now happens inside a fixed step, in `Game::face_speaker`.

## Door locks, skills, powers, fights and saves

`crates/kotor-game/tests/systems_at_speed.rs` runs these with no window, on the Endar Spire's own doors, panels and soldiers:

- **Locks and powers:** an untrained hero fails a door's Security check, a trained one picks it, two Force powers run out on game time, then a fight. Digest, event log and save file match with steps grouped in 1, 16 and 1024.
- **Computer Use:** a panel in `end_m01ab` opens its conversation for a hero with ranks. It does not yet check what the ranks change in that conversation.
- **In bulk:** 300 game seconds of fighting (491 blows) took 2.7 to 3.9 wall seconds in a test build with other tests running. Twenty saves and loads in a row took 0.25 to 0.37 and give back the same save.

## What it does not speed up

- **Drawing.** It is why a window tops out near 180x. Do not judge frame rate or how motion looks at speed.
- **Sound.** Above 1x the viewer plays none, so sound must be judged at 1x.
- **Loading files.** A module loads in wall time at every speed. Game time does not pass meanwhile, so results do not change.
- **`Stage::update_camera`, hover and the overlay.** They run once per frame, on wall time. They only change what is shown.

## What still needs real time

- **`scripts/drive_window.ps1` stays at 1x by default.** It posts real key presses and sleeps on the wall clock, so at 4x and 16x its checks fail: the hero runs past the footlocker during one sleep. That is the script's timing, not a game difference. It takes 19 s at 1x, with 10 of 10 PASS. `-Speed N` is there for trying.
- **A person's input is not repeatable.** The proof covers the pilot, which decides once per fixed step. Keys arrive between frames, so a run played by hand does not repeat.
- **Nothing here compares with the original game.** It shows that this engine agrees with itself at every speed.
