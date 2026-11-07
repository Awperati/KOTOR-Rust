# Creatures that act before the crossing: plan and size

Today a joined neighbour's creatures are a still scene (`Scenery::figures`: template, spot, heading) drawn with an idle clip
(`Stage::neighbour_figures`). At the crossing `World::transition` loads the module for real, and every creature appears at
the spot its file gives it and starts its scripts.

## What the data says

Counted in 35 modules of the 59 links (creatures other than the party, after 30 frames):

- 3 to 41 creatures a module, 680 in all (counted).
- Only **74 of 680** (1 in 9) have a route of waypoints named after it (`WP_<tag>_01`, `_02`...).
- Most of the others move by scripts (`ActionRandomWalk`, conversations, `ActionMoveToObject` after a trigger) that only a
  running script machine can say.

## Version 1: walks and faces, no scripts, no fights (BUILT)

Built as planned, in `crates/kotor-game/src/seamless_walk.rs` (the walk is a pure function of the clock, so there is no state to hand over but the way each one faces, which `World.seamless.facing` keeps), `Figure::{tag, route}`, `Stage::walkers_tick` and `neighbour_figures`, and the hand-over in `World::seamless_cross`. Walk speed 1.4 m/s, 2 to 5 s at each waypoint, looks at the player within 6 m while standing. The route is `[its file spot, WP_<tag>_01, ...]` looped. Seen on screen: not yet (no joined neighbour in the 59 links has a walker in view from its door that I looked at); tested by `seamless_walk` unit tests and `a_walker_of_the_neighbour_stands_where_the_picture_showed_it_after_the_crossing`.

What the player sees from a doorway: people who have a route walk it, and people turn to look at the player when near.

1. `Figure` gets `tag` and `route` (the waypoints `WP_<tag>_NN` of the module, in order). The module's `scenery()` fills them.
2. A new `crates/kotor-game/src/seamless_walk.rs`: `Walker { route, leg, wait, at, heading }`, stepped with the frame time by
   the stage (no `World`, no scripts). Walks a straight line to the next waypoint at the creature's walk speed, waits 2 to
   4 s at each, and loops. Turns to face the player when within 6 m of the neighbour's door, returns to the route after.
3. The stage draws a walking figure with the walk clip, a standing one with the idle clip (`Stage::neighbour_figures`).
4. Hand-over: `Crossing` carries `(tag, at, heading)` of every walker. After `transition`, creatures with that tag are put
   there, so nobody jumps back to a spawn spot when the player steps through.
5. Tests: a walker follows its route and loops; it faces the player in range; the hand-over keeps the spot (one crossing,
   one tagged creature moved).

Size: about 400 lines (150 for the walker, 80 for the stage clip choice, 50 for the hand-over, 120 for tests). One tip.
Risk: a walker may pass through a wall (straight lines, no floor check; the route is the designer's, so mostly fine) and
the hand-over spot may be inside a door-blocked line; checked by putting the creature on the nearest floor.
Not covered: 8 in 9 creatures stay idle, talking, fighting and reacting to the player.

## Version 2: a second `World` for the neighbour (not recommended)

`World` has 152 fields. About 35 are the whole game, not one area: the script globals (`global_numbers`, `global_booleans`),
`journal`, `party`, `gold`, `tokens`, `available`, `bench`, the clock, difficulty. A neighbour world needs those shared
(or copied in and the changes merged out each frame) so that its scripts do not give XP, open a journal entry, start a
conversation or change a global before the player arrives.

1. Split `World` into game-wide and area parts, or add a `shadow` flag that `host.rs` (2,452 lines of script functions)
   checks in every function that touches the game-wide part: every one of them that touches it (not counted).
2. A player stand-in in the neighbour for perception, so creatures notice, talk and fight.
3. Load on a thread (the module load now happens in the crossing), step at a lower rate, drop it at detach.
4. Hand-over: at the crossing the shadow *becomes* the world; the party is moved into it, and the old world becomes the
   neighbour (the swap that `seamless_cross` already does for the floor).

Size: 3,000 to 5,000 lines changed, most in `host.rs` and `world.rs`, all 739 tests touched by the split. Cost every frame:
a second script machine (not measured)
and the module's memory twice. Risk: high. A script in the neighbour that reads or writes a global gives a different game
from the one with loading screens.

## Recommendation

Do version 1 as one tip when asked. Do not start version 2 unless version 1 shows people want fights and talk seen through a
door; the same effect for talk is cheaper as a bark line (a speech bubble) than as a second world.

## Crossing while the neighbour's creatures are hostile or talking

What happens now (read from the code, not run against a hostile case): up to the crossing every creature of the neighbour is
a figure. It has no faction, no perception, no scripts and no fight, so nothing it would do can happen before the crossing.
At the crossing `World::transition` loads the module for real: each creature appears at its file spot (a V1 walker where the
picture showed it) and its spawn script and heartbeat run from the first frame. A hostile one within sight of the party
begins to fight on that frame; a creature whose perception script starts a conversation starts it on that frame. The
player has just walked through a door and is a few metres from them. With a loading screen the player arrives at an
arrival waypoint instead; a join is only made when that waypoint lies within `MAX_SIDEWAYS` (3 m) to the side of the
doorway, so the two arrivals are near each other.

Nobody can shoot or talk to the player through the doorway before the crossing. That is the one visible difference
from the game: a hostile that stands in view looks idle until the door is crossed.

Options, smallest first:

1. **Leave it.** No change. The surprise at the crossing is the game's own arrival surprise, a few metres nearer.
2. **Hostiles look at the player** (about 80 lines and a test): a figure keeps its faction; hostile ones within range turn
   to face the player like V1 walkers do, and play the combat idle clip. Shows alertness only; no attack.
3. **A grace of one second** (about 20 lines): after a seamless crossing, perception and the fight stay off for 1 s so that
   the arrival is not a hit on the first frame. Not in the game; chosen only if the first frame proves harsh.
4. **The neighbour fights before the crossing** is version 2 (a second `World`), thousands of lines, not recommended.

Recommendation: 1 now. If the owner wants to see enemies react through a door, 2 (one tip). 3 only if a real crossing into
a hostile room shows a first-frame hit. Not tested against a hostile crossing: the 59 links have no hostile within 6 m of
a door that I checked, so this is read from the code, not seen.
