# Playing in the window

`gpu/examples/viewer.rs` runs the game loop in a window. `gpu/src/stage.rs` holds everything but the window (camera, posed bodies, interface, what to play), so `gpu/tests/stage.rs` runs the same loop headless. `scripts/check.ps1` runs that test.

```
cargo run --offline --release --manifest-path C:\Users\User\Projects\kotor-rs\gpu\Cargo.toml --example viewer -- --original
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

Options: `--module M`, `--original`, `--volume V`, `--mute`, `--frames N`, `--hidden`, `--monitor NAME`.

`gpu/examples/shots.rs` writes frames to `docs/shots/play` from the offscreen renderer: no window and no screen capture.

## `--original` and a `.mod` file
A `.mod` beside a module's `.rim` pair is an installed mod. The original game prefers it, and so does this engine by default. The owner's install has `end_m01aa.mod`, whose `k_pend_start` removes Trask when the opening ends and leaves a corpse; the `.rim` pair has no such script. `--original` loads the `.rim` pair (`Install::set_prefer_rim`). The headless test uses it.

## Interface on a wide screen
GUI files lay out 640x480. The interface is drawn 480 units high and as wide as the window's shape makes it, then stretched over the whole window. **Assumed, not compared with the game:** the left of the HUD stays, the party and map keep their distance from the right edge, the action slot and notice icons stay centred, and the dialogue box is a bar from edge to edge.

## Other assumptions
- The player is the recommended Soldier until there is a character creation screen.
- The player's body is column b of `appearance.2da`. Equipped armour does not change it yet.
- A body whose supermodels have no `dead` clip is laid flat.
- A line with no replies lasts as long as its voice-over, or its reading time at 15 characters a second, plus 0.3 s.

## Not done
- Attack and hit animations: fighters stand in their idle clip while the rounds resolve.
- The cutscene camera: `kotor_game::Timeline` is not played yet. In a conversation the camera turns to the speaker.
- Weapons and armour are not drawn on bodies.
- Target display and action slots on the HUD, the map picture, and notices.
- Transparent textures and effects. Everything is drawn opaque.
