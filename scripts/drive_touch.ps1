# The on-screen controls in the running window. The blue button and the stick by the mouse, as posted messages to the viewer's own
# handle. Fingers by --touch-script: a driver cannot post real touch messages, so its lines go in where a touch screen's would.
# Set KOTOR_HIDDEN=1 and no window shows. It holds only at 1x: see docs/FASTTEST.md. It writes no saves, and its settings are a temp file.
param([string]$Size = $(if ($env:KOTOR_DRIVE_SIZE) { $env:KOTOR_DRIVE_SIZE } else { "1280x720" }))
. "$PSScriptRoot\drive_common.ps1"
$wide = [int]($Size -split 'x')[0]; $tall = [int]($Size -split 'x')[1]

$script:fingers = "$scratch\kotor_touch_fingers_$PID.txt"
$settings = "$scratch\kotor_touch_settings_$PID.txt"
Remove-Item $settings -ErrorAction SilentlyContinue

function Open([string[]]$more = @()) {
  $script:stateFile = "$scratch\kotor_touch_$PID.out"
  Remove-Item $script:stateFile -ErrorAction SilentlyContinue
  Set-Content $script:fingers -Value $null -NoNewline
  $args = @("--mute","--no-tutorials","--no-auto-pause","--no-early-force","--module","danm14aa","--skip-menu","--state-log","--place","165.17,136.66,1.85,1.767","--size",$Size,"--settings",$settings,"--touch-script",$script:fingers,"--frames","2000000") + $more
  $script:v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList $args -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_touch.err" -PassThru -NoNewWindow
  $script:h = WaitWindow $script:v
  [Drv]::NeverActive($script:h)
  WaitLog
  $until = (Get-Date).AddSeconds(60)
  do { $s = State } while ($s.dlg -eq "true" -and (Get-Date) -lt $until)
  $s
}

# A place the state line gives, in the layout, as a pixel of the window.
function ToPixel($s, [string]$place) {
  $p = $place -split ','
  @([int]([double]$p[0] * $wide / [double]$s.width), [int]([double]$p[1] * $tall / 480.0))
}
function PartAt($s, [string]$name) {
  foreach ($one in ($s.touchat -split ';')) {
    $p = $one -split ':'
    if ($p[0] -eq $name) { return @([int]([double]$p[1] * $wide / [double]$s.width), [int]([double]$p[2] * $tall / 480.0)) }
  }
  throw "no $name in $($s.touchat)"
}
# The window reads the file as this writes it: a line is tried again if the two meet, and never lost.
function Finger([string]$what, [int]$id, [int]$x, [int]$y) {
  for ($try = 0; $try -lt 200; $try++) {
    try { [IO.File]::AppendAllText($script:fingers, "$what $id $x $y`n"); return } catch [IO.IOException] { Start-Sleep -Milliseconds 5 }
  }
  throw "could not write $what $id"
}
function Tap([int]$id, $at) { Finger "down" $id $at[0] $at[1]; Finger "up" $id $at[0] $at[1]; [void](State); State }
function MouseDown([int]$x, [int]$y) { PostHover $x $y; [void][Drv]::PostMessage($h, 0x201, [IntPtr]1, [IntPtr](($y -shl 16) -bor $x)) }
function MouseUp([int]$x, [int]$y) { [void][Drv]::PostMessage($h, 0x202, [IntPtr]0, [IntPtr](($y -shl 16) -bor $x)) }
# The mouse is not listened to just after a finger: Windows can follow one with a mouse message of its own.
function AfterFingers { Start-Sleep -Milliseconds 900 }

$s = Open
Report "Off until asked" ($s.touch -eq "false") "touch=$($s.touch)"

# The mouse: the blue button, then the stick held and pushed up.
$blue = ToPixel $s $s.blue
PostClick $blue[0] $blue[1]
$s = WaitFor "touch" "true"
Report "The blue button by the mouse" ($s.touch -eq "true") "at $blue, touch=$($s.touch)"
$thumb = PartAt $s "stick"
$push = [int]($tall * 0.17)
$from = $s
MouseDown $thumb[0] $thumb[1]
PostHover $thumb[0] ($thumb[1] - $push)
$until = (Get-Date).AddSeconds(5)
do { $s = State } while ((Moved $from $s) -lt 0.5 -and (Get-Date) -lt $until)
Report "The mouse holds the stick" ((Moved $from $s) -ge 0.5) "moved $(Moved $from $s), stick=$($s.stick)"
MouseUp $thumb[0] ($thumb[1] - $push)
$s = WaitFor "fingers" "0"
Report "And lets it go" ($s.stick -eq "0.00,0.00" -and $s.fingers -eq "0") "stick=$($s.stick) fingers=$($s.fingers)"

# Fingers: the stick and a jump and the camera, all at once.
$from = State
Finger "down" 1 $thumb[0] $thumb[1]
Finger "move" 1 $thumb[0] ($thumb[1] - $push)
$until = (Get-Date).AddSeconds(5)
do { $s = State } while ((Moved $from $s) -lt 0.5 -and (Get-Date) -lt $until)
Report "A finger runs on the stick" ((Moved $from $s) -ge 0.5 -and $s.fingers -eq "1") "moved $(Moved $from $s), stick=$($s.stick)"
$jump = PartAt $s "jump"
Finger "down" 2 $jump[0] $jump[1]
$airborne = $false
$until = (Get-Date).AddSeconds(3)
do { $s = State; if ($s.air -ne "false" -and $s.air -ne "-") { $airborne = $true } } while (-not $airborne -and (Get-Date) -lt $until)
Report "A second finger jumps with the first down" ($airborne -and $s.fingers -eq "2") "air=$($s.air) fingers=$($s.fingers) stick=$($s.stick)"
$yaw = [double]$s.yaw
$x0 = [int]($wide * 0.6); $y0 = [int]($tall * 0.3); $x1 = $x0 + [int]($tall * 0.2)
Finger "down" 3 $x0 $y0
Finger "move" 3 $x1 $y0
$s = State; $s = State
Report "A third finger turns the camera" ([Math]::Abs([double]$s.yaw - $yaw) -gt 0.1 -and $s.fingers -eq "3") "yaw $yaw to $($s.yaw), fingers=$($s.fingers), stick=$($s.stick)"
Report "With the stick still held" ($s.stick -ne "0.00,0.00") "stick=$($s.stick)"
Finger "up" 2 $jump[0] $jump[1]
Finger "up" 3 $x1 $y0
Finger "up" 1 $thumb[0] ($thumb[1] - $push)
$s = WaitFor "fingers" "0"
$s = WaitFor "stick" "0.00,0.00"
Report "All up" ($s.fingers -eq "0" -and $s.stick -eq "0.00,0.00") "fingers=$($s.fingers) stick=$($s.stick)"

# Each button.
Start-Sleep -Milliseconds 800
$s = Tap 4 (PartAt $s "pause")
Report "Pause" ($s.paused -eq "true") "paused=$($s.paused)"
$s = Tap 4 (PartAt $s "pause")
Report "And go on" ($s.paused -eq "false") "paused=$($s.paused)"
$s = Tap 4 (PartAt $s "mode")
Report "Attack mode" ($s.mode -eq "true") "mode=$($s.mode)"
$s = Tap 4 (PartAt $s "mode")
$s = WaitFor "mode" "false"
$before = [int]$s.floats
$s = Tap 4 (PartAt $s "pickup")
Report "Pickup" ($s.opened -ne "false" -or [int]$s.floats -gt $before -or $s.menu -ne "-") "opened=$($s.opened) floats=$($s.floats) menu=$($s.menu)"
if ($s.menu -ne "-") { Key 0x1B 1; $s = WaitFor "menu" "-" }
$s = Tap 4 (ToPixel $s $s.inventory)
$s = WaitFor "menu" "inventory"
Report "A menu key" ($s.menu -eq "inventory") "menu=$($s.menu)"
Key 0x1B 1
$s = WaitFor "menu" "-"

# A tap on the blue button turns it off, and the file remembers.
$s = Tap 4 (ToPixel $s $s.blue)
$s = WaitFor "touch" "false"
Report "The blue button by a finger" ($s.touch -eq "false") "touch=$($s.touch)"
AfterFingers
$blue = ToPixel $s $s.blue
PostClick $blue[0] $blue[1]
$s = WaitFor "touch" "true"
FocusReport
Stop-Process -Id $script:v.Id -Force
Start-Sleep -Milliseconds 300

# A new start, in Classic movement: it is still on, and the stick steers there too.
$s = Open @("--classic-movement")
Report "A new start remembers" ($s.touch -eq "true") "touch=$($s.touch)"
$thumb = PartAt $s "stick"
$from = $s
Finger "down" 1 $thumb[0] $thumb[1]
Finger "move" 1 $thumb[0] ($thumb[1] - $push)
$until = (Get-Date).AddSeconds(5)
do { $s = State } while ((Moved $from $s) -lt 0.3 -and (Get-Date) -lt $until)
Report "The stick in Classic" ((Moved $from $s) -ge 0.3) "moved $(Moved $from $s)"
Finger "up" 1 $thumb[0] ($thumb[1] - $push)
FocusReport
Stop-Process -Id $script:v.Id -Force
Start-Sleep -Milliseconds 200
Remove-Item $settings, $script:fingers -ErrorAction SilentlyContinue
Get-Content "$scratch\kotor_touch.err" -Tail 3
"failed: $($script:failed)"
