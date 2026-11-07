# The swoop race in the running window: the lights, Space shifting, the keys steering, the finish and what comes after.
# Like drive_window.ps1 it posts messages to the viewer's own handle only, and holds only at 1x: see docs/FASTTEST.md.
param([string]$Size = "1280x720", [string]$Shots)
# Run from the repo root after building the viewer example. -Shots is a folder for pictures of the window.
. "$PSScriptRoot\drive_common.ps1"
$script:ClickScale = [double]($Size -split "x")[1] / 720.0
$script:stateFile = "$scratch\kotor_race.out"
$v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList @("--size",$Size,"--mute","--no-tutorials","--no-auto-pause","--state-log","--skip-menu","--module","tar_m03mg","--frames","400000") -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_race.err" -PassThru -NoNewWindow
for ($i = 0; $i -lt 600 -and $v.MainWindowTitle -notlike "kotor-rs*"; $i++) { Start-Sleep -Milliseconds 50; $v.Refresh() }
$h = $v.MainWindowHandle; [Drv]::NeverActive($h)
"window $h"
$SPACE = @(0x20, 0x39); $D = @(0x44, 0x20); $A = @(0x41, 0x1E)
# speed,gear,x,y,time
function Race { $s = State; if ($s.race -and $s.race -ne "-") { $p = $s.race -split ','; @{ speed = [double]$p[0]; gear = [int]$p[1]; x = [double]$p[2]; y = [double]$p[3]; on = $true; s = $s } } else { @{ on = $false; s = $s } } }

WaitLog
$r = Race
Report "The race opens with its lights" ($r.on -and $r.speed -eq 0 -and $r.gear -lt 0) "race=$($r.s.race) module=$($r.s.module)"
Shot "race_lights"
# A shift before Go moves nothing.
Key $SPACE[0] $SPACE[1]
$r = Race
Report "Space before Go does not start it" ($r.speed -eq 0) "race=$($r.s.race)"
$until = (Get-Date).AddSeconds(10)
do { $r = Race } while ($r.gear -lt 0 -and (Get-Date) -lt $until)
Report "The lights count down to gear 0" ($r.gear -eq 0) "race=$($r.s.race)"
Key $SPACE[0] $SPACE[1]
$until = (Get-Date).AddSeconds(3)
do { $r = Race } while ($r.speed -le 5 -and (Get-Date) -lt $until)
Report "Space at Go sets off" ($r.gear -eq 1 -and $r.speed -gt 5) "race=$($r.s.race)"
Shot "race_gear1"
# Space again when the meter is near full. Only above 60 does it shift.
Key $SPACE[0] $SPACE[1]
$r = Race
Report "A shift too soon is refused" ($r.gear -eq 1 -or $r.speed -gt 60) "race=$($r.s.race)"
$until = (Get-Date).AddSeconds(8)
do { $r = Race } while ($r.speed -lt 65 -and (Get-Date) -lt $until)
Key $SPACE[0] $SPACE[1]
$r = Race
Report "A shift at speed takes" ($r.gear -eq 2) "race=$($r.s.race)"
# Hold D: it goes right. Hold A: back.
$x0 = $r.x
[void][Drv]::PostMessage($h, 0x100, [IntPtr]$D[0], [IntPtr](1 -bor ($D[1] -shl 16)))
Start-Sleep -Milliseconds 400
$r = Race
$up = [long]0xC0000001 -bor ([long]$D[1] -shl 16)
[void][Drv]::PostMessage($h, 0x101, [IntPtr]$D[0], [IntPtr][long]$up)
Report "D steers right" ($r.x -gt $x0 + 1) "from $x0 race=$($r.s.race)"
Shot "race_right"
$x1 = $r.x
[void][Drv]::PostMessage($h, 0x100, [IntPtr]$A[0], [IntPtr](1 -bor ($A[1] -shl 16)))
Start-Sleep -Milliseconds 1500
$r = Race
$up = [long]0xC0000001 -bor ([long]$A[1] -shl 16)
[void][Drv]::PostMessage($h, 0x101, [IntPtr]$A[0], [IntPtr][long]$up)
Report "A steers left" ($r.x -lt $x1) "from $x1 race=$($r.s.race)"
# Shift whenever the meter allows, to the end.
$gearAt = @(0, 0, 105, 156, 217)
$until = (Get-Date).AddSeconds(90)
do {
  $r = Race
  if ($r.on -and $r.gear -ge 2 -and $r.gear -le 4 -and $r.speed -gt $gearAt[$r.gear]) { Key $SPACE[0] $SPACE[1] }
  Start-Sleep -Milliseconds 100
} while ($r.on -and (Get-Date) -lt $until -and $r.gear -lt 6)
Report "The bike reaches the finish" ($r.gear -eq 6) "race=$($r.s.race)"
Shot "race_finish"
$until = (Get-Date).AddSeconds(10)
do { $s = State } while ($s.module -ne "tar_m03af" -and (Get-Date) -lt $until)
Report "The race ends in the swoop module" ($s.module -eq "tar_m03af") "module=$($s.module) race=$($s.race)"
Start-Sleep -Milliseconds 500
Shot "race_after"
FocusReport
Stop-Process -Id $v.Id -Force -ErrorAction SilentlyContinue
if ($script:failed) { "FAILED: $($script:failed)"; exit 1 }
"race drive: all passed"
