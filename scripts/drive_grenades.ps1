# An enemy that throws, in the running window: a soldier given grenades throws them at the party, and the burst shows.
# Like drive_window.ps1 it posts messages to the viewer's own handle only, and holds only at 1x: see docs/FASTTEST.md.
param([string]$Shots)
# Run from the repo root after building the viewer example. -Shots is a folder for pictures of the window. It writes no saves.
# The pilot plays up to the first fight at its own speed and hands over when an enemy is seen. --grenadier then arms them.
. "$PSScriptRoot\drive_common.ps1"
$script:stateFile = "$scratch\kotor_grenades.out"
$v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList @("--mute","--no-tutorials","--no-auto-pause","--no-early-force","--state-log","--autopilot","--pilot-until","fight","--grenadier","--frames","400000") -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_grenades.err" -PassThru -NoNewWindow
for ($i = 0; $i -lt 600 -and $v.MainWindowTitle -notlike "kotor-rs*"; $i++) { Start-Sleep -Milliseconds 50; $v.Refresh() }
$h = $v.MainWindowHandle; [Drv]::NeverActive($h)
"window $h"

WaitLog
$until = (Get-Date).AddSeconds(240)
do { $s = State; Start-Sleep -Milliseconds 100 } while ((-not $s.grenades -or $s.grenades -eq "-") -and (Get-Date) -lt $until)
Report "A soldier is armed" ($s.grenades -and $s.grenades -ne "-") "grenades=$($s.grenades) hp=$($s.hp)"
$had = [int]$s.grenades
$hp = [int]$s.hp
Shot "grenades_before"

$until = (Get-Date).AddSeconds(90)
$seen = 0
do {
  $s = State
  if ($s.grenades -ne "-" -and [int]$s.grenades -lt $had) { $seen = 1; break }
  if ($s.grenades -eq "-") { break }
} while ((Get-Date) -lt $until)
Report "It throws one" ($seen -eq 1) "grenades $had -> $($s.grenades)"
# A burst is over in a moment: pictures as fast as they come.
if ($Shots) { foreach ($n in 1..8) { [Drv]::Save($h, "$Shots\grenades_burst$n.png") } }
$after = State
Report "It hurts" ([int]$after.hp -lt $hp) "hp $hp -> $($after.hp)"

FocusReport
Stop-Process -Id $v.Id -Force
Get-Content "$scratch\kotor_grenades.err" -Tail 4
if ($script:failed) { exit 1 }
exit 0
