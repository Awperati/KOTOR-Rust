# One fight in the running window: an enemy sighted, the pause, the attack slot, the feat slot, floats and the HUD.
# Like drive_window.ps1 it posts messages to the viewer's own handle only, and holds only at 1x: see docs/FASTTEST.md.
param([string]$Shots)
# Run from the repo root after building the viewer example. -Shots is a folder for pictures of the window.
# The pilot plays up to the first fight at its own speed and hands over when an enemy is seen. It writes no saves.
. "$PSScriptRoot\drive_common.ps1"
$script:stateFile = "$scratch\kotor_fight.out"
$v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList @("--mute","--no-tutorials","--state-log","--autopilot","--pilot-until","fight","--frames","400000") -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_fight.err" -PassThru -NoNewWindow
for ($i = 0; $i -lt 600 -and $v.MainWindowTitle -notlike "kotor-rs*"; $i++) { Start-Sleep -Milliseconds 50; $v.Refresh() }
$h = $v.MainWindowHandle
"window $h"
$before = [Drv]::GetForegroundWindow()
$ENTER = @(0x0D, 0x1C); $SPACE = @(0x20, 0x39); $ONE = @(0x31, 0x02)
# A slot's place in the state line is in layout units, across a width the window's shape gives.
function ToPixel($s, [string]$place) {
  $p = $place -split ','
  @([int]([double]$p[0] * 1280 / [double]$s.width), [int]([double]$p[1] * 1.5))
}
function ClickSlot($s, [string]$slot) { $p = ToPixel $s $s[$slot]; Click $p[0] $p[1] }
function HoverLayout($s, [string]$place) { $p = ToPixel $s $place; Hover $p[0] $p[1] }

WaitLog
# The pilot's run takes it there at its own speed.
$until = (Get-Date).AddSeconds(180)
do { $s = State } while ($s.pilot -ne "false" -and (Get-Date) -lt $until)
Report "The pilot hands over" ($s.pilot -eq "false") "pilot=$($s.pilot) module=$($s.module) pc=$($s.pc) xp=$($s.xp) hp=$($s.hp)"
$s = WaitFor "paused" "true"
Report "An enemy pauses the game" ($s.paused -eq "true" -and $s.note -like "ENEMY*") "paused=$($s.paused) note=$($s.note) foe=$($s.foe)"
Shot "fight_sighted"
# The cursor on the enemy makes them the target: the bar with its slots shows.
$until = (Get-Date).AddSeconds(30)
do {
  $s = State
  $x = if ($s.foe -ne "-") { [double]($s.foe -split ',')[0] } else { -1 }
  # The arrow key turns the camera, in the pause too, until they are near the middle of the picture.
  if ($x -lt [double]$s.width / 3 -or $x -gt [double]$s.width * 2 / 3) { Key 0x25 0x14B 120 } else { HoverLayout $s $s.foe }
  Start-Sleep -Milliseconds 50
  $s = State
} while ($s.target -notlike "Sith*" -and (Get-Date) -lt $until)
Report "The cursor finds them" ($s.target -like "Sith*" -and $s.slot1 -ne "-") "foe=$($s.foe) target=$($s.target) slot0=$($s.slot0) slot1=$($s.slot1)"
Shot "fight_target"

# The second slot: its name under the cursor, and a click queues the feat and starts the attack.
HoverLayout $s $s.slot1
[void](State)
Shot "fight_slot_hover"
$s = State
ClickSlot $s "slot1"
$until = (Get-Date).AddSeconds(5)
do { $s = State } while ([int]$s.queue -eq 0 -and $s.engaged -ne "true" -and (Get-Date) -lt $until)
Report "The feat slot queues it" ([int]$s.queue -gt 0 -or $s.engaged -eq "true") "queue=$($s.queue) engaged=$($s.engaged) target=$($s.target)"
Shot "fight_queued"
Key $SPACE[0] $SPACE[1]
$s = WaitFor "paused" "false"
Report "Space goes on" ($s.paused -eq "false") "paused=$($s.paused)"

# Blows land, and the words rise.
$until = (Get-Date).AddSeconds(45)
$floated = $false
do {
  $s = State
  if ([int]$s.floats -gt 0 -and -not $floated) { $floated = $true; Shot "fight_float" }
} while (([int]$s.attacks -eq 0 -or -not $floated) -and (Get-Date) -lt $until)
Report "The player attacks" ([int]$s.attacks -gt 0) "attacks=$($s.attacks) hp=$($s.hp) clip=$($s.clip)"
Report "Words rise" $floated "floats=$($s.floats)"
Shot "fight_engaged"

# The fight's lines are on the Messages tab, and the game waits under it.
Key 0x4C 0x26
$s = WaitFor "menu" "messages"
Report "L opens Messages" ($s.menu -eq "messages") "menu=$($s.menu) dlg=$($s.dlg) hp=$($s.hp)"
Press 113 410 279 28   # BTN_SHOW
[void](State)
Shot "fight_feedback"
Press 113 410 279 28
[void](State)
Key 0x1B 0x01
[void](WaitFor "menu" "-")

# F leaves the fight.
Key 0x46 0x21
$until = (Get-Date).AddSeconds(5)
do { $s = State } while ($s.engaged -eq "true" -and (Get-Date) -lt $until)
Report "F disengages" ($s.engaged -eq "false") "engaged=$($s.engaged)"
Shot "fight_disengaged"

$during = [Drv]::GetForegroundWindow()
Report "Never took focus" ($during -ne $h) "foreground before=$before now=$during viewer=$h"
Stop-Process -Id $v.Id -Force
Get-Content "$scratch\kotor_fight.err" -Tail 4
if ($script:failed) { exit 1 }
exit 0
