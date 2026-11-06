# Status of kotor-rs (6 Nov 2026)

A clean-room, std-only Rust reimplementation of KotOR 1's engine that reads the owner's installed game at run time. 13 crates, about 22,000 lines,
**257 tests, all passing** (`scripts/check.ps1`: fmt, clippy `-D warnings`, tests). Nothing is pushed; 33 local commits. Nothing writes to the install.
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
- Not built: the map, party and journal screens, the options pages, minigames (Pazaak, swoop, turrets), area-of-effect objects, damage types and resistances,
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
1. **Using items.** The inventory and equipment screens work for the player (`docs/PLAY.md`), but a medpack cannot be used yet and party members cannot be switched to.
2. **Character creation.** New Game starts the Endar Spire as a fixed Soldier. The main menu, pause menu and save, load and delete screens are in.
3. **Combat controls.** A click attacks, and that is all: no action queue, feats, powers or items from the slots, no pause. Only the first slot is drawn.
4. **Hearing the sound.** Voice, music, shots, blows, doors and steps are wired from the stock tables, and nobody has listened to them yet.
5. **Force power visuals and the other emitter types.** Bolts, muzzle flashes, sparks and rooms' `Fountain` emitters are in.
6. **Level-up, journal, map and party screens.** The rules exist; there are no screens.
7. **Traps, and a chosen Security action.** A click on a locked door tries a carried key, then Security against `OpenLockDC` by itself. Skills reach scripts through `GetSkillRank`, so Computer Use and Repair menus run, but spikes and parts have not been checked.
8. **Camera in tight places.** Line of sight and the camera against walls are in. The camera can still sink into doors and furniture.
9. **Dialogue camera angles, and scripted movement in fights.** Close shots stand in for the game's angles.
10. **The rest of the game.** The autopilot below plays into Taris with an unkillable hero; nobody has judged those modules by eye. Vendors, minigames, the galaxy map and stealth are not built.

## How far the autopilot gets
`cargo run --release -p kotor-game --example autopilot -- end_m01aa 6000 280` plays with no window. `--autopilot` does the same in the viewer. The pilot uses only what a player has: clicks, replies, taking items, attacks, changing leader, swapping a weapon. It cannot die (`Autopilot::immortal`), so none of this says the fights can be won.

Last run: 2,230 game seconds in 5.9 real seconds. `end_m01aa` -> `end_m01ab` -> `STUNT_00` -> `tar_m02af` -> `tar_m02aa` -> `tar_m02ac` -> `tar_m02ab` -> `tar_m02ac` -> `tar_m02ae`. 23 kills, 98 door uses, 12 items, 633 lines, no routine left unhandled. The window run covered the first 240 seconds, as far as the medpac talk.

What is left, by how early it blocks:
1. **Item use and level-up by hand.** Nothing can use a medpac, so `Mod_OnActvtItem` (`k_pend_activate`) never fires. Experience levels the hero by itself, with no choices. The pilot gets past both because later scripts set the tutorial step too.
2. **`k_pend_screenchg`** waits for the inventory and other screens through `GetSubScreenID`, which answers a fixed value. I do not know what calls that script in the original.
3. **Talents ignore their category** (`GetCreatureTalentBest`, `GetCreatureTalentRandom`): a creature casts the first power it can pay for, helpful or not.
4. **Grenades fly unseen.** Their rows in `spells.2da` name a projectile model, which is not drawn. No Force power has a projectile in that table.
5. **Slipping past a locked door.** Once, at `ptar_lockde` in `tar_m02aa`, the hero ended up on the far side of a closed locked door after many tries. Cause not found. The pilot now tries plain locked doors twice, which hides it.
6. **The crowded cantina, `tar_m02ae`: where the last run stops.** From about (120, 113) on, 27 walks make under a metre of progress in 8 seconds, ending at (104, 108), and the way back to `tar_m02ac` fails too. Cause not found; freeing steps away from people pressing on the hero changed nothing.
7. **`tar03_cutdlg` in `tar_m03aa`**, reached by one earlier run: its pause was never lifted. A pause now ends after 60 seconds and is logged as a script error, so a player cannot be stuck there, but the cause is not found and no test covers it.

Logged but no block: `end_door10_cut2`, `end_door19`, `end_door07` and `ptar_lockde` are shut as far as the pilot can tell and the way on lies elsewhere. `k_pend_carth11` and `k_swg_gizka01` are named by conversations but are not in the game's files.

Routines answering a fixed value on these modules, all others being real: `GetAttemptedSpellTarget`, `GetCreatureTalentBest`, `GetCreatureTalentRandom`, `GetEffectType`, `GetIsDebilitated`, `GetIsEncounterCreature`, `GetIsLinkImmune`, `GetLastAttackAction`, `GetNPCAIStyle`, `GetPartyAIStyle`, `GetPlayerRestrictMode`, `GetSoloMode`, `GetUserActionsPending`, `ResistForce`.

## Next, in order
Owner tries the window (`docs/PLAY.md`) and listens to the sound -> attack animations and the cutscene camera -> equipment on bodies -> owner test sheet.
