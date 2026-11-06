# Combat (kotor-game `combat.rs`)

Headless d20 combat run by the game's own scripts. `World::attack(attacker, target)` (what `ActionAttack` does) starts an attack; `step` then
runs heartbeats, perception, movement towards the target and 3-second rounds. `cargo run -p kotor-game --example fight` plays the Endar
Spire battle with the player (blaster pistol) against a Sith soldier and prints the events and the attack log.

## Taken from the install
| What | Where |
|---|---|
| Weapon dice, die size, threat range, crit multiplier, ranged or not | `baseitems.2da` (`numdice`, `dietoroll`, `critthreat`, `crithitmult`, `rangedweapon`) of the item in slot 4 (right hand) |
| Base attack bonus | `classes.2da` `attackbonustable` -> that table's `bab` at the creature's level |
| Armor | `baseitems.2da` `baseac` summed over equipped items |
| Strength to Charisma, natural AC, hit points, class, faction, perception | the creature's UTC (`Str`..`Cha`, `NaturalAC`, `MaxHitPoints`, `ClassList`, `FactionID`, `PerceptionRange` -> `ranges.2da` `primaryrange`) |
| Equipment | the UTC's `Equip_ItemList` (struct id is a bit mask, one bit per slot) |
| Who is an enemy | `repute.2da`, 10 or less |
| What the AI does | the area's own scripts: `ScriptHeartbeat`, `ScriptOnNotice`, `ScriptAttacked`, `ScriptDamaged`, `ScriptEndRound`, `ScriptDeath` (the `k_def_*` scripts calling `k_ai_master`) |

## The rules as implemented
- Attack roll = d20 + base attack bonus + Strength modifier (melee) or Dexterity modifier (ranged), against defense 10 + armor + natural AC +
  Dexterity modifier. A natural 1 misses, a natural 20 hits. Ability modifier = (score - 10) / 2 rounded down.
- A hit inside the weapon's threat range (`critthreat` 1 = 20, 2 = 19-20, ...) threatens a critical; a second attack roll must also hit to
  confirm it. A confirmed critical multiplies damage by `crithitmult`.
- Damage = weapon dice + Strength modifier (melee only), at least 1; unarmed 1d3. A creature dies at 0 hit points (`ScriptDeath` runs).
- Tested: the hit rule over every roll, bonus and defense in a range; threat range and confirmation; damage minimum and multiplier; and in
  the fight, every logged attack agrees with the rules, hit points fall by exactly the logged damage, nobody attacks after dying, there
  is at most one attack per attacker per round, the same seed gives the same fight, and no routine is stubbed.

## Assumed, not measured from the game
- A round is 3 s; one attack per creature per round, resolved at the end of the round, attackers in id order.
- Melee reach 2.0 m, ranged weapons fire from 12 m (`prefattackdist` in `baseitems.2da` has values like 23 that do not look like metres).
- The confirmation roll for criticals (copied from the d20 system, not checked in KotOR).
- Heartbeat every 6 s; perception ignores walls (no line-of-sight test) and hearing is the same as seeing.
- Attackers out of reach walk straight at the target along the navigation mesh (replanning once a second) and slide round other creatures.
- **Not done:** Force powers and talents (`ActionUseTalentOnObject` is only recorded, so the Jedi's power in the opening does nothing),
  grenades, damage types and resistances, saving throws, skills, feats such as Power Attack, dual wielding, multiple attacks per round,
  flurry and rapid shot, combat modes, experience for kills, `EffectDamage` and other effects from scripts (recorded only), the player
  being "knocked out" instead of dying, AI styles of the party (`GetPartyAIStyle` and `GetNPCAIStyle` return 0).
- `GetCreatureTalentBest`/`Random`, `GetIsDebilitated`, `GetLastAttackAction`, `GetNPCAIStyle`, `GetPartyAIStyle`, `GetPlayerRestrictMode` and
  `GetUserActionsPending` still answer fixed values (counted in `World::constant_defaults`): the fight calls them hundreds of times.

## Combat builder's changes, 6 Nov 2026 (`combat-party`)
All sizes below are my assumptions, not measured from the game.
| What | Rule | Where |
|---|---|---|
| Force Push (bug 36) | `EffectForcePushed` throws the target 3 m away from the caster over 0.5 s (stopped by walls, closed doors and creatures) and leaves it down for at least 3 s. The power's own script gives only 0.1 s | `force.rs`, `Shove`, `update_shoves` |
| Party doors (bug 39) | Followers and chasing enemies route through unlocked shut doors and open them within 1.6 m | `follow.rs`, `combat.rs` `chase` |
| Teammates (bug 21) | The player and party never attack one another. A party member with no target takes the leader's | `combat.rs` `assist_leader`, `attack` |
| Knocked out (GAPS 5) | `GetIsDead` is true for someone down. Downed creatures are not attacked. A fight lasts while an enemy still notices a standing party member | `host.rs`, `combat.rs`, `rules.rs` `fight_is_over` |
| Remains (bug 34) | Everything carried goes into the remains whatever Droppable says, except creature claws, bite and hide (`baseitems.2da` rows 81-84) | `play.rs` `leave_loot` |
| Collision (bug 15) | The player is stopped by party members too (walks through after 0.5 s pressed). Scripted walkers go round people | `follow.rs`, `play.rs` `step_move` |
| Sight (bug 25) | Sight is blocked by closed doors and by upright non-floor walkmesh faces at eye height (1.5 m). Floor missing for under 2.5 m (tables, crates) no longer blocks | `combat.rs` `clear_line`, `NavMesh::sight_blocked` |
| Talents (GAPS 8) | `GetCreatureTalentBest` is the dearest affordable power of the right kind (categories 1-3 harmful), `Random` any. `GetIsDebilitated` follows disabling effects | `host.rs` |
| Encounters (GAPS 6) | A creature whose CR is over the player's level plus `encdifficulty.2da` `value` stays home, unless it is the weakest on offer. `GetIsEncounterCreature` is real | `encounters.rs` |

Bug 32 (the dark Jedi fight): nothing hangs. Before `end_door16` is opened the Jedi and the Apprentice swap blows at 1 hit point each, because scripts hold them (`k_pend_cut19`, `k_pend_cut20`), so the fight looks endless. Opening the door runs `end_cut04`, a scripted 38 s sequence: pause 6.7 s, release of the Jedi's hit point at 17.8 s, the Apprentice dies at 21 s, the Jedi at 23 s, then waits and camera holds to 38 s. It needs no click (`the_jedi_duel_ends_without_any_click`). A fixed bound on the duel changed nothing, so I took it out.
Frames: `docs/shots/combat` at 720p and 2160p, made by `gpu/examples/combat_shots.rs`.

## Immunity and weapon sounds (GAPS row 9 leftovers, bug 33)

- `EffectImmunity(type)` stops the effects that `IMMUNITY_TYPE_*` (from the install's `nwscript.nss`) names: stun, paralysis, sleep, confusion, fear, entangle, knockdown, death, poison, movement slow and the stat decreases. **Assumed:** droid stun counts as stun, horror as fear, choke as paralysis, a push as knockdown. `GetIsLinkImmune` is true when the target is immune to the effect or any linked one. Test: `tests/damage_types.rs`.
- Melee swing and blow sounds use the item's `weaponmattype` in `baseitems.2da` as the `weaponsounds.2da` row. A stun baton was playing a blade (label guess), a long sword a small blade. Test: `gpu/tests/stage.rs`. Not known: what bug 33 meant for blasters. Their sounds are read from `ammunitiontypes.2da` and were not changed.
- Live fight, `gpu/examples/resist_shots.rs`: a blaster rifle against a plain soldier did 4.9 a hit over 12 hits, against one with 75% blaster immunity 1.6 over 7.

## The player's death

The owner: "you can die and it plays the animation but the character can keep playing regardless and just lies on the floor".

- **Whole party down** (`World::game_over`, set by `update_knockouts`): `World::step` runs `step_dead`. Input is dead (steer, clicks, containers, conversation), the fight is called off, and only the clock runs. `World::death_phase`: falling for 3 s, fading to black over 2 s, then over.
- **Stage** (`gpu/src/deathscreen.rs`): the camera is held, the HUD goes, the picture fades, then `Screen::GameOver` (`confirm.gui`): Load Game (the save list, newest chosen) and Quit (main menu). Esc does not leave it. Loading clears `game_over`.
- **Player down, someone else standing:** the player lies where it fell and cannot walk; the fight goes on; it stands up with 1 hit point when the fight is over. **A party member down alone** does the same.
- **Assumed, not measured:** the 3 s and 2 s, the words "You have died.", and that Load Game opens the list rather than loading at once. The reference video has no death. Tests: `crates/kotor-game/tests/death.rs`, `gpu/tests/death.rs`. Frames: `gpu/examples/death_shots.rs`.

