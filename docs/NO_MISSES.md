# No misses: what a miss does now

Options > Feedback has a row, **No misses: On** (default). It is in the list with "Extended story branches", because the
Gameplay page has no free row. Off, the game's own rules apply and attacks miss as before.

## What happens

1. An attack that would have missed, by the roll or by a natural 1, **lands as a glancing blow**.
2. A glancing blow does `1 + (flat + level / 4) / 2` damage, rounded down. `flat` is what a hit adds whatever the dice say:
   Strength in melee, Precise Shot in ranged, Weapon Specialization and the gear's own bonus. It is never under 1.
3. The dice are not rolled and a glance cannot be a critical. **Real hits and criticals are exactly as before.**
4. It is a hit in every other way. The target flinches instead of dodging, the weapon's impact sound plays, a bolt lands on the
   target, resistances and the difficulty setting apply, and scripts see a hit (result 1).
5. The float over the target reads "Glancing 2". The Messages tab lists it as a hit.
6. It is the same for the player, the party and every enemy, with every weapon: blasters, blades, lightsabers, fists.

## Where the code is

| What | Where |
|---|---|
| the switch, `World::no_misses` | [world.rs](../crates/kotor-game/src/world.rs) |
| `Outcome::Glance`, `glance_damage` | [combat.rs](../crates/kotor-game/src/combat.rs) |
| the options row and the saved `no_misses=` | [optionpages.rs](../gpu/src/optionpages.rs), [menus.rs](../gpu/src/menus.rs) |
| the float text | [fightscreen.rs](../gpu/src/fightscreen.rs) |
| tests | [no_misses.rs](../crates/kotor-game/tests/no_misses.rs) |

A scripted player (the autopilot and the drivers' pilot) keeps the original rules, as it does for extended story, so that its
routes and digests do not move. A scripted attack in a cutscene keeps its scripted result.

## Does it change the balance?

`cargo run --release -p kotor-game --example no_miss_balance -- 60` fights the level-1 starting character against Sith
soldiers, 60 seeds each, with and without the rule. Measured on 7 Nov 2026:

| Fight | Wins off | Wins on | Seconds off | Seconds on |
|---|---|---|---|---|
| 1 soldier, level 1 | 83% | 87% | 13.0 | 11.2 |
| 2 soldiers, level 1 | 32% | 15% | 16.8 | 13.4 |
| 1 soldier, level 5 | 88% | 90% | 10.4 | 10.1 |
| 2 soldiers, level 5 | 58% | 32% | 17.4 | 13.2 |

One on one it barely moves. Two on one gets harder, because the soldiers miss the player 76% of the time with the starting
armour, and each of those misses is now 1 damage against about 12 hit points. A glance cannot do less than 1, so a lower
formula would not remove this. To make it softer, raise the player's side of the formula or lower the difficulty setting.
