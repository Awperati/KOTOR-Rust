# The galaxy map in the running window: on the Ebon Hawk, the console opens it, a click shows a planet, Cancel leaves, and
# Travel goes. Like drive_window.ps1 it posts messages to the viewer's own handle only, and holds only at 1x: see docs/FASTTEST.md.
param([string]$Shots)
# Run from the repo root after building the viewer example. -Shots is a folder for pictures of the window. It writes no saves.
# K_KOTOR_MASTER 15 is the story's point where the planets open, as `k_pebn_galaxy` has it. No game has got there yet.
. "$PSScriptRoot\drive_common.ps1"
$script:stateFile = "$scratch\kotor_galaxy.out"
$v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList @("--mute","--no-tutorials","--no-auto-pause","--no-early-force","--module","ebo_m12aa","--skip-menu","--state-log","--global","K_KOTOR_MASTER=15","--near","GalaxyMap","--frames","100000") -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_galaxy.err" -PassThru -NoNewWindow
$h = WaitWindow $v; [Drv]::NeverActive($h)
"window $h"
$ENTER = @(0x0D, 0x1C); $ONE = @(0x31, 0x02); $E = @(0x45, 0x12)

WaitLog
# Whatever the ship says on arrival.
$until = (Get-Date).AddSeconds(30)
do { $s = Advance } while ($s.dlg -eq "true" -and (Get-Date) -lt $until)
Report "On the Ebon Hawk" ($s.module -eq "ebo_m12aa" -and $s.dlg -eq "false") "module=$($s.module) dlg=$($s.dlg) menu=$($s.menu)"

# The console.
Key $E[0] $E[1]
$s = WaitFor "menu" "galaxy"
Report "The console opens the map" ($s.menu -eq "galaxy") "menu=$($s.menu)"
Shot "galaxy_opened"

# Cancel leaves, and the console opens it again.
Press 385 413 142 21   # BTN_BACK
$s = WaitFor "menu" "-"
Report "Cancel leaves" ($s.menu -eq "-" -and $s.module -eq "ebo_m12aa") "menu=$($s.menu) module=$($s.module)"
Key $E[0] $E[1]
$s = WaitFor "menu" "galaxy"
Report "It opens again" ($s.menu -eq "galaxy") "menu=$($s.menu)"

# Tatooine, its planet zooming, and Travel.
Press 468 135 24 24    # LBL_Planet_Tatooine
Start-Sleep -Seconds 2
[void](State)
Shot "galaxy_tatooine"
Press 113 413 142 21   # BTN_ACCEPT
$until = (Get-Date).AddSeconds(60)
do { $s = State } while (($s.menu -eq "galaxy" -or $s.module -eq "ebo_m12aa") -and (Get-Date) -lt $until)
Report "Travel leaves the ship" ($s.menu -ne "galaxy" -and $s.module -ne "ebo_m12aa") "menu=$($s.menu) module=$($s.module)"
Shot "galaxy_travelled"

FocusReport
Stop-Process -Id $v.Id -Force
Get-Content "$scratch\kotor_galaxy.err" -Tail 4
if ($script:failed) { exit 1 }
exit 0
