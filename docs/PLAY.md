# Playing in the window

`gpu/examples/viewer.rs` runs the game loop in a window. `gpu/src/stage.rs` holds everything but the window (camera, posed bodies, interface, what to play), so `gpu/tests/stage.rs` runs the same loop headless. `scripts/check.ps1` runs that test.

```
cargo run --offline --release --manifest-path C:\Users\User\Projects\kotor-rs\gpu\Cargo.toml --example viewer
```

The window opens only on DISPLAY2, and never takes focus by itself: click it to give it the keyboard.

| Input | What it does |
|---|---|
| Left click on the floor | walk there |
| Left click on a creature, door or placeable | walk up and talk, open or use; an enemy is attacked |
| 1-9 | take that reply |
| Space, Enter or a click | end a line that has no replies (it also ends by itself) |
| Q/E or the arrow keys | turn the camera |
| Wheel | zoom |
| Esc | quit |

Options: `--module M`, `--installed-mod`, `--orbit D`, `--volume V`, `--mute`, `--frames N`, `--hidden`, `--monitor NAME`.

`gpu/examples/shots.rs` writes frames to `docs/shots/play` from the offscreen renderer: no window and no screen capture.

## Original files and an installed `.mod`
A `.mod` beside a module's `.rim` pair is an installed mod. The original game prefers it. The viewer loads the `.rim` pair by default, and `--installed-mod` uses the `.mod`; the label at the top left of the window says which. The owner's `end_m01aa.mod` has a `k_pend_start` that removes Trask when the opening ends and leaves a corpse; the `.rim` pair has no such script.

## The camera and the sky
The camera starts 3.3 m behind and 1.7 m above the player, and comes closer where the floor ends so that it stays inside the room. A mesh over 150 m wide with no lightmap counts as a sky (an assumption): it is drawn first, around the camera, with no depth. `compare.rs` turns that off to match the software renderer.

## Interface on a wide screen
GUI files lay out 640x480. The interface is drawn 480 units high and as wide as the window's shape makes it, then stretched over the whole window. **Assumed, not compared with the game:** the left of the HUD stays, the party and map keep their distance from the right edge, the action slot and notice icons stay centred, and the dialogue box is a bar from edge to edge.

## Other assumptions
- The player is the recommended Soldier until there is a character creation screen.
- The player's body is column b of `appearance.2da`. Equipped armour does not change it yet.
- A body whose supermodels have no `dead` clip is laid flat.
- A node with no replies lasts its cutscene beat (`docs/CUTSCENE.md`), then moves on.
- Combat clips: `weaponwield` in `baseitems.2da` picks the digit in the clip name (1, 2, 3 as they are; pistol 5, rifle 7, heavy 9; no weapon 8), read from the clip names. `b<n>a1` fires, `c<n>a1` swings a one-handed or double weapon, `g<n>a1` swings anything else, `g<n>g1` is a flinch, `g<n>d1` a dodge and `g<n>r1` the ready stance. Only the first of each is used. All of it is unchecked against the game.
- A fixed area camera's `Orientation` is (w, x, y, z), and with no turn it looks along +y.
- Cutscene `Angle` codes are not modelled: those beats use the follow camera turned to the speaker.

## Measured
- A camera model's `camerahook` and a stunt's animation are in world space, and the hook looks along its local -z: in the opening's 18 shots the stunt's head lies at -0.91 to -1.00 along that axis.
- A player's head model has its origin at the neck and rides the body's `headhook`.

## Not done
- Weapons and armour are not drawn on bodies.
- Target display and action slots on the HUD, the map picture, and notices.
- Transparent textures and effects. Everything is drawn opaque.
