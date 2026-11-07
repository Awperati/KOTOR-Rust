# Making your own clothing in Blender

You model a garment over the game's own body in Blender, weight it to the game's bones, and the game dresses the player in it. It moves with every animation the body plays. Nothing of the game's files changes.

Read `docs/BLENDER_ANIMATION.md` first for the export and the add-on; this goes on from there.

## What the game takes

| Rule | Why |
| --- | --- |
| A `.glb` (or `.gltf` + `.bin`) in `%LOCALAPPDATA%\kotor-rs\meshes` | the folder the game reads |
| Skinned to the game's bones, vertex groups named as the bones (the rig's names) | the game moves it with the body's own skeleton |
| At most 4 bones per vertex, each vertex weighted | the game's skin has 4 slots |
| At most 8000 triangles in all | the body is 1500 to 1800 |
| A UV map, and a material whose **name is the picture's name** (lower case) | the picture comes from your texture folder (`docs/TEXTURES.md`), or from inside the file |
| Modelled over the body in the rig's rest pose | the game draws it in the rest pose of the body and skins it |

## Steps

1. Export the body you dress (`docs/BLENDER_ANIMATION.md`, step 1) and **Load export** in the add-on. The stock body meshes come in as a reference to model over; hide or delete them when you export.
2. Model the garment as one or more meshes over the body. Give each a UV map and a material; name the material for the picture, e.g. `vest01`.
3. Parent it to the *KotOR Rig* with automatic weights (select the mesh, then the rig, Ctrl+P > With Automatic Weights), then fix the weights by painting if you like. Weights > Limit Total 4.
4. If it takes the place of part of the body, tick what it **covers** in the panel:
   - *the stock body* is the trunk, the pelvis, the legs and the feet in one piece in the game: your garment then has to be all of that;
   - *the stock arms and hands* are the two arms with their hands.
   Leave both unticked for something that goes over the body (a vest, a belt).
5. Select the garment mesh(es) and press **Validate mesh for KotOR-RS**: it names vertex groups that are not the game's bones, vertices with no weight, too many triangles, no UV map, no material, no Armature modifier.
6. Press **Export garment**. It writes `<name>.glb` in the folder shown and its line in `items.txt` (`vest covers=arms`).
7. Paint the picture and save it as `vest01.png` in `%LOCALAPPDATA%\kotor-rs\textures` (same size as it needs; see `docs/TEXTURES.md`), or leave the picture inside the file (Blender packs it for you when the material has an image texture).

## Play it

1. In the viewer press **F8**: it reads the meshes (and animations and textures). A file the game cannot use is left out and the reason is in `mesh_problems.txt` in the folder.
2. Press **F10**: the player puts on your first garment; again for the next; after the last, the game's own clothes.

## items.txt

```
# name of the file without .glb, then what it covers of the stock body
vest covers=arms
robe covers=body,arms
belt
```

## Limits of this version

- The player wears it with F10. Wearing it by an item (an armour the game gives) comes next.
- What a garment covers is by piece of the stock body, not by region: a vest that covers nothing is drawn over the body, and the body shows through where it is thin or too small.
- The garment is drawn in the body's rest pose and skinned to the body's bones; it has no cloth movement of its own.
- Underwear of the player is part of the stock body: while that is covered it is not drawn.

## Tested

`gpu/tests/custom_mesh.rs` makes a vest from the stock torso (its upper half, a centimetre out) as a `.glb` the way Blender writes one (Y up, named joints, a picture inside), and checks that: F8 reads it and names a bad file; F10 puts it on the player with the picture from the file; the arms it covers are gone; it moves as the body walks; the next F10 takes it off. A frame: `docs/shots/vest/vest.png`.

The add-on's two mesh buttons were not run in Blender here (see `docs/BLENDER_ANIMATION.md`, "Not tried").
