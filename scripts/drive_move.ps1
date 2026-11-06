# Modern movement in the running window: walks, sprints, stops, rolls and falls off a ledge, and writes down speeds and heights.
# Like the other drivers it posts messages to the viewer's own handle only. The window opens on DISPLAY2, is never active, and
# nothing here looks at the primary display. It holds only at 1x: see docs/FASTTEST.md. It writes no saves.
param([string]$Shots, [string]$Size = "1280x720", [string]$Log)
# Run from the repo root after building the viewer example. -Shots is a folder for pictures, -Log a file for the samples.
. "$PSScriptRoot\drive_common.ps1"
$W = @(0x57, 0x11); $SHIFT = @(0x10, 0x2A); $CTRL = @(0x11, 0x1D); $ENTER = @(0x0D, 0x1C); $ONE = @(0x31, 0x02)
# Found by crates\kotor-render\examples\ledges.rs: Dantooine, a run-up to a drop of 4.4 m.
$PLACE = "284.40,108.71,7.27,4.320"
# Sprint is 1.25 times today's run of 5.4 m/s, and a walk is 1.7 m/s: see crates\kotor-game\src\movement.rs.
$SPRINT = 6.75; $WALK = 1.7; $ROLL_METRES = 3.2

function Down($k) { WatchFocus; [void][Drv]::PostMessage($h, 0x100, [IntPtr]$k[0], [IntPtr](1 -bor ($k[1] -shl 16))) }
function Up($k) { $up = [long]0xC0000001 -bor ([long]$k[1] -shl 16); [void][Drv]::PostMessage($h, 0x101, [IntPtr]$k[0], [IntPtr][long]$up) }
function Open($name, $extra) {
  $script:stateFile = "$scratch\kotor_move_$name.out"
  Remove-Item $script:stateFile -ErrorAction SilentlyContinue
  $argv = @("--mute","--no-tutorials","--no-auto-pause","--no-early-force","--module","danm14aa","--skip-menu","--state-log","--size",$Size,"--frames","100000") + $extra
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
    [pscustomobject]@{ t = [int]$o.steps / 60.0; x = [double]$xy[0]; y = [double]$xy[1]; z = [double]$o.pcz; speed = [double]$o.speed; air = $o.air; roll = $o.roll; fov = [double]$o.fov; clip = $o.clip }
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

# The run: at full speed, then a stop, a walk and a roll, over 20 m of clear floor from the spawn.
$s = Open "run" @("--place", "165.17,136.66,1.85,1.767")
Report "In the module" ($s.module -eq "danm14aa") "module=$($s.module) pc=$($s.pc) z=$($s.pcz)"
$m = Mark
Down $W; Start-Sleep -Milliseconds 1100
$s = State; Shot "move_sprint_$tag"
Start-Sleep -Milliseconds 300; Up $W; Start-Sleep -Milliseconds 700
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

$m = Mark
Down $SHIFT; Down $W; Start-Sleep -Milliseconds 800; Up $W; Up $SHIFT; Start-Sleep -Milliseconds 400
$rows = @(Samples $m); Note "walk" $rows
$top = TopSpeed $rows
Report "Walk speed" ([Math]::Abs($top - $WALK) -lt 0.1) "top $top m/s, want $WALK"

$m = Mark
$before = State
Key $CTRL[0] $CTRL[1] 30
Start-Sleep -Milliseconds 150
Shot "move_roll_$tag"
Start-Sleep -Milliseconds 1100
$after = State
$rows = @(Samples $m); Note "roll" $rows
$rolled = @($rows | Where-Object { $_.roll -eq "true" })
$xa = $before.pc -split ','; $xb = $after.pc -split ','
$dist = [Math]::Sqrt([Math]::Pow([double]$xb[0] - [double]$xa[0], 2) + [Math]::Pow([double]$xb[1] - [double]$xa[1], 2))
Report "The key rolls" ($rolled.Count -ge 1) "$($rolled.Count) samples rolling, clip $($rolled[0].clip)"
Report "Roll distance" ($dist -gt $ROLL_METRES - 0.2 -and $dist -lt $ROLL_METRES + 1.5) "$([Math]::Round($dist, 2)) m start to stop, the roll itself is $ROLL_METRES m and the rest is its slide"
if ($rolled.Count -ge 2) { Report "Roll time" (($rolled[-1].t - $rolled[0].t) -lt 0.7) "$([Math]::Round($rolled[-1].t - $rolled[0].t, 2)) s between first and last samples" }
FocusReport
Close

# The ledge.
$s = Open "ledge" @("--place", $PLACE)
Report "At the ledge" ([Math]::Abs([double]$s.pcz - 7.27) -lt 0.2) "pc=$($s.pc) z=$($s.pcz)"
$m = Mark
$high = [double]$s.pcz
Down $W; Start-Sleep -Milliseconds 950
Shot "move_ledge_top_$tag"
$until = (Get-Date).AddSeconds(6)
do { $s = State; if ($s.air -eq "true" -and $Shots) { [Drv]::Save($h, "$Shots\move_fall_$tag.png") } } while ($s.air -eq "true" -or ((Get-Date) -lt $until -and [double]$s.pcz -gt 3))
Up $W
Start-Sleep -Milliseconds 300
Shot "move_landed_$tag"
Start-Sleep -Milliseconds 300
$rows = @(Samples $m); Note "ledge" $rows
$air = @($rows | Where-Object { $_.air -eq "true" })
$low = ($rows | Measure-Object z -Minimum).Minimum
Report "It fell" ($air.Count -ge 1) "$($air.Count) samples in the air, z $high to $low"
Report "It landed below" ($low -lt 3 -and $rows[-1].air -eq "false") "z $($rows[-1].z), air=$($rows[-1].air)"
if ($air.Count -ge 3) {
  $falls = for ($i = 1; $i -lt $air.Count; $i++) { ($air[$i-1].z - $air[$i].z) / [Math]::Max($air[$i].t - $air[$i-1].t, 0.001) }
  Report "An arc: the fall quickens" ($falls[-1] -gt $falls[0]) "m/s down: $(($falls | ForEach-Object { [Math]::Round($_, 1) }) -join ', ')"
}
FocusReport
Close
# The roll at a tenth of the speed, for pictures through it.
if ($Shots) {
  $s = Open "slow" @("--place", "165.17,136.66,1.85,1.767", "--speed", "0.1")
  Key $CTRL[0] $CTRL[1] 30
  for ($i = 1; $i -le 5; $i++) { Start-Sleep -Milliseconds 900; [Drv]::Save($h, "$Shots\move_roll_step$($i)_$tag.png") }
  FocusReport
  Close
}
"gate: $(if ($script:failed) { 'FAILED ' + $script:failed } else { 'all PASS' })"
exit $script:failed
