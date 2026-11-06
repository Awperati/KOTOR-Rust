# Our menus against the reference video

The owner's reference is a 21.3 hour longplay on `E:\Videos`, 1920x1080 and 30 frames a second, despite "4K" in its name. Frames from it stay in the git-ignored `cache\ref`, and nothing from it goes in the repository. `gpu/examples/menu_shots.rs` writes our frames to `docs/shots/menus`, also git-ignored.

The video's player runs at 16:9 with a mod or setting we cannot identify, so it shows one way the game can look, not proof of how the stock game looks at 4:3. Its starting clothes match neither the game's packs nor the owner's Override, so its game may be modded: unconfirmed. Layout may still hold, but its looks are weak evidence. Its corners also carry a watermark.

| Screen | Their time | Our frame | Seen |
|---|---|---|---|
| Main menu | 0:21 | `main_menu` | yes |
| Pause menu | 22:50 | `pause` | yes |
| Equipment | 7:05, and 3:41 choosing an item | `equip`, `equip_item` | yes |
| Inventory | 3:32 | `inventory` | yes |
| Character | 1:23:50 | `character` | yes |
| Container | 3:24 | `remains_open` | yes |
| HUD | 3:28, 3:46 | `remains_target` | yes |
| Conversation | 2:30, 2:35 | `dialogue` | yes |
| Load screen between modules | 3:59:45 | none, not built | yes |
| Level-up | 22:20, 1:23:25 | `level_panel`, `level_attributes`, `level_skills`, `level_feats`, `character_level_up` | yes |
| Journal | 51:30 | `journal` | yes |
| Map | 3:50:35 | `map` | yes |
| Party selection | 3:12:00, 7:17:05 | `party` | yes |
| Character creation | 0:25 | none, not built | yes |
| Upgrade | 20:00 | none, not built | yes |
| Save and load screens | not found | `save_list`, `load_list`, `save_name` | **no** |
| Gameplay, Graphics, Sound pages, main menu Options | not found | `opt_*`, `options` | **no** |

**What "not found" means:** I sampled one keyframe every 5 seconds over the whole video, matched our screens against it, and looked at 12 of the 88 moments the scan flagged just after the pause menu. A screen up for under 5 seconds, or cut from the video, would not show. So our save, load and option pages have nothing to be compared with.

## Changed to match
Checked by the frames at the times above, and by `gpu/tests/menus.rs`.

| What | Before | Now |
|---|---|---|
| Screens on a wide window | centred, black at the sides | stretched over the window, 3 across to 2.25 down at 1080p |
| Tabs over equipment, inventory, character and pause | none | `top.gui`; the screen that is up uses `lbl_icn_*2`, which is green. Abilities and Messages tabs say they are not built |
| HUD | `maininterface.gui`, portraits at the right | `mipc28x6.gui`: portraits bottom left, four action slots bottom right, eight menu buttons top right that open their screens, map frame top left |
| Target name and health | fixed at the top | a bar floating over the target |
| Pictures with `blending additive` in their TXI | black boxes | their black is see-through. This applies to every interface picture |
| Empty equipment slots | blank | stock pictures `ihead`, `iimplant`, `ihands`, `iforearm_l`, `iforearm_r`, `iarmor`, `iweap_l`, `iweap_r`, `ibelt` |
| Conversation | a see-through box with the speaker's name | black bars top and bottom, no name, the line being answered in the top bar, replies in the bottom bar |
| Main menu | BioWare and LucasArts logos, a box round Quit | none of those |
| Character sheet | `+4` saves, `+0` modifiers, a Force row at 0/0, a large alignment arrow and a marker | `4` saves, `0` modifiers, no Force row without Force, a small arrow, no marker |
| Pause menu, right side | empty | the words for Load, Save and Exit under the cursor, from the talk table |
| Container title | the object's name | "Container Inventory" |
| `fnt_console` at 4K | letters run together | the stock font: its upscaled picture was the cause |
| What has not been seen of a map | all of it showed | a grid, as at 1:25:00 and 3:50:35: spacing and colours measured from those two frames, so close, not exact. Seen is a circle 14 of the picture's pixels round the player, with a soft edge |
| Key Mapping and Mouse Settings | greyed buttons | the stock pages. Keys are the first keys here, not the table's, and only what is built is listed: see `gpu/src/keyscreen.rs`. Not seen in the video, so not compared. Tested in code; a changed key was not pressed live |
| Feedback and Auto-Pause pages | greyed buttons | the stock pages, with the talk table's words beside them. Not seen in the video, so not compared. Not clicked live |
| Auto Level Up, Combat Movement | greyed boxes | working. Not clicked live |
| Tutorial pop-ups | none | a box with words and OK or Continue, from `tutorial.2da`, once each, as at 3:15, 3:36 and 4:00. What sets each off is read from the table's labels. Frames: `tutorial_screen`, `tutorial_game`. Over a screen and over the game tested in code; not clicked live, the driver turns them off |
| Notices | none | the HUD's own icons for quest, credits, XP from scripts, items and alignment, as at 3:46. Frame: `notices`. Not clicked live |
| Pause | none | Space or the hourglass, which is framed in yellow; as at 5:50, an enemy first seen pauses the game with the box at the top right. The video's ring on the hourglass is a plain frame here. Frame: `enemy_sighted`. Space was driven live. The pause by itself is tested in code, not live: the driver runs with `--no-auto-pause` |
| Combat slots and floats | empty | as at 6:05: feats in the second slot and hostile powers in the third, arrows through them, yellow under the cursor with the name on the bar. Damage, Miss and XP rise over the target. The numbers are the talk table's. Frame: `combat_slots`. Tested in code, not clicked live |
| Combat HUD | none | as at 5:50 and 6:05: the panel at the foot with the action under way and Disengage, the red line at the top, the enemy's bar red, slots under it. The first slot and Disengage were clicked in a test, not in the live window: the driver cannot start a fight. Frames: `combat_target`, `combat_engaged` |
| Small map on the HUD | a frame and a still arrow | the map round the player, 120 of the picture's pixels, with `mm_barrow` turning, compared with 3:46. A click opens the map. Which way the arrow's picture points is read from the frame `hud_minimap` |
| Choosing equipment | a click wore at once, a click on a worn slot took off | a click on a slot lists it, `inone` and None first, `(Equipped)` marked, words on the left, OK and Cancel. The owner chose this |

## Differences left
| What | Theirs | Ours | Why left |
|---|---|---|---|
| Text size | pixel-sized, about 12 pixels high at 1080p | scales with the window, about 27 pixels | Theirs would be too small to read at 4K. The owner chose to keep ours |
| List rows | about 13 layout units high, a small icon, no border until chosen | the file's 50, with a border and a large icon | The rows follow `PROTOITEM`, and their height depends on text size |
| HUD size | pixel-sized, so smaller at 1080p | scales with the window | as text size |
| Bottom bar in a conversation | about 12% of the height | about 22% | our larger text needs the room |
| Party figure | the 3D figure on a coloured backdrop | the portrait | no 3D in menus |
| Party OK | asks "Are you sure this is the party composition you want?" at 3:12:00 | no question | I did not look for where it asks or for its string |
| Level-up figure | the 3D figure on the panel | an empty space | no 3D in menus |
| Feat rows | about 22 pixels high at 1080p, so all fit | the file's 56 units, so they scroll | as list rows |
| Feat button | "Add Feat", "Remove Feat" | "Add", "Remove", the file's words | I found no stock string with theirs |
| Map picture | pixel-sized, small, placed low on the right at 3:50:35 | the map part, 440 of its 512 across, fills the file's place for it | as text size |
| Map notes | circles with a small mark inside | plain circles | the mark's picture not found |
| Main menu | Malak in 3D with moving fog on the left | the still picture | no 3D in menus |
| Character sheet | the 3D figure on a glowing backdrop | the portrait | no 3D in menus |
| Party portrait on sheets | a small portrait to change member, bottom left | none | party switching not built |
| Tutorial pop-ups over the game | the HUD stays under the box | the HUD goes while it is up | the box is drawn as a screen |
| Tutorial pop-up words | at 5:50 the words differ from the talk table's row 21 and 34 | the talk table's | where theirs come from not found |
| Notice timing | not measured | 6 s, assumed | the video's are short and I sampled every 5 s |
| Target slot words | also "Activate Left Action : 1" by the slot | the name on the bar only | that line is about keys, which are not built |
| Hover and chosen colour | yellow-green text; I could not measure it exactly, since small text in a compressed video has no clean pixels | yellow | the difference is small and unmeasured |
| Sounds | clicks and hovers | none in menus | not compared: I looked at frames only |
| Transitions | not compared | none | I looked at frames only, one every 5 seconds |
