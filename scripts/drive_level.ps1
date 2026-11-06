# A level earned in the running window: the pilot plays until one kill short of it, and the player makes that kill.
# Like drive_window.ps1 it posts messages to the viewer's own handle only, and holds only at 1x: see docs/FASTTEST.md.
param([string]$Shots)
# Run from the repo root after building the viewer example. -Shots is a folder for pictures of the window.
# There is no --xp: all the experience comes from kills and scripts. The pilot's player is far along for its level, so it
# is kept standing. It writes no saves.
. "$PSScriptRoot\drive_common.ps1"
$script:stateFile = "$scratch\kotor_level.out"
$v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList @("--mute","--no-tutorials","--no-auto-pause","--state-log","--autopilot","--pilot-until","level","--frames","2000000") -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_level.err" -PassThru -NoNewWindow
for ($i = 0; $i -lt 600 -and $v.MainWindowTitle -notlike "kotor-rs*"; $i++) { Start-Sleep -Milliseconds 50; $v.Refresh() }
$h = $v.MainWindowHandle
"window $h"
$before = [Drv]::GetForegroundWindow()
$KEY_C = @(0x43, 0x2E)
function ToPixel($s, [string]$place) {
  $p = $place -split ','
  @([int]([double]$p[0] * 1280 / [double]$s.width), [int]([double]$p[1] * 1.5))
}

WaitLog
$until = (Get-Date).AddSeconds(300)
do { $s = State } while ($s.pilot -ne "false" -and (Get-Date) -lt $until)
Report "The pilot hands over" ($s.pilot -eq "false") "pilot=$($s.pilot) module=$($s.module) level=$($s.level) xp=$($s.xp) hp=$($s.hp)"
$start = $s
Shot "level_handed_over"

# Find the enemy, and attack until a kill pays. Whoever of the party lands it, the player is paid.
$until = (Get-Date).AddSeconds(150)
do {
  $s = State
  if ($s.engaged -ne "true") {
    $x = if ($s.foe -ne "-") { [double]($s.foe -split ',')[0] } else { -1 }
    if ($x -lt [double]$s.width / 3 -or $x -gt [double]$s.width * 2 / 3) {
      Key 0x25 0x14B 120
    } else {
      $p = ToPixel $s $s.foe
      Click $p[0] $p[1]
      Start-Sleep -Milliseconds 100
      $s = State
      if ($s.target -ne "-" -and $s.engaged -ne "true") { Click $p[0] $p[1] }
    }
  }
  Start-Sleep -Milliseconds 100
} while ([int]$s.xp -le [int]$start.xp -and $s.dlg -eq "false" -and (Get-Date) -lt $until)
Report "A kill pays" ([int]$s.xp -gt [int]$start.xp) "xp $($start.xp) -> $($s.xp), player's attacks $($start.attacks) -> $($s.attacks) dlg=$($s.dlg)"
Report "The level waits" ($s.level -eq $start.level) "level=$($s.level)"
Shot "level_kill"
for ($i = 0; $i -lt 60 -and $s.dlg -eq "true"; $i++) { Key 0x0D 0x1C; Key 0x31 0x02; $s = State }

# The sheet offers it, and the panel takes it.
Key $KEY_C[0] $KEY_C[1]
$s = WaitFor "menu" "character"
Report "C opens the character" ($s.menu -eq "character") "menu=$($s.menu)"
Shot "level_sheet"
Press 126 267 127 40   # BTN_LEVELUP
$s = WaitFor "menu" "levelup"
Report "Level Up opens the panel" ($s.menu -eq "levelup") "menu=$($s.menu)"
Press 377 177 195 31   # BTN_STEPNAME2
$s = WaitFor "menu" "level_skills"
Press 113 410 138 28   # BTN_RECOMMENDED
[void](State)
Press 253 410 134 28   # BTN_ACCEPT
$s = WaitFor "menu" "levelup"
# Not every level has a feat to choose.
Press 377 209 195 31   # BTN_STEPNAME3
Start-Sleep -Milliseconds 300
$s = State
if ($s.menu -eq "level_feats") {
  Press 100 410 146 28   # BTN_RECOMMENDED
  [void](State)
  Press 247 410 146 28   # BTN_ACCEPT
  $s = WaitFor "menu" "levelup"
}
"feats step: $($s.menu)"
Press 377 271 195 31   # BTN_STEPNAME5
$s = WaitFor "menu" "character"
Report "Accept gives the level" ($s.menu -eq "character" -and [int]$s.level -eq [int]$start.level + 1) "menu=$($s.menu) level $($start.level) -> $($s.level)"
Shot "level_done"

$during = [Drv]::GetForegroundWindow()
Report "Never took focus" ($during -ne $h) "foreground before=$before now=$during viewer=$h"
Stop-Process -Id $v.Id -Force
Get-Content "$scratch\kotor_level.err" -Tail 4
if ($script:failed) { exit 1 }
exit 0
