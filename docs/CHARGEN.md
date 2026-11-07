# Character creation (kotor-game `chargen.rs`)

The player is no longer a placeholder: `World::create_character(&CharacterSpec)` checks a spec against the tables and turns the player
object into that character; `character_sheet(id)` prints it. `cargo run -p kotor-game --example sheet` prints one per player class.
Tests: `tests/chargen.rs` (6).

| Choice | Checked against |
|---|---|
| Class (Soldier, Scout, Scoundrel) | `classes.2da` `playerclass` = 1 |
| Portrait, which fixes gender, race, head and body | `portraits.2da` `forpc` = 1; `sex`, `race`, `appearancenumber` / `appearance_s` / `appearance_l`; head from `appearance.2da` `normalhead` -> `heads.2da` `head`; body model and texture from `appearance.2da` `modela` / `texa` |
| Attributes | bought from 8 with a pool of 30 points: 1 point a step up to 14, 2 for 15-16, 3 for 17-18; race adjustments from `racialtypes.2da` |
| Skills | level-1 points = 4 x (class base + Intelligence modifier), spent with the class/cross-class costs and caps of `rules.rs` |
| Feats | granted ones from the class list, then the level-1 slots from `featgain.2da`, each checked for list, level, ability and prerequisite feats |
| Hit and Force points | level-1 rules of `rules.rs` (full hit die + Constitution modifier) |

**Evidence for the point schedule:** the three player classes' recommended scores in `classes.2da` each cost exactly 30 points under it (tested).
That is the only check; the 1/2/3 steps and the pool are otherwise my inference.

Not known / assumed: which of `appearance_s`, `appearancenumber`, `appearance_l` is the small, medium and large body (I used the order of their row
numbers); only the first-level build exists (no multiclass, no Jedi class choice, no alignment or voice); the player's name is taken as given.
Class, attributes, hit points, skills, feats, force and the look are saved with the world.

The screens over these rules are `gpu/src/chargenscreen.rs`. Each class's frame is from the reference video at 0:24: Scoundrel small, Scout medium, Soldier large (`BodySize::of_class`). `World::random_name` reads `humanm.ltr`, `humanf.ltr` and `humanl.ltr`; how long a name runs is my guess. Feats in a spec are taken as a level's page takes them (`choose_feat`), so a regular slot holds a feat of either list.

## Underwear

For a female body the Portrait page has a row the game's `portcust.gui` has not got: the variants of
`kotor_render::underwear::list_underwear_variants` that fit, under the picture. Turning it shows the whole body in its
underwear in place of the head. Its Cancel takes the choice back, and New Game puts it on the player and in the saves.
With one variant, or for a male body, there is no row.
