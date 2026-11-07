# Status of kotor-rs (6 Nov 2026)

A clean-room, std-only Rust reimplementation of KotOR 1's engine that reads the owner's installed game at run time. 13 crates, about 22,000 lines,
**689 tests, all passing** (`scripts/check.ps1`: fmt, clippy `-D warnings`, tests). Nothing is pushed; 33 local commits. Nothing writes to the install.
Only `gpu/` (GPU renderer, window, MP3, audio output) needs crates.io. It is fetched, builds offline, and passes clippy; its picture matches the software renderer.

## What works (each checked by tests against the real install)
- **Formats**: KEY/BIF, ERF/MOD/SAV/RIM (read and write), GFF, 2DA, TLK, TGA/TPC (DXT1/5), MDL/MDX (meshes, skins, animations), BWM/LYT/VIS, NCS bytecode.
- **Scripts**: a VM runs NCS. Over all **12,838 scripts** of the game (1,784 in the BIFs, 11,054 in 117 modules) the 469 routines they call cover
  347,763 call sites: 95.3% are handled (344 routines, of which some only *record* the request: animations, music, UI), 4.5% answer a fixed value on purpose
  (25 routines: combat-AI queries and the like, `World::constant_defaults`), **0.2% (100 routines, 612 sites) are not handled** (mostly `SWMG_*`).
  `cargo run --release -p kotor-game --example census` reprints this.
- **Endar Spire** (`end_m01aa`): all 195 scripts run with no VM fault and no unhandled routine; the opening conversation, Trask's talk, doors, triggers, the
  transition to `end_m01ab`, party following, collision, and a whole headless battle run by the game's own AI scripts.
- **Rules from the 2DAs**: d20 combat with rounds, saves, skills, feats, level-up, kill XP, knock-outs, 42 Force powers through their real impact scripts with timed
  effects, character creation, save/load round-trip, cutscene timeline, WAV/ADPCM decoding, GUI/font layout and a software-rendered dialogue box and HUD.
- **Movement**: Modern (momentum, gravity, a sprint at 1.25 times the run, a roll) or Classic, by a setting: `docs/PLAY.md`.
- **Pictures**: software renderer for areas, characters with skinning and animation, walk sheets (`docs/RENDER.md`).

## What is guessed (documented where each is used)
NPC turn rate; which side's door clip; creature radius 0.4 m and follow distances; combat
(one attack per round, reach 2 m / 12 m, critical confirmation roll, no line of sight); **kill XP = challenge rating x 25, paid in full to each standing party member**; Force points per level;
spell save DC; skill points as the video's Soldier has them (`docs/RULES.md`); knocked-out recovery to 1 HP; the 30-point attribute schedule (the three classes' recommended scores do cost exactly
30, which supports it); which appearance column is which body size; cutscene lengths (animation ids from 1200 index the stunt model's animations: ids 1200-1203
match the four animation lengths in order, ids 1219/1220 unresolved); GUI text alignment codes, control draw order, and the font fallback for `fnt_d16x16`.
See `COMBAT.md`, `RULES.md`, `CHARGEN.md`, `CUTSCENE.md`, `UI.md`, `GAME.md`. Run speed, turns, round length and door time are now measured against the reference video: `REFERENCE.md`.

## Unverified against the real game
- **Nothing has been compared with the original engine running.** Every check is against the game's *data* (tables, scripts, models), not its behaviour.
- `docs/OWNER_TEST_SHEET.md` (replace `end_m01aa.mod`, console warp) is prepared and **not run**.
- Only the Endar Spire has been exercised end to end; other modules load and their scripts parse, but their scenes have not been played.
- Saves are this engine's own layout; the original's saves are not read and ours do not open in the original.
- MP3 (all voice-over, most music): all 558 files decode through `gpu/audio` (symphonia); lengths agree with the header estimate within 4.4%. The game loop runs in a window on DISPLAY2 (`docs/PLAY.md`). Its sound has only been run at volume 0, so nobody has heard it.
- **The owner's install is modded.** `end_m01aa.mod` replaces the module's `.rim` pair and its `k_pend_start` removes Trask after the opening; `--original` ignores it. Tests that open the module without `set_prefer_rim` ran against that `.mod`, so their Endar Spire results describe the modded module.
- Not built: the map, party and journal screens, the options pages, the other swoop races, area-of-effect objects, damage types and resistances,
  grenades, dual wielding, speed/haste, stealth, lock picking and traps, vendors, the galaxy map, and cutscenes that use models not in the opening.

## What the owner could check by playing the real game side by side
1. **Kill XP**: the XP a first Sith soldier gives (we say challenge rating x 25; the real number decides that rule).
2. **Level 1 numbers**: a new Soldier with the class's recommended scores: HP (we get 12), defense (12), attack bonus (+1), saves (+4/+2/+1), skill points (4 at Intelligence 10, the video's 8 at 12), feats.
3. **Attribute buying**: the point cost of raising a score to 15, 16, 17, 18 (we use 2, 2, 3, 3 per step).
4. **Run speed**: seconds from the start position to the first door (we run 5.4 m/s; door about 5.7 m away).
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
10. **The rest of the game.** The autopilot below plays into Taris with an unkillable hero; nobody has judged those modules by eye. Vendors, minigames, the galaxy map and stealth are not built. `docs/GAPS.md` ranks what is missing.

## How far the autopilot gets
CI runs `scripts/check.ps1` on a self-hosted runner with no game files (`.github/workflows/ci.yml`). All 357 tests pass there, but those that need the install return at once, so CI proves the build, the lints and the tests with no install, not the game.

`cargo run --release -p kotor-game --example autopilot -- end_m01aa 80000 280` plays with no window. `--autopilot` does the same in the viewer. Speeds and the proof that they play the same game are in `docs/FASTTEST.md`. The pilot uses only what a player has: clicks, replies, taking items, attacks, changing leader, swapping a weapon. It cannot die (`Autopilot::immortal`), so none of this says the fights can be won.

Last run: 15,706 game seconds in 49 wall seconds, 145 kills, experience 10,563, level 5, 5,323 door tries, 87 items, 7,699 lines. It ends when no new module has come for 8,000 s. Script errors: 2. The 633 `locked` stalls are doors it tried and gave up on.

| Module | First entered, game s | Visits |
|---|---|---|
| `end_m01aa`, Endar Spire | 0 | 1 |
| `end_m01ab`, its second half | 259 | 1 |
| `STUNT_00`, cutscene | 441 | 1 |
| `tar_m02af`, upper city start | 441 | 1 |
| `tar_m02aa`, upper city | 468 | 1 |
| `tar_m02ac` | 613 | 2 |
| `tar_m02ae`, cantina and duel ring | 1,030 | 1 |
| `tar_m02ab` | 1,604 | 5 |
| `tar_m03aa`, lower city | 1,843 | 27 |
| `tar_m03ab` | 2,110 | 1 |
| `tar_m03ae` | 2,655 | 1 |
| `tar_m11aa`, hidden Bek base | 2,936 | 21 |
| `tar_m04aa`, undercity | 3,422 | 2 |
| `tar_m03ad` | 3,898 | 1 |
| `tar_m09aa`, Sith base | 4,453 | 3 |
| `tar_m02ad` | 5,152 | 1 |
| `tar_m09ab`, Sith base, governor | 5,684 | 1 |
| `tar_m10aa`, Black Vulkar base | 6,078 | 3 |
| `tar_m05ab`, sewers | 6,714 | 2 |
| `tar_m10ac` | 7,201 | 1 |
| `tar_m05aa`, sewers | 7,706 | 1 |

That is 20 of the 97 named modules in the install, not counting `STUNT_` ones, and 18 of Taris's 23. Being in a module does not mean its story is done.

**Quests:** 17 of the 101 in `global.jrl` were started, and 4 completed: `end_attack`, `tar_diabounty`, `tar_infectedoutcasts`, `tar_rancor`. Still open with the state reached: `tar_largobounty` 66, `tar_matrik` 66, `tar_bastsearch` 35, `k_swg_carthtalk` 20, `tar_duelring` 10, `tar_rakghoulserum` 2, `k_pazaak` 1, `k_rapidtransit` 1, `tar_bendakbounty` 1, `tar_planetinfo` 1, `tar_rukilapprentice` 1, `tar_selvenbounty` 1, `tar_vulkarbase` 1.

**No module holds it now. What it cannot reach:** every way left is story, a talk or a door it cannot open.

| Not reached | What leads there | What stops the pilot |
|---|---|---|
| `tar_m03af`, then `tar_m03mg`, the swoop race | Gadon's `tar11_gadon112` runs `k_ptar_racecs0` once `Tar_SwoopEng` is set, by `k_ptar_takeaccel` taking the item `tar10_accelerator`. `k_ptar_kandrace` is Kandon's way to the same race. | The accelerator is in `tar_m10ac` behind `tar10_dockdoor`, plot, DC 100. A droid opens it with `k_ptar_junkdro1`, which needs repair parts and Repair. `tar10_accompdlg` opens it with `k_ptar_unlockdrs`, which needs spikes and Computer Use. The pilot held 3 spikes and no parts, and did not get it open. Past that is the race, item 32. |
| `tar_m08aa`, Davik's estate | `k_ptar_davikest`, `k_ptar_freeda_ud`, `k_ptar_godav_en` | Comes after the race, as far as I read. Not reached. |
| `ebo_m12aa`, the way off Taris | `k_ptar_goebhawk` in `tar_m02aa` | Reached by the immortal pilot at 24,745 s, after the turret. |
| `tar_m11ab` | door `ptar_secdoor` in `tar_m11aa`, past `tar11_secdoor`, DC 100 | `k_ptar_lockdoor2` opens that door only if `Tar_KandonMission` is set. Otherwise the Bek base turns hostile. The pilot gets that mission by taking Kandon's job in `tar_m10ac`. |
| `tar_m10ab` | I found no link or script naming it in the Taris modules. | I do not know. |

The pilot takes the way out nearest to a module it has not seen, then the one it has used least, so it finds things by itself. It takes along whoever is on offer. I do not know that this is the intended order of the story.

Read, and not faults:
- **Long fights.** `tar05_stampy` has 90 hit points and defense 26, and a hero at +7 hits only on 19 or 20. `tar10_thermturret` went 37 hits in 72 blows over three turrets. The other long fights came from two faults, fixed below. Fights that used to stand idle were scripts ending the attack with `CancelCombat`. The pilot now attacks again, as a person would.
- **`tar_m09aa`'s doors.** `ptar_elevdoor` wants the card `ptar_sbpasscrd`, or the terminals' `k_ptar_openelev`. The pilot now gets through, at 4,522 s. I did not trace which of the two it used. `ptar_armdoor` is DC 24 with no key and stays shut, so the armoury's footlockers go untouched.
- **`tar04_sithpatrol`.** Its talk now runs to its end, 11 lines, with no pause running out.

Fixed on the way, earliest first, each with a test:
1. **A closed door blocked only a 1.1 m disc.** The pilot walked round it, past locked doors. It is now a line as wide as its model's mesh, which nothing crosses and which sight stops at. Faces would not do: a door's faces can reach metres to either side.
2. **The second module's real route.** With no way round, the pilot needs the footlocker's spikes for the security panel. It wrote off anything out of reach for good, and now keeps it until a door opens.
3. **A door could not be reached when a neighbouring door shares its face.** `tar_m02af` has two. A route may now end on a blocked face.
4. **The player stuck on people**, at (104, 108) in the cantina and elsewhere. Someone stood on a route corner, or beside a wall, or the player was wedged between two. It now walks through in those cases, and stops when someone stands where the route ends, except in a fight (17).
5. **Global names ignored case nowhere.** `k_ptar_dueler_ud` sets `TAR_DUELOVER` and the announcer reads `Tar_DuelOver`, so a duel never ended.
6. **`SurrenderToEnemies` did nothing.** It now ends the fight on both sides and makes the fighter neutral. The second half is an assumption.
7. **`tar03_cutdlg`: the pause that ran out.** A feat used as a talent did nothing, and `EffectAssuredHit` was ignored, so the Bek whose death resumes the talk never died.
8. **Every module ran the first copy of a script it met.** Scripts were remembered by name across modules, and modules have their own copies. So the last room's troopers in `end_m01ab` ran `end_m01aa`'s `k_pend_sith_9`, which turned their AI off, and a script missing from one module stayed missing in the next. This is the biggest fix here: every module after the first was running some wrong scripts.
9. **Modules forgot.** A module now stays as the player left it, through a save and load too. The dead stay dead, doors stay as they were, containers stay emptied.
10. **Going round people.** When that gets the player no nearer for half a second it walks through them. When someone stands where its route ends it stops there. A click on the floor, and the pilot's next goal, end a fight.
11. **A walk went on during a conversation.** The player reached the panel in `end_m01ab` during Carth's talk, and the panel could not start its own. A talk now holds the walk.
12. **`GetAbilityScore` read its arguments the wrong way round,** which stopped the dance in `tar_m03ae`.
13. **Things that are not `Useable`** were offered as targets, such as a cutscene prop beside the first Sith. The menus builder found this one.
14. **Plot creatures could be killed**, such as `tar03_turret`. They now take no harm, and the pilot leaves them alone. **That no harm is an assumption.**
15. **A foe with a conversation was never fought.** The pilot marked it done when it looked for people to talk to. `SithPatrol091` stood hitting it in `tar_m09aa`.
16. **Each terminal's replies now count apart,** since the Sith base's terminals say the same things and open different doors.
17. **A chase stopped short at a bystander.** A beaten duellist stood where the route ended, and Gerlon and Ice shot the player for 90 s each.
18. **`GetFirstObjectInShape` walks could not nest.** `k_ai_master` began its outer walk again for good, and ran to the step limit 54 times a run. Each size and mask now walks on its own. **That is an assumption.**
19. **A ladder beside a door kept the pilot out of a module.** It used `tar05_lad04aai` first, each visit. What took it away now waits until the module is done.
20. **Talks did not move quests.** Their state is in `QuestEntry`, and the code read `PlotIndex`, which is nearly always -1. Only scripts moved quests before this.
21. **The pilot took a reply a real player dies of.** `k_ptar_killpc` sets the plot guard at `tar03_underdoor`, who hits at +21, on the player. It now avoids a reply that can turn someone who cannot be hurt. **That this rule covers all such replies is an assumption.** It takes one only when nothing else is on offer.
22. **`ActionUnlockObject` and `ActionLockObject` did nothing.** So the guard could not let anyone by, and the undercity's inside was out of reach. **That they always work, with no skill check, is an assumption.**
45. **Everyone on the Character, Level-up and Party screens stands inside the figure's hole.** The camera took every body as 1.7 m high, so a man's head went past the top of the hole, as in the owner's frame. Now each body is measured once through its idle, then sized and centred to leave 5% of the hole on each side. Tested with 13 bodies from T3-M4 to Zaalbar at 720p to 2160p and at 4:3. **Assumed:** a body shorter than 1.75 m stands at that scale and is not enlarged to fill the hole.
44. **One frame no longer pays for all the pictures that come into view.** In the viewer an upscaled picture is read on a thread with the stock one standing in, a frame reads at most three stock pictures and poses one new body, and a picture that has to wait is grey for a frame or two. Side by side for 240 s at 4K, on a busy machine: the worst frame's texture cost went from 164.9 ms to 32.9 ms. Frames over 100 ms hardly moved, 261 to 251, because most follow a costly scene build, 159 of the 251. Autosave at a fight's start, `save_to` alone, is 5 to 14 ms in a temp folder and is not the cause. A hero's painted body and head are still read whole, since the wrong paint must not show.
43. **The floor at a door between two modules no longer fights itself.** The owner's jagged patches in `tar_m02ac`: with Seamless areas on, the Apartments were drawn beside Upper City South, and each has a pocket behind its door where the other stands. Sampled every 25 cm, 72 square metres of the two floors lay at a height difference of 0.0 m, and each one's walls stood in the other's rooms. A door with a pocket on either side now shows its neighbour through the doorway only, as a portal does (`Attached::seen_through_doorway`), and the walking is as it was. Driven through `scripts/drive_seamless.ps1`'s driver at the Apartments and at the Sith base door, both ways, and at a Korriban trigger, which stays stitched: all checks pass. **Now seen:** behind the Apartments door, once it is open, are the module's own green blocks and the sky, which the original's loading screen never showed.
42. **The HUD no longer stutters the game at 3840x2160.** Anything that changed on it drew all of it again, copied the 33 MB picture and sent it to the card, several times a second in a fight. `Ui::hud_part` now compares each control with how it was last drawn and draws again only the boxes that changed, which the test `crates/kotor-ui/tests/hud_part.rs` checks pixel for pixel against the whole at 720p and 2160p. The small map no longer fills the picture cache at each step, a newly seen cell changes only its own pixels, and a conversation's box no longer costs a copy. Measured with `--frame-log`, the old and new builds running at the same time through 240 s of the scripted player, so that other work on the machine hit both: frames over 33 ms 1,897 to 459, and in fights 577 to 21; 99th percentile in fights 75.9 ms to 32.1 ms; game update total 74.1 s to 28.1 s. **Not fixed:** frames over 100 ms hardly moved (184 to 172), a run of very fast conversation lines stays costly, and a frame with new textures still takes up to 170 ms.
41. **The pilot passes Zhar's test of the Jedi Code, and leaves a module when it is done.** A menu of three or more replies of which one alone leads on is taken as a test: a wrong line of the Code gets a rebuke and the end of the talk. Its walk into scripted triggers took transition triggers too, so it left modules early and went back and forth. Immortal pilot: `DAN_JEDI_PLOT` reaches 3, `dan_ruins` and `dan_raiders` complete, `danm15` at 30,054 s, softlock at 41,831 s with `dan_trials` at 35. Juhani is next: `DAN_JUHANI_PLOT` stays at 1. Mortal pilot: falls in `tar_m11aa` at 6,507 s, 128 fights won and 5 not.
40. **The immortal pilot stood frozen after a wipe, and did not walk the story's triggers.** Since the death sequence, `game_over` holds the world still, and the pilot got up without clearing it. Its fall in `tar_m09aa` at about 3,700 s ended the run at 5,281 s. This touched the pilot only. It also now walks a module's scripted triggers while exploring, as the Jedi Enclave's scenes need. Immortal pilot: `dan_trials` reaches state 35, with `danm14aa` to `danm14ae` and `danm16` visited by 27,429 s. It then goes up and down the Ebon Hawk's ramp until the 8,000 s limit, at 38,737 s. Mortal pilot: falls in `tar_m05ab` at 6,971 s, 136 fights won and 3 not.
39. **A mesh with no alpha is no longer drawn.** Bug 42: the Arena Monitors in the cantina have a plate over the screen, in the same plane, at alpha 0. Drawn anyway, it fought the screen for the pixels and cut it along the diagonal. Controller 132 now counts, from the model file, or from an animation that keys it. 262 meshes in 90 models rest at 0, among them the star map's, and the swoop bike's thrusters, which its gear animations bring up. Alpha between 0 and 1 is still drawn solid.
38. **The pilot reaches Dantooine.** Three things a pilot has to do on the way. It fires the turret in `m12ab` at the nearest fighter. It chooses a planet on the galaxy map, each once. And it walks into a module's scripted triggers where there is no door out, which is how the Ebon Hawk's ramp works: the ramp's middle lies past the floor, so it aims inside the outline. Immortal pilot, with item 41's changes: `ebo_m12aa` at 24,745 s, `danm13` at 24,837 s, `danm14aa` at 26,023 s.
37. **The pilot leaves Taris.** Three things stood in the way, two a player would meet. `GetNearestCreature` counted the dead, so remains two metres off kept Davik's cage release shut, and with it the hangar. `GetObjectByTag` and `GetIsObjectValid` found bodies for good, where the game fades them, and the last reply on the Ebon Hawk's ramp waits for Calo Nord to be gone: a body now lasts 8 s for scripts, which is assumed, and its remains can still be looted. And the pilot gave up too soon: it now keeps exploring where there is no way out, and goes round a module up to three times. Immortal pilot: `tar_m08aa` at 20,584 s, then `STUNT_06`, `STUNT_07` and the Ebon Hawk's turret in `m12ab` at 21,229 s, where it does not fire. Mortal pilot: not past Taris.
36. **Module events run, and the story reaches Davik's estate.** A module's `Mod_OnModLoad`, `Mod_OnClientEntr`, `Mod_OnHeartbeat` and `Mod_OnAcquirItem` now run. Nothing but the turret ran any of them. 22 modules have one: the cantina's sets the scene with Mission and the Rodians, and picking up the Sith Governor's shield codes writes them into the journal. Still not run: `Mod_OnActvtItem`, `Mod_OnClientLeav`, `Mod_OnPlrDeath`. The immortal pilot reaches `tar_m08aa` at 18,605 s, and stops there at 19,066 s behind locked doors.
35. **The pilot follows the story past the race.** It wins the heats, Bastila is freed and joins, and `tar_escape` opens. It now goes back to where it has been when the journal has news, and it loots remains, as the Sith Governor's hold the shield codes Canderous asks for. On this tree the immortal pilot buys T3-M4 and stops in `tar_m02ad` at 14,953 s, unable to walk to the way out, cause not found. It does not reach `tar_m08aa`. The mortal pilot walks into the Sith base early and the party falls at 5,440 s, so its route still decides.
34. **The camera stays inside the room.** Tilted up it rose through the ceiling and showed the room from outside, as the walkmesh has no ceiling. It now stops at the rooms' own surfaces, 0.2 m short, in `gpu/src/roomshell.rs`. What stands in a room does not stop it.
33. **Far-off sounds fade, and the stutter's main causes went.** The owner's loud "lightsaber strike" was a duel 43 m from where a new game starts, at full strength. A sound from a place now fades from 8 m to nothing at 32 m, which is assumed. The HUD is a window-sized picture redrawn on one frame in three in a fight, and each redraw scaled every panel again. Scaled panels are kept now, so `Stage::update` in fights falls from 21.3 to 4.6 ms at 3840x2160 and from 5.3 to 0.85 ms at 1280x720. The overlay stays one GPU texture and only the part that changed goes up. Creatures are drawn between the game's 60 steps a second. Not measured: frame times in a real window on the owner's screen.
32. **The swoop race runs and can be played.** `crates/kotor-game/src/swoop.rs` reads `MiniGame` whole, for all four in the install, and holds a bike on its track with the layout's pads and buoys. The area's own scripts run the race through the `SWMG_` routines: the lights, gears, clock and finish, then back to `tar_m03af` with the time in the globals. Space shifts, A and D steer. On screen it is the camera's model with its clock and meter, the bike and the rooms, in `gpu/src/racescreen.rs`. Measured against the reference video's race: the meter over all five gears and the distance agree. **Not done:** the race's sounds and speed blur, and the floor and bike are brighter than the video's. Manaan's and Tatooine's are not run. The immortal pilot reaches it at 13,733 s, runs three heats and loses the last by 0.11 s, 29.39 against 29.28, then stops in `tar_m02ab` at 21,748 s with no new module. Whether a win is what the story wants next I have not read.
31. **Nobody is stranded just off the floor.** A route that hugs a corner can leave someone a step off the walkmesh, and routes started from the nearest face centre within 1.5 m. Faces are large, so beside a table in the Hidden Bek base Carth had none near, and could go nowhere. The player, knocked out, stayed down. A walker now starts from the nearest face edge. Nobody is sent to stand in a gap in the floor to speak to someone either, as around the cantina's stage. The pilot now stops heading for a module after standing three times beside a way there it never got through: a gate held it between `tar_m03aa` and `tar_m04aa`, with Kandon waiting in `tar_m10aa`. Immortal pilot: `tar_m03mg` at 13,749 s, 23 modules. Mortal pilot: the whole party falls in `tar_m05ab` at 9,429 s, 21 modules, 144 fights won and 5 not. Without that rule it reached `tar_m03mg` at 9,822 s, so its route decides, not its strength.
30. **The pilot reaches the swoop race.** Three bugs stood in the way, for a player too. A thing with no floor under it could not be used, and the Garage Head's desk holds the key to the Vulkars' lab, where Kandon and the Swoop Accelerator are. A module entered with no waypoint left the player at the last module's place. And what the player was about to do followed into the next module, so Kandon's talk ended in a jump off the track. Both go the Vulkars' way when they get there, killing Gadon for Kandon, as the pilot's replies fall; Gadon's way is not tried. See item 31 for the numbers. The race itself is `docs/MINIGAMES.md`, not built, and the pilot stops there.
29. **Someone behind a counter can be spoken to.** Janice Nall stands off the walkmesh, so a click on her did nothing, for a player too. The player now walks to the nearest place within talking range. Her shop sells spikes, parts and T3-M4.
28. **The conventional style's skin is seamless.** Where cloth went, the fill meets the skin round it with no step, across the sheet's seams too, which are found by place on the body. It has grain from clean skin and a little shade from the body's shape, and a belly under cloth borrows the one bare belly's muscles. The bra's edge is under 2 texels wide. The old top's hems no longer show in the shading. A head's neck fades to the skin of its body, and reaches down inside it, as a conversation's pose left a gap a collar used to hide. Men's bodies have a collar at the neck, so men's heads stay as they are. `tests/underwear.rs` measures all of it in every skin family. In memory only. **Not done:** the chest stays darker than the shoulders under the engine's fixed light, and the low-poly outline is the model's.
27. **Early Force sensitivity,** the owner's change, off the story and on by default for new games: `docs/EARLYFORCE.md`. **`AddMultiClass` now works**, which Dantooine needs either way.
26. **Shops, encounters, grenades.** `OpenStore` now opens a store from its `.utm`, with prices from `Cost` and `AddCost` and its mark-ups, and the pilot sells spares and buys medpacks. Encounters in an area's `Encounter List` bring out `RecCreatures`, once. `use_item_on` throws a grenade. **How the game scales encounters, and a shop's price formula, are assumptions.** The store screen is not built yet.
25. **Droids, rakghouls and 24 other appearances stood in a T-pose.** 26 of the 125 appearances the 21 modules can field had no idle: 25 carry their clips in their own model and the code read only supermodels, and they name clips `cpause1` and `cwalk`, not `pause1` and `walk`. A body's own clips now count, and a person's clip names find a creature's. Which clip stands in for which is **an assumption**, read from the names.
24. **A skill counts for the leader.** The pilot now sends whoever is best: Security for a plain lock, Computer Use for a terminal. A door with a say of its own still answers to who it answers to. It also takes skills and feats when immortal.
23. **The pilot never took Carth along.** Nothing in a run with no party screen brought him. It now fills the party once a visit, leaving out those a script took aside: the duel ring empties the party and puts it back, and a second Carth made `k_ptar_duelring` loop to the step limit.

## A pilot that can die

`AUTOPILOT_MORTAL=1` turns off `Autopilot::immortal`. It then takes levels as Auto Level Up does, wears the best armour and melee weapon it has found, and gives whoever is worst off a medpack below half their hit points, at most one every 3 s. It uses the first feat on offer with each attack. A party that is down ends the run. A leader knocked out hands over to someone standing, and takes the lead back when up. Each fight is logged as `FIGHT won` or `FIGHT lost`.

Last run: **no wipe.** All 21 modules, the last by about 9,000 game seconds, level 5, 164 fights won and 10 not, 174 knock-outs of the main character, 26 medpacks bought and 19 grenades thrown. It stopped at 16,929 s because nothing new came for 8,000 s. Encounters now add fights too.

Results move with the route. Before shops, one run was wiped in `tar_m10aa` at 4,628 s: out of medpacks, nothing else healing, walking into `VulkarPool` at 1 hit point. Another of the same kind reached all 21. So read this as "survivable with a party and medpacks, not balanced".

**It lives on two things to doubt.** Everyone gets up with 1 hit point when a fight ends, which is an assumption, and the video gives no help: nobody was knocked out in the 562 frames sampled through Taris. Also, the pilot walks into the next fight at 1 hit point, so most of its losses are small ones.

How the video's hero heals is measured: medpacks, in a jump of one frame, and levels. See `docs/REFERENCE.md`. No rest or slow healing was seen, so none was built. It does not retreat, as nothing would heal it if it did.

Balance still cannot be judged. Only medpacks and a new level heal. The video shows no healing over 30 s out of combat, but it does not rule out a rest button. The player's feats are the class's recommended ones, not what the video's player chose.

What is left, by how early it matters:
1. **Item use and level-up by hand.** Nothing can use a medpac, so `Mod_OnActvtItem` (`k_pend_activate`) never fires. Experience levels the hero by itself, with no choices.
2. **`k_pend_screenchg`** waits for the inventory and other screens through `GetSubScreenID`, which answers a fixed value. I do not know what calls that script in the original.
3. **Talents ignore their category** (`GetCreatureTalentBest`, `GetCreatureTalentRandom`): a creature casts the first power it can pay for, helpful or not. A feat used as a talent is a plain attack.
4. **Grenades fly unseen.** Their rows in `spells.2da` name a projectile model, which is not drawn. No Force power has a projectile in that table.
5. **The pilot has no skills.** `recommended_character` spends no skill points, so it never passes a skill check.
6. **Blows can land on the dead.** The log shows `Black Vulkar attacks Remains` right after a death. It harms nothing.
7. **Some scripts are named and not there:** `talktest` too, in `tar_m02ac`.

8. **Neck in the owner's screenshot: not reproduced.** It shows the neck cut flat with a dark plate at the collar. `cargo run --release -p kotor-render --example head_fit` follows the neck's vertices through all 1,072 animation samples: the join stays under 6 cm, and at 2.2 to 2.7 cm in idle, walk, run and combat ready. `PFHC01` reads 8.6 cm because its ponytail is its lowest part. No pose or texture I tried gives that plate. Which build, pose and camera the owner had is not known.

Logged but no block: `end_door10_cut2`, `end_door19`, `end_door07`, `end_door08`, `tar02_labdoor`, `tar02_arenadoor` and `ptar_lockde` are shut as far as the pilot can tell and the way on lies elsewhere. `k_pend_carth11` is named by `end_trask01` in `end_m01aa` but only `end_m01ab` has the file. The talk carries on past it, which `the_pilot_plays_off_the_endar_spire` checks. `k_swg_gizka01` and `talktest` are named by conversations and I found neither file.

Routines answering a fixed value on these modules, all others being real: `GetAttemptedSpellTarget`, `GetCreatureTalentBest`, `GetCreatureTalentRandom`, `GetEffectType`, `GetIsDebilitated`, `GetIsEncounterCreature`, `GetIsLinkImmune`, `GetLastAttackAction`, `GetNPCAIStyle`, `GetPartyAIStyle`, `GetPlayerRestrictMode`, `GetSoloMode`, `GetUserActionsPending`, `ResistForce`.

## The container panel (owner's job, 7 Nov)

- `container.gui`'s "Get Items" says Take All. The hidden "Switch To" slot says Take Selected, greyed until a row is chosen.
- A click chooses one row, Control adds or removes one, and a bar marks what is chosen. A click no longer takes the item.
- A stack shows `< 1 >`, the skill panel's own arrows. It stops at 1 and at the stack. `World::take_some` splits a stack.
- **Assumed, not in the owner's words:** a stack of 1 has no stepper. An arrow chooses its row. Shift steps by ten. Take Selected keeps the panel up until it is empty, and nothing stays chosen.
- The label of remains or a container with nothing left is drawn at 33%, as soon as it is emptied: `Plate::opacity`. Tests: `the_label_of_remains_with_nothing_left_shows_at_a_third_of_its_strength`, `a_containers_label_fades_when_it_is_emptied_in_play`. Its words still say `[empty]`, unless Feedback hides it.
- Test: `a_container_takes_the_rows_chosen_with_control_in_the_numbers_their_steppers_say`. Live: `scripts/drive_container.ps1`, 13 checks at 720p and 2160p.

## The slots over an enemy (owner's job, 7 Nov)

- They scale with the distance from the player: 125% up to 4 m, 60% from 24 m, in steps of 5. **Assumed:** those numbers, by eye from frames.
- They stay on their enemy with the cursor off it, and a click on one acts on that enemy. They go on a click on nothing, a click on something else, the enemy's death, or the enemy out of the window or behind a wall.
- **Assumed, asked:** nothing takes them for just leaving. For 0.5 s after the cursor leaves the enemy, its slots and the gap between, another target cannot take them either. Past that, the cursor on another target does.
- Only an enemy's slots do this. A locked door's stay as they were.
- Tests in `gpu/tests/plates.rs`, three on staying and going, one on size. Live: `scripts/drive_slots.ps1`, 6 checks at 720p and 2160p.

## The cursor on a screen (bug 1)

A change of what is under the cursor drew the whole picture again, 20 ms at 2160p, and sent all 33 MB of it to the card. Now `Ui::menu_part` draws only the controls that differ, and only that box goes to the card. Anything else about the screen changing draws the whole, as before. Test: the part is the same, pixel for pixel, as the whole on the main menu, Inventory and Equipment.

## Credits and Pickup

All the credits in a container are one row with their sum. The capsule in the first action slot is now a Pickup button, with F and a radius in the Options (3 m): everything open in reach is one panel, and what is taken leaves its own container. Tests: `every_pile_of_credits_in_a_container_is_one_row_with_their_sum`, `pickup_opens_what_is_near_as_one_container_and_what_is_taken_leaves_its_own`. Frames from `pickup_shots`.

## Item names and the bench's pictures (B2, B5)

Sixteen item templates have a designer's tag for a name in the talk table, such as `end_onedam`, a Blaster Rifle. They now go by the name of their kind, in old saves too. `examples/raw_names.rs` lists them over all 5,334 templates. The rest of its list stays: names shown only to tests or cutscenes (`Run_soldier`, `Extra01`, `zonecontroller`), or real ones with a digit (`T3-M4`, `HK-47`). Every kind on the bench has its picture, greyed where there is nothing to upgrade. Tests: `an_item_the_talk_table_names_with_a_tag_goes_by_its_kind_through_a_save_too`, `every_kind_on_the_bench_has_its_picture_and_what_cannot_be_chosen_is_greyed`.

## Next, in order
Owner tries the window (`docs/PLAY.md`) and listens to the sound -> attack animations and the cutscene camera -> equipment on bodies -> owner test sheet.
