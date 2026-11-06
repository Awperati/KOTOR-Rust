# The Ebon Hawk's turret in the running window: the mouse and keys turn the view, the guns fire, fighters come apart, and the
# story goes on.
# Like drive_window.ps1 it posts messages to the viewer's own handle only, and holds only at 1x: see docs/FASTTEST.md.
param([string]$Shots)
# Run from the repo root after building the viewer example. -Shots is a folder for pictures of the window. It writes no saves.
# --auto-aim points the guns for it: a driver cannot chase a fighter by posted mouse moves.
. "$PSScriptRoot\drive_common.ps1"
$script:stateFile = "$scratch\kotor_turret_hands.out"
$v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList @("--mute","--no-tutorials","--no-auto-pause","--no-early-force","--module","M12ab","--skip-menu","--state-log","--frames","200000") -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_turret_hands.err" -PassThru -NoNewWindow
for ($i = 0; $i -lt 600 -and $v.MainWindowTitle -notlike "kotor-rs*"; $i++) { Start-Sleep -Milliseconds 50; $v.Refresh() }
$h = $v.MainWindowHandle; [Drv]::NeverActive($h)
WaitLog
Hover 640 360
$s = State
$from = [int]($s.turret -split '/')[2]
Hover 740 360
$s = State
$to = [int]($s.turret -split '/')[2]
Report "The mouse turns it, and only by how far it moved" ($to -lt $from -and $from -gt -30) "round $from -> $to"
Shot "turret_turned"
Key 0x41 0x1E 500   # A
$s = State
$back = [int]($s.turret -split '/')[2]
Report "A key turns it back" ($back -gt $to) "round $to -> $back"
FocusReport
Stop-Process -Id $v.Id -Force

$script:stateFile = "$scratch\kotor_turret.out"
$v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList @("--mute","--no-tutorials","--no-auto-pause","--no-early-force","--module","M12ab","--skip-menu","--state-log","--auto-aim","--global","K_KOTOR_MASTER=10,K_TURRET_SKYBOX=5","--frames","200000") -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_turret.err" -PassThru -NoNewWindow
for ($i = 0; $i -lt 600 -and $v.MainWindowTitle -notlike "kotor-rs*"; $i++) { Start-Sleep -Milliseconds 50; $v.Refresh() }
$h = $v.MainWindowHandle; [Drv]::NeverActive($h)
"window $h"
$SPACE = @(0x20, 0x39)

WaitLog
$s = State
Report "The module is the turret" ($s.module -ieq "M12ab" -and $s.turret -ne "-") "module=$($s.module) turret=$($s.turret)"
$first = $s.turret -split '/'
Shot "turret_start"

# A second of the guns at a time, to the last fighter.
$until = (Get-Date).AddSeconds(200)
$shot = $false
do {
  Key $SPACE[0] $SPACE[1] 1000
  $s = State
  if ($s.turret -eq "-") { break }
  $now = $s.turret -split '/'
  if (-not $shot -and [int]$now[1] -lt [int]$first[1]) { Shot "turret_first_kill"; $shot = $true }
} while ([int]$now[1] -gt 0 -and (Get-Date) -lt $until)
Report "Every fighter comes down" ($s.turret -eq "-" -or [int]($s.turret -split '/')[1] -eq 0) "turret=$($s.turret)"
Shot "turret_last"

# The scripts then take the story on.
$until = (Get-Date).AddSeconds(30)
do { $s = State } while ($s.module -ieq "M12ab" -and (Get-Date) -lt $until)
Report "The story goes on" ($s.module -ine "M12ab") "module=$($s.module)"
Shot "turret_after"

FocusReport
Stop-Process -Id $v.Id -Force
Get-Content "$scratch\kotor_turret.err" -Tail 4
if ($script:failed) { exit 1 }
exit 0
