# The owner's Space and X in the running window: Space jumps and never pauses, X pauses, both can be rebound, and no jump in a
# conversation or in Classic movement. A minigame's Space is in drive_race.ps1, and being down in gpu/tests/movement.rs.
# Like the other drivers it posts messages to the viewer's own handle only. With KOTOR_HIDDEN set nothing shows. It works on
# settings files of its own, in the temp folder, and writes no saves.
param([string]$Size = "1280x720")
# Run from the repo root after building the viewer example.
. "$PSScriptRoot\drive_common.ps1"
$SPACE = @(0x20, 0x39); $X = @(0x58, 0x2D); $Y = @(0x59, 0x15); $G = @(0x47, 0x22)
$scratch = $env:TEMP

function Open($name, $settings, $extra) {
  $file = "$scratch\kotor_jump_$name.txt"
  $settings | Set-Content $file -Encoding ascii
  $script:stateFile = "$scratch\kotor_jump_$name.out"
  Remove-Item $script:stateFile -ErrorAction SilentlyContinue
  $argv = @("--mute","--no-tutorials","--no-auto-pause","--no-early-force","--skip-menu","--state-log","--settings",$file,"--size",$Size,"--frames","200000") + $extra
  $script:v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList $argv -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_jump_$name.err" -PassThru
  $script:h = WaitWindow $script:v; [Drv]::NeverActive($script:h)
  WaitLog
}
function Close { FocusReport; Stop-Process -Id $script:v.Id -Force -ErrorAction SilentlyContinue; Start-Sleep -Milliseconds 300 }
# Past what is said on arrival, and half a second of the game's clock on from it.
function Settle {
  $until = (Get-Date).AddSeconds($script:WALL_LIMIT)
  do { $s = Advance } while ($s.dlg -eq "true" -and (Get-Date) -lt $until)
  WaitSteps 60
}
# Press a key, and say how high the player went and whether it paused, over a jump's worth of the game's clock.
function Press1($k) {
  $s = State
  $floor = [double]$s.pcz
  Key $k[0] $k[1]
  $top = $floor; $air = $false
  $last = [int]$s.steps + 70
  $until = (Get-Date).AddSeconds(20)
  do {
    $s = State
    $top = [Math]::Max($top, [double]$s.pcz)
    if ($s.air -eq "true") { $air = $true }
  } while ($s.paused -ne "true" -and [int]$s.steps -lt $last -and (Get-Date) -lt $until)
  @{ up = $top - $floor; air = $air; paused = $s.paused -eq "true"; s = $s }
}
$PLACE = @("--module", "danm14aa", "--place", "165.17,136.66,1.85,1.767")

# As it comes: Modern.
Open "modern" "movement_modern=true" $PLACE
Settle
$r = Press1 $SPACE
Report "Space jumps" ($r.air -and $r.up -gt 0.5 -and $r.up -lt 1.2) "up $([Math]::Round($r.up, 2)) m, air seen $($r.air)"
Report "Space does not pause" (-not $r.paused) "paused=$($r.paused)"
$before = (Press1 $X)
Report "X pauses" $before.paused "paused=$($before.paused)"
$r = @{ s = State }
$z = [double]$r.s.pcz
Key $SPACE[0] $SPACE[1]
$r = State; $r = State
Report "Space in a pause does nothing" ($r.paused -eq "true" -and $r.air -ne "true" -and [double]$r.pcz -eq $z) "paused=$($r.paused) air=$($r.air) z=$($r.pcz)"
Key $X[0] $X[1]
$r = WaitFor "paused" "false"
Report "X goes on" ($r.paused -eq "false") "paused=$($r.paused)"
WaitSteps 20
Report "And no jump was kept for after" ($r.air -ne "true") "air=$($r.air)"
Close

# Rebound in the settings file, as the Key Mapping page writes it. gpu/tests/menus.rs goes through the page itself.
Open "rebound" "movement_modern=true`nkey.Jump=KeyY`nkey.Pause=KeyG" $PLACE
Settle
$r = Press1 $SPACE
Report "Rebound: Space does nothing" (-not $r.air -and -not $r.paused) "air=$($r.air) paused=$($r.paused)"
$r = Press1 $X
Report "Rebound: X does nothing" (-not $r.air -and -not $r.paused) "air=$($r.air) paused=$($r.paused)"
$r = Press1 $Y
Report "Rebound: Y jumps" ($r.air -and $r.up -gt 0.5) "up $([Math]::Round($r.up, 2)) m"
$r = Press1 $G
Report "Rebound: G pauses" $r.paused "paused=$($r.paused)"
Close

# Classic: no jump, and the pause all the same.
Open "classic" "movement_modern=false" ($PLACE + @("--classic-movement"))
Settle
$r = Press1 $SPACE
Report "Classic: Space does not jump" (-not $r.air -and $r.up -lt 0.05) "up $([Math]::Round($r.up, 2)) m, air seen $($r.air)"
Report "Classic: Space does not pause" (-not $r.paused) "paused=$($r.paused)"
$r = Press1 $X
Report "Classic: X pauses" $r.paused "paused=$($r.paused)"
Close

# A conversation: Space goes on with the line and the player stays on the floor.
Open "talk" "movement_modern=true" @("--no-chargen", "--near", "end_locker01")
$s = State
for ($i = 0; $i -lt 300 -and $s.dlg -ne "true"; $i++) { $s = State }
Report "A conversation is under way" ($s.dlg -eq "true") "dlg=$($s.dlg)"
$air = $false; $paused = $false; $said = [int]$s.said; $z = [double]$s.pcz; $high = $z
$until = (Get-Date).AddSeconds($script:WALL_LIMIT)
for ($i = 0; $i -lt 40 -and $s.dlg -eq "true" -and [int]$s.replies -eq 0 -and (Get-Date) -lt $until; $i++) {
  Key $SPACE[0] $SPACE[1]
  $s = State
  if ($s.air -eq "true") { $air = $true }
  if ($s.paused -eq "true") { $paused = $true }
  $high = [Math]::Max($high, [double]$s.pcz)
}
Report "In a conversation Space goes on" ([int]$s.said -gt $said -or [int]$s.replies -gt 0) "said $said -> $($s.said), replies $($s.replies)"
Report "In a conversation Space is no jump" (-not $air -and $high - $z -lt 0.05) "air seen $air, up $([Math]::Round($high - $z, 2)) m"
Report "In a conversation Space does not pause" (-not $paused) "paused seen $paused"
Close

"gate: $(if ($script:failed) { 'FAILED ' + $script:failed } else { 'all PASS' })"
exit $script:failed
