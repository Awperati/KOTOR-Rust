# Modern combat: design and size

Design only. No code for this exists yet. All sizes and numbers for new work are my guesses, until a stage measures them.

The job, as Top Dog relayed the owner's addition to the PRD: two combat modes, Classic and Modern. Modern has a crosshair, right-click over-the-shoulder aim, Force Unleashed-style sabre combat, and every Force power keybindable.

**In short.** Classic stays as it is. Modern changes how the player's own attacks start and land. Enemies, party members, scripts and the rules' numbers stay. Stages 0 to 6 come to about 2,400 lines. Only stage 7 needs a video from the owner.

## What exists now

| Part | What it does today | Where |
| --- | --- | --- |
| Camera | Orbits the player, looking at a point 1.2 m up. Field of view 55. It stops at walls, ceilings and closed doors. | [stage.rs](../gpu/src/stage.rs) `camera`, [moving.rs](../gpu/src/moving.rs) `arm_tick` |
| Right button | Hold and drag to turn the camera. The Controls page can swap the two buttons. | `Stage::look`, [viewer.rs](../gpu/examples/viewer.rs) `look_button` |
| Stepping aside | The camera can already step 0.35 or 0.7 m to one side, to see past a wall. | `camera_side`, `ARM_SIDES` |
| Left click | Uses or attacks what is under the cursor, or walks to the floor. | `Stage::click` |
| Finding a target | A ray from the cursor against one sphere per object: 0.7 m radius, 0.9 m up on a creature. It ignores walls. It only sees objects marked useable. | [play.rs](../crates/kotor-game/src/play.rs) `pick`, `pick_sphere` |
| B, Attack mode | A switch for clicks and the law. Off, a click on someone talks. On, it attacks. It is not a combat mode. | [law.rs](../crates/kotor-game/src/law.rs), [LAW.md](LAW.md) |
| The fight | One attack per creature in each 3 s round: d20 against Defense, then dice. The round length is measured from the reference video. | [combat.rs](../crates/kotor-game/src/combat.rs), [COMBAT.md](COMBAT.md) |
| Clips | Follow the fight's log. A sabre attack always plays the first clip of its stance. | [game.rs](../crates/kotor-game/src/game.rs) `style_clip` |
| Quick slots | Three slots with arrows. A click uses a thrown item or a power on the player. | [fightscreen.rs](../gpu/src/fightscreen.rs) `use_quick_slot` |
| Target slots | Three slots under an enemy's bar: feats, powers that hurt, things to throw. | [combat_actions.rs](../crates/kotor-game/src/combat_actions.rs) |
| Action pop-up | None by that name. I took it to mean the target slots and the first action slot. | `Stage::click`, `BTN_ACTION0` |
| Powers | 42 rows of `spells.2da`, read by the `powers` example. Each runs the game's own script through `cast_spell`. | [force.rs](../crates/kotor-game/src/force.rs) |
| Key page | 28 actions in a fixed list, each with one key. A key in use swaps. Three page buttons, and the third, Mini Games, is greyed. | [keyscreen.rs](../gpu/src/keyscreen.rs) |
| Touch | A stick and round buttons, with Attack on a button. | [touchscreen.rs](../gpu/src/touchscreen.rs) |

The player's sabre clips, read from the game's models with a scratch tool:

| Clips | How many, per stance 2, 3 and 4 | Used now |
| --- | --- | --- |
| `c_a1` to `c_a5`, about 1.47 s each | 5 | Only `c_a1` |
| `c_a6`, 3.0 s | 1 | No |
| `c_d1` to `c_d5`, `c_p1` to `c_p5` | 10 | No |
| `f_a` feat attacks | 2 in stance 2, 4 in stances 3 and 4 | Not checked |
| `g_a1`, `g_a2`, `m_a1`, `m_a2` | 2 of each | Not checked |
| `throwsab`, `throwsablp` | One pair | Not checked |

The names suggest `d` and `p` are the other side of the five attacks. I did not check that they line up.

## What stays as it is

- Classic is the default and does not change.
- Modern combat needs Modern movement, so the setting turns it on. Classic movement has no body to lunge.
- Enemies and party members keep their 3 s rounds and the game's own AI scripts.
- Defense, feats, saves, Force points and each power's script keep their numbers.
- Cutscenes, conversations, the race and the turret take the camera and the keys first, as now.
- Force Unleashed describes the feel only. No names, moves, art or sound are copied.

## What Modern needs

### Crosshair and aim camera

- Holding right is aim. It still turns the camera with the mouse, so nothing new has to be learned.
- While aiming, the camera comes in over the right shoulder, the field of view narrows, and a crosshair shows. The body turns to face the aim. `camera_side`, `fov_extra` and `camera_arm` carry most of this.
- The cursor is hidden and held still while aiming. The turret already hides the cursor. Holding it still is new, and only the real window can show it.
- The HUD needs the cursor, so it comes back when right is let go.

### Ray-cast hits

- A shot takes a ray through the crosshair, then checks from the gun to that point. The second check stops a shot going through a wall beside the shoulder.
- `pick` cannot do this. It ignores walls, and it misses anyone not marked useable. A new `aim_hit` uses a body-shaped capsule, and `can_see_along` for walls and closed doors.
- Each click fires one shot, no faster than the weapon's clip. `volley.rs` has measured gaps of about 0.5 s.
- A shot enters `resolve_hand`, so feats, gear and criticals keep working. The log still drives the clips and sounds.
- The range is no longer capped at 12 m. A limit of 30 m is my guess.
- With B off, a shot passes neutrals harmlessly. With B on, it hits them and the law reacts. Party members are never hit.

### Aim sway

- The crosshair drifts in a slow pattern, by clock and not by dice, so tests can repeat it.
- Standing still drifts least. Walking, sprinting and just having fired drift more.
- Rifle, pistol and heavy each get a size. Blaster feats shrink it.
- The shot follows the drift you see. It adds no hidden spread.

### Keys for 42 powers

- The list gets 42 rows on a new Force page. The greyed Mini Games button is the natural place, since no minigame uses it.
- Names come from the talk table through `spells.2da`. Old settings files keep working, because unknown keys are already ignored.
- **The catch: only nine letters are free.** They are G, H, N, P, Q, R, T, Y and Z. So rows start empty, which the page cannot do today, and Alt plus a key becomes allowed.
- A pressed power goes to the lock-on target, else the crosshair's target, else the player for a self power.
- A power that fails for want of Force says nothing today. It needs a sound or a notice, or a key press feels broken.
- The key works in Modern only, unless the owner wants both.

### Sabre combos

- Left click with a sabre in hand is a blow. Clicks in time chain `c_a1` to `c_a5`, so the first version needs no new art.
- Those clips last 1.47 s, which is too slow. They play about twice as fast and cut to the next at the moment of the blow. That moment is read from the clip's motion, as `volley.rs` did for shots.
- A blow lunges the body a little through `movement.rs`, and catches everyone in a 2 m arc in front.
- Rolls stay. A blow hits, misses or is a critical as now.
- `g_g1`, the flinch, lasts 1.47 s, so a chain needs a shorter stagger, or the enemy freezes through it.
- Guard and bolt deflection use `c_p` clips. Blaster bolts are not simulated objects, so a deflect is a failed enemy roll with a bolt drawn going back.

**Balance is the biggest risk.** One blow in 0.5 s is six times the rate of one attack in 3 s. So a blow's damage has to come down. A test logs time to kill in both modes, and the owner sets the target. Every number goes in one tuning table.

### Lock-on

- A key picks the nearest enemy in view near the crosshair. The same key lets go, and the wheel moves to the next. `seen_enemy` and `hover_target` give a start.
- While locked, the camera eases to keep the enemy in frame, and the body strafes around them. Omnidirectional movement already allows that.
- Blows lunge at the lock-on target, and powers go to it.
- Lock-on ends when the enemy dies, is out of sight for a moment, or is far away.
- Aiming wins over the lock while right is held.

## Staged plan

Each stage is behind the setting, passes the gate on its own, and adds frames for the owner. All tests run hidden.

| Stage | What | Who | Lines | Done when |
| --- | --- | --- | --- | --- |
| 0 | The Classic or Modern setting on Gameplay, saved in settings | Menus | 80 | Classic is identical in the existing tests |
| 1 | Aim camera, crosshair, held cursor | Movement | 350 | Frames from both sides. It holds at walls and doors |
| 2 | Shots by ray, walls and doors stop them, B and the law | Combat | 450 | A shot at a wall hits nothing. A clean shot logs an attack |
| 3 | Sway | Combat | 120 | Drift grows with speed, and the same clock gives the same path |
| 4 | Force page, empty rows, Alt keys, power goes to its target, failure notice | Menus and Combat | 450 | All 42 bind, cast and save. Old settings files still load |
| 5 | Lock-on | Movement | 250 | Lock, switch and lose by rules above, in a real fight |
| 6 | Sabre combos, arcs, lunges, stagger, guard, the tuning table | Combat and Animation | 700 | Five-step chain from existing clips, time to kill in the agreed range |
| 7 | Force Unleashed feel: new Blender clips, launches, holding and throwing enemies and objects | All three | Not sized | After the owner's video and stage 6's feel |

The line counts are guesses. Stages 0 to 6 add up to 2,400 lines, against 1,098 in `combat.rs` today.

Stage 7 cannot be sized yet. Enemies have no body that can leave the floor, and the engine has no physics for thrown objects. `Shove` in `force.rs` only slides a creature along the floor.

Touch is out of these stages. The Attack button keeps working as it does now.

## Decisions for the owner

Each has my recommendation, and the build follows it until told otherwise.

1. **Default for new and old games?** Classic.
2. **Does an aimed shot still roll?** Yes, but a crosshair on the body never misses outright: the roll gives a graze at half damage, a hit, or a critical. Defense then still counts.
3. **Crosshair only while right is held?** Yes. The HUD needs the cursor the rest of the time.
4. **Does right with a sabre also aim?** Yes, for powers and sabre throw. Left is the blow.
5. **Does aiming turn on outside a fight?** Only with a weapon drawn or B on. Otherwise right stays the plain camera turn.
6. **Force keys in Classic too?** No, Modern only.
7. **Pause in Modern?** X still pauses, with no queue of actions.

## Reference video

Stages 0 to 6 need none. KotOR has no such mode to measure, and the owner's frames can judge them.

Stage 7 needs one thing: a clip of a minute or two showing the sabre combat the owner means. Without it, "the feel" would be my guess. I will ask through Top Dog when stage 6 is done.

## What I checked and assumed

- **Read in the code:** every row of the first table, the key list, and the clip names and lengths from the game's models.
- **Not checked:** whether the key page's list scrolls to 70 rows, how the `f`, `m` and `g` attack clips look, or whether attack and parry clips match.
- **Assumed:** every new number here, and the meaning of "action pop-up".
