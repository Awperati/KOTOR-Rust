# Our menus against the reference video

The owner's reference is a 21.3 hour longplay on `E:\Videos`, 1920x1080 and 30 frames a second, despite "4K" in its name. Frames from it stay in the git-ignored `cache\ref`, and nothing from it goes in the repository. `gpu/examples/menu_shots.rs` writes our frames to `docs/shots/menus`, also git-ignored.

The video's player runs at 16:9 with a mod or setting we cannot identify, so it shows one way the game can look, not proof of how the stock game looks at 4:3. Its corners also carry a watermark.

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
| Journal | 51:30 | none, not built | yes |
| Map | 3:50:35 | none, not built | yes |
| Party selection | 2:16:45 | none, not built | yes |
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
| Tabs over equipment, inventory, character and pause | none | `top.gui`; the screen that is up uses `lbl_icn_*2`, which is green. Abilities, Messages, Journal and Map tabs say they are not built |
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
| Choosing equipment | a click wore at once, a click on a worn slot took off | a click on a slot lists it, `inone` and None first, `(Equipped)` marked, words on the left, OK and Cancel. The owner chose this |

## Differences left
| What | Theirs | Ours | Why left |
|---|---|---|---|
| Text size | pixel-sized, about 12 pixels high at 1080p | scales with the window, about 27 pixels | Theirs would be too small to read at 4K. The owner chose to keep ours |
| List rows | about 13 layout units high, a small icon, no border until chosen | the file's 50, with a border and a large icon | The rows follow `PROTOITEM`, and their height depends on text size |
| HUD size | pixel-sized, so smaller at 1080p | scales with the window | as text size |
| Bottom bar in a conversation | about 12% of the height | about 22% | our larger text needs the room |
| Level-up figure | the 3D figure on the panel | an empty space | no 3D in menus |
| Feat rows | about 22 pixels high at 1080p, so all fit | the file's 56 units, so they scroll | as list rows |
| Feat button | "Add Feat", "Remove Feat" | "Add", "Remove", the file's words | I found no stock string with theirs |
| Map | a picture of the area, top left | a frame and an arrow | no map built |
| Main menu | Malak in 3D with moving fog on the left | the still picture | no 3D in menus |
| Character sheet | the 3D figure on a glowing backdrop | the portrait | no 3D in menus |
| Party portrait on sheets | a small portrait to change member, bottom left | none | party switching not built |
| Tutorial pop-ups | a black box with words and OK over any screen, from 3:16 | none | not built |
| Notices | XP and journal icons at the top left | none | not built |
| Combat HUD | icons over the target, a queue at the bottom | none | not built |
| Hover and chosen colour | yellow-green text; I could not measure it exactly, since small text in a compressed video has no clean pixels | yellow | the difference is small and unmeasured |
| Sounds | clicks and hovers | none in menus | not compared: I looked at frames only |
| Transitions | not compared | none | I looked at frames only, one every 5 seconds |
