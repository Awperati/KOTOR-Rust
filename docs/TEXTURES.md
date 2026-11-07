# Your own textures

A picture you paint can stand in for any texture of the game. Nothing of the game's files changes.

## Steps

1. Make the folder `%LOCALAPPDATA%\kotor-rs\textures` (in Explorer, type that in the address bar, then make the folders `kotor-rs` and `textures`).
2. Get the game's own picture to paint over. In a terminal, from the project folder:

   `target\release\kotor-tool.exe png n_traskh "%LOCALAPPDATA%\kotor-rs\textures\n_traskh.png"`

   The name is the texture's name in lower case. The `blender-export` command (see `docs/BLENDER_ANIMATION.md`) writes all the textures of a model under the same names.
3. Paint it in Krita, GIMP or Blender, and save it in that folder with the same name, as `.png` (8-bit, not interlaced) or `.tga`.
4. In the running game press **F8**. The game draws your picture. No restart is needed for people and things that move; a room's own walls are read when the room loads, so go through a door and back.

## Rules

- The picture must be the game's size, or exactly twice, four times or half of it in **both** directions (a 256x256 texture may be 512x512 or 1024x1024).
- A body texture you replace is drawn as it is: the style's underwear paint and the neck's matching are not laid over it, because it is what you painted. Painting the player's body, keep to the game's layout of the sheet.
- If a picture cannot be used (wrong size, not a picture) the game keeps its own texture and says why in `texture_problems.txt` in the same folder. Look there first when nothing changes.
- Take your file out of the folder and press F8 to go back to the game's own.

## For a test

`KOTOR_RS_TEXTURES` names another folder to use in place of the one above. The test `gpu/tests/texture_override.rs` shows a picture replacing a texture, a wrong size and a bad file falling back, and F8 bringing the game's own back.
