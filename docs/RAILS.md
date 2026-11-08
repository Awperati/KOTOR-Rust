# Rails: a push over the side

A Force push (or any shove) sends a creature along the floor. At the edge of the floor it used to stop, as against a wall. Now a
creature other than the player can go over an edge that is only a low barrier, and fall. Code: `begin_shove` and `land_after_fall`
in [force.rs](../crates/kotor-game/src/force.rs). Tests: [rail.rs](../crates/kotor-game/tests/rail.rs). The list of rails:
[rails.tsv](rails.tsv).

- **The player keeps the invisible wall** at every edge, as the owner asked for the time being.
- A **ledge over a floor below** (a drop of more than a metre onto another walkable floor) needs no row. The creature goes over, falls to
  that floor, takes 1d6 for each 3 m of the fall (10 dice at most, none under 2 m) and gets up there.
- An **edge over nothing** is a rail only if `rails.tsv` lists it: a module, a point and a radius in metres. A creature pushed over it falls
  out of sight and is killed. Any other edge over nothing stays a wall, whatever the room's model shows.
- Neither goes over a wall of the walkmesh, a closed door, or another creature.
- A fall never leaves a creature in the air: it ends on a floor of the walkmesh, or is dead.

## Why a list

The walkmesh has no object for a railing: the floor just ends. Most rooms have model walls that the walkmesh does not mark, so every
floor edge cannot be made a rail without sending creatures through visible walls. `cargo run --release -p kotor-game --example rail_scan
-- <module>` counts the open edges of a module. In Taris's modules every floor edge over nothing is at the same height as the rest of
the floor, so the walkway in the owner's frame cannot be told by its height. Give the module and a point on its barrier, and it is a row.

## Assumed

- The fall damage and the 2 m that hurts no one are mine: the game's files give no rule.
- A creature lost over the side is dead (its death script runs), and its body is taken 40 m down, out of sight.
- Other knockback (a shove from any power) uses the same code. A creature at a rail edge stands inside the area: the push never
  places it off the walkmesh.
