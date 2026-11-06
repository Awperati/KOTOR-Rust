# Seamless areas

A door that leads to another module used to load it and put the party at a waypoint. With **Seamless areas** on, the door opens, the next area is already standing behind it, and the party walks in. The old way stays: the setting is `seamless_areas` in the settings file (default on), and any link that cannot be joined safely still loads the old way. Off, nothing here runs.

## How it works

1. **Plan** (`crates/kotor-game/src/seamless.rs`, maths in `crates/kotor-walkmesh/src/seamless.rs`). For a link door in the loaded module, the neighbour's own door back to us is laid on ours: the neighbour is turned about the vertical axis and shifted until the two doorways coincide and the two ways through point at each other. This is one rigid move per link (`Rigid`: a yaw and a shift).
2. **Check both directions.** The neighbour's arrival waypoint must come out beyond our door, within 3 m of its centre, facing the way through to within 0.5 rad. Our own arrival waypoint (the one the neighbour's link names) must be on our side and face the way back. If either is off, or the neighbour has no door back, the link falls back.
3. **Check overlap.** Every walkable face of the neighbour, moved, is tested against ours (`first_overlap`: triangles in plan, on the same floor within 1.5 m, sinking into each other by more than 5 cm). Faces within 1.5 m of the doorway plane are the jamb and not tested. The faces that only a closed door's doorway reaches (its *stub*: a pocket behind the door, in the original game a dead end) are given up on both sides, because the other module stands where they were. Two neighbours of one module are also tested against each other: the first stays seamless, the later one falls back.
4. **Load in the background** (`gpu/src/seamless_stage.rs`). When the player is within 24 m of a link door, a thread opens its own copy of the install, loads the neighbour, plans, and loads its picture. The game keeps running. Nothing loads on the main thread except the stitching of the picture.
5. **Join.** The neighbour's walkmesh (moved) joins the world's `NavMesh`, with a strip of floor across the doorway and bridges so that paths, the party and followers cross it (`World::seamless_attach`). The picture is stitched: the neighbour's scene, moved, is added to this one's (`Scene::moved`, `Scene::append`).
6. **Door.** Using the door opens it (`use_door`) with the door's own animation and sound; nothing is requested. A closed door is still a wall.
7. **Cross.** When the player is 0.4 m past the doorway of an open door, the neighbour becomes the loaded module (`World::seamless_cross`, which uses `transition`, so scripts, journal, globals, `OnEnter`, saved module state work as they always did). The world's space becomes the neighbour's, so the player, the party and the camera are carried by the same move and nobody is anywhere new. The module left behind stays joined (floor and picture) and its door in the new module is set open.
8. **Let go.** When every party member is 30 m clear of that door, the old module is dropped from the nav and the picture.
9. **Back.** The module behind is an ordinary joined neighbour, so walking back out crosses the same way.

## Portals: when the two floors would overlap

Two areas often cannot stand side by side: the Cantina's floor would lie under the Upper City South plaza. When the whole floors overlap, the link is still taken as a **portal** (`Attached::portal`, `Fallback::Overlaps` only when the parts near the door overlap too):

- **Walkable near the door only.** Each side keeps just the faces within 20 m of the door (`PORTAL_REACH`; faces beyond are `far_ours` and `far_theirs`). A crowded street that still overlaps is tried again at 10 m and at 6 m (`PORTAL_REACHES`). The neighbour's floor on our side of the doorway, a pocket outside its own door, is given up like a stub. Both near parts are tested against the other module's whole floor, so after the crossing nobody stands on a face that overlaps.
- **Seen through the doorway only.** The neighbour is never stitched on to the picture. `GpuRenderer::set_portal` keeps its draws apart, and the shader cuts them to the view through the doorway: five planes, one for the door's plane and one through the eye and each side of the opening (`Aperture`, 3.6 m high). Our own picture has that view cut out, so the two never mix. After the crossing the roles swap on which side the camera is on, so the picture does not change at the swap.
- **Modern movement.** The body the movement builder keeps (momentum, a roll, a fall) is turned and shifted with everything else at the crossing (`Body::carry`), or it would be rebuilt standing still. A roll begun 1.2 m before a door goes on through the crossing.
- **Ghosts.** Doors and creatures of the module just entered that stand beyond the doorway (where the other module is drawn) are not blockers for the nav or the party (`World::beyond_the_portal`).
- **Weld grid.** The nav welds room seams where vertices round to one point of a 2 cm grid. A moved module has its vertices put on that grid before the move (`move_bwm`), and a quarter turn is shifted by whole cells (`join`), so seams stay closed at any turn. Without this followers were cut off by invisible breaks in the street.

## What stays as it was

- `OnEnter` of the new module runs at the crossing; the old module's state is saved as in a transition. Globals, journal, party and items carry over.
- Area music and effects switch at the crossing (`enter_module` runs without loading the picture again).
- Autosave runs after a crossing, and saves record the module the player is in and the position in that module's own space.
- Only the loaded module is simulated. The neighbour is floor and picture until the crossing, so its creatures do not move and enemies do not follow.

## Assumptions and limits

- **Triggers do not go seamless.** A trigger link has no door to open, so it loads the old way. The planner can place one, but nothing draws it.
- **The neighbour's creatures and door models are shown before the crossing, standing still.** They are drawn once, one a frame so that no frame pays for all (`Stage::scenery_step`), at the spot and heading they have in the neighbour's own module, in the armour they wear, with the default idle pose. They do not move, talk or react until the crossing, when the real ones take over. The neighbour's own door back to us is left out, because ours stands there.
- **Lights and fog** need nothing: this engine lights areas with baked lightmaps, which are part of the neighbour's picture, and has no area fog to carry.
- **Minimap and map notes** switch at the crossing, not before.
- **Uploading the stitched picture** costs a frame when a neighbour joins or leaves (worst update 30 to 100 ms measured, median 0.1 ms).
- **A portal shows one doorway at a time**, the first loaded.
- **Elevators** (`tar04_elevdoor`, `tar10_elev02`, `tar05_elevator`, `ptar_secdoor` of the Lower City and others) still load. Their door gives no clear outside: the module behind is a lift car whose floor is on both sides of the door line, or the arrival waypoint faces the other way (the two directions disagree by 180 degrees). A lift is also a change of level and of place by design: the old game hides it behind the loading screen on purpose.
- **Saves** do not record what is joined. A save made with a neighbour joined, before or after the crossing, loads as a plain module (`State::forget_neighbours`), and joining begins again by itself near a door.
- **A portal's neighbour is a still picture** until the crossing, with no creatures or lights, as for a joined one. Creatures of the module just entered that stand beyond the doorway are in the world and can be targeted, though they are not drawn.
- A stub is assumed to be a dead end. If a stub holds floor the player needs, that link would have to fall back; the survey does not check it.
- **One-way links** (the neighbour has no door back) fall back, so that the area left can stay in view.

## Taris

Run `cargo run --release -p kotor-game --example seamless_survey -- tar_m02aa tar_m02ab ...` to rebuild this table. Shift is the neighbour's move in the unit of metres, turn in degrees. The last column of a placed link is how far the two arrival waypoints miss, in metres and degrees.

| From | Door (tag) | At | To | Result |
|---|---|---|---|---|
| tar_m02ac (Upper City South) | Apartments | (90.0, 129.5) | tar_m02aa | **placed** shift (-22.5, 47.8), turn 0, miss 0.09 m |
| tar_m02ac | tar02_cantdoor | (90.0, 24.9) | tar_m02ae (Cantina) | **portal** shift (200.0, -116.8), turn 90, miss 0.18 m. Whole floors overlap at (95, 71), 46 m from the door |
| tar_m02ac | UpperNorth | (187.5, 117.7) | tar_m02ab | **portal** shift (-33.8, 56.8), turn 0, miss 0.21 m |
| tar_m02aa | ptar_stnddr | (112.5, 81.8) | tar_m02ac | **placed** shift (22.5, -47.8), turn 0, miss 0.09 m |
| tar_m02aa | tar02_aprtdoor | (90.8, 134.2) | tar_m02af | **placed** shift (0, 0), turn 0, miss 0.72 m |
| tar_m02af | tar02_aptdoor | (90.7, 134.3) | tar_m02aa | **placed** shift (0, 0), turn 0, miss 0.72 m |
| tar_m02ab (Upper City North) | tar02_sithdoor | (112.5, 227.6) | tar_m09aa (Sith base) | **placed** shift (111.8, 249.9), turn 0, miss 0.21 m |
| tar_m09aa | ptar_secdoor | (0.7, -22.3) | tar_m02ab | **placed** shift (-111.8, -249.9), turn 0, miss 0.21 m |
| tar_m02ab | tar02_door02ad | (221.2, 142.9) | tar_m02ad | **placed** shift (108.7, 61.1), turn 0, miss 0.20 m |
| tar_m02ad | ptar_stnddr | (112.5, 81.8) | tar_m02ab | **placed** shift (-108.7, -61.1), turn 0, miss 0.20 m |
| tar_m02ab | tar02_lowdoor | (130.1, 131.2) | tar_m03aa (Lower City) | falls back: overlaps at (135, 93) |
| tar_m02ab | UpperSouth | (221.2, 60.9) | tar_m02ac | **portal** shift (33.8, -56.8), turn 0, miss 0.21 m |
| tar_m02ae | ptar_stnddr | (141.7, 110.0) | tar_m02ac | **portal** shift (116.8, 200.0), turn -90, miss 0.18 m |
| tar_m03aa (Lower City) | tar04_elevdoor | (402.6, 134.8) | tar_m02ab | **portal** shift (533.8, 4.7, 8.2), turn 90 |
| tar_m03aa | tar03_blkdoor | (464.0, 150.1) | tar_m10aa | **portal** shift (407.0, 303.3, 6.0), turn -90, miss 0.72 m and 11 deg |
| tar_m03aa | tar03_bekdoor | (359.0, 197.6) | tar_m11aa | **portal** shift (325.0, 265.4, 8.3), turn -85 |
| tar_m03ae | TarisDoor2 | (141.7, 110.0) | tar_m03aa | **portal** shift (-222.8, -32.3, -8.2), turn -5 |
| tar_m03aa | tar04_stddoor (two), tar03_javydoor | (402.6, 165.0), (307.1, 208.7), (350.7, 173.6) | tar_m03ad, m03ab, m03ae | fall back: the shop's floor overlaps the street at (404, 162), (306, 212), (346, 174) even within 6 m of the door |
| tar_m03aa | tar04_elevdoor | (189.2, 221.2) | tar_m04aa | falls back: no clear side of the door |
| tar_m03ab, m03ad | shop doors | | tar_m03aa | fall back: overlaps near the door |
| tar_m04aa | tar04_elevdoor | (179.6, 166.3) | tar_m03aa | falls back: the two arrival waypoints disagree by 38 deg |
| tar_m04aa | TarisDoor3, trigger tar04_goto05aa10 | | tar_m05aa | fall back: no door back |
| tar_m05aa | tar05_forcefield | (281.8, 130.8) | tar_m05ab | **portal** shift (-27.3, -83.2, -2.0), turn -45 |
| tar_m05ab | swoop and elevator doors | | tar_m05aa, m10aa | fall back: no clear side |
| tar_m10aa | tar10_elev03 | (196.2, 144.3) | tar_m05ab | **portal** shift (-114.4, 433.3, -21.5), turn -90 |
| tar_m11aa | ptar_secdoor | (70.5, 97.5) | tar_m11ab | **portal** shift (47.3, 66.7, -2.2) |
| tar_m09aa | ptar_elevdoor | (36.7, -13.9) | tar_m09ab | falls back: the door names no arrival waypoint |
| tar_m10aa, m10ac, m11aa | elevators, `ptar_secdoor` | | tar_m10ac, m03aa | fall back: arrival points face opposite ways, or no clear side |
| end_m01aa (Endar Spire) | end_door10 | (47.4, 120.1) | end_m01ab | falls back: end_m01ab has no door back |

**Reading the table.** The doors of Taris that most people think of, Upper City to Apartments, to the Sith base, to the shop `tar_m02ad` and to the Cantina, join. The Apartments, the Sith base and `tar_m02ad` stand beside the street. The Cantina and the two Upper City street links (north and south) overlap it, so they are portals. Of the Lower City's doors the street doors that lead to a shop whose floor reaches the street itself (three of them) and the elevators keep the loading screen; the survey names the place. The Endar Spire's one link is one-way.

## Tests and the driver

- `kotor-walkmesh`: unit tests for the move (`place`, `join`, inverse, `beyond`) and for overlap (touching rooms, crossing rooms, other floors, a big face over a small one).
- `kotor-game` `seamless.rs`: planner on two synthetic rooms: placed side by side, overlap falls back, no door back falls back, a waypoint facing the wrong way falls back, two colliding neighbours leave the second on the old way, a missing waypoint falls back.
- `crates/kotor-game/tests/seamless.rs`, on the real game: the player walks from Upper City South through the open Apartments door and on; the module changes with no step larger than the 10 cm walked, the module behind is joined and then let go; and the Apartments fit while the Cantina is a portal; a party of four (the leader and three companions) goes through the Cantina doorway with nobody moving more than a metre in a frame and all four within 4.5 m of the leader afterwards. Skipped when the game is not installed.
- `gpu/tests/seamless_boot.rs` (the boot smoke test): a save made in Upper City South with a party of four is loaded from the main menu with a settings file in the old format (no `seamless_areas` line, which reads as on), the Cantina joins, the party walks through, no menu shows, no error is logged. It uses a folder of its own, never the player's settings or saves.
- A sprint and a roll through the Apartments door and through the Cantina portal: the roll is still under way at the crossing, the biggest step is 0.145 m a frame, and the body is on the floor afterwards (modern movement).
- A save made with a neighbour joined, before and after the crossing, loads into the same world with nothing joined, the player where they were and standing on the floor.
- A party of four through a Lower City door turned 85 degrees.
- `gpu/tests/menus.rs`: the Seamless areas row of the Feedback page flips, is saved, and turns joining off and on.
- The shader cut is checked by eye, in the frames below; no test reads pixels.
- `scripts/drive_seamless.ps1` (runs `gpu/examples/seamless_drive.rs`): no window, frames drawn offscreen, so it cannot take focus or capture a screen. Checks: the area loads in the background, the player walks up, using the door opens it without a module change, walking through changes module, **no step larger than 1.6 times a frame's run** across the crossing, no menu or loading screen on any frame, the party on the new floor, and back out of the same door. `-Door tar02_cantdoor` runs the Cantina portal (the script unlocks that door for the run: it starts locked). Writes frames of the door opening and the walk to `docs/shots/seamless` at 1280x720 and 3840x2160 (not committed: they hold the game's textures).

Measured on 7 Nov 2026 (Upper City South, Apartments): largest step 0.090 m a frame against a bound of 0.144 m, 0 jumps, both ways; update time median 0.08 ms, 99th percentile 3.6 ms, worst 34 ms at 720p and 38 ms at 2160p (the frame where a neighbour joins or leaves).

Cantina portal, same day: largest step 0.113 m against 0.144 m, 0 jumps, no menu frames, both ways, at 1280x720 and 3840x2160; update median 0.09 to 0.10 ms, 99th percentile 7.7 to 8.1 ms, worst 78 ms at 2160p and 100 ms at 720p.

## Not done

- The elevators, three Lower City shop doors and other links that overlap near the door or have no clear side still load the old way.
- The neighbour's creatures stand still before the crossing.
- Triggers.
- Every module outside Taris and the Endar Spire has not been surveyed.
