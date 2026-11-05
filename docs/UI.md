# Interface drawing (kotor-ui)

Reads the owner's install at runtime and draws to an image; no window. `cargo run -p kotor-ui --example ui -- <out_dir> [scale]`
writes `dialogue_speech.png`, `dialogue_replies.png` and `hud.png` (kept out of git: they contain game textures).

| Piece | Notes |
|---|---|
| `Gui` | `.gui` files (resource type 2047) are GFFs: a root panel (`EXTENT`, `BORDER`) plus a `CONTROLS` list. Control types seen: 2 panel, 4 label, 5 button, 9 scrollbar, 10 progress bar, 11 list box. Layout space is 640x480 |
| `Font` | a TPC (rows stored bottom-up; flipped on load) plus the TXI inside it: `upperleftcoords` / `lowerrightcoords`, v = 0 at the bottom. Glyph width = cell width; line height = tallest cell |
| `Ui` | texture, font and GUI caches over an `Install`; `dialogue_box(&DialogueView, scene)` and `hud(&[HudMember], target)` |

Checked against the install: `dialog.gui` panel is at (48, 378) 544x100; subtitles wrap and centre inside it; the reply list numbers and wraps
replies, scrolls to keep the selected one visible and colours it; the HUD shows portraits (`portraits.2da` `baseresref`), vitality and Force
bars filled in proportion, and the target bar and name.

## Assumed, not measured
- Text alignment codes: horizontal 1 left, 2 centre, 3 right, plus vertical 0 top, 8 middle, 16 bottom.
- **`fnt_d16x16`, the font `dialog.gui` names, ships a TXI that does not describe its picture** (cells are empty). The loader checks that
  `A`, `e` and `0` have ink and otherwise uses `fnt_dialog16x16`. The original engine's behaviour for this is unknown.
- The first control in a file is the topmost, so controls are drawn last to first (otherwise the portrait frames cover the portraits).
- Border corners and edges are stretched, not tiled; edges and corners turn a quarter per side.
- The selected reply is yellow; a dark translucent backing and a yellow speaker name are drawn for readability (the real box may differ).
- HUD hides level-up, queue, target-bar, combat-effect, debilitated, disabled and level-up-background controls unless asked for.
- Vertical progress bars (taller than wide) fill upward; horizontal ones from the left (`STARTFROMLEFT` respected).
