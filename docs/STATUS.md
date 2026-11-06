# Status of kotor-rs (6 Nov 2026)

A clean-room, std-only Rust reimplementation of KotOR 1's engine that reads the owner's installed game at run time. 13 crates, about 22,000 lines,
**344 tests, all passing** (`scripts/check.ps1`: fmt, clippy `-D warnings`, tests). Nothing is pushed; 33 local commits. Nothing writes to the install.
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
NPC turn rate; which side's door clip; creature radius 0.4 m and follow distances; combat
(one attack per round, reach 2 m / 12 m, critical confirmation roll, no line of sight); **kill XP = challenge rating x 25, paid in full to each standing party member**; Force points per level;
spell save DC; x4 skill points at level 1; knocked-out recovery to 1 HP; the 30-point attribute schedule (the three classes' recommended scores do cost exactly
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
- Not built: the map, party and journal screens, the options pages, minigames (Pazaak, swoop, turrets), area-of-effect objects, damage types and resistances,
  grenades, dual wielding, speed/haste, stealth, lock picking and traps, vendors, the galaxy map, and cutscenes that use models not in the opening.

## What the owner could check by playing the real game side by side
1. **Kill XP**: the XP a first Sith soldier gives (we say challenge rating x 25; the real number decides that rule).
2. **Level 1 numbers**: a new Soldier with the class's recommended scores: HP (we get 12), defense (12), attack bonus (+1), saves (+4/+2/+1), skill points (8), feats.
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
10. **The rest of the game.** The autopilot below plays into Taris with an unkillable hero; nobody has judged those modules by eye. Vendors, minigames, the galaxy map and stealth are not built.

## How far the autopilot gets
`cargo run --release -p kotor-game --example autopilot -- end_m01aa 80000 280` plays with no window. `--autopilot` does the same in the viewer. Speeds and the proof that they play the same game are in `docs/FASTTEST.md`. The pilot uses only what a player has: clicks, replies, taking items, attacks, changing leader, swapping a weapon. It cannot die (`Autopilot::immortal`), so none of this says the fights can be won.

Last run: 19,373 game seconds in 43 wall seconds, 168 kills, experience 12,335, level 5, 3,338 door tries, 106 items, 9,182 lines. It ends when no new module has come for 8,000 s. No script errors. The 447 `locked` stalls are doors it tried and gave up on.

| Module | First entered, game s | Visits |
|---|---|---|
| `end_m01aa`, Endar Spire | 0 | 1 |
| `end_m01ab`, its second half | 260 | 1 |
| `STUNT_00`, cutscene | 426 | 1 |
| `tar_m02af`, upper city start | 426 | 7 |
| `tar_m02aa`, upper city | 453 | 9 |
| `tar_m02ac` | 521 | 10 |
| `tar_m02ae`, cantina and duel ring | 767 | 6 |
| `tar_m02ab` | 1,042 | 10 |
| `tar_m03aa`, lower city | 1,249 | 9 |
| `tar_m03ab` | 1,431 | 2 |
| `tar_m03ae` | 1,822 | 2 |
| `tar_m11aa`, hidden Bek base | 2,109 | 1 |
| `tar_m03ad` | 2,508 | 2 |
| `tar_m09aa`, Sith base | 3,021 | 7 |
| `tar_m02ad` | 3,642 | 5 |
| `tar_m09ab`, Sith base, governor | 4,522 | 5 |
| `tar_m10aa`, Black Vulkar base | 8,390 | 12 |
| `tar_m05ab`, sewers | 9,045 | 8 |
| `tar_m05aa`, sewers | 9,417 | 12 |
| `tar_m04aa`, undercity, outside the gate | 9,433 | 8 |
| `tar_m10ac` | 11,372 | 7 |

That is 20 of the 97 named modules in the install, not counting `STUNT_` ones, and 18 of Taris's 23. Being in a module does not mean its story is done.

**Quests:** 15 of the 101 in `global.jrl` were started, and 2 completed: `end_attack` and `tar_rancor`. Still open with the state reached: `tar_duelring` 30, `tar_bastsearch` 30, `tar_bendakbounty` 10, `tar_largobounty` 10, `k_swg_carthtalk` 10, `tar_rakghoulserum` 2, and at 1 `tar_vulkarbase`, `k_rapidtransit`, `tar_diabounty`, `tar_matrik`, `tar_selvenbounty`, `k_pazaak`, `tar_planetinfo`.

**No module holds it now. What it cannot reach:** every way left is story, a talk or a door it cannot open.

| Not reached | What leads there | What stops the pilot |
|---|---|---|
| Inside the `tar_m04aa` gate, by the lift | `tar03_underdoor`, DC 100, opened by `k_ptar_unlockund` from `tar03_sithpat31` with the Sith papers | The pilot took the reply that runs `k_ptar_killpc` before it had the papers. That guard hits at +21 and is plot, so it can be neither talked to again nor killed. A real player would have died there. |
| Inside the gate, from outside | `tar04_maingate`, DC 100, opened in `tar04_outman045` and `tar04_outwom043` | Those people stand inside. |
| `tar_m11ab` | door `ptar_secdoor` in `tar_m11aa` | `tar11_secdoor`, DC 100, tried 25 times. I found no script in that module naming it. Not read further. |
| `tar_m03af`, then `tar_m03mg`, the swoop race | `k_ptar_kandrace`, `k_ptar_bekrace`, then `k_ptar_swooprun` | Talks it did not reach. Which replies, not read. The minigame is not built. |
| `tar_m08aa`, Davik's estate | `k_ptar_davikest`, `k_ptar_freeda_ud`, `k_ptar_godav_en` | Not reached. Not read further. |
| `tar_m10ab` | I found no link or script naming it. | I do not know. |
| `ebo_m12aa`, the way off Taris | `k_ptar_goebhawk` in `tar_m02aa` | Not reached. Not read further. |

The pilot takes the way out it has used least, so it finds things by itself. I do not know that this is the intended order of the story.

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

## A pilot that can die

`AUTOPILOT_MORTAL=1` turns off `Autopilot::immortal`. It then takes levels as Auto Level Up does, wears the best armour and melee weapon it has found, takes a medpack below half its hit points at most once a round, and a party that is down ends the run. A leader knocked out hands over to someone standing. Each fight is logged as `FIGHT won` or `FIGHT lost`.

Last run: 23 fights won, 1 lost, 1,262 game seconds. 13 in `end_m01aa`, 3 in `end_m01ab`, 4 in `tar_m02ac`, 1 in `tar_m02ae`, 3 in `tar_m03aa`. It looks round before it talks and takes `[`skill`]` replies first, so in `end_m01ab` it finds the spikes and overloads the panel, which kills the last room's troopers and opens the door. That is what the reference video's player does at about 781 s. **It dies in `tar_m03aa`** at 1,258 s, level 2 with 24 hit points, to `tar03_vulkcut2` in 3 s. The last blow was a critical for 24.

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

## Next, in order
Owner tries the window (`docs/PLAY.md`) and listens to the sound -> attack animations and the cutscene camera -> equipment on bodies -> owner test sheet.
