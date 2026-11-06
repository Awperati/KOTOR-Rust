# Playing in the window

`gpu/examples/viewer.rs` runs the game loop in a window. `gpu/src/stage.rs` holds everything but the window (camera, posed bodies, interface, what to play), so `gpu/tests/stage.rs` runs the same loop headless. `scripts/check.ps1` runs that test.

```
cargo run --offline --release --manifest-path C:\Users\User\Projects\kotor-rs\gpu\Cargo.toml --example viewer
```

The window opens only on DISPLAY2, and never takes focus by itself: click it to give it the keyboard.

It opens on the main menu (`mainmenu.gui`). New Game starts the Endar Spire, and Exit closes the window. `--skip-menu` starts straight in the game, which is what `Stage::open` and the test driver do. The movie button, the 3D model behind the menu, the debug warp and the new-content notice are left out; Options opens the pages below.

| Input | What it does |
|---|---|
| W A S D | run in that direction, relative to the camera; Shift walks |
| Right button drag, or the arrow keys | turn the camera round the player |
| Left click on the floor | walk there |
| Left click on a creature, door or placeable | walk up and talk, open or use; an enemy is attacked |
| E | use the nearest thing within 2.5 m |
| Left click on a reply, or 1-9 | take that reply, or take an item from a container or a body |
| Space or Enter | end a line, or close a container |
| Wheel | zoom |
| Esc | back out of a screen, or pause |
| I | inventory |
| U | equipment |
| J | journal |
| M | map |
| C | character |

Options: `--skip-menu`, `--module M` (where New Game starts), `--installed-mod`, `--orbit D`, `--volume V`, `--mute`, `--frames N`, `--hidden`, `--monitor NAME`, and for test drivers `--state-log`, `--near TAG`, `--autopilot` (a scripted player takes the controls and prints what stalls) and `--speed N` (`docs/FASTTEST.md`).

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
Esc in the game opens `optionsingame.gui` and stops the game's clock. Exit Game goes back to the main menu with the game gone, and Close or Esc goes back to the game. Feedback and Auto-Pause are greyed and do nothing.

## Options
`optionsmain.gui` from the main menu, or straight from the pause menu. The choices are kept in `%LOCALAPPDATA%\kotor-rs\settings.txt`, and a `--skip-menu` run ignores that file.

| Page | Works | Greyed or left out, because nothing behind it is built |
|---|---|---|
| Gameplay | difficulty, `CB_INVERTCAM` (tilts the camera the other way), Autosave (writes `autosave.sav` on each change of module, off by default) | Auto Level Up, mini-game axis, Combat Movement, Key Mapping, Mouse Settings |
| Graphics | Brightness, Screen Resolution (goes through 1280x720, 1920x1080, 2560x1440, 3840x2160) | Shadows, Grass, Advanced Options |
| Sound | music, voice and effects levels | Movie Volume and Advanced Options, left out |

**Assumed, not compared with the game:** difficulty multiplies what the party takes by the `multiplier` of `difficultyopt.2da` (0.5, 1, 1.5), which is a table fact, but where the game applies it is a guess; left is easier; brightness is a layer in the overlay, an offset instead of the game's gamma; the label of `CB_INVERTCAM` reads "Mouse Look", and I went by its tag. Nobody has heard the sound levels.

## Saving
Saves are files in `%LOCALAPPDATA%\kotor-rs\saves` and nowhere else, and writing inside the install is refused. Save Game lists them newest first with a New save row: choose a row, press Save, type a name, press OK or Enter. Choosing an old save overwrites it; a file is written beside the old one and then moved over it, so a crash does not cost the old save. Load Game works from the pause menu and the main menu, from any module to any module. Delete asks once, then takes the file away. A file this engine cannot read is not listed.

Saving is refused during a conversation or a fight, because neither is saved. Also not saved: scripts' pending `DelayCommand` blocks and queued actions. There is no thumbnail of the scene, so `LBL_SCREENSHOT` is left dark. Area names like `Endar Spire - Command Module` are split into the planet and area lines (an assumption about the names).

## Inventory and equipment
`inventory.gui` and `equip.gui`, for the player only. The inventory lists what is carried with its picture and stack, the portrait, vitality, defense and credits. A click on an item shows its words. The equipment screen lists what fits the last slot under the cursor, with what is worn marked "(Equipped)", to look at only. A click on a slot lists it to choose from: None comes first, the words of the row under the cursor or chosen take the place of the slots, and OK or Enter puts on what is chosen. Cancel or Esc goes back to the slots with nothing changed. Damage and to hit follow `combat.rs`: the left hand's only show when it holds a weapon, and combat does not use them yet.

Item pictures are `i<itemclass>_<number>`. Measured over the 557 item templates in the BIFs: 430 go by `ModelVariation`, 97 (the armour) by `TextureVar`, and 30 have no picture at all, `defaulticon` included. `baseitems.2da` `equipableslots` is a mask of slots. **Assumed, not compared with the game:** every `reqfeat` of a row must be known; `droidorhuman` 1 is humans and 2 droids; a weapon that fits only the right hand takes both hands; identical items join a stack up to `stacking`, weapons included; a click in the inventory wears nothing. Party switching and quest items are not built: the first buttons are hidden and the second greyed.

**Use Item** works on the item whose words are on show, if it has a `CastSpell` property. Measured: a medpack's property 10 has `Subtype` 64, row 64 of `spells.2da` is `ITEM_ABILITY_MED_PACK`, and its script `k_sup_healing` does the healing. **Assumed:** using an item takes no time, and has no skill or feat check.

## Character
`character.gui`, for the player, with the numbers from the rules: class, level, experience and what the next level needs, vitality, Force, defense, the six attributes, the three saves and alignment. **Assumed:** one `LBL_GOOD` mark down the scale shows the alignment, light at the top. The 3D figure is not built, so the portrait stands in its place. Level-up and party switching are hidden, and Scripts is greyed.

## Levelling up
Experience does not raise the player's level by itself: `World::hold_levels`, which the stage sets, makes the level wait. A mark shows on the portrait, and the character screen shows Auto Level Up and Level Up. `maincg.gui` with `leveluppnl.gui` over it lists the steps, and a step opens once those before it are done: Attributes on levels divisible by 4, Skills, Feats when there are slots, Powers when the class gets some, Accept. Back or Esc on the panel puts the creature back as it was. Cancel or Esc on a page puts back that page.

Feats and powers are chains of up to three pictures, each with an arrow to what it leads to: green is had already, dim cannot be taken, yellow is chosen or under the cursor. Recommended follows the class's columns in `feat.2da`, `skills.2da` and `classes.2da`, and the alignment's in `spells.2da`. Party members still level up by themselves. **Assumed, not compared with the game's code:** see the top of `crates/kotor-game/src/levelup.rs`.

## Journal
J, the HUD's button or the tab opens `journal.gui`. It lists the quests in `World::journal` with their names and the words of their state from `global.jrl`, with the planet first as the file has one. One button swaps active and completed quests, and one goes through the four orders. Scripts and conversations both set a state, and a save keeps the order. Quest items are not built, so their button is greyed. Experience for a quest's state, the file's `XP_Percentage`, is not paid here. **Assumed, not compared with the game's code:** see the top of `crates/kotor-game/src/journal.rs`.

## Pause
Space or the HUD's hourglass pauses and goes on, outside conversations and with no container open. Space does nothing for half a second after a conversation, so one press too many does not pause. The camera still turns. Things pause the game by themselves as the Auto-Pause page says: a new enemy seen, a party member down, a change of target, the end of a round, the action menu. A box at the top right says why. A conversation lifts a pause. `--no-auto-pause` turns those off for a run, and `--autopilot` has none.


## Fights
An enemy under the cursor has a red bar and three slots: the attack, a feat and a Force power. The arrows go through what each has, and a click attacks with it next. Feats are Power Attack, Power Blast, Critical Strike, Sniper Shot, Flurry and Rapid Shot, at the strongest level known and for the weapon held. The numbers are the talk table's words for each. Blows show over the target, with Floating Numbers on. Disengage stops attacking.


## Feedback and Auto-Pause
From the pause menu or the main menu's Options. Feedback has Hide Unequippable, Tutorial Popups, Subtitles, Mini Map and Floating Numbers. Auto-Pause has End of Combat Round, Enemy Sighted, Party Member Down, Action Menu Used and New Target Selected; Mine Sighted is greyed, as there are no mines. See Pause. Auto Level Up on the Gameplay page takes the player's levels as they come, and with Combat Movement off the keys and the floor do not move the player in a fight. Enemy Sighted and Party Member Down start ticked: **assumed**. All of it is kept in the settings file.


## Pop-ups and notices
A tutorial pop-up comes up once in a game for what `tutorial.2da` names, or when a script calls `ShowTutorialWindow`. The game waits, and OK, Continue or Esc goes on. `tutorials=false` in the settings file turns them off, and `--no-tutorials` does for one run; `--autopilot` has none. Icons at the top of the HUD flash for a new journal entry, a change in credits, experience from a script, items gained or lost outside the screens, and a change of alignment. Experience from a kill has no icon.


## Party
`partyselection.gui` shows who is on offer, from `World::available`, with the file's dark pictures for who is not. A click shows someone, Add or Remove changes who comes along, up to two, and nothing happens until OK. A script asks for it with `ShowPartySelectionGUI`: those it forces are chosen already and stay, Cancel is greyed, and its exit script runs on OK. The map's button will open it too. The viewer's `--autopilot` has no screens, so forced members join straight away and levels are taken as they come. A member who is left behind is gone from the world and keeps their levels, abilities, skills, feats, powers and items for when they come back, through a save too. They gain no experience meanwhile. A save keeps the leader, the character the player made, and `min_one_hp`.


## Map
M, the HUD's button or the tab opens `map.gui` with the area's name and its picture, `lbl_map<area>`. Marks show the player, with an arrow that turns, and the notes the area's waypoints have, and the arrows go through them. The HUD's small map shows the picture round the player, and a click on it opens the map. Party Selection opens that screen, except in a fight. Return to Hideout is greyed. What has not been seen shows a grid, and a save keeps what has. **Not built:** `SetMapPinEnabled`. **Assumed:** see the top of `crates/kotor-game/src/area_map.rs`.


## Loot
All of this is the game's own data, measured with `crates/kotor-game/examples/loot_census.rs` over the BIFs and 117 modules. There is no `itemcreate` table in the game: its 2DAs with item in the name are `baseitems`, `itempropdef`, `itemprops`, `itemvalue` and `skillvsitemcost`. `treasurescale.2da` exists and no template or script here was seen using it.

- **Placeables:** 408 of 1,467 templates have `HasInventory`, 354 have an `ItemList` of `InventoryRes` names, and those items go in the first time one is opened. 45 are locked, and since locks are not built they stay shut.
- **Treasure scripts:** 15 `k_plc_tres*` scripts in the BIFs, one per table of the source `k_inc_treasure` (civilian, military, corpse, shadowlands, droid, Rakatan, sand people), 7 of them set as `OnOpen` on placed containers in Kashyyyk. They roll `Random(100)` against their tables and call `CreateItemOnObject(template, container, count)`. The VM runs them as they are, so no table is copied here.
- **Creatures:** a UTC's `ItemList` is carried from the start, and an item with `Dropable` set stays on the body at death: 390 of 1,414 such items, and 147 of 2,148 worn ones. The rest goes with the creature. A body with something on it can be clicked, is called what `bodybag.2da` row 0 calls it ("Remains"), and opens like a container. Every creature template has `BodyBag` 0, so the other rows' bag models are not used.
- **The window:** `container.gui` over the scene, with the game running behind it and a dark backing behind its see-through panel (an assumption). A click on a row takes that item, Get Items takes all and closes, Close, Esc or Space closes, and running away closes it too. Give Items is hidden, because nothing can be put into a container yet.
- **Scripts on containers:** `OnOpen` runs at every opening, `OnClosed` at closing, and `OnInvDisturbed` when something is taken, with `GetLastDisturbed` and `GetInventoryDisturbItem` answering.
- **Credits** (`baseitems.2da` label `Credits`) become party credits when taken. Identical items join a stack up to `stacking`.

**Assumed, not compared with the game:** an item a script gives a creature stays on the body when it dies; `OnInvDisturbed` only ever hears of a removal, since nothing can be put into a container yet.

## Menus
Stock GUI files drawn by `crates/kotor-ui/src/menus.rs`, with the screens' logic in `gpu/src/menus.rs`. A 640x480 screen is stretched over the whole window, as in the reference video: see `docs/MENU_REFERENCE.md` for what was compared and what still differs. **Assumed:** controls are drawn in file order, later ones on top; a button under the cursor has yellow text. Text alignment is read as bits: horizontal 1 left, 2 centre, 4 right, vertical 8 top, 16 middle, 32 bottom.

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

## Checking fast
`scripts\check.ps1` with no switches is the gate for commits: fmt, clippy, every test and doc tests. It prints a last line `gate: P passed, F failed`, and exits 0 only when nothing failed. `-Fast` leaves out fmt, clippy and doc tests, for the inner loop. `-Only kotor-game` runs one crate's tests, and `-Only gpu` everything in `gpu\`. Test binaries run at once, half as many as the machine has logical processors, `-Jobs N` to change it.

The time went to reading every file of the game unoptimised, and to test binaries running one after another. The crates that read files are now optimised in test builds, with overflow checks and debug assertions still on. `scripts\drive_window.ps1` waits for the window to report instead of sleeping fixed times. It only holds a key down for a set time where the distance walked is the point.

## When a read fails
One gate run lost 62 scripts in a row as "not found" in `every_endar_spire_script_runs_in_the_world`, and the cause was not caught. Then 1.15 million reads and 600 test runs under load gave no failure, and neither did about 30 minutes of a watcher (`crates/kotor-archive/examples/read_watch.rs`). Antivirus is off, the System log has no disk errors, and the 1440p Builder had not touched the files. `Get-Volume E` does say `Full Repair Needed` on the exFAT USB drive that holds the game. Another program holding a module file with no sharing does give that exact pattern with one open per read: os error 32 on every read while it holds the file. So archives now stay open from their first read, a run of lost scripts cannot come from that, and a failed read names the file and the operating system's error.

## Measured
- TPC rows are stored bottom-up. `load_texture` flips them, so every picture has its first row on top. Before this fix all model textures were upside down, which scrambled faces and clothes.
- A camera model's `camerahook` and a stunt's animation are in world space, and the hook looks along its local -z: in the opening's 18 shots the stunt's head lies at -0.91 to -1.00 along that axis.
- A player's head model has its origin at the neck and rides the body's `headhook`.

## Not done
- More than one action slot on the HUD (the first shows the target's action), the map picture, and notices.
- Force powers' projectiles, and `Explosion` bursts timed to a model's own detonate key.
