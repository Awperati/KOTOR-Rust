# Drive the viewer in Classic and then in Modern, through its OWN window handle with posted messages: no global mouse or
# keyboard, and no focus. Set KOTOR_HIDDEN=1 and no window shows. It holds only at 1x: see docs/FASTTEST.md.
# Run from the repo root after building the viewer example. Each run has a saves folder of its own (--saves), writes no save,
# and sets its mode and Click to move in memory only (--game-mode, --click-to-move): the player's settings are not touched.
#
# What it checks, in each mode: the E key opens the footlocker; W walks; D turns the body in Classic (tank controls) and
# steps to the side in Modern; Caps Lock does nothing in Classic and in Modern gives the cursor back and takes it again; a
# click on the floor walks there (in Modern with the cursor back); B pauses in Classic and not in Modern.
param([string]$Size = $(if ($env:KOTOR_DRIVE_SIZE) { $env:KOTOR_DRIVE_SIZE } else { "1280x720" }), [string]$Shots, [string[]]$Modes = @("classic", "modern"))
. "$PSScriptRoot\drive_common.ps1"
$script:ClickScale = [double]($Size -split "x")[1] / 720.0
$SPACE = @(0x20, 0x39); $E = @(0x45, 0x12); $W = @(0x57, 0x11); $D = @(0x44, 0x20); $CAPS = @(0x14, 0x3A); $PAUSE = @(0x42, 0x30)

# How far round the camera went from one state to the next, in radians, the short way.
function Turned($a, $b) {
  $d = [double]$b.yaw - [double]$a.yaw
  while ($d -gt [Math]::PI) { $d -= 2 * [Math]::PI }
  while ($d -lt -[Math]::PI) { $d += 2 * [Math]::PI }
  [Math]::Abs($d)
}

# Hold a key for this many of the game's steps (sixty a second). Returns the state after.
function HoldSteps($k, [int]$steps) {
  KeyDown $k
  $s = UntilSteps { param($o) $false } $steps
  KeyUp $k
  [void](State)
  State
}

function Drive([string]$mode) {
  $saves = Join-Path $scratch "kotor_modes_saves_$PID"
  New-Item -ItemType Directory -Force $saves | Out-Null
  $out = "$scratch\kotor_modes_$mode.out"
  $v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList @("--size",$Size,"--mute","--no-tutorials","--no-auto-pause","--no-chargen","--state-log","--click-to-move","--game-mode",$mode,"--saves",$saves,"--skip-menu","--near","end_locker01","--frames","200000") -RedirectStandardOutput $out -RedirectStandardError "$scratch\kotor_modes_$mode.err" -PassThru -NoNewWindow
  $script:h = WaitWindow $v; [Drv]::NeverActive($script:h)
  $script:stateFile = $out
  WaitLog
  $modern = $mode -eq "modern"

  # Through the opening and Trask's talk.
  $quiet = 0
  for ($i = 0; $i -lt 400 -and $quiet -lt 6; $i++) {
    $s = Advance
    if ($s.dlg -eq "false" -and [int]$s.said -gt 3) { $quiet++ } else { $quiet = 0 }
  }
  Report "$mode`: talk done" ($quiet -ge 6) "dlg=$($s.dlg) said=$($s.said)"
  Report "$mode`: the mode" ($s.game -eq $mode) "game=$($s.game) mlook=$($s.mlook)"

  # E opens the footlocker. The left arrow turns the view until it is what E works on: Trask may stand nearer.
  $s = State
  if ($s.use -ne "end_locker01") {
    WatchFocus
    [void][Drv]::PostMessage($script:h, 0x100, [IntPtr]0x25, [IntPtr](1 -bor (0x4B -shl 16) -bor (1 -shl 24)))
    for ($t = 0; $t -lt 200 -and $s.use -ne "end_locker01"; $t++) { $s = State }
    [void][Drv]::PostMessage($script:h, 0x101, [IntPtr]0x25, [IntPtr][long]([long]0xC0000001 -bor (0x4B -shl 16) -bor (1 -shl 24)))
    $s = State
  }
  Key $E[0] $E[1]
  $s = WaitFor "opened" "true"
  Report "$mode`: E opens" ($s.opened -eq "true") "opened=$($s.opened) use=$($s.use)"
  if ($Shots) { [Drv]::Save($script:h, "$Shots\modes_${mode}_locker.png") }
  Key $SPACE[0] $SPACE[1]
  $s = WaitFor "opened" "false"

  # Turn the view away from the footlocker, which E had it face, so that a click below finds the floor and not the locker. The
  # left arrow, held until the camera is half round.
  $a = State
  WatchFocus
  [void][Drv]::PostMessage($script:h, 0x100, [IntPtr]0x25, [IntPtr](1 -bor (0x4B -shl 16) -bor (1 -shl 24)))
  $s = $a
  for ($t = 0; $t -lt 300 -and (Turned $a $s) -lt 2.6; $t++) { $s = State }
  [void][Drv]::PostMessage($script:h, 0x101, [IntPtr]0x25, [IntPtr][long]([long]0xC0000001 -bor (0x4B -shl 16) -bor (1 -shl 24)))
  [void](State)
  "$mode`: turned from the locker by $([Math]::Round((Turned $a (State)), 2)) rad"

  # Caps Lock: nothing in Classic. In Modern the mouse looks by itself until Caps Lock gives the cursor back.
  $a = State
  Key $CAPS[0] $CAPS[1]
  [void](State)
  $b = State
  if ($modern) {
    Report "$mode`: Caps frees cursor" ($a.mlook -eq "1" -and $b.mlook -eq "0") "mlook $($a.mlook) -> $($b.mlook)"
  } else {
    Report "$mode`: Caps does nothing" ($a.mlook -eq "0" -and $b.mlook -eq "0") "mlook $($a.mlook) -> $($b.mlook)"
  }

  # A click on the floor walks there: in Modern with the cursor back from Caps Lock.
  $ok = $false; $detail = ""
  foreach ($y in 600, 560, 520, 480, 640) {
    foreach ($x in 640, 400, 880) {
      $a = State
      for ($k = 0; $k -lt 80 -and $a.dlg -eq "true"; $k++) { $a = Advance }
      Click $x $y
      $b = UntilSteps { param($o) (Moved $a $o) -gt 0.3 -or $o.opened -eq "true" } 30
      $detail = "click $x,$y $($a.pc) -> $($b.pc) clip=$($b.clip) opened=$($b.opened)"
      if ($b.opened -eq "true") { Key $SPACE[0] $SPACE[1]; [void](WaitFor "opened" "false") }
      if ((Moved $a $b) -gt 0.3) { $ok = $true; break }
    }
    if ($ok) { break }
  }
  Report "$mode`: click walks" $ok $detail

  if ($modern) {
    Key $CAPS[0] $CAPS[1]
    [void](State)
    $s = State
    Report "$mode`: Caps looks again" ($s.mlook -eq "1") "mlook=$($s.mlook)"
  }

  # W walks in both. After the click, so that the tank turn has not swung the camera onto the footlocker.
  $a = State
  $b = HoldUntilMoved $W $a 0.3 60
  Report "$mode`: W moves" ((Moved $a $b) -gt 0.3) "$($a.pc) -> $($b.pc)"

  # D: Classic turns the body and the camera behind it, and does not step; Modern steps to the right, the camera still.
  $a = State
  $b = HoldSteps $D 40
  $moved = Moved $a $b; $turn = Turned $a $b
  if ($modern) {
    Report "$mode`: D steps aside" ($moved -gt 0.3 -and $turn -lt 0.2) ("moved {0:N2} m, turned {1:N2} rad" -f $moved, $turn)
  } else {
    Report "$mode`: D turns (tank)" ($moved -lt 0.2 -and $turn -gt 0.5) ("moved {0:N2} m, turned {1:N2} rad" -f $moved, $turn)
  }

  # B: Classic's pause key holds the game, and W does nothing until B again. Modern has no pause in play.
  $before = State
  Key $PAUSE[0] $PAUSE[1]
  [void](State)
  $a = State
  "B: before paused=$($before.paused) dlg=$($before.dlg) menu=$($before.menu) clip=$($before.clip); after paused=$($a.paused) dlg=$($a.dlg) menu=$($a.menu) note=$($a.note)"
  Key $W[0] $W[1] 300
  $b = State
  if ($modern) {
    $b = HoldUntilMoved $W $a 0.3 60
    Report "$mode`: B does not pause" ((Moved $a $b) -gt 0.3 -and $b.paused -notin @("1", "true")) "paused=$($b.paused) $($a.pc) -> $($b.pc)"
  } else {
    Report "$mode`: B pauses" ((Moved $a $b) -lt 0.05) "paused=$($b.paused) $($a.pc) -> $($b.pc)"
    Key $PAUSE[0] $PAUSE[1]
    [void](State)
  }

  FocusReport
  Stop-Process -Id $v.Id -Force
  Get-Content "$scratch\kotor_modes_$mode.err" -Tail 3
  Remove-Item -Recurse -Force $saves -ErrorAction SilentlyContinue
}

foreach ($m in $Modes) { Drive $m }
# PowerShell names ignore case: $PAUSE is the B key, never $b, which holds a state.
# A failed check fails the script, so nothing that reads only its exit code takes it for a pass.
if ($script:failed) { exit 1 }
exit 0
