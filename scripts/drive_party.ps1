# Two in the party in the running window: who sits where on the HUD, and whose face each screen shows.
# Like drive_window.ps1 it posts messages to the viewer's own handle only, and holds only at 1x: see docs/FASTTEST.md.
param([string]$Shots)
# Run from the repo root after building the viewer example. -Shots is a folder for pictures of the window.
# The pilot plays up to the first fight at its own speed and hands over when an enemy is seen. It writes no saves.
. "$PSScriptRoot\drive_common.ps1"
$script:stateFile = "$scratch\kotor_party.out"
$v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList @("--mute","--no-tutorials","--state-log","--autopilot","--pilot-until","fight","--frames","400000") -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_party.err" -PassThru -NoNewWindow
$h = WaitWindow $v; [Drv]::NeverActive($h)
"window $h"
$ESC = @(0x1B, 0x01)
function Screen([int]$vk, [int]$scan, [string]$menu, [string]$name) {
  Key $vk $scan
  $s = WaitFor "menu" $menu
  Report "$menu opens" ($s.menu -eq $menu) "menu=$($s.menu)"
  Shot $name
  Key $ESC[0] $ESC[1]
  [void](WaitFor "menu" "-")
}

WaitLog
$until = (Get-Date).AddSeconds(180)
do { $s = State } while ($s.pilot -ne "false" -and (Get-Date) -lt $until)
Report "The pilot hands over" ($s.pilot -eq "false") "pilot=$($s.pilot) module=$($s.module) pc=$($s.pc) hp=$($s.hp)"
$s = WaitFor "paused" "true"
Shot "party_hud"
$before = $s.leader
# The face beside the leader's takes over. Its place in the state line is in layout units, across a width the window's shape gives.
$p = $s.beside -split ','
Click ([int]([double]$p[0] * 1280 / [double]$s.width)) ([int]([double]$p[1] * 1.5))
$until = (Get-Date).AddSeconds(5)
do { $s = State } while ($s.leader -eq $before -and (Get-Date) -lt $until)
Report "The face beside the leader's takes over" ($s.leader -ne $before) "leader $before -> $($s.leader)"
Shot "party_second_leads"
Screen 0x43 0x2E "character" "party_sheet"
Screen 0x49 0x17 "inventory" "party_inventory"
FocusReport
Stop-Process -Id $v.Id -Force
Get-Content "$scratch\kotor_party.err" -Tail 4
if ($script:failed) { exit 1 }
exit 0
