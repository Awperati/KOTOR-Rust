# The mouse and the camera in the running window: going down looks down, with the Mouse Look box on or off, in both movements.
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
  foreach ($box in @($false, $true)) {
    $name = "movement " + $(if ($modern) { "Modern" } else { "Classic" }) + ", box " + $(if ($box) { "On" } else { "Off" })
    $file = "$scratch\kotor_look_settings.txt"
    "movement_modern=$($modern.ToString().ToLower())`nmouse_look=$($box.ToString().ToLower())`n" | Set-Content $file -Encoding ascii
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

    # No button held.
    $a = Reading; Sweep 300 400 0; $b = Reading; Sweep 400 200 0; $c = Reading
    "$name, no button: down {0:n4} -> {1:n4} -> up {2:n4}; pitch {3:n4} -> {4:n4} -> {5:n4}" -f $a.down, $b.down, $c.down, $a.pitch, $b.pitch, $c.pitch
    if ($box) {
      Report "$name, no button: down looks down" ($b.down -gt $a.down + 0.05) "lookdown $($a.down) then $($b.down)"
      Report "$name, no button: up looks up" ($c.down -lt $b.down - 0.05) "lookdown $($b.down) then $($c.down)"
    } else {
      Report "$name, no button: the camera stays" ([Math]::Abs($c.pitch - $a.pitch) -lt 0.0001) "pitch $($a.pitch) then $($c.pitch)"
    }

    # The button held: the drag looks with the box off, and frees the cursor with it on.
    Sweep 200 300 0
    [void][Drv]::PostMessage($h, $WM_RBUTTONDOWN, [IntPtr]2, (Point 640 300))
    [void](State)
    $a = Reading; Sweep 300 400 2; $b = Reading; Sweep 400 200 2; $c = Reading
    [void][Drv]::PostMessage($h, $WM_RBUTTONUP, [IntPtr]0, (Point 640 200))
    "$name, right button: down {0:n4} -> {1:n4} -> up {2:n4}; pitch {3:n4} -> {4:n4} -> {5:n4}" -f $a.down, $b.down, $c.down, $a.pitch, $b.pitch, $c.pitch
    if ($box) {
      Report "$name, right button: the cursor is free" ([Math]::Abs($c.pitch - $a.pitch) -lt 0.0001) "pitch $($a.pitch) then $($c.pitch)"
    } else {
      Report "$name, right drag: down looks down" ($b.down -gt $a.down + 0.05) "lookdown $($a.down) then $($b.down)"
      Report "$name, right drag: up looks up" ($c.down -lt $b.down - 0.05) "lookdown $($b.down) then $($c.down)"
    }
    FocusReport
    Stop-Process -Id $v.Id -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 300
  }
}
Remove-Item "$scratch\kotor_look_settings.txt" -ErrorAction SilentlyContinue
"gate: $(if ($script:failed) { 'FAILED ' + $script:failed } else { 'all PASS' })"
exit $script:failed
