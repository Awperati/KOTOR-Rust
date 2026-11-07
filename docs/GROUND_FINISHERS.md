# Ground finishers and aiming at the floor

Options > Feedback has a row **Ground finishers: On** (default). Off, melee against someone on the floor rolls as it always has.
The blaster's aim down at the floor is not behind the switch.

## What happens

1. Someone **on the floor** is a creature knocked down by a Force push or a knockdown, a body, or one who is down.
2. **Blasters.** A pistol, two pistols, a rifle or a heavy weapon, in its ready and firing clips, pitches down at a target on the floor:
   the spine leans forward and the arms drop 35 degrees in all. The bolts leave the muzzle and land 0.3 m above the floor, on the body.
   Player and NPCs alike. The numbers are assumed (`AIM_DOWN_DEGREES`, `PRONE_AIM` in [stage.rs](../gpu/src/stage.rs)).
3. **Melee.** A blow with a blade, lightsaber, staff or fists on someone **knocked down** is an **automatic critical**: no attack roll, no
   confirmation. It takes the weapon's usual critical multiplier. A creature that is only down (a party member at 0) cannot be
   hurt at all and is not finished. Blasters are not finishers: their shots roll as ever.
4. **The move.** The attacker plays the clip named `finisher` if the owner has one in the custom animations folder
   (`*.anim.txt`, F8 reads them again). Until then the stock swing of the weapon plays. A clip named `finisher` is stretched to its own
   length. The clip is played by [game.rs](../crates/kotor-game/src/game.rs) `play_finisher`, started in [stage.rs](../gpu/src/stage.rs).
5. A scripted player (autopilot, pilot drivers) keeps the old rules, as it does for No misses.

## How it fits with the rest

- With No misses on, a finisher is a critical, not a glance. Everything else about No misses is as it was.
- The critical multiplier, the gear's extra dice on a critical, and the difficulty setting apply as for any critical.
- Stunned creatures are not on the floor: they stand, so they are not finished and not aimed down at.

## Tests and frames

[ground_finisher.rs](../crates/kotor-game/tests/ground_finisher.rs) covers the blade, the lightsaber, fists, the switch off, a
blaster and a standing foe. `a_blaster_aims_down_at_someone_on_the_floor` in [stage.rs](../gpu/tests/stage.rs) covers the pose.
`cargo run --release --example floor_shots` writes frames of a pistol, a rifle and a blade against someone on the floor to
`docs/shots/combat`.
