# Lifts

Every lift door (tag or template with "elev"/"lift" and a link to another module) starts a ride instead of a cut.
`docs/lifts.tsv` is the one table: `from<TAB>to<TAB>way<TAB>seconds`, either direction. Rows missing: level, 4 s.

Tip 1 (this one): the cheap ride. Input is held, the screen darkens over the first 15 % of the ride, the camera shakes,
`as_ot_rumble_01..05` hums every 1.8 s, the module changes in the dark, the screen returns over 0.9 s.
Tip 2: the full ride, for lifts the table gives as `down` or `up` and whose far module is loaded ahead (the player stood near the
door): the camera and the party are held, the area left slides up (down) or down (up) by 1.5 m a second of the ride, and the area
arriving is the module's own picture, placed with its arrival waypoint at the party and facing as it faces, one ride's height
lower (higher), coming in from below (above). They meet at a height that sweeps across the view (`LiftView`, two planes in the
shader, the doorway's portal slot). No dark. The door stays with the car. A level lift, or one whose module was not ready, keeps
the tip 1 ride (dark, shake, hum). Frames: `cargo run --release --example lift_strip -- tar_m03aa tar04_elevdoor out`.
Assumed: 1.5 m a second, so 6 s is 9 m; the car has no floor of its own (the old floor slides out from under the party).

Assumed: the install has no lift model, shaft or lift sound. The rumble and the shake stand in. Lift doors are never
seamlessly joined. Leviathan's console conversation (`plev_elevctrl`) is not a door ride yet.
Code: `crates/kotor-game/src/lift.rs`, `gpu/src/lift_stage.rs`. Tests: `crates/kotor-game/tests/lift.rs`, `gpu/tests/lift_ride.rs`.
