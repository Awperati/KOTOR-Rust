# Nameplates in the running window: the pilot plays to a fight, then plates are read, clicked and turned off.
# Like the other drivers it posts messages to the viewer's own handle only. The window opens on DISPLAY2 and is never active.
# It holds only at 1x: see docs/FASTTEST.md. It writes no saves.
param([string]$Shots, [string]$Size = $(if ($env:KOTOR_DRIVE_SIZE) { $env:KOTOR_DRIVE_SIZE } else { "1280x720" }))
# Run from the repo root after building the viewer example. -Shots is a folder for pictures of the window.
. "$PSScriptRoot\drive_common.ps1"
$wide = [int]($Size -split 'x')[0]; $tall = [int]($Size -split 'x')[1]
$tag = $wide
$script:stateFile = "$scratch\kotor_plates.out"
Remove-Item $script:stateFile -ErrorAction SilentlyContinue
$v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList @("--mute","--no-tutorials","--no-auto-pause","--state-log","--autopilot","--pilot-until","fight","--size",$Size,"--frames","2000000") -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_plates.err" -PassThru -NoNewWindow
$h = WaitWindow $v; [Drv]::NeverActive($h)
function ToPixel($s, [string]$place) {
  $p = $place -split ','
  @([int]([double]$p[0] * $wide / [double]$s.width), [int]([double]$p[1] * $tall / 480.0))
}
function Plates { [int](State).plates }
# An enemy's plate in the middle third of the window, where it stays for a while.
function InTheMiddle($s) {
  if ($s.enemyplate -eq "-") { return $false }
  $x = [double]($s.enemyplate -split ',')[0]
  $x -gt [double]$s.width / 3 -and $x -lt [double]$s.width * 2 / 3
}

WaitLog
$until = (Get-Date).AddSeconds(420)
do { $s = State } while ($s.pilot -ne "false" -and (Get-Date) -lt $until)
Report "The pilot hands over" ($s.pilot -eq "false") "pilot=$($s.pilot) module=$($s.module) hp=$($s.hp)"

# Turn until a plate is in sight.
$until = (Get-Date).AddSeconds(60)
do {
  $s = State
  if (-not (InTheMiddle $s)) { Key 0x25 0x14B 120 }
  for ($i = 0; $i -lt 40 -and $s.dlg -eq "true"; $i++) { Key 0x0D 0x1C; Key 0x31 0x02; $s = State }
} while (-not (InTheMiddle $s) -and (Get-Date) -lt $until)
Report "A plate over an enemy" ($s.enemyplate -ne "-") "plates=$($s.plates) enemy's=$($s.enemyplate) foe=$($s.foe) target=$($s.target)"
Shot "plates_seen_$tag"

# The F6 key turns them off and on.
$F6 = @(0x75, 0x40)
Key $F6[0] $F6[1]
$until = (Get-Date).AddSeconds(3)
do { $off = Plates } while ($off -ne 0 -and (Get-Date) -lt $until)
Report "F6 turns them off" ($off -eq 0) "plates=$off"
Shot "plates_off_$tag"
Key $F6[0] $F6[1]
$until = (Get-Date).AddSeconds(3)
do { $on = Plates } while ($on -lt 1 -and (Get-Date) -lt $until)
Report "F6 turns them on" ($on -ge 1) "plates=$on"

# A click on a plate does what a click on the body does.
$s = State
if ($s.enemyplate -ne "-") {
  $p = ToPixel $s $s.enemyplate
  Hover $p[0] $p[1]
  [void](State)
  $s = State
  Report "The cursor on a plate makes that the target" ($s.target -ne "-") "target=$($s.target) at $($p -join ',')"
  Shot "plates_hover_$tag"
  # It may have moved.
  $s = State
  $p = ToPixel $s $s.enemyplate
  Click $p[0] $p[1]
  $until = (Get-Date).AddSeconds(8)
  do { $s = State } while ($s.engaged -ne "true" -and (Get-Date) -lt $until)
  "after the click: engaged=$($s.engaged) target=$($s.target) attacks=$($s.attacks)"
  Report "A click on the plate attacks" ($s.engaged -eq "true") "engaged=$($s.engaged) target=$($s.target)"
  Start-Sleep -Milliseconds 800
  Shot "plates_target_$tag"
}

# Not over a menu.
Key 0x43 0x2E
$s = WaitFor "menu" "character"
[void](State)
Shot "plates_menu_$tag"
Key 0x1B 0x01
[void](WaitFor "menu" "-")

# Frames a second with the camera turning, which redraws them: state lines are one a frame.
function Turning {
  $long = (Get-Content $script:stateFile -Tail 1).Length + 2
  $a = (Get-Item $script:stateFile).Length; $t0 = Get-Date
  [void][Drv]::PostMessage($h, 0x204, [IntPtr]2, [IntPtr]((300 -shl 16) -bor 400))
  for ($i = 0; $i -lt 150; $i++) { [void][Drv]::PostMessage($h, 0x200, [IntPtr]2, [IntPtr]((300 -shl 16) -bor (400 + ($i % 2) * 8))); Start-Sleep -Milliseconds 20 }
  [void][Drv]::PostMessage($h, 0x205, [IntPtr]0, [IntPtr]((300 -shl 16) -bor 400))
  $b = (Get-Item $script:stateFile).Length; $t1 = Get-Date
  [Math]::Round(($b - $a) / $long / ($t1 - $t0).TotalSeconds, 1)
}
$on = Turning
$count = (State).plates
Key $F6[0] $F6[1]
[void](State)
$off = Turning
Key $F6[0] $F6[1]
"frames a second while turning: $on with $count plates, $off with them off"

FocusReport
Stop-Process -Id $v.Id -Force
Get-Content "$scratch\kotor_plates.err" -Tail 3
"gate: $(if ($script:failed) { 'FAILED ' + $script:failed } else { 'all PASS' })"
exit $script:failed
