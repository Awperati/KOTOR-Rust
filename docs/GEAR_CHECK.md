# Gear check: what worn items do to AC, damage and resistance (fixed in the same branch)

Checked on 8 Nov by reading [gear.rs](../crates/kotor-game/src/gear.rs), [combat.rs](../crates/kotor-game/src/combat.rs) and the
item files, with `cargo run --release -p kotor-game --example gear_scan -- [word]` (lists items whose name has the word, their
properties, and which ones the code reads; `--columns` shows the armour table). Classic and Modern use the same
`resolve_hand`, so every finding holds in both. Modern only multiplies the blow by its share first.

## Right

- AC is 10 + the armour's `baseac` + `Armor` properties (upgrades count once in their slot) + natural AC + Dex + AC effects.
- Enhancement and flat damage go on every blow, dice are rolled, a massive critical adds on a critical only.
- Ability, skill, save, attack and damage properties from worn items work. Sith Armor (`ptar_sitharmor`, a disguise) gives +2 AC.

## Fixed (what was wrong, now built in [gear_effects.rs](../crates/kotor-game/src/gear_effects.rs))

| # | Property | Items | Now |
|---|---|---|---|
| 1 | `DamageResist` (17) | 69 | Flat amount off a hit of that type, in `adjust_damage` |
| 2 | `Immunity` (24), `DamageImmunity` (14) | 61 | Percent off a type; immune to the effect a row names (mind, poison, fear, paralysis, knockdown, critical hits, death) |
| 3 | `Blaster_Bolt_Deflect_Increase` (55) | 32 | 5% of ranged blows per point turn aside, at most 75% |
| 4 | `OnHit` (32) | 62 | Sleep, stun, paralysis, confusion and fear, with a save of DC 14, for 6 s |
| 5 | `Regeneration` (35), `Regeneration_Force_Points` (54) | 18 | The amount every 3 s, for the party |
| 6 | `BonusFeats` (9), `ImprovedMagicResist` (25) | 30 | Feats known while worn, not saved; force resistance added to `ResistForce` |
| 7 | Max Dex to AC (`dexbonus`) | all armour | Dex counts up to the worst cap among the pieces that give armour class |

Tests: [gear_effects.rs](../crates/kotor-game/tests/gear_effects.rs), one per row. Calo Nord's armour and weapon now resist and stun, so
the tests that use him as a dummy clear his gear first.

**Assumed, not compared with the game:** damage type bit `1 << row` of `iprp_damagetype`; the 5% a point, 3 s, DC 14 and 6 s above; a
property whose amount the tables do not give counts as 1. **Still not built:** `Monster_damage` (51), `Trap`, `ThievesTools`,
`True_Seeing`, `Computer_Spike`, `EnhancementAlignmentGroup`, `EnhancementRacialGroup`, slow and ability drain as on-hit riders, and
the `Backstab` and `death` immunities that nothing asks.

Cause of the old gaps: `adjust_damage` in [force.rs](../crates/kotor-game/src/force.rs) read only effects, and no code turned a worn
item's property into an effect.

## Modern note

Modern's blows each do a share of a hit, so a flat resistance is scaled by that share (`adjust_damage_share`). A chain loses
about what one Classic hit of the same total would.

## Not checked

Light armour and robe properties with no `baseac` beyond the table above; the item level tables; the `UseLimitation*` rows.
