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
