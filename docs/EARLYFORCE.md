# Early Force sensitivity

The owner's change, off the story on purpose: the main character feels the Force from the first room, not from Dantooine. It is a setting, "Early Force sensitivity: On / Off (original)", on by default in a new game. A game keeps what it started with, in its save, so a game begun Off stays Off, and a save from before this has it Off.

Only the main character changes. Party members and everyone else stay as they were. With it Off the rules are exactly today's.

All the numbers here are my proposal, and none comes from the game. Change them in `crates/kotor-game/src/earlyforce.rs`.

## The rules

| What | Rule | Why this number |
|---|---|---|
| Starting pool | 12 Force points, plus the Wisdom modifier if above 0 | Force Push costs 10 in `spells.2da`. So one push at the start, and a little over |
| A level | 2 more, plus the Wisdom modifier if above 0 | Half a Jedi Consular's level, whose `forcedie` of 8 pays 5. A Guardian's pays 3 |
| Getting it back | 1 point every 2 s outside a fight, none during one | The engine has no regeneration for anyone yet, so this is new, and for this rule only |
| First power | Force Push, row 23, with its own script `k_sp1_generic`, cost and effects | The owner's screenshot |
| New powers | One pick at every even level, from the normal list | Slower than any Jedi: `classpowergain.2da` gives them one or more each level |
| Which powers | `usertype` powers, at the level of the `guardian` column, with their prerequisites | The strictest of the three, and non-Jedi have no column of their own |
| Light and dark | A power of the other side costs double: `goodevil` E for alignment 50 and over, G for under 50. Force Push is neither | An assumption: the factor and the line are not measured |

## The slots

The attack window reads what the character has: `combat_feats` for the middle slot, so Power Attack if owned, and `combat_powers` for the right one, so Force Push. With this rule on, a power too dear still shows, with `usable` false, and the screen greys it.

## Becoming a Jedi

`AddMultiClass`, which Dantooine's scripts call, had no effect until now. It now makes the class the Jedi one.

- With the rule on, powers carry over and the pool stays what it is, or the Jedi class's first-level die if that is more. Nothing is paid twice.
- From then on the Jedi class's own progression applies: `forcedie` for points, `classpowergain.2da` for picks. Early picks and the early pool's growth stop.
- If that never comes, because the script is never reached, the early rules simply go on.

## For the screens

- `World::early_force`: the rule for this game. `Stage::start_module` sets it from `Settings.early_force` when the viewer enabled it, before `create_character`.
- `World::power_slots_for(id, level)`: picks at a level, for this creature. Use it where `power_slots_at(class, level)` is used for the player.
- `power_choices(id)`, `can_take_power(id, power)`, `recommended_power(id)` and `owned_powers(id)` already answer for the early rule.
- `CombatChoice.usable`: false to grey it.

## Not done

Chargen's Powers step and the Abilities screen are the menus builder's. Enemies do not react to the early powers, and no script knows of them, so a story line about the Force on Dantooine reads as it always did.
