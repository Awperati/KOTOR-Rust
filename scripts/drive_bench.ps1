# The upgrade bench in the running window: on the Ebon Hawk, the repair table opens it. A scope and an energy cell go into Carth's
# Blaster, and the attack bonus goes up. Like drive_window.ps1 it posts messages to the viewer's own handle only, and holds only
# at 1x: see docs/FASTTEST.md.
param([string]$Shots)
# Run from the repo root after building the viewer example. -Shots is a folder for pictures of the window. It writes no saves.
. "$PSScriptRoot\drive_common.ps1"
$script:stateFile = "$scratch\kotor_bench.out"
$v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList @("--mute","--no-tutorials","--no-auto-pause","--no-early-force","--module","ebo_m12aa","--skip-menu","--state-log","--give","g_w_blstrpstl010!,g_i_upgrade001,g_i_upgrade002","--near","RepairTable1","--frames","100000") -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_bench.err" -PassThru -NoNewWindow
for ($i = 0; $i -lt 600 -and $v.MainWindowTitle -notlike "kotor-rs*"; $i++) { Start-Sleep -Milliseconds 50; $v.Refresh() }
$h = $v.MainWindowHandle; [Drv]::NeverActive($h)
"window $h"
$ENTER = @(0x0D, 0x1C); $ONE = @(0x31, 0x02); $E = @(0x45, 0x12)

WaitLog
# Whatever the ship says on arrival.
$until = (Get-Date).AddSeconds(30)
do { Key $ENTER[0] $ENTER[1]; Key $ONE[0] $ONE[1]; $s = State } while ($s.dlg -eq "true" -and (Get-Date) -lt $until)
$attack = [int]$s.attack
Report "On the Ebon Hawk with the blaster worn" ($s.module -eq "ebo_m12aa" -and $s.dlg -eq "false" -and [int]$s.worn -ge 1) "module=$($s.module) dlg=$($s.dlg) worn=$($s.worn) attack=$attack"

function RowClick($place) {
  $p = $place -split ','
  Click ([int]([double]$p[0] * 2)) ([int]([double]$p[1] * 1.5))
  [void](State)
}

# Into the bench, and on to the blaster.
Key $E[0] $E[1]
$s = WaitFor "menu" "bench_kinds"
Report "The table opens the bench" ($s.menu -eq "bench_kinds") "menu=$($s.menu)"
Shot "bench_kinds"
Press 322 104 140 128    # BTN_RANGED
Press 113 410 182 28     # BTN_UPGRADEITEMS
$s = WaitFor "menu" "bench_items"
Report "Ranged lists the blaster" ($s.menu -eq "bench_items" -and $s.row0 -ne "-") "menu=$($s.menu) row0=$($s.row0)"
RowClick $s.row0
Shot "bench_items"
Press 113 410 182 28     # BTN_UPGRADEITEM
$s = WaitFor "menu" "bench"
Report "The blaster comes up with its slots" ($s.menu -eq "bench") "menu=$($s.menu)"

# The scope, in the first slot.
Press 60 325 64 64       # BTN_UPGRADE41
$s = State
RowClick $s.row1
Shot "bench_scope"
# The energy cell, in the second.
Press 127 325 64 64      # BTN_UPGRADE42
$s = State
RowClick $s.row1
Shot "bench_both"
$s = State
Report "Nothing changes before Assemble" ([int]$s.attack -eq $attack) "attack=$($s.attack)"
Press 113 410 182 28     # BTN_ASSEMBLE
$s = WaitFor "menu" "bench_items"
Report "Assemble goes back to the list" ($s.menu -eq "bench_items") "menu=$($s.menu)"
Report "The scope adds to the attack" ([int]$s.attack -eq $attack + 1) "attack $attack -> $($s.attack)"

# And out.
Press 344 410 182 28     # BTN_BACK
[void](WaitFor "menu" "bench_kinds")
Press 344 410 182 28     # BTN_BACK
$s = WaitFor "menu" "-"
Report "Back leaves" ($s.menu -eq "-") "menu=$($s.menu)"
Shot "bench_after"

FocusReport
Stop-Process -Id $v.Id -Force
Get-Content "$scratch\kotor_bench.err" -Tail 4
if ($script:failed) { exit 1 }
exit 0
