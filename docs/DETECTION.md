# Detection bars

Each enemy that has noticed the party has a bar on the screen, and attack mode ends only when every bar is empty. Code:
[detection.rs](../crates/kotor-game/src/detection.rs) for the numbers, [detection.rs](../crates/kotor-ui/src/detection.rs) for the
drawing. Tests: [detection.rs](../crates/kotor-game/tests/detection.rs). Frames: `cargo run --release --example detection_shots`
in `gpu/`, which writes `docs/shots/combat/detection_*.png`.

## The number

Each hostile creature has a number from 0 to 1. It rises while the creature perceives the leader or a companion: range, a clear
line and the Stealth check are the game's own (`update_perception`). It rises faster when the party is closer: 0.8 s from empty to
full beside it, 3.5 s at the end of its perception range. It falls over 3 s once it no longer perceives them. A creature that is
attacking one of the party is at 1. All three times are constants at the top of the file.

## On the screen

A ring of small arrows round the middle of the screen, one for each such enemy, at most the 8 most alerted. Each points the way its
enemy lies from where the camera looks. An arrow fills from its base to its tip with the number, white at first, then yellow, and
red when full. When the enemy loses the party the arrow drains and fades. Options > Feedback has **Detection bars: On** (default).
The ring is 0.30 of the screen's short side from the middle, an arrow 0.045 long: constants in the UI file.

## Attack mode

Attack mode goes off by itself (with Auto attack mode on) only when nobody is fighting, every bar is empty, and then
`AUTO_OFF_SECONDS` (10) have passed. A new sighting starts the count again. See [ATTACK_MODE.md](ATTACK_MODE.md).

## Assumed

- The game's AI is not slowed by the number: an enemy that perceives the party reacts at once, as it always did. So the bar mostly
  fills while it is already coming.
- Light and sound do not change the rate. The game has no rule for either in noticing.
- The sizes, colours and the 8 are mine. The owner gave none.
