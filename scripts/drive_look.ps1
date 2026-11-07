# The mouse and the camera in the running window: dragging down looks down, and the Invert Camera box flips it, in both movements.
# Like the other drivers it posts messages to the viewer's own handle only. The window opens on DISPLAY2 and is never active.
# It holds only at 1x: see docs/FASTTEST.md. It keeps its settings in a temporary file, and writes no saves.
param([string]$Size = "1280x720")
# Run from the repo root after building the viewer example.
. "$PSScriptRoot\drive_common.ps1"
$ENTER = @(0x0D, 0x1C); $ONE = @(0x31, 0x02)
$WM_RBUTTONDOWN = 0x204; $WM_RBUTTONUP = 0x205

function Point([int]$x, [int]$y) { [IntPtr](($y -shl 16) -bor $x) }
# Move in steps, each one handled before the next. wParam says which buttons are down.
function MoveTo([int]$x, [int]$y, [int]$buttons) {
  [void][Drv]::PostMessage($h, 0x200, [IntPtr]$buttons, (Point $x $y))
  [void](State)
}
function Sweep([int]$from, [int]$to, [int]$buttons) {
  $step = if ($to -gt $from) { 20 } else { -20 }
  for ($y = $from; $y -ne $to; $y += $step) { MoveTo 640 ($y + $step) $buttons }
}
function Reading { $s = State; @{ pitch = [double]$s.pitch; down = [double]$s.lookdown } }

foreach ($modern in @($true, $false)) {
  foreach ($invert in @($false, $true)) {
    $name = "movement " + $(if ($modern) { "Modern" } else { "Classic" }) + ", Invert Camera " + $(if ($invert) { "On" } else { "Off" })
    $file = "$scratch\kotor_look_settings.txt"
    "movement_modern=$($modern.ToString().ToLower())`ninvert_y=$($invert.ToString().ToLower())`n" | Set-Content $file -Encoding ascii
    $script:stateFile = "$scratch\kotor_look.out"
    Remove-Item $script:stateFile -ErrorAction SilentlyContinue
    $argv = @("--mute","--no-tutorials","--no-auto-pause","--no-early-force","--module","danm14aa","--place","165.17,136.66,1.85,1.767","--skip-menu","--state-log","--settings",$file,"--size",$Size,"--frames","100000")
    $v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList $argv -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_look.err" -PassThru -NoNewWindow
    for ($i = 0; $i -lt 600 -and $v.MainWindowTitle -notlike "kotor-rs*"; $i++) { Start-Sleep -Milliseconds 50; $v.Refresh() }
    $h = $v.MainWindowHandle; [Drv]::NeverActive($h)
    WaitLog
    $until = (Get-Date).AddSeconds(30)
    do { Key $ENTER[0] $ENTER[1]; Key $ONE[0] $ONE[1]; $s = State } while ($s.dlg -eq "true" -and (Get-Date) -lt $until)
    # A scripted camera moves by itself. Measure once this one has stopped.
    $until = (Get-Date).AddSeconds(30)
    do { $was = (State).lookdown; Start-Sleep -Milliseconds 300; $s = State } while (($s.lookdown -ne $was -or $s.dlg -eq "true" -or [double]$s.speed -gt 0.05) -and (Get-Date) -lt $until)
    MoveTo 640 300 0
    [void](State)

    # No button held: nothing moves the camera.
    $a = Reading; Sweep 300 400 0; Sweep 400 300 0; $b = Reading
    Report "$name, no button: the camera stays" ([Math]::Abs($b.pitch - $a.pitch) -lt 0.0001) "pitch $($a.pitch) then $($b.pitch)"

    # The right button held: the drag looks. The range is -0.2 to 1.2, so go the way there is room first.
    [void][Drv]::PostMessage($h, $WM_RBUTTONDOWN, [IntPtr]2, (Point 640 300))
    [void](State)
    $a = Reading; Sweep 300 340 2; $b = Reading; Sweep 340 300 2; $c = Reading
    [void][Drv]::PostMessage($h, $WM_RBUTTONUP, [IntPtr]0, (Point 640 300))
    "$name, mouse down: lookdown {0:n4} -> {1:n4}, back up -> {2:n4}; pitch {3:n4} -> {4:n4} -> {5:n4}" -f $a.down, $b.down, $c.down, $a.pitch, $b.pitch, $c.pitch
    if ($invert) {
      Report "$name, mouse down looks up" ($b.down -lt $a.down - 0.02) "lookdown $($a.down) then $($b.down)"
      Report "$name, mouse up looks down" ($c.down -gt $b.down + 0.02) "lookdown $($b.down) then $($c.down)"
    } else {
      Report "$name, mouse down looks down" ($b.down -gt $a.down + 0.02) "lookdown $($a.down) then $($b.down)"
      Report "$name, mouse up looks up" ($c.down -lt $b.down - 0.02) "lookdown $($b.down) then $($c.down)"
    }
    FocusReport
    Stop-Process -Id $v.Id -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 300
  }
}
Remove-Item "$scratch\kotor_look_settings.txt" -ErrorAction SilentlyContinue
"gate: $(if ($script:failed) { 'FAILED ' + $script:failed } else { 'all PASS' })"
exit $script:failed
