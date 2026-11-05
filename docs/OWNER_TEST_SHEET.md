# Owner test sheet: first hand-built module in the real game

**Status: prepared, NOT run.** Nothing here has been done to your game. Every step that touches `E:\Games\Star Wars - KotOR`
is yours to do, and each is small and undoable. Stop at any step you do not like.

## What you are testing

Whether the original game accepts a module file written by kotor-rs. The test module is a copy of the Endar Spire
(`end_m01aa`) with a new display name and a renamed area, so it sits next to the original and replaces nothing.

## 0. Before anything: backup (2 minutes)

1. Copy `E:\Games\Star Wars - KotOR\saves` to a safe place (for example `D:\kotor-backup\saves`).
2. Copy `E:\Games\Star Wars - KotOR\swkotor.ini` to the same place.
3. Note: the test adds **one new file** (`modules\zz_test01.mod`). It overwrites nothing. Undo = delete that one file.
   Your 1440p setup (exe patch, k1hrm menus, Override) is not touched.

## 1. Build the test module (safe: writes outside the game folder)

From `C:\Users\User\Projects\kotor-rs` in PowerShell:

```powershell
cargo build -p kotor-tool
New-Item -ItemType Directory -Force C:\Users\User\Desktop\kotor-test | Out-Null
target\debug\kotor-tool.exe author clone end_m01aa "Test Spire" C:\Users\User\Desktop\kotor-test\zz_test01.mod zz_spire
```

Expected output: lines starting `renamed m01aa.are -> zz_spire.are` (also `.git`, `.pth`, and `.lyt`/`.vis` copied from base data),
then `N string fields rewritten, M bytes written`. The tool refuses to write inside the game folder.

## 2. Install it (your step)

Copy `C:\Users\User\Desktop\kotor-test\zz_test01.mod` to `E:\Games\Star Wars - KotOR\modules\`.

## 3. Enter the module (open question, I have not verified this)

A new module is not reachable from the menus. The usual way is the debug console command `warp zz_test01`, which I
believe needs cheats enabled in `swkotor.ini`. **I have not checked that and will not edit your ini.** Top Dog should
ask the 1440p Builder whether cheats/console are already on in your setup. If they are not, say so and decide whether you
want to turn them on; that is your call.

## 4. What to check

| # | Check | Pass looks like |
|---|---|---|
| 1 | `warp zz_test01` loads | the Endar Spire bridge area appears, no crash, no black screen |
| 2 | Layout and textures | same rooms and textures as the normal Endar Spire |
| 3 | Walking | the player can walk and doors open |
| 4 | NPCs and dialogue | Carth/Trask style conversations start as in the original |
| 5 | Module name | if a name is shown anywhere it says "Test Spire" |
| 6 | Exit | quitting or loading the normal game afterwards works |

Known limits: compiled scripts that mention `m01aa` by name were not rewritten, so some scripted events may behave oddly or
point back at the original area. That is expected and is useful to report.

## Test B: a talking NPC (only after test A passes)

Same module plus one new creature ("Guide", copied from Trask's template, invulnerable, stock AI scripts, no story scripts)
standing about 3 m east of where you spawn, with an authored conversation.

```powershell
target\debug\kotor-tool.exe author clone end_m01aa "Test Spire B" C:\Users\User\Desktop\kotor-test\zz_test02.mod zz_spireb --npc zz_guide Guide docs\examples\guide.dlg.txt
```

Expected output also lists `added creature zz_guide at (x, y, z) talking zz_guide.dlg`. Copy `zz_test02.mod` to `modules` the
same way and `warp zz_test02`. Check:

| # | Check | Pass looks like |
|---|---|---|
| B1 | Guide appears near the spawn point, standing on the floor | visible, not floating or in a wall |
| B2 | Click Guide | the conversation opens with "You made it off the bridge. Few do." |
| B3 | Reply options | "Who are you?" and "I have no time for this." appear and lead where the text says |
| B4 | Ending the talk | the conversation closes cleanly, the game continues |
| B5 | Guide in a fight | Guide cannot be killed (plot flag set) |

Known unknowns: the stock `k_hen_dialogue01` script is what starts the talk; I believe it opens the creature's own
conversation but have not run it. If B2 does nothing, tell Top Dog; the likely fix is a different stock dialogue script.
Names stay "Guide" because the text is stored inline, not in `dialog.tlk`.

## 5. Report back / undo

- Tell Top Dog: which checks passed, any crash text, any screenshot.
- Undo: delete `E:\Games\Star Wars - KotOR\modules\zz_test01.mod`. If anything else looks wrong, restore the backups from step 0.

## Dialogue sample (not part of the test above)

`docs/examples/hello.dlg.txt` is a tiny conversation in kotor-rs's text format. Build it with:

```powershell
target\debug\kotor-tool.exe author dlg docs\examples\hello.dlg.txt C:\Users\User\Desktop\kotor-test\hello.dlg
```

A DLG only runs when a creature or placeable points at it. Test B above does that without any compiled script, using the
creature's `Conversation` field and a stock base-game dialogue script. The authoring path is checked here by reading
everything back and by 117 real dialogues round-tripping; running it in the game is test B.
