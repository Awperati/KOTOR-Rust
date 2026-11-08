# One Taris, no sections (design, before code)

Owner: "Still noticeable loading between sections, need to find a way to seamlessly join them all as one collective Taris area."

## Where Taris stands (from `docs/link_drive.tsv`, 39 Taris links, hidden drive)

| State | Links | Frozen time |
|---|---|---|
| joined (floor + picture, 0 ms) | 21 | none |
| loaded ahead (not joined) | 9 | avg 733 ms, worst 2401 ms (loaded machine; alone about 100 ms) |
| cold after 8 s near the door | 9 | avg 614 ms, worst 1509 ms |

## Which links can be stitched (door/trigger pair gives offset + yaw, `seamless.rs`)

- **Joined already (21):** the whole Upper City ring, Apartments, Cantina, Sith base door, Dark Lower City doors, Bek's door.
- **Lifts (8):** `tar04_elevdoor` both ways, `tar09_elevdoor`/`ptar_elevdoor`, `tar10_elev02`, `tar10_elevator` x2, `tar05_elevator`. Floors differ by design. Keep a ride (`docs/LIFTS.md`), made seamless by loading the far end during the ride.
- **Shop doors that overlap the street within 6 m (5):** `tar_m03aa` to `m03ab`/`m03ad`/`m03ae`, and back. Cannot be stitched (shops lie on the street). Keep a short fade (0.3 s dip, door animation) with the shop already resident.
- **Scripted or cut-scene (rest):** `tar_m04aa` to `m05aa`, `m05ab` to `m05aa`, `ptar_secdoor` m10aa/m11aa to m03aa, `tar02_aptdoor`. Stitch where the survey places them; else the short fade.

## Design

1. **Resident by range, not one kept job.** Now: one job, one module kept, dropped 32 m from its exit. New: every link within 40 m of any party member has its neighbour resident (built, drawn, walkable). Far ones unload at 60 m. Cap 3 non-joined neighbours at once.
2. **Memory.** Measure bytes per resident module first (stage 1). Budget: free RAM stays above 4 GB (existing guard). Over budget: drop the farthest first, never the one in front of the player.
3. **Creatures and walkers in neighbours.** Stage 5: waypoint walks only (about 400 lines, `docs/SEAMLESS_CREATURES.md`), no scripts. Until then neighbours show idling bodies.
4. **Walkmesh crossing** for player and party is done (`seamless_cross`). Reuse it for the fade links: same crossing, plus the dip.
5. **Save/load.** Unchanged: a save records the module the player is in. Joined state is rebuilt near doors.
6. **Quest scripts and area names.** Unchanged: `transition` runs at the crossing, so `GetArea`, `OnEnter`, journal and globals see the module entered when the player crosses.
7. **Minimap** switches at the crossing (as now). Later: show the joined neighbour's map when the player is within 10 m of the door.

## Stages, one tip each, biggest stall first

1. **Measure + resident cache.** Memory per module logged by the link drive. Several loaded-ahead modules kept. Aim: no link above 300 ms. Covers the 9 ahead and most cold ones (worst 2401 ms, 1960 ms, 1509 ms).
2. **Lift tip 2** (full ride). Hides the lift loads (8 links).
3. **Shop doors:** resident shop + 0.3 s fade, no loading screen (5 links).
4. **Scripted links:** `tar_m04aa` to `m05aa`, `tar02_aptdoor` freeze, `ptar_secdoor` pair.
5. **Living neighbours:** waypoint walkers in resident neighbours.

## Assumptions

- Times in the table come from a loaded machine and overstate the "ahead" rows. Stage 1 re-measures them quiet.
- 40 m / 60 m / cap 3 are starting values, tuned by stage 1's measurements.
- Shop interiors accept a 0.3 s dip. Streets, plazas and the lifts must not show a loading screen.
