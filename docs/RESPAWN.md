# Respawning creatures

Options > Feedback has a row **Respawning creatures: On** (default; the Gameplay page has no free row). Everything else is in
[`docs/respawn.tsv`](respawn.tsv), which you can open in any text editor. The game reads it when it is built.

## What happens

1. An **enemy killed in a fight** gets a timer, `timer_seconds` (300).
2. When it is up, the creature comes back to **where it began** if all of these hold:
   - you are more than `range_metres` (60) from that spot, and
   - you cannot see the spot (`require_no_line_of_sight 1`; set 0 and the distance alone decides), and
   - no fight is on and no conversation is open, and
   - fewer than `per_minute` (3) came back in the last minute and fewer than `per_module` (40) in this visit.
   Otherwise it keeps waiting.
3. It is made again from the area's own file, as at your first visit: the same template, name, faction and scripts, full health, and its
   loot rolled again (the old body keeps what it held, so nothing is copied).
4. **It pays half the experience** (`xp_share 0.5`) when it dies again, so one spawn point cannot be farmed for levels.
5. Leaving the area clears the waiting list: the area is made fresh on the next visit as it always was.
6. A save keeps each waiting creature and what is left of its timer.

## What never comes back

By the rules in [respawn.rs](../crates/kotor-game/src/respawn.rs):

- a plot creature (it cannot be hurt anyway), a party member, anyone with a conversation of their own;
- anyone whose death script is the story's own (only the game's default scripts `k_def_*` and `nw_c2_default*` count as plain);
- anyone a script or a cutscene killed, since only a fight's blows set a timer;
- a creature that is not an enemy (a bystander, a guard) unless an `include` row names it;
- the scripted openings (`exclude end_`, `exclude lev_`), the named characters and bosses in the `exclude` rows, and any row you add.

A scripted player (the autopilot and the drivers' pilot) has respawning off, so its routes do not move.

## Where the code is

| What | Where |
|---|---|
| the table, the queue, the rules, `bring_back` | [respawn.rs](../crates/kotor-game/src/respawn.rs) |
| the timer is set | `hurt` in [combat.rs](../crates/kotor-game/src/combat.rs) |
| half the experience | `award_kill` in [rules.rs](../crates/kotor-game/src/rules.rs) |
| saved as `Respawns` | [save.rs](../crates/kotor-game/src/save.rs) |
| tests | [respawn.rs](../crates/kotor-game/tests/respawn.rs) |

`cargo run --release -p kotor-game --example respawn_survey -- tar_m04aa` lists the creatures of an area with what decides it.

## Assumed

The numbers (60 m, 300 s, 3 a minute, 40 a visit, half the experience) are the owner's request or a first guess; the list of names is
a first guess too, and it is the part to correct as you meet creatures that should not come back.
