# Gear check: what worn items do to AC, damage and resistance

Checked on 8 Nov by reading [gear.rs](../crates/kotor-game/src/gear.rs), [combat.rs](../crates/kotor-game/src/combat.rs) and the
item files, with `cargo run --release -p kotor-game --example gear_scan -- [word]` (lists items whose name has the word, their
properties, and which ones the code reads; `--columns` shows the armour table). Classic and Modern use the same
`resolve_hand`, so every finding holds in both. Modern only multiplies the blow by its share first.

## Right

- AC is 10 + the armour's `baseac` + `Armor` properties (upgrades count once in their slot) + natural AC + Dex + AC effects.
- Enhancement and flat damage go on every blow, dice are rolled, a massive critical adds on a critical only.
- Ability, skill, save, attack and damage properties from worn items work. Sith Armor (`ptar_sitharmor`, a disguise) gives +2 AC.

## Wrong (count = items in the install that carry it)

| # | Property | Items | What happens now | What it should do |
|---|---|---|---|---|
| 1 | `DamageResist` (17) | 69 | Nothing. Cinnagar Plate vs 10 energy: 10 taken | Flat amount off hits of that type |
| 2 | `Immunity` (24), `DamageImmunity` (14) | 61 | Nothing | Percent off that type, or immune to an effect |
| 3 | `Blaster_Bolt_Deflect_Increase` (55) | 32 | Nothing | Deflect bolts |
| 4 | `OnHit` (32) | 62 | Nothing | Weapon rider (stun, slow, and so on) |
| 5 | `Regeneration` (35), `Regeneration_Force_Points` (54) | 18 | Nothing | Heal over time |
| 6 | `BonusFeats` (9), `ImprovedMagicResist` (25), `Monster_damage` (51) | 49 | Nothing | The feat, the resist, the damage |
| 7 | Max Dex to AC (`dexbonus` of `baseitems.2da`) | all armour | Full Dex counts. Class 9 armour caps it at 0 | Cap Dex at 5 (class 4) down to 0 (class 9) |

Cause of 1 and 2: `adjust_damage` in [force.rs](../crates/kotor-game/src/force.rs) reads only effects, and no code turns a worn
item's property into an effect. The note at the top of gear.rs ("damage types not built") is out of date for the weapon side.

## Modern note

The blow share is applied before resistance, and a flat resistance takes a full amount off each small blow. Once 1 is built,
a Modern chain loses more to resistance than one Classic hit of the same total. Subtract resistance after the chain's sum, or
scale it by the share.

## Not checked

Light armour and robe properties with no `baseac` beyond the table above; the item level tables; the `UseLimitation*` rows.
