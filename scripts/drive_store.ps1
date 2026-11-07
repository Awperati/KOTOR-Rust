# A shop in the running window: Zelka's in tar_m02ac, opened as its script does. Sell, buy, the question and Close.
# Like drive_window.ps1 it posts messages to the viewer's own handle only, and holds only at 1x: see docs/FASTTEST.md.
param([string]$Shots)
# Run from the repo root after building the viewer example. -Shots is a folder for pictures of the window. It writes no saves.
. "$PSScriptRoot\drive_common.ps1"
$script:stateFile = "$scratch\kotor_store.out"
$v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList @("--mute","--no-tutorials","--no-auto-pause","--no-early-force","--module","tar_m02ac","--skip-menu","--state-log","--give","g_w_lngswrd01","--gold","1000","--store","zelkastore2","--frames","100000") -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_store.err" -PassThru -NoNewWindow
$h = WaitWindow $v; [Drv]::NeverActive($h)
"window $h"

WaitLog
$s = WaitFor "menu" "store"
Report "The shop opens" ($s.menu -eq "store") "menu=$($s.menu) gold=$($s.gold) inv=$($s.inv)"
Shot "store_buy"
$gold = [int]$s.gold; $inv = [int]$s.inv

# Nothing is chosen yet, so Buy does nothing.
Press 113 405 125 36   # BTN_Accept
$s = State
Report "Buy with nothing chosen" ([int]$s.gold -eq $gold) "gold=$($s.gold)"

# The first row, and Buy. If it is dear the question comes, and OK answers it.
Press 76 95 239 50     # the first row of LB_SHOPITEMS
[void](State)
Shot "store_chosen"
Press 113 405 125 36
$s = State
if ([int]$s.gold -eq $gold) { Shot "store_question"; Press 268 244 100 22; $s = State }   # BTN_OK of confirm.gui over it
$until = (Get-Date).AddSeconds(5)
while ([int]$s.gold -ge $gold -and (Get-Date) -lt $until) { $s = State }
Report "Buy takes the credits and gives the item" ([int]$s.gold -lt $gold -and [int]$s.inv -gt $inv) "gold $gold -> $($s.gold), inv $inv -> $($s.inv)"
$gold = [int]$s.gold; $inv = [int]$s.inv

# The other list, its first row, and Sell.
Press 240 405 160 36   # BTN_Examine
[void](State)
Press 76 95 239 50     # the first row of LB_INVITEMS
[void](State)
Shot "store_sell"
Press 113 405 125 36
$until = (Get-Date).AddSeconds(5)
do { $s = State } while ([int]$s.gold -le $gold -and (Get-Date) -lt $until)
Report "Sell gives credits" ([int]$s.gold -gt $gold) "gold $gold -> $($s.gold), inv $inv -> $($s.inv)"

# Close, and the game goes on.
Press 402 405 125 36   # BTN_Cancel
$s = WaitFor "menu" "-"
Report "Close leaves the shop" ($s.menu -eq "-") "menu=$($s.menu)"
Shot "store_closed"

FocusReport
Stop-Process -Id $v.Id -Force
Get-Content "$scratch\kotor_store.err" -Tail 4
if ($script:failed) { exit 1 }
exit 0
