# Playing in the window

`gpu/examples/viewer.rs` runs the game loop in a window. `gpu/src/stage.rs` holds everything but the window (camera, posed bodies, interface, what to play), so `gpu/tests/stage.rs` runs the same loop headless. `scripts/check.ps1` runs that test.

```
cargo run --offline --release --manifest-path C:\Users\User\Projects\kotor-rs\gpu\Cargo.toml --example viewer
```

The window opens only on DISPLAY2, and never takes focus by itself: click it to give it the keyboard.

It opens on the main menu (`mainmenu.gui`). New Game starts the Endar Spire, and Exit closes the window. `--skip-menu` starts straight in the game, which is what `Stage::open` and the test driver do. The movie button, the 3D model behind the menu, the debug warp and the new-content notice are left out; Options says it is not built.

| Input | What it does |
|---|---|
| W A S D | run in that direction, relative to the camera; Shift walks |
| Right button drag, or the arrow keys | turn the camera round the player |
| Left click on the floor | walk there |
| Left click on a creature, door or placeable | walk up and talk, open or use; an enemy is attacked |
| E | use the nearest thing within 2.5 m |
| Left click on a reply, or 1-9 | take that reply, or take an item from a container |
| Space or Enter | end a line, or close a container |
| Wheel | zoom |
| Esc | back out of a screen |

Options: `--skip-menu`, `--module M` (where New Game starts), `--installed-mod`, `--orbit D`, `--volume V`, `--mute`, `--frames N`, `--hidden`, `--monitor NAME`, and for test drivers `--state-log`, `--near TAG` and `--autopilot` (a scripted player takes the controls and prints what stalls).

`scripts/drive_window.ps1` plays the running window with messages posted to its own handle only, with no global input and no focus change, and prints PASS or FAIL for replies, the footlocker, WASD and click-to-walk.

`gpu/examples/shots.rs` writes frames to `docs/shots/play` from the offscreen renderer: no window and no screen capture.

## Resolutions
Any size works, and 1280x720, 1920x1080, 2560x1440 and 3840x2160 are tested. The 3D picture is drawn at the window's size. The interface is laid out 480 units high and drawn pixel for pixel, so it keeps its proportions, and its corners stay at the corners on any shape. Use `--size WxH` or `--fullscreen`, or drag the window.

From 960 pixels high the stock interface pictures would be enlarged at least twice. `scripts/upscale_ui.ps1` exports them, upscales them with Topaz Gigapixel, and leaves them in `%LOCALAPPDATA%\kotor-rs\hd`, outside the repository and the install because they are made from game files. The interface uses them from then on, and `--no-hd` turns them off. A picture's alpha is upscaled separately as a grey picture (`name__a.png`), because the colour upscale cannot be relied on to keep it.

Upscaled 3D textures work the same way: `scripts/upscale_area.ps1` makes them for one area, 2x by default, in `%LOCALAPPDATA%\kotor-rs\hd3d`, leaving out lightmaps and pictures over 1024 pixels. The owner asked for HD, so they are used by default whenever they are there, and `--no-hd` turns them off.

Both scripts go through `scripts/gigapixel_guard.ps1`, because the GPU is shared: one job at a time, below normal priority, and they will not start, or will stop, with under 6 GB of video memory free. A stopped run can be started again, and it skips what is done.

### Frame times
Measured on this PC (AMD Radeon AI PRO R9700) in a hidden window, `--mute`, Endar Spire opening, 600 frames, with particles:

| Size | Upscaled textures | Frames per second | Frame time |
|---|---|---|---|
| 1920x1080 | on | 126 | 7.9 ms |
| 1920x1080 | off | 150 | 6.7 ms |
| 3840x2160 | on | 120 | 8.3 ms |
| 3840x2160 | off | 145 | 6.9 ms |

## Effects and sound
Rooms run their own `Fountain`, `Single` and `Lightning` emitters within 40 m of the camera. Scripts' `EffectVisualEffect` and `EffectBeam`, and powers' `casthandvisual`, play the stock models from `visualeffects.2da` and `spells.2da`. Ranged attacks fire a bolt with a muzzle flash and sparks where it lands. Sounds come from the stock tables: shots and impacts, melee swings and blows, doors, and the player's steps. **Assumed:** see `gpu/src/effects.rs` and `Stage::play_world_sounds`. Melee weapons pick their sound row by label, and the floor always sounds like metal.

## Original files and an installed `.mod`
A `.mod` beside a module's `.rim` pair is an installed mod. The original game prefers it. The viewer loads the `.rim` pair by default, and `--installed-mod` uses the `.mod`; the label at the top left of the window says which. The owner's `end_m01aa.mod` has a `k_pend_start` that removes Trask when the opening ends and leaves a corpse; the `.rim` pair has no such script.

## The camera and the sky
The camera starts 3.3 m behind and 1.7 m above the player, and comes closer where the floor ends so that it stays inside the room. A mesh over 150 m wide with no lightmap counts as a sky (an assumption): it is drawn first, around the camera, with no depth. `compare.rs` turns that off to match the software renderer.

## Interface on a wide screen
GUI files lay out 640x480. The interface is drawn 480 units high and as wide as the window's shape makes it, then stretched over the whole window. **Assumed, not compared with the game:** the left of the HUD stays, the party and map keep their distance from the right edge, the action slot and notice icons stay centred, and the dialogue box is a bar from edge to edge.

## Pause
Esc in the game opens `optionsingame.gui` and stops the game's clock. Exit Game goes back to the main menu with the game gone, and Close or Esc goes back to the game. Gameplay, Feedback, Auto-Pause, Graphics and Sound are greyed and do nothing.

## Menus
Stock GUI files drawn by `crates/kotor-ui/src/menus.rs`, with the screens' logic in `gpu/src/menus.rs`. **Assumed, not compared with the game:** a 640x480 screen stays centred on a wide window with black at the sides, except the main menu's background, which stretches; controls are drawn in file order, later ones on top (the HUD draws the other way round); a button under the cursor has yellow text. Text alignment is read as bits: horizontal 1 left, 2 centre, 4 right, vertical 8 top, 16 middle, 32 bottom.

## Other assumptions
- The player is the recommended Soldier until there is a character creation screen.
- The player's body is column b of `appearance.2da` until armour is worn. Worn armour picks the column from its `bodyvar` in `baseitems.2da`.
- Held weapons are always drawn in the hand, in and out of combat.
- Alpha: a TXI that names an environment or bump map makes it a shininess mask, so opaque. `blending additive` makes alpha follow brightness. Otherwise mostly clear-or-solid pictures are cut out and pictures with many in-between pixels are blended, with shares chosen by eye.
- A body whose supermodels have no `dead` clip is laid flat.
- A node with no replies lasts its cutscene beat (`docs/CUTSCENE.md`), then moves on.
- Combat clips: `weaponwield` in `baseitems.2da` picks the digit in the clip name (1, 2, 3 as they are; pistol 5, rifle 7, heavy 9; no weapon 8), read from the clip names. `b<n>a1` fires, `c<n>a1` swings a one-handed or double weapon, `g<n>a1` swings anything else, `g<n>g1` is a flinch, `g<n>d1` a dodge and `g<n>r1` the ready stance. Only the first of each is used. All of it is unchecked against the game.
- A fixed area camera's `Orientation` is (w, x, y, z), and with no turn it looks along +y.
- Cutscene `Angle` codes are not modelled: those beats use the follow camera turned to the speaker.

- A speaker runs up to the listener before a conversation starts, unless the script says to talk from where it stands. Trask's first talk is one that does (`k_pend_cut09`, from 7.5 m).
- In a conversation the camera shows the owner close up, 1.3 m in front of them. It stands in for the game's dialogue camera angles, which are not modelled.
- Creatures walk where scripts send them, and open doors that are not locked on the way, with no collision. Their speed and the rule that a queued action waits for arrival are assumed.

- Sight: enemies notice only along a line that stays over open floor, so walls and closed doors hide them. The walkmesh stands in for sight.
- The camera stops at walkmesh walls and closed doors, not at furniture.
- Chests with a `close2open` animation are drawn each frame. Lids stay open once opened.

## Measured
- TPC rows are stored bottom-up. `load_texture` flips them, so every picture has its first row on top. Before this fix all model textures were upside down, which scrambled faces and clothes.
- A camera model's `camerahook` and a stunt's animation are in world space, and the hook looks along its local -z: in the opening's 18 shots the stunt's head lies at -0.91 to -1.00 along that axis.
- A player's head model has its origin at the neck and rides the body's `headhook`.

## Not done
- More than one action slot on the HUD (the first shows the target's action), the map picture, and notices.
- Force powers' projectiles, and `Explosion` bursts timed to a model's own detonate key.
