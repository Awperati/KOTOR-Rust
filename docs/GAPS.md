# What the engine lacks, against the real game

Ranked by what it stops. The evidence is what the pilot's runs and the owner's reference video show, on 6 Nov 2026. Counts are calls in one immortal run of 15,796 game seconds, which covers 18 of Taris's 23 modules. Run `cargo run --release -p kotor-game --example autopilot -- end_m01aa 80000 280` with `AUTOPILOT_EVENTS` set to see them.

Sizes are guesses, for one builder: **S** a day or so, **M** a few days, **L** a week or more. I have not built any of these, so treat them as rough. Nothing here was compared with the game's code.

## Stops the story

| # | Gap | Evidence | Size |
|---|---|---|---|
| 1 | **Swoop race and turret minigames.** | The Taris story goes through `tar_m03mg`: `k_ptar_swooprun` leads there, and nothing else I found does. The video has the race at about 14,700 s. `nwscript.nss` has 97 `SWMG_` routines and our code answers none by name. `manm26mg` and `tat_m17mg` are more of the kind. | L |
| 2 | **Galaxy map and leaving a planet.** | `tar_m02aa`'s `k_ptar_goebhawk` goes to `ebo_m12aa`, and the video leaves Taris at about 18,000 s. `SetPlanetSelectable` and `SetPlanetAvailable` only record themselves. Not reached yet, so unproven beyond that. | M |
| 3 | **Store screen: built.** | `store.gui`, compared with the video at 50:10. The price formula is still unchecked. | Done |

## Stops fights being judged

| # | Gap | Evidence | Size |
|---|---|---|---|
| 4 | **Healing: measured, nothing missing.** | The video's hero heals by medpacks, in jumps of one frame, and by levels. `docs/REFERENCE.md` has the times. No rest or slow healing was seen. | Done |
| 5 | **How people stand up after a fight.** | `update_knockouts` gives 1 hit point, an assumption. Mortal runs live and die by it: the main character is knocked out 83 to 217 times a run. The video does not help: nobody was knocked out in 562 frames of Taris. | Unknown |
| 6 | **Encounters: built, not scaled.** | They now fire. `Difficulty` and `CR` are ignored, so every one brings its `RecCreatures`. `GetIsEncounterCreature` still answers a fixed value. | S |
| 7 | **Area effects.** | Grenades have the fight screen's right slot now. Enemies throw none, see 8. `docs/STATUS.md` lists area-of-effect objects as not built. | S, then M |
| 8 | **Enemy and party AI.** | `GetCreatureTalentBest` answered a fixed value 8,178 times, `GetCreatureTalentRandom` 898, `GetNPCAIStyle` 14,554, `GetPartyAIStyle` 2,426, and `GetIsDebilitated` 2,426. So AI choices run on made-up answers. In the mortal run the player landed 253 blows, Mission 48, Carth 37. | M to L |
| 9 | **Effects that change nothing.** | `force.rs` keeps these but ignores them: damage types, resistances, immunities, regeneration, haste, speed, invisibility, shields, area effects. That makes many Force powers and items do less than they say. `ResistForce` answers a fixed value. | L |
| 10 | **Alignment.** | `AdjustAlignment`, 9 calls, only records itself. Light and dark then change nothing, which matters for Force costs and story. | S |
| 11 | **Two weapons.** | The pilot leaves its left hand empty, as a second weapon is not built. Feats for it then mean nothing. | M |

## Stops side content

| # | Gap | Evidence | Size |
|---|---|---|---|
| 12 | **Pazaak.** | `PlayPazaak` only records itself, and `GetLastPazaakResult` answers nothing. `k_pazaak` is a quest the pilot starts. | M |
| 13 | **Workbench and upgrades: built.** | `upgradesel`, `upgradeitems` and `upgrade`, opened by `ShowUpgradeScreen`. The pilot does not use them. | Done |
| 14 | **Traps and mines.** | `docs/STATUS.md` lists traps as missing, and a chosen Security action too. Demolitions then means nothing. | M |
| 15 | **Stealth.** | `AwardStealthXP`, 1 call, only records itself, and invisibility is not modelled. `docs/STATUS.md` lists stealth as not built. | M |
| 16 | **Scenes whose pause runs out.** | `tar02_zelka021`, `tar04_outwom043`, `tar02_sithintero`. Each ended by our time limit, so some script's walk or death never came. Not read yet. | S each |
| 17 | **Recorded with no effect.** | `SetPlotFlag`, 6 calls, in the duel ring among others. `SetSoloMode`, `SetNPCSelectability`, `SetPlayerRestrictMode` and `SetItemNonEquippable`. `AdjustReputation`. Each may matter somewhere. | S each |

## Exists, so not a gap

Containers, the party, journal and map screens, levelling up, saving, conversations and doors all work in these runs. Nobody has judged how they look against the game outside the Endar Spire.

Vehicles: I found none beyond the swoop and turret minigames above. Crafting: the workbench above is all I found.

## How to split this

1. One builder can take 3, 4 and 5 together, as they decide if mortal runs mean anything.
2. Number 1 is its own job, and the largest.
3. Numbers 6, 7 and 8 touch combat and the world's loader, so they should not run at the same time as each other without a word between builders.
