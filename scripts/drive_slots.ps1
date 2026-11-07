# The slots over an enemy in the running window: their size near and far, and that they stay on the way to them.
# Like the other drivers it posts messages to the viewer's own handle only. The window opens on DISPLAY2 and is never active.
# It holds only at 1x: see docs/FASTTEST.md. It writes no saves.
param([string]$Shots, [string]$Size = $(if ($env:KOTOR_DRIVE_SIZE) { $env:KOTOR_DRIVE_SIZE } else { "1280x720" }))
# Run from the repo root after building the viewer example. -Shots is a folder for pictures of the window.
. "$PSScriptRoot\drive_common.ps1"
$wide = [int]($Size -split 'x')[0]; $tall = [int]($Size -split 'x')[1]
$tag = $tall

function ToPixel($s, [string]$place) {
  $p = $place -split ','
  @([int]([double]$p[0] * $wide / [double]$s.width), [int]([double]$p[1] * $tall / 480.0))
}

# An enemy this far ahead, on open floor. It comes for the player, so what is looked at is looked at soon.
function Open([double]$metres) {
  $script:stateFile = "$scratch\kotor_slots_$metres.out"
  Remove-Item $script:stateFile -ErrorAction SilentlyContinue
  $script:v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList @("--mute","--no-tutorials","--no-auto-pause","--no-early-force","--module","danm14aa","--skip-menu","--state-log","--place","165.17,136.66,1.85,1.767","--foe","$metres","--size",$Size,"--frames","2000000") -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_slots.err" -PassThru -NoNewWindow
  for ($i = 0; $i -lt 900 -and $script:v.MainWindowTitle -notlike "kotor-rs*"; $i++) { Start-Sleep -Milliseconds 50; $script:v.Refresh() }
  $script:h = $script:v.MainWindowHandle
  [Drv]::NeverActive($script:h)
  WaitLog
  $until = (Get-Date).AddSeconds(30)
  do { $s = State } while ($s.enemyplate -eq "-" -and (Get-Date) -lt $until)
  $s
}

function OnThePlate($s) {
  $p = ToPixel $s $s.enemyplate
  Hover $p[0] $p[1]
  [void](State)
  State
}

$s = Open 18
$s = OnThePlate $s
Report "A far enemy has slots" ($s.target -eq "Test_Soldier" -or $s.target -like "Test_Soldier*") "target=$($s.target) slot0=$($s.slot0) size=$($s.slotsize)"
$far = [int]$s.slotsize
Shot "slots_far_$tag"
Stop-Process -Id $script:v.Id -Force

$s = Open 5
$s = OnThePlate $s
$near = [int]$s.slotsize
Report "A near one has bigger ones" ($near -gt $far) "$far at 18 m, $near at 5 m"
Shot "slots_near_$tag"

# Up through the gap, away from it all, and past the grace.
$slot = ToPixel $s $s.slot0
$plate = ToPixel $s $s.enemyplate
Hover $slot[0] ([int](($slot[1] + $plate[1]) / 2))
$s = State
Report "They stay in the gap" ($s.target -like "Test_Soldier*") "target=$($s.target)"
Hover ([int]($wide * 0.2)) ([int]($tall * 0.15))
Start-Sleep -Milliseconds 700
$s = State
Report "They stay off it all" ($s.target -like "Test_Soldier*" -and $s.slot0 -ne "-") "target=$($s.target) slot0=$($s.slot0)"
Shot "slots_kept_$tag"

# The click on the slot goes for that one.
$slot = ToPixel $s $s.slot0
Click $slot[0] $slot[1]
$until = (Get-Date).AddSeconds(5)
do { $s = State } while ($s.engaged -ne "true" -and (Get-Date) -lt $until)
Report "The click attacks it" ($s.engaged -eq "true") "engaged=$($s.engaged) target=$($s.target)"
Shot "slots_clicked_$tag"

FocusReport
Stop-Process -Id $script:v.Id -Force
Get-Content "$scratch\kotor_slots.err" -Tail 3
if ($script:failed) { exit 1 }
exit 0
