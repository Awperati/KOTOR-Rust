# Plan: a detailed character creation

The owner: "Or just have selectable 'underwear' meshes in the character creation screen. I want the character creation to be complex and detailed like the skyrim one".

This is a plan. Stages 0 and 1 are built, stage 2 but its skin tone, stage 3 but its eye colour, stage 7, stage 5 for women, stage 4 without pictures, stage 8, and stage 6's hair colour; hair and beard pieces are not. Sizes are my estimates of lines of Rust with tests, not measurements. It builds on [CHARGEN.md](CHARGEN.md).

## What the game's files allow

I read these from the install on 2026-11-07.

| Thing | What is there | What it means |
|---|---|---|
| Player heads | 30 models: `PFHA01` to `PFHC05` and `PMHA01` to `PMHC05`, 15 for each sex | A pick of 30, as now |
| One head (`PFHA01`) | 990 vertices, 1,075 faces, one texture | Few vertices to shape, so sliders stay broad |
| A head's parts | `head`, `eyeLA`, `eyeRA`, `eyeLlid`, `eyeRlid`, `hairalpha`, `teethupper`, `teethlower`, `tongue` | Eyes are their own meshes. Their colour can change on its own |
| Morph targets | None. The model format has no vertex morphs | Every face slider needs our own deformer |
| Hair | In the head's mesh and texture, with only the see-through strands apart in `hairalpha` | No hair can be swapped or removed with the game's files alone |
| Facial hair, scars, make-up | Painted into each head's one texture, or absent | New ones need art |
| Bodies | Small, medium and large for each sex, one model for each armour | Body sliders must work on every model the player can wear |
| Underwear | Ours is a run-time change to the three female bare bodies. Men have no plan yet, and their boots stay | Men need work before they have a choice |

**Checked:** one head's vertex and part counts. I did not read the other 29. **Assumed:** the A, B and C families are skin tones, and the others have about the same parts.

## What can be real and what cannot

**A slider on the face is our own deformer, not the game's.** `shape_chest` in [underwear.rs](../crates/kotor-render/src/underwear.rs) already does this for the chest: a smooth bump, given in metres, moves vertices when the model loads. Faces work the same way: find the brow, nose, jaw, cheeks, eyes and mouth by where they are on the head, and move those vertices. This needs no art, but three limits stay:

- With 990 vertices, shapes are broad. A nose can be longer or wider, but not finely sculpted.
- The texture moves with the vertices, so the painted features stretch with them. Big moves will look wrong, so the ranges stay small.
- The 30 heads differ. Each slider must be checked on all 30, and some need their own limits.

**Hair styles need new art.** There is no way round this. Our custom mesh pipeline, on the Feedback page's Custom content rows, already takes `.glb` garments, so hair and beards can be `.glb` pieces held by the head. But the head's own hair stays painted and modelled under them, so a short style over long painted hair needs a cut-down head too. Hair colour can be done without art, by shifting the colour of the hair in the texture, if the hair can be told from skin in all 30 textures. I have not tried that yet.

**Scars, tattoos and make-up can be projected.** Underwear already paints a shape given in metres onto a texture when it loads. The same method can put a mark, given as a small picture in head space, onto any head. That needs one small picture per mark, and none per head.

## Stages

| Stage | What the player gets | Size | Art needed |
|---|---|---|---|
| 0. The page | A new appearance page: a list of rows on the left, the figure on the right. The look is kept in `CharacterSpec` and in saves | 500 | None |
| 1. Preview | Turn the figure by dragging. Head, upper body and full body views. A choice of idle poses. A lighting switch | 350 | None |
| 2. Body | Sex and small, medium or large, as now. Height. Shoulders, chest, waist, hips, arms and legs sliders. Skin tone | 900 | None |
| 3. Head | The 30 heads as a list. Eye colour. Brow, nose, jaw, cheeks, eyes and mouth sliders | 900 | None |
| 4. Marks | Scars, tattoos and make-up, projected, each with a strength | 400 | 8 to 12 small pictures |
| 5. Underwear | Standard, the pack's and None, for each sex | 300, plus men's plan 400 | None, once the pack is integrated |
| 6. Hair | Hair colour first. Then hair and beard pieces | 300, then 500 | Colour: none. Pieces: 6 to 10 hair and 3 to 5 beards for each sex, as `.glb`, with cut-down heads |
| 7. Presets | Save and load looks in a file beside the settings. Random. Reset | 250 | None |
| 8. Portrait | The HUD's picture made from the face, so it matches | 300 | None |

That is about 4,700 lines before the hair pieces' art. Stages 0 to 3 are the heart of it, about 2,650 lines. Each stage goes in as its own tip, with tests and frames.

**Order:** 0 and 1 first, as everything is seen through them. Then 2, 3, 7, 5, 4, 8 and 6. Hair is last because it depends on art.

## Built

**Stage 0.** Custom's first step is now Appearance, on the Options frame, with the whole body on the right. Its rows are Head, Body and, for a female body, Underwear. Each has two arrows, and goes on the figure at once. OK keeps it, and Cancel puts back what the page found. Quick Character keeps the game's portrait page. `CharacterLook` holds the sliders by name, in `CharacterSpec` and on the player in saves. No slider exists yet, so it is empty.

**Stage 1.** A drag across the figure turns it, with the mouse or a finger, whether the touch layer is on or not. Three rows only change how the page shows it, and none is kept with the character:

- **View:** Full body, Upper body or Head. The wheel over the figure goes closer or further too. Each view is measured once on the body at rest, so the picture does not breathe with the idle, and its middle turns with the body.
- **Pose:** Idle 1, 2 and 3 and Listening, the clips `pause1` to `pause3` and `listen`, which every player body has.
- **Lighting:** Standard, Front, Side and Low. The renderer has one lamp, fixed in the world, that lights both faces of a surface alike. So a light turns the figure and camera together under the lamp, and makes the figure brighter or darker. Left and right are the same light.

For a finger, a choice row's arrows answer over the row's whole height and a little beyond their pictures, and a tap on the row's words goes on to the next choice. **Assumed:** the views' sizes, the turn's speed, the lights' names and strengths, and the poses' names. I have not seen a real touch screen run it.

**Stage 2.** Seven rows of sliders on the Appearance page: Height, Shoulders, Chest, Waist, Hips, Arms and Legs, from -100 to +100 in steps of 10, kept in the look by their names (`height` and so on) only when not 0. The list is longer than the page now, and the wheel or a finger's drag goes down it. [shape.rs](../crates/kotor-render/src/shape.rs) moves the vertices of the body at rest by the bones that carry them, once for each look: the arms' and legs' bones push their vertices out from or in to the line down the bone, the shoulders take the arms and the top of the torso out, and the chest, waist and hips are bands of the torso round the skeleton's own heights. It works on every body of the player, bare or in armour, as they share one skeleton, and the head is left as it is. Height scales the whole posed figure about the feet, from 0.92 to 1.08, only for how it looks: doors, walking and fights keep their sizes. In the game the player's body is shaped by the look the save keeps, on the screens' figures too. The page's whole view is framed for the tallest body, so that a taller one is seen to be taller.

**Not built in stage 2:** skin tone. The game ties the skin to the head: the A, B and C heads are its tones. A tone of its own would recolour the body's and head's skin in their textures. **Assumed:** each slider's size at its ends, by eye from frames: shoulders 3.5 cm each side, chest 3 cm forward, waist 16%, hips 12%, arms 25% and legs 20% thicker or thinner. Checked on the bare bodies only, not on each armour.

**Stage 3.** Six more slider rows: Brow, Nose, Jaw, Cheeks, Eyes and Mouth, -100 to +100 as the body's. The heads have no morph targets and no bones of a face to shape by, but each carries the game's own points of its face, the `f_*_g` nodes: the brows, the nostrils, the mouth's middle and corners, the jaw and under each eye. [face.rs](../crates/kotor-render/src/face.rs) moves the head's vertices near those points at rest: the brow forward and up, the nose out and wider, the jaw wider and the chin forward, fading out under the chin so that the neck still meets the body, the cheeks out, the eyes and their lids wider apart, the mouth wider. A face slider brings the view from the whole body to the head. The head row stays the 15 of the chosen sex, which with the other sex's are the 30.

**Not built in stage 3:** eye colour. The eyes are painted in the head's own texture, and a colour of their own would need a texture made at run time for them. **Assumed:** each face slider's size at its ends, a few millimetres, by eye from frames of one head, `PFHA01`. Not checked on the other 29.

**Stage 7.** Four rows after the sliders. Random look rolls every slider, most of them a little way from 0 and some at 0, at most 60 either way, and a head and a body of the chosen sex. Sliders puts every slider back to 0. Save this look keeps the head, the body and the sliders as the next "Look" by number in `looks.txt` beside the settings file, one look a line. Saved look goes through them with its arrows, and each goes on the figure; a look saved with a head of the other sex keeps this head. **Assumed:** the names "Look 1" and so on, as the page has no box to type in, and the random range.

**Stage 5.** A woman's Underwear row has Standard, the packs and folders in the mods folder as before, and last "None (the game's own)": the game's body with its painted top, as decided. Its id, `*none`, cannot be a folder's name. The texture of a body or head that wears it has no paint of ours. It is the player's in the new game, as the other choices are. **Not built:** a man's row. A man's body has no paint of ours and wears the game's own already, and a plan of underwear for men is new work in `underwear.rs`, not begun. **Assumed:** with None, the chest keeps the small shape the standard paint gives it under its cups.

**Stage 4.** Nine rows after the face's sliders, each from None to 100% in steps of 10: three scars (nose, lip, chin), three tattoos (cheeks, brow, chin) and three make-ups (eyes, lips, cheeks). A look keeps them with its sliders, so saved looks and saves keep them too. They need no pictures: [marks.rs](../crates/kotor-render/src/marks.rs) draws each in code, as a line or a soft patch given in metres on the face at rest, and paints it into the head's texture as it loads. A head with marks has its texture's name followed by `#` and the marks, so each set is painted once. A mark's arrow brings the view to the head.

**Found in stage 4:** the heads' textures are mirrored, so one half of the picture paints both sides of the face. A scar on one side shows on both. So every mark is the same on both sides, or on the middle line. The game's points of the face are not all at the features: the jaw's is its hinge, and the mouth's middle is above the lips. The marks go by the brows, the points under the eyes and the mouth's corners. **Assumed:** each mark's shape, place and colour, by eye from frames of `PFHA01`. Not checked on the other 29 heads.

**Stage 8.** A player whose face sliders or marks are moved gets a portrait made from their face. The game's window draws the head once, off the screen with the figure alone, and the stage keeps the square in the middle of that picture, 128 pixels across, as `kotorrs_face_` and a number from the face. The HUD, the character sheet and the other screens that show the player's picture use it from then on. A changed face is drawn again. Body sliders alone keep the game's portrait, as a portrait does not show them. Until the window has drawn it, and in tests with no window, the game's portrait stands in. **Assumed:** the camera's view, the head's turn and the picture's size, by eye. The light is the renderer's one lamp, so the picture is darker than the game's painted portraits.

**Stage 6, hair colour.** A Hair colour row after Body and Underwear: the head's own, Black, Dark brown, Brown, Auburn, Red, Blonde, Platinum, Grey and White. A look keeps it as `hair_colour`, a place in that list, and it goes on the head's texture with the marks. A texel is hair when its colour is nearer the crown of the head than the cheeks, by hue and light, on the top and back of the head and on the brows. Each keeps how light it is against the hair's mean, so the strands keep their shading. Its arrows bring the view to the head. Checked on four heads by frames. **Known:** a see-through fringe hanging below the brows keeps its own colour, as hair is not looked for below the brows, to keep the eyes' lashes. **Not built:** hair and beard pieces, which need new art.

**Portraits as presets.** The owner: "Can we make the portraits themselves just presets and then allow for full facial customisation as well?" The Head row is now Preset, "Portrait 3 of 15". Its arrows load that portrait's head as the game made it: the face's sliders, marks and hair colour go back to 0, and the body's sliders stay. Every row then changes it, and Save this look keeps the whole look. The portrait on the creation sheet and in the game is the one made from the face, once the window has drawn it. **Assumed:** a game portrait's preset is its head with nothing changed, as the game's portraits have no shape of their own. The playable races are planned in [CHARGEN_RACES.md](CHARGEN_RACES.md).

## What each stage must not break

- **Armour.** Body sliders move the vertices of whatever the player wears. Each armour model must be checked, or clothing will clip. Height scales the whole figure, and only for how it looks: doors, walking and fights keep their sizes.
- **The neck.** `close_neck_gap` joins head to body. Skin tone and head sliders must keep that join.
- **Speed.** Reshaping happens when a model loads. A slider being dragged reshapes only the figure on the page, and must keep the page at full frame rate.
- **Saves.** Old saves have no look and show the game's defaults. The look lives only on the player, not on party members or other characters.
- **The repo.** No game files, and nothing from the pack. Our own art goes under `assets/ours`. The pack is read at run time from `Mods`, as the [animation session](#underwear) will do it.

## Underwear

The animation session answered on 2026-11-07. The integration is **not built yet**, and nothing of it is on master:

- It will add entries to `list_underwear_variants()`, and that call stays as it is.
- A choice is applied by `World::set_player_underwear(id)`. An empty id is ours.
- The pack is **textures only**: 32 TGA files on the same layout as the stock bodies, so no meshes. The owner's "underwear meshes" would be new art, by the `.glb` route above.
- Ours fits the three female bodies with every skin. Men's bodies have no plan yet.
- "None" is not defined yet. See question 3.

## Decided

Top Dog gave these on 2026-11-07, under the owner's standing order to take the recommendations. The owner may overrule any of them.

1. **Hair pieces:** new art, but last. Hair colour comes first.
2. **Quick Character:** it keeps its short path. Custom has the new page.
3. **"None" for underwear:** the game's own painted top, the stock body with no change. There is no bare body in the game's textures, and none will be made.
4. **Changing a look:** only at creation, as in the game.
5. **The panel's look:** the Options pages' frame and sliders, with a big figure on the right.
6. **"Underwear choice":** texture variants of our built-in underwear, as the pack has no meshes. New meshes would be new art, if the owner wants them.
