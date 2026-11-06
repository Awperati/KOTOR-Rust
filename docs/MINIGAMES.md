# Minigames: a brief for the job

**The turret is built:** `crates/kotor-game/src/minigame.rs` and `gpu/src/turretscreen.rs`, live in `drive_turret.ps1`, compared with the video at 19,533 s.

**Taris's swoop race runs and can be played:** `crates/kotor-game/src/swoop.rs` and `gpu/src/racescreen.rs`, live in `drive_race.ps1`. Space shifts, A and D steer. Manaan's and Tatooine's are the same kind and have not been run. See `docs/GAPS.md`, number 1.

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

Taris's story reads `TAR_SWOOP_ACCEL` around the race, in `k_ptar_swooprun`. A race's result goes back in `TAR_SWOOP_MIN`, `TAR_SWOOP_SEC` and `TAR_SWOOP_MSEC`, and `TAR_SWOOP_RUN` is set. `heartbeat` compares the time with `TAR_SWOOP_*_BEAT`.

## How the race works, as read and measured

Its layout has the track and each pad and buoy, under `trackcount` and `obstaclecount`. The track is `m03mg_TR01` at x 95, whose `modelhook` starts at x 5, height 2, and whose `track` animation runs 4,980 units in 48 s. The bike's speed is that animation's rate, with 100 as its own rate from `MovementPerSec`.

`onfire` holds the gears. Each shift asks for the speed at which the next one starts. `acc` is `TAR_SWOOP_ACCEL` over 10, or 1 at 0.

| Shift to gear | Allowed above | Lowest speed | Highest | Acceleration |
|---|---|---|---|---|
| 1 | Go | 35 | 70 | 20 x acc |
| 2 | 60 | 60 | 120 | 8 x acc |
| 3 | 100 | 100 | 190 | 6 x acc |
| 4 | 150 | 150 | 225 | 3 x acc |
| 5 | 210 | 210 | 420 | 2 x acc |

A pad: speed x 1.05 and acceleration x 1.1, unless in gear 5 or the bike is briefly invulnerable. A buoy: speed x 0.7, unless not yet moving. It finishes past y 3,800.

What the reference video shows, from its timer and meter, 14,681 to 14,705 s:

- The rider finishes in 23.63 s and shifts at about 3.8, 6.6, 10.6 and 16.2 s, with the meter full.
- The meter shows the script's `meter` number plus 2 chevrons, up to the red `Sp` at 9. Against `s / 60 x 8` in gear 1 at 20 a second it agrees at every reading from 0.42 s to 3.62 s.
- **A shift does not lift the bike to its lowest speed.** The meter starts from empty at 0 s, with a lowest speed of 35. That number only stops a bump taking it lower.
- Speeds read off the meter over the race, added up, make 3,663. With the track's 1.0375 units for each point of speed that is 3,800, the finish.

**Assumed, not measured:** the box a bike touches, how it steers across the track, and how a speed above the highest falls.

## What is left of the race

1. Its sounds and speed blur. The floor and the bike are brighter than the video's.
2. Run Manaan's and Tatooine's.

## What I do not know

What `DOF` means, and what `Bump_Plane` numbers mean. `Type` 1 as a swoop and `Type` 2 as a turret come from one area each and the models' names.
