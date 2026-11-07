# No misses: what a miss does now

This is a rule of the game, with no switch. (The owner, 7 Nov 2026: "Don't make it a setting either but a hard rule of how the
in-game balancing feels".) A scripted player (the autopilot and the drivers' pilot) keeps the original rules, so its routes and
digests do not move.

## What happens

1. **The player's or a companion's would-be miss on an enemy is a glancing blow.** It lands for the least damage it can do:
   `1 + (flat + level / 4) / 2`, rounded down. `flat` is what a hit adds whatever the dice say: Strength in melee, Precise Shot at
   range, Weapon Specialization and the gear's bonus. The dice are not rolled and a glance is never a critical. It is a hit in every
   other way: the impact sound, the flinch, resistances and the float "Glancing 2". The same goes for anyone's would-be miss that is
   not against the player's side at all (an enemy on a neutral).
2. **An enemy's would-be miss on the player or a companion is a near miss, with no damage.** It stays `Outcome::Miss`. A blaster bolt
   passes 0.3 to 0.6 m from the body and flies on, with a ricochet sound (the game's bolt on metal, `cb_bu_metalmed`: the archives have no
   whizz). A blow plays the parry sound of its weapon (`cb_pa_unarm1`, `cb_pa_saber1`, `cb_pa_blade1`, `cb_pa_bldlrg1`) and the
   target plays its dodge clip, not a hit flinch. The 0.3 to 0.6 m and the sounds are assumed.
3. **Real hits and criticals are as they were.**
4. **An aimed shot** (for the movement system's `aim_hit`, [combat.rs](../crates/kotor-game/src/combat.rs) `resolve_aimed`) never
   misses: what would miss is a graze, `Outcome::Glance` of half the rolled damage, at least 1.

## Where the code is

| What | Where |
|---|---|
| `World::no_misses`, `Outcome::Glance`, `glance_damage`, `resolve_aimed` | [combat.rs](../crates/kotor-game/src/combat.rs) |
| near-miss bolts and sounds | [stage.rs](../gpu/src/stage.rs) (`NEAR_MISS_*`, `fire_on_frames`, `attack_effects`) |
| the float text | [fightscreen.rs](../gpu/src/fightscreen.rs) |
| tests | [no_misses.rs](../crates/kotor-game/tests/no_misses.rs) |

## Does it change the balance?

`cargo run --release -p kotor-game --example no_miss_balance -- 60` fights the level-1 starting character against Sith soldiers,
60 seeds each, with the original rules (off) and these (on). Measured on 7 Nov 2026.

| Fight | Wins off | Wins on | Seconds off | Seconds on |
|---|---|---|---|---|
| 1 soldier, level 1 | 83% | 88% | 13.0 | 11.3 |
| 2 soldiers, level 1 | 32% | 37% | 16.8 | 16.8 |
| 1 soldier, level 5 | 88% | 92% | 10.4 | 10.0 |
| 2 soldiers, level 5 | 58% | 60% | 17.4 | 17.1 |

Enemy misses cost the player nothing, so crowd fights are as they were. The player's own glances add a little: a few points of
win rate and a second or two off a fight. (The first version had enemies glance on the player too. Two soldiers then fell from
32% to 15% at level 1, and from 58% to 32% at level 5.)
