# One fight in the running window: an enemy sighted, the pause, the three slots, floats and the HUD.
# Like drive_window.ps1 it posts messages to the viewer's own handle only, and holds only at 1x: see docs/FASTTEST.md.
param([string]$Shots, [switch]$EarlyForce)
# Run from the repo root after building the viewer example. -Shots is a folder for pictures of the window.
# The pilot plays up to the first fight at its own speed and hands over when an enemy is seen. It writes no saves.
# -EarlyForce plays with the owner's rule, so the middle slot's Force Push is the character's own, not one given to it.
. "$PSScriptRoot\drive_common.ps1"
$script:stateFile = "$scratch\kotor_fight.out"
$power = if ($EarlyForce) { @("--early-force") } else { @("--power","23") }
$v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList (@("--mute","--no-tutorials","--state-log","--autopilot","--pilot-until","fight") + $power + @("--give","g_w_adhsvgren001","--frames","400000")) -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_fight.err" -PassThru -NoNewWindow
$h = WaitWindow $v; [Drv]::NeverActive($h)
"window $h"
$ENTER = @(0x0D, 0x1C); $SPACE = @(0x20, 0x39); $ONE = @(0x31, 0x02)
# A slot's place in the state line is in layout units, across a width the window's shape gives.
function ToPixel($s, [string]$place) {
  $p = $place -split ','
  @([int]([double]$p[0] * 1280 / [double]$s.width), [int]([double]$p[1] * 1.5))
}
# The bar is over the target, and the camera may still be turning: wait until it has stopped.
function ClickSlot($s, [string]$slot) {
  $until = (Get-Date).AddSeconds(5)
  do { $a = State; Start-Sleep -Milliseconds 60; $s = State } while ($a[$slot] -ne $s[$slot] -and (Get-Date) -lt $until)
  $p = ToPixel $s $s[$slot]
  Click $p[0] $p[1]
}
function HoverLayout($s, [string]$place) { $p = ToPixel $s $place; Hover $p[0] $p[1] }

function FindFoe {
  # The arrow key turns the camera, in the pause too, until they are near the middle of the picture.
  $until = (Get-Date).AddSeconds(30)
  do {
    $s = State
    $x = if ($s.foe -ne "-") { [double]($s.foe -split ',')[0] } else { -1 }
    if ($x -lt [double]$s.width / 3 -or $x -gt [double]$s.width * 2 / 3) { Key 0x25 0x14B 120 } else { HoverLayout $s $s.foe }
    Start-Sleep -Milliseconds 50
    $s = State
  } while ($s.target -notlike "Sith*" -and (Get-Date) -lt $until)
  $s
}

WaitLog
# The pilot's run takes it there at its own speed.
$until = (Get-Date).AddSeconds(180)
do { $s = State } while ($s.pilot -ne "false" -and (Get-Date) -lt $until)
Report "The pilot hands over" ($s.pilot -eq "false") "pilot=$($s.pilot) module=$($s.module) pc=$($s.pc) xp=$($s.xp) hp=$($s.hp)"
$s = WaitFor "paused" "true"
Report "An enemy pauses the game" ($s.paused -eq "true" -and $s.note -like "ENEMY*") "paused=$($s.paused) note=$($s.note) foe=$($s.foe)"
Shot "fight_sighted"
# The cursor on the enemy makes them the target: the bar with its slots shows.
$s = FindFoe
Report "The cursor finds them" ($s.target -like "Sith*" -and $s.slot1 -ne "-") "foe=$($s.foe) target=$($s.target) slot0=$($s.slot0) slot1=$($s.slot1)"
Shot "fight_target"

# The left slot attacks, and the Disengage button leaves it. All of this in the pause, so that the Sith is still there.
ClickSlot $s "slot0"
$until = (Get-Date).AddSeconds(5)
do { $s = State } while ($s.engaged -ne "true" -and (Get-Date) -lt $until)
Report "The left slot attacks" ($s.engaged -eq "true") "engaged=$($s.engaged) target=$($s.target)"
Shot "fight_attack_slot"
$p = ToPixel $s $s.disengage
Click $p[0] $p[1]
$until = (Get-Date).AddSeconds(5)
do { $s = State } while ($s.engaged -eq "true" -and (Get-Date) -lt $until)
Report "The Disengage button" ($s.engaged -eq "false") "engaged=$($s.engaged) disengage=$($s.disengage)"
Shot "fight_disengage_button"
$s = FindFoe
Report "The cursor finds them again" ($s.target -like "Sith*") "foe=$($s.foe) target=$($s.target)"

# Key 1 is the left slot, and Disengage leaves it again.
Key 0x31 0x02
$until = (Get-Date).AddSeconds(5)
do { $s = State } while ($s.engaged -ne "true" -and (Get-Date) -lt $until)
Report "Key 1 is the left slot" ($s.engaged -eq "true") "engaged=$($s.engaged) target=$($s.target)"
$p = ToPixel $s $s.disengage
Click $p[0] $p[1]
$until = (Get-Date).AddSeconds(5)
do { $s = State } while ($s.engaged -eq "true" -and (Get-Date) -lt $until)
$s = FindFoe

# The left slot's arrow goes to a feat: its name under the cursor, and a click queues it and starts the attack.
ClickSlot $s "down0"
[void](State)
$s = State
HoverLayout $s $s.slot0
[void](State)
Shot "fight_slot_hover"
$s = State
ClickSlot $s "slot0"
$until = (Get-Date).AddSeconds(5)
do { $s = State } while ([int]$s.queue -eq 0 -and $s.engaged -ne "true" -and (Get-Date) -lt $until)
Report "The left slot queues the feat" ([int]$s.queue -gt 0 -or $s.engaged -eq "true") "queue=$($s.queue) engaged=$($s.engaged) target=$($s.target)"
# The middle slot: the power given for this run.
$queued = [int]$s.queue
$s = State
ClickSlot $s "slot1"
$until = (Get-Date).AddSeconds(5)
do { $s = State } while ([int]$s.queue -le $queued -and (Get-Date) -lt $until)
Report "The middle slot queues the power" ([int]$s.queue -gt $queued) "queue $queued -> $($s.queue) slot1=$($s.slot1)"
Shot "fight_queued"
# The right slot: the grenade given for this run goes at once. An adhesive one, or a frag grenade kills this Sith first.
# The pilot may have looted others on the way, so its arrow goes round to the one given.
for ($i = 0; $i -lt 8; $i++) {
  $s = State
  if ($s.thrown -like "Adhesive*") { break }
  ClickSlot $s "down2"
  [void](State)
}
$s = State
Report "The right slot's arrow finds the adhesive grenade" ($s.thrown -like "Adhesive*") "thrown=$($s.thrown)"
$carried = [int]$s.inv
ClickSlot $s "slot2"
$until = (Get-Date).AddSeconds(5)
do { $s = State } while ([int]$s.inv -ge $carried -and (Get-Date) -lt $until)
Report "The right slot throws the grenade" ([int]$s.inv -lt $carried) "inv $carried -> $($s.inv) slot2=$($s.slot2)"
Shot "fight_thrown"
Key 0x58 0x2D
$s = WaitFor "paused" "false"
Report "X goes on" ($s.paused -eq "false") "paused=$($s.paused)"

# Blows land, and the words rise.
$until = (Get-Date).AddSeconds(45)
$floated = $false
do {
  $s = State
  if ([int]$s.floats -gt 0 -and -not $floated) { $floated = $true; Shot "fight_float" }
  # A party that joins in may have finished this one, and the next is still there.
  if ($s.dlg -eq "true") { [void](Advance) }
  elseif ($s.engaged -ne "true" -and $s.foe -ne "-") {
    HoverLayout $s $s.foe
    $s = State
    if ($s.target -like "Sith*") { ClickSlot $s "slot0" }
  }
} while (([int]$s.attacks -eq 0 -or -not $floated) -and (Get-Date) -lt $until)
Report "The player attacks" ([int]$s.attacks -gt 0) "attacks=$($s.attacks) hp=$($s.hp) clip=$($s.clip) engaged=$($s.engaged) paused=$($s.paused) queue=$($s.queue) target=$($s.target) foe=$($s.foe) pilot=$($s.pilot) note=$($s.note)"
Report "Words rise" $floated "floats=$($s.floats)"
Shot "fight_engaged"

# The fight's lines are on the Messages tab, and the game waits under it.
$until = (Get-Date).AddSeconds(30)
do { $s = Advance } while ($s.dlg -eq "true" -and (Get-Date) -lt $until)
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

FocusReport
Stop-Process -Id $v.Id -Force
Get-Content "$scratch\kotor_fight.err" -Tail 4
if ($script:failed) { exit 1 }
exit 0
