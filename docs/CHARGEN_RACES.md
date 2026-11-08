# Plan: playable alien races

The owner: "Can we make the portraits themselves just presets and then allow for full facial customisation as well? Include the playable alien races as well".

This is a survey and a design. Portraits as presets, the Twi'lek and the pack's Jawa are built; the rest is not. It builds on [CHARGEN_PLAN.md](CHARGEN_PLAN.md).

## Built: Twi'lek

A Race row first on the Appearance page: Human or Twi'lek. A Twi'lek woman has the game's 4 Twi'lek women's heads as her presets, and a man the 3 men's. The head goes on the player's own body in place of the human head, by [races.rs](../crates/kotor-render/src/races.rs), so the human skeleton, clips and armour stay. The body's skin, bare or in armour, takes the head's tone: the player's own head's cheeks give the tone it was painted in, the Twi'lek head's cheeks the tone it goes to, and texels of that hue are moved, keeping their light ([marks.rs](../crates/kotor-render/src/marks.rs) `tint_skin`). The face's sliders and marks work on the Twi'lek heads, which carry the same face points. Hair colour is not offered, as Twi'leks have no hair. A **Lekku** slider, -100 to +100, makes the head-tails up to 30% longer or shorter: they are the head's `hair` mesh, which grows away from the back of the skull, more the further down it is ([face.rs](../crates/kotor-render/src/face.rs) `grow_lekku`). The Race row going back to Human takes all of it away.

**The Twi'lek pack.** With the mod packs switched on, the K1 HD Female Twi'lek Diversity pack's playable heads join a Twi'lek woman's presets after the game's four: the 41 `P_TwiH*` heads its own player install adds, so 45 in all. Only those models, their pictures and their portraits are read, from the pack's folder at run time. Its characters, bodies and tables are left, as they are for its installer, which changes the game's own files. The heads carry the game's face points, so the sliders and marks work on them, and the body takes each one's tone.

**Assumed:** the skin test's limits, the tails' growth and its top, by eye from frames of all 7 game heads and 9 of the pack's. **Known:** at full length the tails touch the shoulders. A man's bare body keeps a few orange marks on the forearms, which the skin test takes for cloth. Not checked: every armour model on a Twi'lek, and the portrait page of Quick Character, which stays Human.

## Built: the Jawa, from its pack

With the mod packs on and the Playable Jawa pack in the mods folder, the Race row offers **Jawa** after the Twi'lek. Without the pack, the row passes it by, and a saved Jawa look comes back Human. The pack's body is read under a name of its own, `p_jawab`, so the game's own large man stays as he is ([overrides.rs](../crates/kotor-archive/src/overrides.rs) `rename`). The pack's folders carry a download date in their names, so a pack's inner folders are found by the start of their names.

The Jawa wears its body whatever the player has on: robe, hood and bandoliers are its outfit. Its one preset is "Jawa 1 of 1". The Appearance page shows only Race, Preset, the look rows and the view rows, as there is no body size, underwear, face or hair to change. The pack's head is an empty neck with no picture, so it is not used: the hood is the head.

**Eyes.** The pack's hood is empty. The game's own Jawa, `c_jawa`, has an `eyes` mesh, which is added to the body ([races.rs](../crates/kotor-render/src/races.rs) `add_jawa_eyes`). The pack's Jawa is the game's, about 1.46 times larger and leaning a little further forward. So the eyes go to the same place in the hood's box, then up to the middle of the hood's opening, where its chin no longer hides them. Each point of the eyes moves as the nearest point of the hood does.

**Voice.** The pack's 22 lines become `p_jawamw_*`. When the player is a Jawa, a line of a hero's voice set, `p_plyer??_<line>`, is said as `p_jawamw_<line>` where the pack has it, and as the hero's where it does not.

**Checked:** frames of the Appearance page (close and whole, as a man and as a woman), the character sheet, the HUD portrait made from the hood, and the player in the game. A test checks the pack's names, the eyes' place in the posed hood and the voice's names. **Assumed:** the Jawa is offered to women too, as Jawas look alike. It stands as tall as the hero, as the pack made it. **Not checked:** the voice in a fight, by ear; every clip on the Jawa body.

## What the game's files have

I read `appearance.2da` and `heads.2da` from the install on 2026-11-08. I also compared each model's nodes with a human's, using `cargo run -p kotor-model --example bones`. A body's *supermodel* is the model whose animations it plays.

| Race | Appearance rows | Kind of model | Heads | Skeleton and animations | Armour | Face points |
|---|---|---|---|---|---|---|
| Human (now) | the player's rows | `B`: body and head apart | 30 player heads | `S_Female02` (men), `S_Female03` (women) | every armour model | 14 |
| Twi'lek | 47, 48, 269, 270, 365, 366, 387 | `B`, as a human | 4 women's (`twilek_f`, `_f02`, `_f03`, `Twilek_F04`), 3 men's (`twilek_m`, `_m02`, `_m03`) | the human ones: 59 of a man's 61 nodes | its rows name the human `PFBC*` and `PMBC*` armour models, so human armour fits | 14, as a human's |
| Cathar (Juhani only) | Juhani's party rows | `B` | `P_JuhaniH`, one woman | 62 of 64 nodes shared with a human woman's body | Juhani's own bodies | 14 |
| Rodian | 39, 256 to 258, 451 | `F`: one model, body and head together | none apart | `S_Female02`, 53 nodes shared | none: the body is fixed | 13 |
| Duros | 24, 373, 374 | `F` | none apart | `S_Female02`, 51 shared | none | 14 |
| Wookiee | 50, 51, 271 to 278, 327, 427 | `F` | none apart | `S_Female02` and `03`, 51 shared | none | 14 |
| Trandoshan | 452, 453 | `F` | none apart | `S_Female02`, 56 shared | none | 14 |
| Bith | 12 | `F` | none apart | `S_Male02`, another set | none | 14 |
| Aqualish (`N_Walrusman`) | 49, 418 to 420 | `F` | none apart | `S_Male02` | none | 13 |
| Selkath | 40, 259, 260, 323, 335, 336 | `L` | none apart | its own, 17 shared | none | 7 |
| Jawa | 73, 319 | creature | none | its own | none | 0 |
| Ithorian, Gamorrean | creature rows | creature | none | their own | none | 0 |
| Zabrak, Sullustan, Gran | none | | | | | |

**Not in the first game's files at all:** Zabrak, Sullustan and Gran. Making them is new art. A Cathar exists only as Juhani: one woman's head and her own bodies.

## The mods folder

These are read at run time from `Mods`, through the mod packs switch. Nothing goes into the repo.

- **K1 HD Female Twi'lek Diversity and PCs 1.3:** 26 new playable women's Twi'lek heads (`P_TwiH*`, 50 models, 188 textures). It has its own `appearance.2da` and `heads.2da`. Each head comes with a bare body in matching skin, which the mod calls Handmaiden-style underwear. The mod is meant for an installer that patches the game, so we read its files and its `.2da` rows ourselves.
- **Playable Jawa:** `pmbal` and `pmha01` replaced by a Jawa. That is the large man's bare body and the first man's head, so the Jawa is skinned to the human skeleton. It also has 22 player sounds in place of a human man's `P_PLYERMW_*`.

## Design

**The player stays Human for scripts and talk.** Race is how the player looks: their appearance row, heads, skin and voice. The game's scripts test the player's race in places. A cosmetic race keeps every conversation and quest as the game made them. This is the recommended option, assumed.

**A Race row** on the Appearance page, before Head. It sets which heads the Head row goes through, the body's skin, and the voice:

1. **Human:** as now.
2. **Twi'lek:** the human body and armour, with a Twi'lek head. The body's skin takes the head's tone, the way hair colour is done now. The head's skin is sampled from its cheeks, and the body's skin is moved to it. Face sliders, marks and hair colour work, as the heads carry the same 14 face points. A **Lekku** slider stretches the head-tails by our own deformer: they have no bones. With the Twi'lek mod on, its 26 heads join the Head row, with its bare bodies.
3. **Jawa**, with the Jawa pack on: its body in place of the player's, in every outfit, and its sounds as the voice. Built: see above.
4. **Rodian, Duros, Wookiee, Trandoshan:** one model each, which is their body and clothes too. So armour does not show on them, and only height and the marks fit them. They play the human animations. Offered after the races above, marked so.
5. **Bith, Aqualish, Selkath, Ithorian:** other animation sets. Later, after a check of each clip the player needs.

**Voice.** The human player's voice sets for every race but the Jawa. No alien in the game has a player's voice set.

**Armour.** It fits where the skeleton is the human one and the race has a body apart from its head: Human and Twi'lek. The pack's Jawa is one model, robe and all, so it wears that in every armour. Rodian, Duros, Wookiee and Trandoshan do not wear the human armour models. They show their own body in every armour, as the game's characters of those races do.

**Portraits as presets** (the owner's first point). Choosing a head on the Head row loads it as a preset: the head, and the look kept with it. Every row stays editable on top of it, and Save this look still keeps the whole look. The picture follows the face, which stage 8 already does.

## Order

1. Portrait as preset.
2. Twi'lek, both sexes: the Race row, the heads, the body's skin and the Lekku slider.
3. Twi'lek heads from the mod pack.
4. The Jawa from its pack.
5. Rodian, Duros, Wookiee and Trandoshan, each a full model.

**Checked:** the appearance rows, the head rows and the nodes of each model named above. **Assumed:** that a Twi'lek head sits on a human body's neck as a human head does, as their nodes match. Not yet seen in a frame. Also assumed: that the full-model races play the human clips cleanly, as they share most of the bones. Not checked clip by clip.
