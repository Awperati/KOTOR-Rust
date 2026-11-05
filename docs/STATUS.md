# Status of kotor-rs (6 Nov 2026)

A clean-room, std-only Rust reimplementation of KotOR 1's engine that reads the owner's installed game at run time. 13 crates, about 22,000 lines,
**220 tests, all passing** (`scripts/check.ps1`: fmt, clippy `-D warnings`, tests). Nothing is pushed; 33 local commits. Nothing writes to the install.
Only `gpu/` (GPU renderer, window, MP3, audio output) needs crates.io. It is fetched, builds offline, and passes clippy; its picture matches the software renderer.

## What works (each checked by tests against the real install)
- **Formats**: KEY/BIF, ERF/MOD/SAV/RIM (read and write), GFF, 2DA, TLK, TGA/TPC (DXT1/5), MDL/MDX (meshes, skins, animations), BWM/LYT/VIS, NCS bytecode.
- **Scripts**: a VM runs NCS. Over all **12,838 scripts** of the game (1,784 in the BIFs, 11,054 in 117 modules) the 469 routines they call cover
  347,763 call sites: 95.3% are handled (344 routines, of which some only *record* the request: animations, music, UI), 4.5% answer a fixed value on purpose
  (25 routines: combat-AI queries and the like, `World::constant_defaults`), **0.2% (100 routines, 612 sites) are not handled** (mostly the minigames, `SWMG_*`).
  `cargo run --release -p kotor-game --example census` reprints this.
- **Endar Spire** (`end_m01aa`): all 195 scripts run with no VM fault and no unhandled routine; the opening conversation, Trask's talk, doors, triggers, the
  transition to `end_m01ab`, party following, collision, and a whole headless battle run by the game's own AI scripts.
- **Rules from the 2DAs**: d20 combat with rounds, saves, skills, feats, level-up, kill XP, knock-outs, 42 Force powers through their real impact scripts with timed
  effects, character creation, save/load round-trip, cutscene timeline, WAV/ADPCM decoding, GUI/font layout and a software-rendered dialogue box and HUD.
- **Pictures**: software renderer for areas, characters with skinning and animation, walk sheets (`docs/RENDER.md`).

## What is guessed (documented where each is used)
Walk speed 1.6 m/s and turn rate; door block radius, door swing 1.0 s (measured) and which side's clip; creature radius 0.4 m and follow distances; combat
(3 s rounds, one attack per round, reach 2 m / 12 m, critical confirmation roll, no line of sight); **kill XP = challenge rating x 25**; average level-up HP;
spell save DC; x4 skill points at level 1; knocked-out recovery to 1 HP; the 30-point attribute schedule (the three classes' recommended scores do cost exactly
30, which supports it); which appearance column is which body size; cutscene lengths (animation ids from 1200 index the stunt model's animations: ids 1200-1203
match the four animation lengths in order, ids 1219/1220 unresolved); GUI text alignment codes, control draw order, and the font fallback for `fnt_d16x16`.
See `COMBAT.md`, `RULES.md`, `CHARGEN.md`, `CUTSCENE.md`, `UI.md`, `GAME.md`.

## Unverified against the real game
- **Nothing has been compared with the original engine running.** Every check is against the game's *data* (tables, scripts, models), not its behaviour.
- `docs/OWNER_TEST_SHEET.md` (replace `end_m01aa.mod`, console warp) is prepared and **not run**.
- Only the Endar Spire has been exercised end to end; other modules load and their scripts parse, but their scenes have not been played.
- Saves are this engine's own layout; the original's saves are not read and ours do not open in the original.
- MP3 (all voice-over, most music): all 558 files decode through `gpu/audio` (symphonia); lengths agree with the header estimate within 4.4%. The game loop runs in a window on DISPLAY2 (`docs/PLAY.md`). Its sound has only been run at volume 0, so nobody has heard it.
- **The owner's install is modded.** `end_m01aa.mod` replaces the module's `.rim` pair and its `k_pend_start` removes Trask after the opening; `--original` ignores it. Tests that open the module without `set_prefer_rim` ran against that `.mod`, so their Endar Spire results describe the modded module.
- Not built: window and input, menus, inventory/equip/map screens, minigames (Pazaak, swoop, turrets), area-of-effect objects, damage types and resistances,
  grenades, dual wielding, speed/haste, stealth, lock picking and traps, vendors, the galaxy map, and cutscenes that use models not in the opening.

## What the owner could check by playing the real game side by side
1. **Kill XP**: the XP a first Sith soldier gives (we say challenge rating x 25; the real number decides that rule).
2. **Level 1 numbers**: a new Soldier with the class's recommended scores: HP (we get 12), defense (12), attack bonus (+1), saves (+4/+2/+1), skill points (8), feats.
3. **Attribute buying**: the point cost of raising a score to 15, 16, 17, 18 (we use 2, 2, 3, 3 per step).
4. **Walk speed**: seconds from the start position to the first door (we walk 1.6 m/s; door about 5.7 m away).
5. **Opening cutscene**: its length (we get about 19 s for the path the script takes) and where the camera cuts.
6. **First fight**: HP of the first Sith soldier (15 in our data), whether a knocked-out party member stands up with 1 HP, and how long a Force Armor lasts.
7. **Dialogue box and HUD**: compare `cargo run -p kotor-ui --example ui -- <dir>` PNGs with a screenshot (font, spacing, where the reply list sits).
8. **Level-up**: XP at which level 2 arrives (1,000 from `exptable.2da`) and what the level-up screen offers (we give feats/skill points per `featgain.2da`).

## What a first playable still lacks
Most to least blocking, all absent unless stated:
1. **Inventory and equip screen.** Items can be taken from a footlocker but not worn or used, so the opening's own tutorial cannot be followed.
2. **Character creation and a main menu.** The game starts straight in the Endar Spire as a fixed Soldier. There is no new game, load or save screen, though saves work underneath.
3. **Combat controls.** A click attacks, and that is all: no action queue, feats, powers or items from the slots, no pause. Only the first slot is drawn.
4. **Sound effects.** Only voice and music are wired, and nobody has heard those. No blaster, door or footstep sounds.
5. **Blaster bolts, particles and hit effects.** Rounds resolve with animation only.
6. **Level-up, journal, map and party screens.** The rules exist; there are no screens.
7. **Locks, skills and traps on doors.** A locked door just stays shut.
8. **Line of sight, and camera against walls.** Enemies notice through walls, so distant fights start at once. The camera can sink into doors and furniture.
9. **Dialogue camera angles, and scripted movement in fights.** Close shots stand in for the game's angles.
10. **The rest of the game.** Only the Endar Spire has been played; other modules load but are not tested. Vendors, minigames, the galaxy map and stealth are not built.

## Next, in order
Owner tries the window (`docs/PLAY.md`) and listens to the sound -> attack animations and the cutscene camera -> equipment on bodies -> owner test sheet.
