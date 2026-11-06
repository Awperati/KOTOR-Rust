# Modern movement in the running window: walks, sprints, stops, rolls and falls off a ledge, and writes down speeds and heights.
# Like the other drivers it posts messages to the viewer's own handle only. The window opens on DISPLAY2, is never active, and
# nothing here looks at the primary display. It holds only at 1x: see docs/FASTTEST.md. It writes no saves.
param([string]$Shots, [string]$Size = "1280x720", [string]$Log)
# Run from the repo root after building the viewer example. -Shots is a folder for pictures, -Log a file for the samples.
. "$PSScriptRoot\drive_common.ps1"
$W = @(0x57, 0x11); $SHIFT = @(0x10, 0x2A); $CTRL = @(0x11, 0x1D); $ENTER = @(0x0D, 0x1C); $ONE = @(0x31, 0x02)
# Found by crates\kotor-render\examples\ledges.rs: Taris, a drop of 2.25 m with a walk back up.
$LEDGE_MODULE = "tar_m10aa"; $PLACE = "106.01,129.28,2.25,1.963"; $LEDGE_Z = 2.25; $RUN_MODULE = "danm14aa"
# Sprint is 1.25 times today's run of 5.4 m/s, and a walk is 1.7 m/s: see crates\kotor-game\src\movement.rs.
$SPRINT = 6.75; $WALK = 1.7; $ROLL_METRES = 3.2

function Down($k) { WatchFocus; [void][Drv]::PostMessage($h, 0x100, [IntPtr]$k[0], [IntPtr](1 -bor ($k[1] -shl 16))) }
function Up($k) { $up = [long]0xC0000001 -bor ([long]$k[1] -shl 16); [void][Drv]::PostMessage($h, 0x101, [IntPtr]$k[0], [IntPtr][long]$up) }
function Open($name, $module, $extra) {
  $script:stateFile = "$scratch\kotor_move_$name.out"
  Remove-Item $script:stateFile -ErrorAction SilentlyContinue
  $argv = @("--mute","--no-tutorials","--no-auto-pause","--no-early-force","--module",$module,"--skip-menu","--state-log","--size",$Size,"--frames","100000") + $extra
  $script:v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList $argv -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_move_$name.err" -PassThru -NoNewWindow
  for ($i = 0; $i -lt 600 -and $script:v.MainWindowTitle -notlike "kotor-rs*"; $i++) { Start-Sleep -Milliseconds 50; $script:v.Refresh() }
  $script:h = $script:v.MainWindowHandle; [Drv]::NeverActive($script:h)
  WaitLog
  # Whatever is said on arrival.
  $until = (Get-Date).AddSeconds(20)
  do { Key $ENTER[0] $ENTER[1]; Key $ONE[0] $ONE[1]; $s = State } while ($s.dlg -eq "true" -and (Get-Date) -lt $until)
  # Whatever the fall to the spot was.
  $until = (Get-Date).AddSeconds(5)
  do { $s = State } while ([double]$s.speed -gt 0.05 -and (Get-Date) -lt $until)
  $s
}
# Wait by the game's clock, sixty steps a second: a slow frame at 2160p can drop time, so a sleep is not enough.
function WaitSteps([int]$n) {
  $start = [int](State).steps
  $until = (Get-Date).AddSeconds(60)
  do { Start-Sleep -Milliseconds 20 } while ([int](State).steps -lt $start + $n -and (Get-Date) -lt $until)
}
function Close { Stop-Process -Id $script:v.Id -Force -ErrorAction SilentlyContinue }
# Every state line from a size of the log on, as numbers.
function Samples($from) {
  $f = New-Object IO.FileStream($script:stateFile, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite); $r = New-Object IO.StreamReader($f); $text = $r.ReadToEnd(); $r.Close()
  foreach ($line in ($text.Substring($from) -split "`n")) {
    if ($line -notlike "state *") { continue }
    $o = @{}
    foreach ($p in ($line.Trim() -split ' ')) { if ($p -match '^(\w+)=(.*)$') { $o[$Matches[1]] = $Matches[2] } }
    if (-not $o.steps -or -not $o.target) { continue }
    $xy = $o.pc -split ','
    [pscustomobject]@{ t = [int]$o.steps / 60.0; x = [double]$xy[0]; y = [double]$xy[1]; z = [double]$o.pcz; speed = [double]$o.speed; air = $o.air; roll = $o.roll; fov = [double]$o.fov; clip = $o.clip; yaw = [double]$o.yaw; heading = [double]$o.heading }
  }
}
function Mark { (Get-Item $script:stateFile).Length }
function Note($what, $rows) {
  if ($Log) { "## $what" | Add-Content $Log; $rows | ForEach-Object { "{0:n3} x={1:n2} y={2:n2} z={3:n3} speed={4:n2} air={5} roll={6} fov={7:n1} clip={8}" -f $_.t, $_.x, $_.y, $_.z, $_.speed, $_.air, $_.roll, $_.fov, $_.clip } | Add-Content $Log }
}
function TopSpeed($rows) { ($rows | Measure-Object speed -Maximum).Maximum }
# Metres on the floor over seconds, between two samples: what the player did, not what the body says it does.
function Measured($a, $b) { [Math]::Sqrt(($b.x - $a.x) * ($b.x - $a.x) + ($b.y - $a.y) * ($b.y - $a.y)) / [Math]::Max($b.t - $a.t, 0.001) }

if ($Log) { Remove-Item $Log -ErrorAction SilentlyContinue }
$tag = $Size -replace 'x.*', ''

# The run: a walk, a roll, then at full speed and a stop, over 20 m of clear floor from the spawn.
$s = Open "run" $RUN_MODULE @("--place", "165.17,136.66,1.85,1.767")
Report "In the module" ($s.module -eq $RUN_MODULE) "module=$($s.module) pc=$($s.pc) z=$($s.pcz)"
# Walk and roll first: on a slow frame the sprint can run on until the ground ends, and then nothing moves after it.
$m = Mark
Down $SHIFT; Down $W; WaitSteps 48; Up $W; Up $SHIFT; WaitSteps 24
$rows = @(Samples $m); Note "walk" $rows
$top = TopSpeed $rows
Report "Walk speed" ([Math]::Abs($top - $WALK) -lt 0.1) "top $top m/s, want $WALK"

$m = Mark
$before = State
Key $CTRL[0] $CTRL[1] 30
Start-Sleep -Milliseconds 150
Shot "move_roll_$tag"
WaitSteps 66
$after = State
$rows = @(Samples $m); Note "roll" $rows
$rolled = @($rows | Where-Object { $_.roll -eq "true" })
$xa = $before.pc -split ','; $xb = $after.pc -split ','
$dist = [Math]::Sqrt([Math]::Pow([double]$xb[0] - [double]$xa[0], 2) + [Math]::Pow([double]$xb[1] - [double]$xa[1], 2))
Report "The key rolls" ($rolled.Count -ge 1) "$($rolled.Count) samples rolling, clip $($rolled[0].clip)"
Report "Roll distance" ($dist -gt $ROLL_METRES - 0.2 -and $dist -lt $ROLL_METRES + 1.5) "$([Math]::Round($dist, 2)) m start to stop, the roll itself is $ROLL_METRES m and the rest is its slide"
if ($rolled.Count -ge 2) { Report "Roll time" (($rolled[-1].t - $rolled[0].t) -lt 0.7) "$([Math]::Round($rolled[-1].t - $rolled[0].t, 2)) s between first and last samples" }
FocusReport

$m = Mark
Down $W; WaitSteps 66
Shot "move_sprint_$tag"
WaitSteps 18; Up $W; WaitSteps 42
$rows = @(Samples $m); Note "sprint and stop" $rows
$top = TopSpeed $rows
$fast = @($rows | Where-Object { $_.speed -ge $SPRINT - 0.05 })
$ground = if ($fast.Count -ge 2) { Measured $fast[0] $fast[-1] } else { 0 }
Report "Sprint speed" ([Math]::Abs($top - $SPRINT) -lt 0.1) "top $top m/s, want $SPRINT"
Report "Over the ground" ([Math]::Abs($ground - $SPRINT) -lt 0.5) "$([Math]::Round($ground, 2)) m/s between $($fast.Count) samples"
Report "1.25 times the run" ([Math]::Abs($top / 5.4 - 1.25) -lt 0.025) "ratio $([Math]::Round($top / 5.4, 3))"
$ramp = @($rows | Where-Object { $_.speed -gt 0.2 -and $_.speed -lt $SPRINT - 0.2 })
Report "Speeds up and slows down" ($ramp.Count -ge 1) "$($ramp.Count) samples between a stand and a sprint"
Report "The view widens" ((($rows | Measure-Object fov -Maximum).Maximum) -gt (($rows | Measure-Object fov -Minimum).Minimum + 2)) "fov $(($rows | Measure-Object fov -Minimum).Minimum) to $(($rows | Measure-Object fov -Maximum).Maximum)"
Close

# The ledge.
$s = Open "ledge" $LEDGE_MODULE @("--place", $PLACE)
Report "At the ledge" ([Math]::Abs([double]$s.pcz - $LEDGE_Z) -lt 0.2) "pc=$($s.pc) z=$($s.pcz)"
$m = Mark
$high = [double]$s.pcz
Down $W; WaitSteps 57
Shot "move_ledge_top_$tag"
$until = (Get-Date).AddSeconds(6)
do { $s = State } while ($s.air -eq "true" -or ((Get-Date) -lt $until -and [double]$s.pcz -gt $LEDGE_Z - 1.8))
Up $W
Start-Sleep -Milliseconds 300
Shot "move_landed_$tag"
Start-Sleep -Milliseconds 300
$rows = @(Samples $m); Note "ledge" $rows
$air = @($rows | Where-Object { $_.air -eq "true" })
$low = ($rows | Measure-Object z -Minimum).Minimum
# A slow machine can draw no frame in the air: the slow run below shows the fall itself.
"It fell: $($air.Count) samples in the air, z $high to $low"
Report "It landed below" ($high - $low -gt 1.8 -and $rows[-1].air -eq "false") "z $($rows[-1].z), air=$($rows[-1].air)"
if ($air.Count -ge 3) {
  $falls = for ($i = 1; $i -lt $air.Count; $i++) { ($air[$i-1].z - $air[$i].z) / [Math]::Max($air[$i].t - $air[$i-1].t, 0.001) }
  Report "An arc: the fall quickens" ($falls[-1] -gt $falls[0]) "m/s down: $(($falls | ForEach-Object { [Math]::Round($_, 1) }) -join ', ')"
}
FocusReport
Close
# The roll at a tenth of the speed, for pictures through it.
if ($Shots) {
  $s = Open "slow" $RUN_MODULE @("--place", "165.17,136.66,1.85,1.767", "--speed", "0.1")
  Key $CTRL[0] $CTRL[1] 30
  for ($i = 1; $i -le 5; $i++) { Start-Sleep -Milliseconds 900; [Drv]::Save($h, "$Shots\move_roll_step$($i)_$tag.png") }
  FocusReport
  Close
}
# The fall at a tenth of the speed: at full speed it is over in half a second.
if ($Shots) {
  $s = Open "slowfall" $LEDGE_MODULE @("--place", $PLACE, "--speed", "0.1")
  Down $W
  $until = (Get-Date).AddSeconds(60)
  do { $s = State } while ($s.air -ne "true" -and (Get-Date) -lt $until)
  Up $W
  Report "Slowly, it falls" ($s.air -eq "true") "air=$($s.air) z=$($s.pcz)"
  [Drv]::Save($h, "$Shots\move_fall1_$tag.png")
  Start-Sleep -Milliseconds 2000
  $s = State
  [Drv]::Save($h, "$Shots\move_fall2_$tag.png")
  "fall2 at z=$($s.pcz) air=$($s.air)"
  $until = (Get-Date).AddSeconds(30)
  do { $s = State } while ($s.air -eq "true" -and (Get-Date) -lt $until)
  [Drv]::Save($h, "$Shots\move_landing_$tag.png")
  "landing at z=$($s.pcz) air=$($s.air)"
  FocusReport
  Close
}
# Hold W and drag the camera a quarter turn right, with the look button: the path curves to the new way. Both modes.
function Off($a, $b) { $d = [Math]::Abs($a - $b) % (2 * [Math]::PI); [Math]::Min($d, 2 * [Math]::PI - $d) }
foreach ($mode in @("modern", "classic")) {
  $extra = @("--place", "165.17,136.66,1.85,1.767"); if ($mode -eq "classic") { $extra += "--classic-movement" }
  $s = Open "turn_$mode" $RUN_MODULE $extra
  # The picture first: one can take a second or two, and the ground ends 20 m on.
  Shot "move_turn_before_${mode}_$tag"
  Down $W; WaitSteps 20
  $m = Mark
  $start = State
  Hover 300 300
  [void][Drv]::PostMessage($h, 0x204, [IntPtr]2, [IntPtr]((300 -shl 16) -bor 300))
  foreach ($x in 378, 456, 534, 614) { [void][Drv]::PostMessage($h, 0x200, [IntPtr]2, [IntPtr]((300 -shl 16) -bor $x)); WaitSteps 2 }
  WaitSteps 30
  Shot "move_turn_after_${mode}_$tag"
  $end = State
  $rows = @(Samples $m); Note "camera turn, $mode" $rows
  Report "$mode`: the camera turned" ((Off ([double]$start.yaw - [Math]::PI / 2) ([double]$end.yaw)) -lt 0.1) "yaw $($start.yaw) to $($end.yaw)"
  Report "$mode`: the player faces the new way" ((Off ([double]$end.yaw) ([double]$end.heading)) -lt 0.15) "yaw $($end.yaw), heading $($end.heading)"
  # Measured over the ground: the stretches at a run, as the state's speed is the Modern body's. One stretch is a frame or two, and a slow
  # frame at 2160p throws it out, so the middle value of the later half says the way, and the middle of all of them the speed.
  # Only once the camera has finished turning.
  $legs = @(for ($i = 1; $i -lt $rows.Count; $i++) { if ((Measured $rows[$i-1] $rows[$i]) -gt 3 -and (Off $rows[$i-1].yaw ([double]$end.yaw)) -lt 0.02) { $i } })
  if ($legs.Count -ge 1) {
    $later = @($legs | Select-Object -Last ([Math]::Max([int]($legs.Count / 2), 1)))
    $ways = @(foreach ($i in $later) { [Math]::Atan2($rows[$i].y - $rows[$i-1].y, $rows[$i].x - $rows[$i-1].x) } ) | Sort-Object
    $way = @($ways)[[int](@($ways).Count / 2)]
    Report "$mode`: it runs the new way" ((Off $way ([double]$end.yaw)) -lt 0.35) "going $([Math]::Round($way, 3)), camera $($end.yaw), $($legs.Count) stretches"
  } else { Report "$mode`: it runs the new way" $false "no stretch at a run" }
  $runs = @(foreach ($i in $legs) { Measured $rows[$i-1] $rows[$i] }) | Sort-Object
  $fastest = @($runs)[[int](@($runs).Count / 2)]
  if (-not $fastest) { $fastest = 0 }
  if ($mode -eq "modern") {
    $most = 0.0; for ($i = 1; $i -lt $rows.Count; $i++) { $steps = [Math]::Max(($rows[$i].t - $rows[$i-1].t) * 60, 1); $most = [Math]::Max($most, (Off $rows[$i].heading $rows[$i-1].heading) / $steps) }
    Report "modern: the turn is bounded" ($most -lt 18.8 / 60 + 0.02) "at most $([Math]::Round($most * 60, 1)) rad/s, limit 18.8"
    # A slow machine can draw no frame in the curve, so this is said and not judged. The test in gpu\tests\movement.rs judges it.
    "modern: $(@($rows | Where-Object { (Off $_.heading ([double]$start.heading)) -gt 0.2 -and (Off $_.heading ([double]$end.heading)) -gt 0.2 }).Count) samples part way round, of $($rows.Count)"
  }
  # Let go of both, and press W again: the same way.
  Up $W
  [void][Drv]::PostMessage($h, 0x205, [IntPtr]0, [IntPtr]((300 -shl 16) -bor 614))
  WaitSteps 20
  Down $W; WaitSteps 20; Up $W
  $again = State
  Report "$mode`: pressed again, the same way" ((Off ([double]$again.yaw) ([double]$again.heading)) -lt 0.15) "yaw $($again.yaw), heading $($again.heading)"
  if ($mode -eq "classic") {
    Report "classic: the run is today's" ([Math]::Abs($fastest - 5.4) -lt 0.6) "$([Math]::Round($fastest, 2)) m/s over the ground, want 5.4"
    $before = State; Down $CTRL; Start-Sleep -Milliseconds 80; Up $CTRL; WaitSteps 10; $after = State
    Report "classic: no roll" ($after.roll -ne "true") "roll=$($after.roll)"
  }
  FocusReport
  Close
}

# F8 reads the pose files again, and a mistake is written down. In a folder of its own: never the player's.
$poses = "$scratch\kotor_move_poses"
Remove-Item $poses -Recurse -Force -ErrorAction SilentlyContinue
$s = Open "poses" $RUN_MODULE @("--pose-folder", $poses)
$roll = "$poses\roll_pose.txt"; $said = "$poses\pose_problems.txt"
Report "The pose files are written" ((Test-Path $roll) -and (Test-Path "$poses\landing_pose.txt")) "in $poses"
$good = [IO.File]::ReadAllText($roll)
[IO.File]::WriteAllText($roll, "turns = 1`n[key]`nat = 0`nlthigh_g = 999`n")
Key 0x77 0x42; Start-Sleep -Milliseconds 800
Report "F8 finds a mistake" ((Get-Content $said -Raw) -match "line 4") ((Get-Content $said -Raw).Trim() -replace "`r?`n", " ")
[IO.File]::WriteAllText($roll, $good)
Key 0x77 0x42; Start-Sleep -Milliseconds 800
Report "F8 reads the good file" ((Get-Content $said -Raw) -match "No problems") ((Get-Content $said -Raw).Trim())
Down $CTRL; Start-Sleep -Milliseconds 80; Up $CTRL; Start-Sleep -Milliseconds 300
FocusReport
Close
Remove-Item $poses -Recurse -Force -ErrorAction SilentlyContinue

"gate: $(if ($script:failed) { 'FAILED ' + $script:failed } else { 'all PASS' })"
exit $script:failed
