# Shields

A shield item (a droid's energy or hazard shield, an Echani or Sith shield) or a power that calls `EffectForceShield(row)`
puts a shield on its user. Code: `arm_shield` and `absorb_with_shields` in [force.rs](../crates/kotor-game/src/force.rs).
Test: [shield.rs](../crates/kotor-game/tests/shield.rs).

- `row` is a row of `forceshields.2da`. `damageflags` says which damage types it takes in. `amount` is how much.
- Every blow and every damage effect meets the target's shields after resistances and difficulty. The shield takes what it can.
- A shield that is spent goes. Its time (200 s in the item's script) also ends it.
- Universal damage (type 8) passes a shield.
- The shield's store is kept in the effect, so a save keeps it.

## Assumed

- `amount` is the shield's whole store, not a cut off each hit. The game's files do not say.
- The table's `resistance` (300 on every row) and `vulnerflags` (ion, 2048) are not used. Ion damage is not stronger against a shield.
- Several shields on one target take a blow in the order they were put on.
