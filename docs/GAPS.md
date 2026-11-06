# What the engine lacks, against the real game

Ranked by what it stops. The evidence is what the pilot's runs and the owner's reference video show, on 6 Nov 2026. Counts are calls in one immortal run of 15,796 game seconds, which covers 18 of Taris's 23 modules. Run `cargo run --release -p kotor-game --example autopilot -- end_m01aa 80000 280` with `AUTOPILOT_EVENTS` set to see them.

Sizes are guesses, for one builder: **S** a day or so, **M** a few days, **L** a week or more. I have not built any of these, so treat them as rough. Nothing here was compared with the game's code.

## Stops the story

| # | Gap | Evidence | Size |
|---|---|---|---|
| 1 | **The rest of the races. The turret and Taris's race are built:** `drive_turret.ps1`, `drive_race.ps1`, `docs/MINIGAMES.md`. | Taris's race has no sound or speed blur yet. `manm26mg` and `tat_m17mg` are the same kind and have not been run. | S |
| 2 | **Galaxy map: built. Leaving a planet: not reached.** | The console on the Ebon Hawk opens the map, and Travel runs `k_sup_galaxymap`, live in `drive_galaxy.ps1`. No game has reached the ship yet: `tar_m02aa`'s `k_ptar_goebhawk` goes to `ebo_m12aa`, and the video leaves Taris at about 18,000 s. Half of the plain jumps go to the turret minigame in `m12ab`, which is row 1. | S left |
| 3 | **Store screen: built.** | `store.gui`, compared with the video at 50:10. The price formula is still unchecked. | Done |

## Stops fights being judged

| # | Gap | Evidence | Size |
|---|---|---|---|
| 4 | **Healing: measured, nothing missing.** | The video's hero heals by medpacks, in jumps of one frame, and by levels. `docs/REFERENCE.md` has the times. No rest or slow healing was seen. | Done |
| 5 | **How people stand up after a fight.** | `update_knockouts` gives 1 hit point, an assumption. Mortal runs live and die by it: the main character is knocked out 83 to 217 times a run. The video does not help: nobody was knocked out in 562 frames of Taris. | Unknown |
| 6 | **Encounters: built, not scaled.** | They now fire. `Difficulty` and `CR` are ignored, so every one brings its `RecCreatures`. `GetIsEncounterCreature` still answers a fixed value. | S |
| 7 | **Area effects: built.** | Enemies with `SetNPCAIStyle` 4 and grenades throw them, by the game's own AI, live in `drive_grenades.ps1`. `GetFirstInPersistentObject` finds who is in a gas room. A burst shows at its place. No stock script calls `EffectAreaOfEffect`, so those objects are not built. What poison and the like do is row 9. | Done |
| 8 | **Enemy and party AI.** | `GetCreatureTalentBest` answered a fixed value 8,178 times, `GetCreatureTalentRandom` 898, `GetNPCAIStyle` 14,554, `GetPartyAIStyle` 2,426, and `GetIsDebilitated` 2,426. So AI choices run on made-up answers. In the mortal run the player landed 253 blows, Mission 48, Carth 37. | M to L |
| 9 | **Effects that change nothing.** | `force.rs` keeps these but ignores them: damage types, resistances, immunities, regeneration, haste, speed, invisibility, shields, area effects. That makes many Force powers and items do less than they say. `ResistForce` answers a fixed value. | L |
| 10 | **Alignment: built.** | `AdjustAlignment` moves the side, the sheet follows, and power costs lean with it, in `crates/kotor-game/src/alignment.rs`. Live in `drive_alignment.ps1`. How far costs lean is an assumption: the talk table, 41887, gives only the direction. | Done |
| 11 | **Two weapons.** | The pilot leaves its left hand empty, as a second weapon is not built. Feats for it then mean nothing. | M |

## Stops side content

| # | Gap | Evidence | Size |
|---|---|---|---|
| 12 | **Pazaak: built, not reached.** | `PlayPazaak` starts it, with wager, side deck and sets, in `crates/kotor-game/src/pazaak.rs`. Live in `drive_pazaak.ps1`. The rules are from knowing the game: I found no game of it in the reference video. No run has reached a table in a game. | Done |
| 13 | **Workbench and upgrades: built.** | `upgradesel`, `upgradeitems` and `upgrade`, opened by `ShowUpgradeScreen`. Item properties now count for everyone, in `crates/kotor-game/src/gear.rs`: attack, damage, defense, abilities, skills, saves, keen, and who may wear what. Live in `drive_bench.ps1`. The pilot does not use it. On-hit effects and regeneration are not built, and damage types are row 9. | Done |
| 14 | **Traps and mines.** | `docs/STATUS.md` lists traps as missing, and a chosen Security action too. Demolitions then means nothing. | M |
| 15 | **Stealth.** | `AwardStealthXP`, 1 call, only records itself, and invisibility is not modelled. `docs/STATUS.md` lists stealth as not built. | M |
| 16 | **Scenes whose pause runs out.** | `tar02_zelka021`, `tar04_outwom043`, `tar02_sithintero`. Each ended by our time limit, so some script's walk or death never came. Not read yet. | S each |
| 17 | **Recorded with no effect: the menu side is built.** | `SetSoloMode` with the V key, `SetNPCSelectability`, `SetItemNonEquippable`, `SetMapPinEnabled` and `RevealMap` work, live in `drive_solo.ps1`. `SetPlayerRestrictMode` keeps its value for the scripts that ask, and I do not know what else it does. `SetPlotFlag` still only records itself. | S |

## Exists, so not a gap

Containers, the party, journal and map screens, levelling up, saving, conversations and doors all work in these runs. Nobody has judged how they look against the game outside the Endar Spire.

Vehicles: I found none beyond the swoop and turret minigames above. Crafting: the workbench above is all I found.

## How to split this

1. One builder can take 3, 4 and 5 together, as they decide if mortal runs mean anything.
2. Number 1 is its own job, and the largest.
3. Numbers 6, 7 and 8 touch combat and the world's loader, so they should not run at the same time as each other without a word between builders.
