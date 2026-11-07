# Plan: a detailed character creation

The owner: "Or just have selectable 'underwear' meshes in the character creation screen. I want the character creation to be complex and detailed like the skyrim one".

This is a plan. No code for it exists yet. Sizes are my estimates of lines of Rust with tests, not measurements. It builds on [CHARGEN.md](CHARGEN.md).

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
