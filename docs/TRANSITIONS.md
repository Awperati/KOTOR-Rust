# Every way the game changes module

Made with `gpu/examples/census.rs` (what can start a module) and `gpu/examples/link_drive.rs` (what each link costs the player). Both read the install, so nothing of the game's is in the repo but the counts and the names of its modules.

## The census

`cargo run --release --example census` reads every module's objects and compiled scripts and lists each call to `StartNewModule` with who runs it. A script that runs another script by name (`ExecuteScript`, or a party screen's exit script) is followed 3 deep. One row per way: `census.tsv` in the folder it is run from. `census -- script:<name>@<module>` says where one script goes; `DUMP=1` with it prints the script's instructions.

All 117 modules with a way out give 274 ways to another installed module (the module name is the text constant nearest before the call, as `quest_route` has it; a name built by code is not seen).

| How the module changes | Ways | Loaded ahead? |
|---|---|---|
| Door that links (`LinkedToModule`) | 116 | Yes, within 24 m (or joined with no load at all) |
| Trigger that links | 40 | Yes, as above |
| Conversation (an end or line script starts it) | 73 | 53 yes. The other 20 all start a cut scene (`stunt_*`), left alone on purpose |
| Door script (`OnOpen`, `OnFailToOpen`) | 10 | 9 yes, by the script probe. One no: `tar02_aptdoor` |
| Trigger script (`ScriptOnEnter`, `OnClick`) | 30 | 2 yes by the probe; 27 are the Ebon Hawk's ramp (yes, by the planet); one no: `unk41_goebon` |
| Placeable script (`OnUsed`: the Undercity ladders) | 2 | Yes, by the probe |
| Module script (`Mod_OnHeartbeat` of the Hawk minigame) | 3 | No: the minigame hands back to the ship |

The script probe is `World::script_destination`: it runs the script on a copy of the globals and records the first `StartNewModule`, changing nothing. The Ebon Hawk's ramp (`k_pebn_leavhawk` → party screen → `k_pebn_exithawk`) picks one of nine planets from the global `K_CURRENT_PLANET` (10 Taris, 15 Dantooine, 20 Kashyyyk, 25 Manaan, 30 Korriban, 35 Tatooine, 40 Leviathan, 45 Lehon, 50 Star Forge, 55 and up the Yavin station), so only the globals say where it goes.

## The galaxy map

Choosing a planet runs `k_sup_galaxymap`, which writes `K_FUTURE_PLANET` (row 3 Dantooine 15, row 4 Tatooine 35, row 5 Kashyyyk 20, row 6 Manaan 25, row 7 Korriban 30) and starts a cut scene (`STUNT_12` the first time, `STUNT_00` after). The cut scene's conversation ends in the ship (`ebo_m12aa`), and the ship's ramp opens the planet. Cold, measured with `load_times` on a busy machine: ship 1.9 s, Dantooine 5.8 s, Tatooine 1.9 s. Now:

1. The ship is loaded while the cut scene's conversation runs (the conversation rule).
2. The planet's first module is loaded from the moment Travel is pressed (`World::planet_first_module`: the travel script, then the ramp's script, on one copy of the globals), so it is there well before the player reaches the ramp.
3. Walking up to the ramp loads it by the probe, for a ship entered by any other way.

## What still freezes, and the plan

| Case | Why | Plan |
|---|---|---|
| `tar02_aptdoor` (apartment door, `k_ptar_extapt`) → `tar_m02aa` | Its script decides by the state of the apartment, so a probe on a copy of the globals alone does not reach `StartNewModule` | Run the probe with the door's and the player's own state; or list it by hand in `quest_routes.tsv`-style table. One door. |
| `unk41_goebon` trigger → `ebo_m41aa` | Same | Same. One trigger |
| Hawk minigame (`m12ab`) ends in the ship or a cut scene | A heartbeat script decides when | Load the ship when the minigame starts (one call, `ebo_m12aa`). Not done: the minigames have their own loading path |
| Cut scenes (`stunt_*`) | Left alone: short, behind a screen of their own, and many | None needed |

## What each link costs the player (`link_drive`)

`cargo run --release --example link_drive` opens every module with a door or trigger link, stands the player 10 m from the link, waits up to 8 s for the neighbour to be joined or loaded ahead, asks for the crossing, and times the frame that the player would see freeze (the world change, the stage build, the first texture loads). One row per link in `drive.tsv`: module, tag, kind, goes to, mode (`joined`: no load; `ahead`: loaded ahead; `cold`: not ready in 8 s), seconds waited, world ms, build ms, texture ms, total ms.

Run on current master with the load-ahead of this change (`docs/link_drive.tsv` holds every row), 153 links of 117 modules, machine otherwise idle:

| Mode | Links | Frozen ms (average / worst) |
|---|---|---|
| joined (no load at all) | 97 | 0 |
| ahead (loaded ahead in time) | 22 | 400 / 2401 |
| cold (not ready after 8 s of standing 10 m away) | 33 | 464 / 1509 |
| error | 1 | `danm15 man15_door02` stayed in its module: the door's script, not a link, decides |

Read the numbers with care:
- **"ahead" is an upper bound.** The two worst rows (`tar_m03aa tar03_javydoor`, 2401 ms, and `tar_m03ab ptar_stnddr`, 1960 ms) take 101 ms and 91 ms when timed alone with `load_times` (`PRELOAD=1`). In the drive the thread and the main thread compete for the same cores, and the first crossing after a long wait pays for the page-in of the new module. 32 of the 153 links froze over 250 ms, 49 over 100 ms.
- **"cold" means the load was not done in 8 s.** One job runs at a time, nearest exit first, so a link that is not the nearest waits for the one before it. A player who walks up gives it more than 8 s; the drive does not walk. Cold rows are the worst case, not what a walk shows.
- Worst cold: `tar_m09ab tar09_elevdoor` to `tar_m09aa` 1509 ms, `korr_m33aa k33_trg_ebonhawk` to the ship 983 ms, `tar_m10ab tar10_elevator` 839 ms, `danm13 dan13_door03` 762 ms, `end_m01aa end_door10` 754 ms (this one is loaded ahead when walked up to, see above: 95 ms).
- The first run on the same build, with cargo building at the same time, gave 26 cold and 32 ahead rows and 4146 ms for the Taris lift: the numbers move a lot with a busy machine.

Next step if the cold rows matter: let two jobs run at once (the nearest two exits), at the cost of a second module in memory.
