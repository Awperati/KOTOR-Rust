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

## Open: the colour of that body

Neither texture on this machine matches the video. `PMBAM01` in `swpc_tex_tpa` is 512x512. The owner's `Override\pmbam01.tpc` is 2048x2048 and looks like an upscale of the same picture. Both have a dark brown top and shorts with gold trim, and the top has no sleeves on our model. The video's top is pale with orange trim and short sleeves, and room light does not turn brown pale.

So an opt-in for `Override` textures would not bring this look back, and I did not add one. The video's game may have mods of its own, which I have not confirmed. If so, its other looks need the same care, and its timings and distances are less affected.

Only the bare arms, bare legs and shorts support column a. No other column of the row was compared by eye.

## Changed because of them

- `rig_for_outfit`: with no armour a hero now wears column a, not column b.

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
