# The talking face: what the game's files give, and what is filled in

## From the files
- **`.lip` files** (`lips/*.mod`, one ERF for each module, plus `global.mod`; 18,210 in the install): `LIP V1.0`, the clip's length
  (f32), a count (u32), then keys of a time (f32) and a shape (u8, 0 to 15). A voice clip `nm01aacut101000_` has `nm01aacut101000_.lip`.
  The engine reads them lazily by the line's voice resref (`lip_of` in `gpu/src/stage.rs`, `parse_lip` in `character.rs`).
- **The head's mouth nodes**: `f_jaw_g`, `f_um_g`, `f_llm_g`, `f_rlm_g`, `f_lmc_g`, `f_rmc_g`, `f_lns_g`, `f_rns_g`, `f_tonguetip_g`.
  The `talk` clip (0.5 s) moves them; its jaw is widest 0.10 s in (about 5.6 degrees).
- **Face clips** in the body's supermodel: `tlknorm`, `tlkforce`, `tlkplead`, `tlksad`, `tlklaugh`, `listen`, `good`, `neutral`, `evil`,
  `horror`, `persuade` and more. K1's `.dlg` has no emotion field: a line's `AnimList` ids pick them (`dialoganimations.2da`), and a line
  with none is `tlknorm`.

## What is filled in (assumed)
The files say nothing about what a shape does to the face. `LIP_SHAPE_OPEN` (in `crates/kotor-render/src/character.rs`) is how far the
jaw opens for each, 1 being the `talk` clip's widest (0.10 s in); the mouth moves to each key's shape in the first half of the gap to
the next and holds it, is shut before the first key, after the last, and for 0.04 s at each end of the clip. Tune the numbers here and
in the constant together.

| Shape | Sound | Jaw opening |
| --- | --- | --- |
| 0 | neutral | 0.0 |
| 1 | EE | 0.5 |
| 2 | EH | 0.8 |
| 3 | schwa | 0.7 |
| 4 | AH | 1.5 |
| 5 | OH | 1.0 |
| 6 | OOH | 0.6 |
| 7 | Y | 0.5 |
| 8 | S, T, S | 0.3 |
| 9 | F, V | 0.2 |
| 10 | NG | 0.4 |
| 11 | TH | 0.5 |
| 12 | M, P, B | 0.0 |
| 13 | T, D | 0.5 |
| 14 | SH | 0.5 |
| 15 | L | 0.6 |

The shape names are from the usual order of the format; the game's files do not name them.

## When the mouth moves
Only while a voice clip plays for the line, and only for its speaker. A line with no playable voice (a hint, the narrator, a voice this
install lacks) moves no mouth and starts no talk gesture. A clip with no `.lip` falls back to its loudness over time (`mouth_at`).
