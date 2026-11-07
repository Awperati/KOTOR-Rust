# Moving in the game

How the player moves, in plain numbers. All the keys can be changed: Options, Gameplay, Key Mapping.

## Keys

| Key | What it does |
| --- | --- |
| W A S D | Move, the way the camera looks. Held alone, this is a sprint |
| Shift, held | Walk |
| Space | Jump. In a conversation or a container it goes on instead. In the race it shifts, and on a turret it fires |
| Left Ctrl | Roll |
| X | Pause and go on |
| Right button, dragged | Turn the camera |
| Arrow keys | Turn the camera |
| Wheel | Bring the camera in or out |
| Left click on the floor | Run there |

## Modern and Classic

Options, Gameplay, the Movement box. Modern comes first. Everything below is Modern, unless it says Classic.

Classic is the old walk. It runs at 5.4 m/s, with no speeding up, no falls, no sprint, no roll and no jump. X and the walk on Shift work the same.

## Running

| What | Number |
| --- | --- |
| Walk | 1.7 m/s |
| Sprint | 6.75 m/s, 1.25 times Classic's run of 5.4 m/s |
| Up to a sprint | About a quarter of a second |
| To a stop | About 0.18 s, over 0.6 m |
| Facing the new way | Eases round, at most 18.8 radians a second |
| The way the keys point | Follows the camera, closing in over about 0.1 s |

There is no separate run: a held key goes to a sprint. Going diagonally is no faster than going straight.

## Jump

| What | Number |
| --- | --- |
| Height | 1.1 m |
| In the air | About 0.7 s |
| Steering in the air | 0.3 of what it is on the floor |
| Landing | A short crouch, and the speed is kept |
| Stride | One leg forward, with the other arm forward |
| Paddling | From about 0.35 s in the air: legs and arms change over, and back |

- Only from the floor, on its feet. Not in the air, in a roll, in a conversation, in a minigame or while down.
- Not at a walk: Space does nothing with Shift held and a direction pressed, or with a light push on the touch stick. A run, a sprint and a stand all jump, a stand even with Shift held.
- Space does nothing during a pause, and nothing for half a second after a conversation.
- The legs come out of the run, stride, paddle, and go down into the landing, with no jump between poses.
- The head stops at a low ceiling, and the jump falls from there. A beam or a branch counts as a ceiling.
- A jump does not clear a railing, even one lower than it could reach. It does not land on a higher floor with no walk back.

## Roll

| What | Number |
| --- | --- |
| Distance | 3.2 m |
| Time | 0.55 s |
| Before the next | 0.3 s |
| Speed coming out | 0.7 of a sprint |

A click, a talk or a target during a roll brings it round 2.5 times as fast, and then the player acts. It does not stop dead, because the body would be upside down.

## Falling

| What | Number |
| --- | --- |
| Gravity | 17.7 m/s², a heavy, quick fall |
| Steps walked up | Up to 0.6 m |
| Drops walked down, as a stair | Up to 0.35 m |
| Furthest drop the player falls | 6 m. More than that is a wall |
| Hard landing | From 3.5 m/s: at most 0.45 s, at half speed |

The player does not fall where a railing, a low wall or a fence stands, or where there is no way to walk back. It keeps 0.5 m off a closed door, whose slab is up to 0.35 m thick each way, and may go along it or away.

## Camera

| What | Number |
| --- | --- |
| Distance behind the player | 3.3 m, as far as the room allows |
| Tilt | From 0.2 radians below level to 1.2 above |
| Field of view | 55 degrees, 6 more at a sprint |

- It stops at walls, ceilings and closed doors, and eases in and out. Through a fall and for a second after, only something solid brings it in.
- With rock close behind the player, it rises over it. Under a low ceiling it tries looking from lower down. With something narrow behind, it steps up to 0.7 m to the side.
- From 0.6 m the player's body fades out, with its head and what it holds, and it is gone within 0.25 m. Party members do the same.

## Changing how it looks

The roll, the landing and the jump are poses in text files, which the game reads on F8: [MOVEMENT_POSE.md](MOVEMENT_POSE.md). A clip from Blender named `jump`, `roll` or `landing` takes a file's place: [BLENDER_ANIMATION.md](BLENDER_ANIMATION.md).

## Assumed

Only the walk, the run and the sprint's ratio come from the game or from what was asked. Every other number here is a feel choice, and all of them are in `Tuning` in `crates/kotor-game/src/movement.rs`.
