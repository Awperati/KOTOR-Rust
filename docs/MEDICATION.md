# Medication Sickness

Using a medical item while the cooldown from the last one still runs is allowed, with a price. Code:
[medication.rs](../crates/kotor-game/src/medication.rs). Numbers: [medication.tsv](medication.tsv). Tests:
[medication.rs](../crates/kotor-game/tests/medication.rs).

| Use | Heals | Hits that become misses | Damage |
|---|---|---|---|
| First, with no cooldown running | all | none | all |
| Second, inside the 20 s | half | half | three quarters |
| Third and later, inside the 20 s | a quarter | four in five | half |

- Each use restarts the 20 seconds. The sickness ends when they run out, 20 s after the last use.
- The warning "warning, continuing will cause medication sickness. Are you sure you would like to continue?" is asked once for
  each cooldown window. No cancels, and it asks again next time. Yes uses the item.
- It is per character, and saved.
- Companions and scripts that use an item are not asked, and get the sickness all the same.

## For the menus

`World::medication_cooldown(who)` is the share of the cooldown left, for the white square on the pill. `World::use_consumable(who,
item, confirmed)` returns `Used`, `NeedsConfirm` or `Refused`. `World::medication_stack(who)` is 0, 1 or 2: above 0, show
"Medication Sickness" on the character's panel.

## Assumed

- Only items whose base item is `Medical_Equipment` count: medpacs and stimulants. Repair kits and recovery kits do not.
- "Aim 50% worse" means half of the hits become misses, whatever the level. "Damage 25% worse" takes a quarter off each blow.
- A hit that the sickness turns into a miss follows the rules for a miss, so with No misses on it still glances.
- The stack stops at the second extra use.
