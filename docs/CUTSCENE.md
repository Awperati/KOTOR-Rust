# Cutscene sequencing (kotor-game `cutscene.rs`)

`World::converse` now keeps, for the conversation it played, every visited node's presentation data (`World::conversation`); `World::cutscene_timeline()`
turns it into a `Timeline` of `Beat`s (start, length, camera shot, animation cues, hold) and `Player` walks a timeline against a clock (advance, pause, skip).
Tests: `tests/cutscene.rs` (5).

## Read from the install, per DLG node
`AnimList` (participant tag, animation id), `CameraAnimation`, `CameraAngle`, `CameraID`, `CamFieldOfView`, `Delay`, `WaitFlags`, `FadeType`, `VO_ResRef`,
`Sound`, the script; and per dialogue `CameraModel` and `StuntList` (participant tag -> stunt model). Lengths come from the stunt models' animations and
from the measured duration of the voice-over file (`kotor-audio`).

## What the Endar Spire opening (`m01aa_c01`) came out as
The path the conversation takes is ten nodes (five entries, each followed by a text-less reply): animation ids 1200 to 1203 on the player's stunt model and the
camera model, 4.97 + 3.87 + 4.87 + 4.13 seconds, then a last entry on fixed camera 17 with a pause and a one-second wait: **a timeline of about 19 s**.
The ids 1200-1203 are consecutive, and the stunt models' first four animations (`CUT001W` to `CUT004W`) have exactly those four lengths in that order,
which is the evidence for the id -> animation mapping below.

## Assumed (I do not know these)
- Animation ids from 1200 index the stunt model's animations in file order. Ids 1219 and 1220 (seen in the DLG for the player) fall outside and give no length.
- A beat lasts as long as its longest camera animation, participant animation or voice-over; with none, `Delay` seconds (in seconds: a guess), else reading time
  (15 characters a second, at least 1.5 s). `WaitFlags` and `FadeType` are recorded and unused.
- `ActionPauseConversation` holds the conversation after its node until the matching `ActionResumeConversation`, for the sum of the `ActionWait`s queued between.
  Movement and animation lengths inside the paused action list are not modelled.
- Scripts run when the conversation is resolved, not at the beat's start time (so e.g. `k_pend_rumble01` has already run when beat 0 begins).
- What `CameraAngle` codes mean (4 with an animation is taken as "animated camera"; the others are kept as codes).
