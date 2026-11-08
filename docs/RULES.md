# Rules breadth (kotor-game `rules.rs`, `force.rs`)

Character rules read from the game's tables, and Force powers run by the game's own impact scripts. Tests: `tests/rules.rs` (10).
`cargo run -p kotor-game --example powers [row]` casts every power once and reports what each did (a row number prints that
power's routine trace).

## From the install
| Rule | Source |
|---|---|
| Saving throws | `classes.2da` `savingthrowtable` -> `cls_st_*` `fortsave` / `refsave` / `willsave`, row = level - 1 |
| Class skills, skill point base, key ability, untrained use | `skills.2da` `<class>_class`, `keyability`, `untrained`; `classes.2da` `skillpointbase` |
| Feats | `feat.2da` `<class>_list` (3 granted at `_granted` level, 1 and 0 the two choice lists, 4 not available), `mincharlevel`, `minstr/dex/int/wis`, `prereqfeat1/2`; slots per level from `featgain.2da` `<class>_reg` / `_bon` |
| Hit and Force dice | `classes.2da` `hitdie`, `forcedie` |
| Levels | `exptable.2da` |
| Force powers | `spells.2da` `forcepoints`, `range`, `impactscript` (every power uses `k_sp1_generic`) |

## Rules as implemented
- Save: d20 + class save + ability modifier (Fort Con, Ref Dex, Will Wis) + effects against a DC; natural 1 fails, natural 20 passes.
- Skill check: d20 + ranks + key ability modifier + effects against a DC; a skill that is not usable untrained fails with 0 ranks.
- Ranks: a class skill rank costs 1 point and is capped at level + 3; cross-class costs 2 and is capped at half.
- Level-up (`level_up`, `grant_xp`): hit points, Force points, skill points, feat slots and granted feats; XP crosses as many levels as `exptable` says.
- Kills: the party earns the victim's `ChallengeRating` x 25, paid in full to each of the standing party (an assumption, see `REFERENCE.md`). Party members at 0 hit points are knocked out
  and stand up with 1 hit point when no fight involves them; everyone down at once sets `World::game_over`. Anyone else dies (death script runs).
- Powers (`cast_spell`): must be known, affordable and in range (touch 2.25, short 10, medium 15, long 28 m, `P` = self); the cost is paid; the impact
  script runs with `GetSpellId`, `GetSpellTargetObject`, `GetLastSpellCaster` set; `FortitudeSave`/`ReflexSave`/`WillSave` and `GetSpellSaveDC` work.
- Effects: instant ones (`EffectDamage`, `EffectHeal`, `EffectDeath`, Force point damage/heal, resurrection) act at once; temporary ones last the given
  seconds then drop; permanent stay. While active, AC, attack, ability, saving-throw and skill increases/decreases change the numbers, and stun, paralysis,
  sleep, knockdown, fear, choke, force push, entangle and similar effects stop a creature attacking. `EffectLinkEffects` applies both halves.
- All 42 powers' scripts run with no missing routine; buffs (Force Armor, Valor, Speed, Resist Energy, ...) leave their effects; hostile powers depend on
  the target's save roll.

## Assumed, not measured from the game (I do not know these)
- The save and skill-check formulas above, and a natural 1/20 on saves.
- First-level skill points: half the class base plus the Intelligence modifier, x4. Later levels: half of base plus modifier. Both at least 1. A Soldier with Intelligence 12 has 8, then 1 a level: the video at 0:50 and 1:23:30. The rest of the formula is **assumed**; the first feat list is the "regular" slots and list 0 the "bonus" slots.
- Each level's hit points are the whole die plus Constitution, measured on one sheet in `REFERENCE.md`. Force points after level 1 are half of `forcedie` + 1, which is not measured.
- **Kill experience = challenge rating x 25: the real rule is unknown to me.**
- Knocked-out party members stand up with 1 hit point when the fight ends; real recovery may differ.
- Spell save DC = 10 + half the caster level + Wisdom modifier. Force resistance, link immunity, creature size and movement type are fixed answers
  (`constant_defaults`). Area shapes are treated as circles. A `GetNextObjectInShape` with no `GetFirstObjectInShape` before it starts a new walk
  (the powers' scripts do this).
- Not modelled: damage types, resistances, immunities, regeneration, haste and speed, invisibility, shields, area-of-effect objects, visual effects;
  talents other than Force powers (feat talents are only recorded); multiple classes.
