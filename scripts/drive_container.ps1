# A footlocker's panel in the running window: rows chosen with Control, a stack's stepper, Take Selected, Close and Take All.
# Like drive_window.ps1 it posts messages to the viewer's own handle only, and holds only at 1x: see docs/FASTTEST.md.
param([string]$Shots)
# Run from the repo root after building the viewer example. -Shots is a folder for pictures of the window. It writes no saves.
. "$PSScriptRoot\drive_common.ps1"
$script:stateFile = "$scratch\kotor_container.out"
$v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList @("--mute","--no-tutorials","--no-auto-pause","--no-chargen","--no-early-force","--state-log","--skip-menu","--near","end_locker01","--loot","g_i_credits001*50,g_w_lngswrd01,g_i_medeqpmnt01*3","--frames","200000") -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_container.err" -PassThru -NoNewWindow
for ($i = 0; $i -lt 600 -and $v.MainWindowTitle -notlike "kotor-rs*"; $i++) { Start-Sleep -Milliseconds 50; $v.Refresh() }
$h = $v.MainWindowHandle; [Drv]::NeverActive($h)
"window $h"

$E = 0x45, 0x12
$CTRL = 0x11, 0x1D
$SHIFT = 0x10, 0x2A
function Down($k) { [void][Drv]::PostMessage($h, 0x100, [IntPtr]$k[0], [IntPtr](1 -bor ($k[1] -shl 16))) }
function Up($k) { [void][Drv]::PostMessage($h, 0x101, [IntPtr]$k[0], [IntPtr][long]([long]0xC0000001 -bor ([long]$k[1] -shl 16))) }
# The panel is at (164, 68) of the layout. Its rows start at (196, 129), 39 tall, and are 257 wide.
function Row([int]$n) { Press 196 (129 + 39 * $n) 170 39 }
function More([int]$n) { Press 431 (129 + 39 * $n + 7) 18 24 }
function Less([int]$n) { Press 377 (129 + 39 * $n + 7) 18 24 }
function TakeSelected { Press 174 336 285 22 }

WaitLog
# Past the opening talks: one follows another some seconds later, however busy the machine, so it has to stay quiet for a while.
$s = WaitFor "dlg" "true"
$quietSince = Get-Date
$until = (Get-Date).AddSeconds(90)
while (((Get-Date) - $quietSince).TotalSeconds -lt 10 -and (Get-Date) -lt $until) {
  if ($s.dlg -eq "true") { $quietSince = Get-Date; Key 0x31 0x02; Key 0x0D 0x1C }
  $s = State
}
$until = (Get-Date).AddSeconds(15)
while ($s.opened -ne "true" -and (Get-Date) -lt $until) { Key $E[0] $E[1]; $s = State }
Report "E opens the locker" ($s.opened -eq "true" -and [int]$s.items -gt 3) "items=$($s.items) loot=$($s.loot)"
Report "Bug 12: its own things" ([int]$s.items -gt 3) "the three put in, and $([int]$s.items - 3) of its own"
$items = [int]$s.items
$gold = [int]$s.gold
Shot "container_opened"

Row 0
$s = State
Report "A click chooses a row" ([int]$s.chosen -eq 1 -and [int]$s.items -eq $items) "chosen=$($s.chosen) items=$($s.items)"
Row 2
$s = State
Report "Another moves the choice" ([int]$s.chosen -eq 1) "chosen=$($s.chosen)"
Row 0
[void](State)
Down $CTRL
[void](State)
Row 1
$s = State
Report "Control adds a row" ([int]$s.chosen -eq 2) "chosen=$($s.chosen)"
Up $CTRL
[void](State)

# 20 of the 50: two tens with Shift make 21, and one down.
Down $SHIFT
[void](State)
More 0
More 0
[void](State)
Up $SHIFT
[void](State)
Less 0
$s = State
Shot "container_two_rows"
Report "Take Selected waits" ([int]$s.gold -eq $gold -and [int]$s.chosen -eq 2) "gold=$($s.gold) chosen=$($s.chosen)"

TakeSelected
$s = State
$until = (Get-Date).AddSeconds(3)
while ([int]$s.gold -eq $gold -and (Get-Date) -lt $until) { $s = State }
Report "20 credits come" ([int]$s.gold -eq $gold + 20) "gold $gold -> $($s.gold)"
Report "30 stay, and the rest" ($s.loot -like "g_i_credits001*30,g_i_medeqpmnt01*3,*" -and [int]$s.items -eq $items - 1) "loot=$($s.loot)"
Report "The panel stays up" ($s.opened -eq "true" -and [int]$s.chosen -eq 0) "opened=$($s.opened) chosen=$($s.chosen)"
Shot "container_after"

Press 174 361 285 22   # Close
$s = WaitFor "opened" "false"
Report "Close leaves it" ($s.opened -eq "false" -and [int]$s.gold -eq $gold + 20) "opened=$($s.opened) gold=$($s.gold)"
Key $E[0] $E[1]
$s = WaitFor "opened" "true"
Report "It holds the rest" ([int]$s.items -eq $items - 1) "items=$($s.items)"
Press 174 311 285 22   # Take All
$s = WaitFor "opened" "false"
Report "Take All takes it all" ($s.opened -eq "false" -and [int]$s.gold -eq $gold + 50) "opened=$($s.opened) gold=$($s.gold)"

FocusReport
Stop-Process -Id $v.Id -Force
Get-Content "$scratch\kotor_container.err" -Tail 4
if ($script:failed) { exit 1 }
exit 0
