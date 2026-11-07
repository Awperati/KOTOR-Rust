# An enemy that throws, in the running window: a soldier given grenades throws them at the party, and the burst shows.
# Like drive_window.ps1 it posts messages to the viewer's own handle only, and holds only at 1x: see docs/FASTTEST.md.
param([string]$Shots)
# Run from the repo root after building the viewer example. -Shots is a folder for pictures of the window. It writes no saves.
# The pilot plays up to the first fight at its own speed and hands over when an enemy is seen. --grenadier then arms them.
. "$PSScriptRoot\drive_common.ps1"
$script:stateFile = "$scratch\kotor_grenades.out"
$v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList @("--mute","--no-tutorials","--no-auto-pause","--no-early-force","--state-log","--autopilot","--pilot-until","fight","--grenadier","--frames","400000") -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_grenades.err" -PassThru -NoNewWindow
$h = WaitWindow $v; [Drv]::NeverActive($h)
"window $h"

WaitLog
# --grenadier gives the soldier 5 grenades and the player 500 hit points, unhurt. The first look at the state may come after a throw
# or three (a slow window, a busy machine), so the counts are against 5 and 500, not against what the first look saw.
$HAD = 5; $FULL = 500
$until = (Get-Date).AddSeconds(240)
do { $s = State; Start-Sleep -Milliseconds 100 } while ($s.armed -ne "1" -and (Get-Date) -lt $until)
Report "A soldier is armed" ($s.armed -eq "1") "grenades=$($s.grenades) hp=$($s.hp)"
Shot "grenades_before"

# A thrown grenade is one fewer; the last one thrown is gone from the soldier altogether, which the state shows as "-".
# At 2160p the window draws slowly and the game runs a little slower than the clock: the first throw may take a while.
$until = (Get-Date).AddSeconds(240)
$seen = 0
do {
  if ($s.grenades -eq "-" -or [int]$s.grenades -lt $HAD) { $seen = 1; break }
  $s = State
} while ((Get-Date) -lt $until)
Report "It throws one" ($seen -eq 1) "grenades $HAD -> $($s.grenades) thrower(distance/fights/noticed/hp/template)=$($s.thrower)"
# The grenade is in the air for about a second and a half after the throw: wait for it to land.
$until = (Get-Date).AddSeconds(10)
do { $after = State } while ([int]$after.hp -ge $FULL -and (Get-Date) -lt $until)
# A burst is over in a moment: pictures as fast as they come.
if ($Shots) { foreach ($n in 1..8) { [Drv]::Save($h, "$Shots\grenades_burst$n.png") } }
Report "It hurts" ([int]$after.hp -lt $FULL) "hp $FULL -> $($after.hp) (a frag burst takes about 10 to 30)"

FocusReport
Stop-Process -Id $v.Id -Force
Get-Content "$scratch\kotor_grenades.err" -Tail 4
if ($script:failed) { exit 1 }
exit 0
