# Measurements from the reference video

The owner's longplay of the original game: `E:\Videos\Star Wars_ Knights of the Old Republic Full Walkthrough Gameplay - No Commentary 4K (PC Longplay) [xGPHIQGF4_Y].mp4`.

It is 1920x1080 at 30 frames a second, not 4K despite its name, and 76,684 s long (`ffprobe`). Its hero is a man.

Rules: the video and its frames stay out of git and out of the install. Frames are cached in `cache/ref`, which git ignores. Only written measurements come here. Drive E: reports "Full Repair Needed" and the owner said to leave it alone, so frames come from short seeks, each read once.

## Chapters

Read by eye from contact sheets of one frame every 5 minutes, so each time is good to about 5 minutes. The planets' order is this player's. `cache/ref/m60` holds a frame for every minute, named by the second.

| From (s) | From (h:mm) | Where |
|---|---|---|
| 0 | 0:00 | Logo and opening |
| 300 | 0:05 | Endar Spire |
| 1,500 | 0:25 | Taris |
| 12,000 | 3:20 | Taris, Sith base |
| 14,700 | 4:05 | Taris, swoop race |
| 18,000 | 5:00 | Leaving Taris |
| 19,800 | 5:30 | Dantooine |
| 31,200 | 8:40 | Tatooine |
| 41,100 | 11:25 | Kashyyyk |
| 46,800 | 13:00 | Manaan |
| 57,900 | 16:05 | Leviathan |
| 61,800 | 17:10 | Korriban |
| 70,200 | 19:30 | Unknown World |
| 73,800 | 20:30 | Star Forge, to the end |

## Measured

| What | Value | Time in video | Method | Margin |
|---|---|---|---|---|
| What the hero wears before the footlocker | Pale short-sleeved top and shorts with orange trim, bare arms and legs. The shape is column a of `appearance.2da` | 200 s | Looked at one frame beside our render | Shape clear in the frame. Colour differs, see below |
| What the hero wears after it | Sleeved shirt with a brown vest, column b (`Basic_Clothing`) | 230 s and 300 s | Looked at two frames | Clear in the frames. The shirt looks pale blue in the room's light |
| Run speed | 5.8 m/s | 372.75 to 374.25 s | The door ahead grows on screen. Its width in 7 frames, 6.16 m wide in model `DOR_LHR01`, 55 degree view from `camerastyle.2da` | 0.6 m/s |
| Hero's turn | 180 degrees in 5 frames, about 1,080 degrees a second | 303.08 to 303.22 s | Counted frames, keyboard run from a standstill | 20% |
| Camera's turn | About 85 degrees in 0.7 s, about 120 degrees a second | 301.97 to 302.67 s | Counted frames. The angle is read from where the door ends up | 20 degrees a second |
| Door opening | 21 frames, 0.70 s | 374.87 to 375.57 s | Counted frames of the first change to the last | 0.07 s |
| Combat round | 3.00 s | Bar drops at 471.73, 474.73, 477.73 s | The target's red health bar, measured on each frame at 15 a second | 0.07 s |
| Hits in that fight | 16%, 34%, 39% of the target's bar | Same | 61, 126, 145 px of a 372 px bar | About 1 point of the bar in 100 |
| Other gaps between hits | 2.54 s, 1.27 s, 1.80 s, 2.07 s | 381.43 to 383.97, 391.53 to 392.80, 455.47 to 459.33 s | Same | 0.07 s. More than one attacker may be hitting |
| Experience | 1,275, level 2, needing 3,000 | 657 s, the bridge | Read off the character screen | Exact |
| The hero then | Str 16, Dex 11, Con 14, Int 12, Wis 10, Cha 13. 26/26 vitality, defense 14, saves 5, 0, 0 | 657 s | Same | Exact |
| Dialogue cuts | 13 shots in 78.9 s, 0.9 to 11.0 s each, mean 6.1 s, black bars above and below | 1,705.2 to 1,784.1 s | Picture changes between frames at 10 a second, then one frame of each shot | 0.1 s |
| Who has the shot | All 5 shots with replies up show the hero. 7 of the other 8 show the speaker, and 1 is wide | Same | Looked at those 13 frames | Clear in the frames. One conversation only |

## Open: the colour of that body

Neither texture on this machine matches the video. `PMBAM01` in `swpc_tex_tpa` is 512x512. The owner's `Override\pmbam01.tpc` is 2048x2048 and looks like an upscale of the same picture. Both have a dark brown top and shorts with gold trim, and the top has no sleeves on our model. The video's top is pale with orange trim and short sleeves, and room light does not turn brown pale.

So an opt-in for `Override` textures would not bring this look back, and I did not add one. The video's game may have mods of its own, which I have not confirmed. If so, its other looks need the same care, and its timings and distances are less affected.

Only the bare arms, bare legs and shorts support column a. No other column of the row was compared by eye.

## Changed because of them

- `rig_for_outfit`: with no armour a hero now wears column a, not column b.
- `RUN_SPEED`: 3.96 m/s became 5.4 m/s. The old value is `rundist`, which is metres per run cycle. Over the 0.733 s cycle of `S_Male02` that is 5.40 m/s, and `runrate` in `creaturespeed.2da` says 5.40 too. High confidence: the video rules out 3.96.
- `STEER_TURN_RATE`: 12 became 18.8 radians a second. Medium confidence: one turn counted.
- `TURN_SPEED` in the viewer: 90 became 120 degrees a second. Medium confidence: one turn, and `camerastyle.2da` has 150 as `minturnrate`, whose meaning I do not know.
- `step_pc`: a clicked hero runs too, at `RUN_SPEED`, not at the walking speed. The hero runs everywhere I looked, though I do not know which of those runs were clicked.
- `speaker_camera`: holds the speaker while the line is said, then cuts to the player while the replies wait. Medium confidence: one conversation. Over-the-shoulder and wide shots are not copied.
- `MoveParams::speed`: 1.6 became 1.7 m/s, from the tables and not the video. See below.
- `award_kill`: each standing party member gets the whole kill, not a share. **Weak evidence, an assumption:** see below.

## Agreed, so not changed

- `ROUND_SECONDS` 3.0: the video gives 3.00. High confidence.
- `DOOR_ANIM_SECONDS` 1.0: `opening1` of `DOR_LHR01` lasts 1.0 s and moves from 0.2 s to 1.0 s, so 0.8 s of movement against the video's 0.70 s. Its last part may be outside the frame. I took the door to be that model from its look and `end_door05`, `07` and `08`, and did not confirm its tag. Medium confidence.
- `genericdoors.2da` row 58, `DOR_LHR02`, has animations 5 s long. It is not the door measured.

## How sure each is

The video's game may be modded, which I have not confirmed. Its hero's clothes do not match this install's textures. Speeds and rounds come from tables a texture mod would not touch, so a mod is less likely to change them. It carries a `LACRY` watermark.

Run speed assumes the camera sits 55 degrees wide vertically. That is `camerastyle.2da`, not measured, and a widescreen patch could change it. Measuring the hero's height against the 3.2 m camera distance gives 9% more, so 5.4 sits at the low end of what the video allows. It still rules out 3.96.

## Not measured

- **Walk speed:** the hero runs everywhere I looked, and I timed no walker. `walkrate` is 1.70 for `NORM`. `walkdist` over the walk cycle gives 1.70 for a man (1.813 m, 1.067 s) and 1.60 for a woman (1.6 m, 1.0 s). Only 1.70 is used, for everyone.
- **`creaturespeed.2da` per creature:** `moverate` and `WalkRate` in a creature's file can pick slower or faster rows. Everyone still uses one speed. `PLAYER` has a `walkrate` of 3.20, which I did not check against the hero.
- **NPC turn rate:** still the assumed 4.0 radians a second.
- **Kill experience:** the video shows one total only, so no kill can be read from it. The bridge total of 1,275 is a whole number of 25s, which a split with Trask would rarely give, and that is the only evidence for `award_kill`. I also do not know if Trask stood at those kills. Our pilot has 1,025 at the same place. It skips scripts that pay experience, such as `k_pend_chest01` and `k_pend_bridge04`, so the gap proves nothing yet.
- **Damage numbers:** the words floating over heads are too small here, "miss" is the only one I could read. The percentages above are of the bar. The target may be `n_sithsoldier003` with 15 hit points, which would make them 2.5, 5.1, 5.8. Those are not whole numbers, so either the target or the bar reading is off.

## What the game's files say about skin

Measured by `crates/kotor-render/tests/heads.rs`, average texture colour as red, green, blue, for the three women and three men in that test:

| Row | Body texture | Upper arm | Neck |
|---|---|---|---|
| 92 | `PFBAMA01` | 213, 151, 101 | 145, 97, 66 |
| 107 | `PFBAMB01` | 152, 83, 53 | 106, 51, 32 |
| 122 | `PFBAM01` | 188, 134, 104 | 184, 119, 90 |
| 137 | `PMBAMA01` | 196, 149, 97 | 161, 94, 71 |
| 152 | `PMBAMB01` | 96, 47, 23 | 128, 60, 31 |
| 167 | `PMBAM01` | 222, 165, 134 | 180, 111, 99 |

So a body and its head do not match exactly in the files themselves. Column b has one body texture for all skin families, `PFBBM01` or `PMBBM01`, and its pale parts are the shirt.

## What the video cannot tell

- A woman's body or neck in close-up: its hero is a man. Whether the owner's neck frame matches the original is still open.
- HUD layout at 4K: it is 1080p.
- Anything in this engine's own terms, such as object numbers or script state.
