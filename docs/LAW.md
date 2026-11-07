# Attack mode and the law: what you can change

All of it is in one file, [`docs/law.tsv`](law.tsv). Open it in any text editor. Each line is one setting, with its parts separated by
a tab. A line starting with `#` is a note. The game reads the file when it is built, so a change shows after the next build.
Everything in it today is final (7 Nov 2026, "use the recommended values"). Change a value and rebuild to change the game.

## What happens

1. **Attack mode** is the button beside the hourglass (key X). Off, a click on someone talks to them. On, a click attacks them.
2. **A crime** is a blow on someone who is not your enemy. The first blow puts the **wanted level** at 1. A kill adds one more, up to 3.
3. **Who sees it.** An authority (see the list below) who can see you, or who hears a kill within 15 m, turns on you, and so do its
   authority mates within 10 m. Anyone else who sees it runs away and tells the nearest authority within 40 m, who turns on you.
4. **It dies down.** An authority who has not seen you for 60 s since the crime stands down. When none are left, the level is 0.
   An authority who is killed is gone from the list at once.
5. **While you are wanted at level 2 or more**, shops are shut and creatures refuse ordinary talk. A new area starts clean.
6. **What a kill costs** in dark side points, as the `kill` lines say: an unarmed civilian 10, an armed or authority creature that was not
   your enemy 5, a fleeing enemy 2, an enemy 0. A creature a plot needs cannot be hurt at all. **Only a tenth of that counts**
   (`setting kill_alignment_scale 0.1`): alignment is slow to move, so ten civilians cost what one used to. A part of a point is kept
   and added to the next. The game's own shifts (a dialogue choice, a quest, the Trask question) are never scaled.
7. **Someone with a conversation of their own** who is killed (and was not your enemy) leaves a finished line in the journal,
   "Lost: name", and your party members who are alive say they disapprove of a kill that costs 5 or more.

## The lines

| Line | Meaning |
|---|---|
| `setting hear_metres 15` | how far a kill is heard, through walls |
| `setting call_metres 10` | how far an authority calls its mates |
| `setting call_far_metres 40` | how far a civilian's call for help reaches |
| `setting calm_seconds 60` | how long until they stand down |
| `setting flee_seconds 10` | how long a civilian who saw a crime runs |
| `setting shut_from 2` | the wanted level from which shops shut and talk is refused |
| `setting kill_alignment_scale 0.1` | the share of a kill's dark side points that count; 1 is the whole of them |
| `kill innocent 10` and the like | the dark side points of each kind of kill |
| `authority tar_ sithpat` | in a module whose name starts `tar_`, a creature whose template name contains `sithpat` is an authority |

## Which creatures are authorities

The game's factions are only Neutral, Friendly and Hostile, so the table goes by the creatures' template names. The names of
every creature in every module are listed by `cargo run --release -p kotor-game --example law_survey`. The first guesses are by
planet: Taris Sith patrols and guards, Tatooine Czerka and the Sandral guard, Dantooine Jedi and militia, Manaan Selkath and
security, Kashyyyk Wookiee defenders, Korriban Sith students and port authority, the Endar Spire Republic soldiers, the Leviathan Sith.
The list is final too. To add a creature, add one `authority` line.

## Extended: how the law reaches shops, talk and quests

- **Shops.** `open_store` checks the wanted level first. At the level in `shut_from` (2) or more the shop stays closed and the
  event log says "The shop is shut: the law is after you". At level 1 it still trades.
- **Talk.** A click on a creature that is not on your team says "<name> will not talk to you" at the same level. Attack mode
  skips this check, because a click then attacks.
- **Quest givers.** Killing a creature with a conversation of its own that was not your enemy writes "Lost: name" in the journal as a
  finished line. A party member who is alive says it disapproves when the kill costs 5 or more.
- **Saves.** `attack_mode`, the wanted state and `lost_quests` are the menus builder's to save. `lost_quests` was asked for on 7 Nov.
- **Tests.** [`tests/law.rs`](../crates/kotor-game/tests/law.rs) covers each case in the Taris Upper City, with the real install.

## What it does to a run

| A run with | Before the scale | Now |
|---|---|---|
| 20 enemies killed | 0 points | 0 points |
| 5 civilians killed | 50 points: light to dark in five kills | 5 points |
| 10 armed bystanders killed | 50 points | 5 points |
| a scripted shift of 10 | 10 points | 10 points |

The scripted shifts decide the story's gates (Bastila, the Jedi class, the endings, item limits), and they are as the game gives them,
so those gates are reached as before.
