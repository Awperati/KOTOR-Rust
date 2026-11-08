# Lifts

Every lift door (tag or template with "elev"/"lift" and a link to another module) starts a ride instead of a cut.
`docs/lifts.tsv` is the one table: `from<TAB>to<TAB>way<TAB>seconds`, either direction. Rows missing: level, 4 s.

Tip 1 (this one): the cheap ride. Input is held, the screen darkens over the first 15 % of the ride, the camera shakes,
`as_ot_rumble_01..05` hums every 1.8 s, the module changes in the dark, the screen returns over 0.9 s.
Tip 2: the full visual ride (car, source area slides, destination rises from below).

Assumed: the install has no lift model, shaft or lift sound. The rumble and the shake stand in. Lift doors are never
seamlessly joined. Leviathan's console conversation (`plev_elevctrl`) is not a door ride yet.
Code: `crates/kotor-game/src/lift.rs`, `gpu/src/lift_stage.rs`. Tests: `crates/kotor-game/tests/lift.rs`, `gpu/tests/lift_ride.rs`.
