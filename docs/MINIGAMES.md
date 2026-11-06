# Minigames: a brief for the job

Nothing here is built. The swoop race stops the Taris story for anyone, as `tar_m03af`'s `k_ptar_swooprun` leads to `tar_m03mg` and I found no other way on. See `docs/GAPS.md`, number 1.

## What exists in the install

Four modules hold a minigame, in the `MiniGame` struct of the area's `.are`. This is everything found over the 117 modules.

| Module | Area | `Type` | Scripts in the module | What it is |
|---|---|---|---|---|
| `tar_m03mg` | `m03mg` | 1 | 7 | Taris swoop race |
| `manm26mg` | `m26mg` | 1 | 5 | Manaan swoop race, by the name |
| `tat_m17mg` | `m17mg` | 1 | 6 | Tatooine swoop race, by the name |
| `M12ab` | `m12ab` | 2 | 12 | The Ebon Hawk's turret, by its models |

The reference video's race is at about 14,700 s, 4:05, which is the place to measure feel from.

## The data a game needs

From `MiniGame` in the `.are`, as read from `m12ab` and `m26mg`:

- `Type`, `Near_Clip`, `Far_Clip`, `CameraViewAngle`, `Bump_Plane`, `UseInertia`, `DoBumping`, `DOF`, `MovementPerSec`, `LateralAccel`, `Music`.
- `Mouse`: `AxisX`, `AxisY`, `FlipAxisX`, `FlipAxisY`, for the turret.
- `Player`: `Track`, a model such as `m12ab_mgt01`, `Num_Loops`, `Sphere_Radius`, `Invince_Period`, `Hit_Points`, `Max_HPs`, `Bump_Damage`, `Sounds`, `Models`.
- Past that the struct goes on, with lists I have not read: enemies, gun banks and obstacles, from the names of the routines below.

## The script side

`nwscript.nss` has 97 `SWMG_` routines, and our code answers none by name. They fall into these groups:

| Group | Examples |
|---|---|
| Events | `GetLastEvent`, `OnBulletHit`, `OnObstacleHit`, `OnDeath`, `OnDamage` |
| Objects | `GetPlayer`, `GetEnemy`, `GetEnemyCount`, `GetObstacle`, `IsFollower`, `IsObstacle` |
| Player | `GetPlayerSpeed`, `SetPlayerMaxSpeed`, `GetPlayerTunnelPos`, `SetPlayerOrigin`, `GetPlayerOffset` |
| Guns | `GetGunBankCount`, `GetGunBankDamage`, `GetGunBankTimeBetweenShots`, `SetGunBankTarget`, `GetGunBankInaccuracy` |
| Health | `GetHitPoints`, `SetMaxHitPoints`, `GetIsInvulnerable`, `StartInvulnerability`, `AdjustFollowerHitPoints` |
| Sound and camera | `SetSoundVolume`, `GetCameraFarClip`, `SetSpeedBlurEffect`, `PlayAnimation` |

Taris's story reads `TAR_SWOOP_ACCEL` around the race, in `k_ptar_swooprun`. How a race result gets back to the story is not read yet.

## Suggested order

1. Parse the `MiniGame` struct whole, into `kotor-game`, with a test over all four.
2. A headless race: the player along its `Track`, obstacles by their models, a finish time, with no drawing. The pilot can then pass through it, which opens `tar_m08aa` and the way off Taris for testing.
3. Draw it in `gpu`: the track model, a chase camera from those clip and angle numbers, speed blur.
4. The turret, `Type` 2, with the mouse axes and gun banks.

The pilot needs step 2 only. Everything past the race is unreached until then.

## What I do not know

How the game scores a race, what `DOF` means, and what `Bump_Plane` numbers mean. `Type` 1 as a swoop and `Type` 2 as a turret come from one area each and the models' names.
