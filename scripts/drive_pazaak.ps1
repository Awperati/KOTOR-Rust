# A game of Pazaak in the running window: the wager, a side deck, and sets to the end, by clicks. The credits move by the wager.
# Like drive_window.ps1 it posts messages to the viewer's own handle only, and holds only at 1x: see docs/FASTTEST.md.
param([string]$Shots)
# Run from the repo root after building the viewer example. -Shots is a folder for pictures of the window. It writes no saves.
. "$PSScriptRoot\drive_common.ps1"
$script:stateFile = "$scratch\kotor_pazaak.out"
$v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList @("--mute","--no-tutorials","--no-auto-pause","--no-early-force","--no-chargen","--skip-menu","--state-log","--near","end_locker01","--gold","500","--pazaak","100","--frames","200000") -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_pazaak.err" -PassThru -NoNewWindow
$h = WaitWindow $v; [Drv]::NeverActive($h)
"window $h"
$ENTER = @(0x0D, 0x1C); $ONE = @(0x31, 0x02)

WaitLog
# The opening's lines come first: the game opens when they end.
$until = (Get-Date).AddSeconds(60)
do { $s = Advance } while ($s.menu -ne "pazaak" -and (Get-Date) -lt $until)
Report "The game opens" ($s.menu -eq "pazaak") "menu=$($s.menu) gold=$($s.gold) dlg=$($s.dlg)"
$gold = [int]$s.gold
Shot "pazaak_wager"
Press 206 308 106 22   # BTN_WAGER

# Two each of the first five cards, by their place on the screen, then Play.
foreach ($row in @(@(52,41),@(53,108),@(53,174),@(53,241),@(52,308))) {
  foreach ($n in 1..2) {
    Press $row[0] $row[1] 64 64
    [void](State)
    Press 270 168 114 32   # BTN_YTEXT
    [void](State)
  }
}
Shot "pazaak_setup"
Press 247 446 146 28   # BTN_ATEXT
[void](State)
Shot "pazaak_table"

# Stand on anything, and answer each message, until it is over.
$until = (Get-Date).AddSeconds(120)
$messages = 0
do {
  $s = State
  if ($s.menu -ne "pazaak") { break }
  if ($s.ok -ne "-") {
    if ($messages -eq 0) { Shot "pazaak_message" }
    $messages++
    $p = $s.ok -split ','
    Click ([int]([double]$p[0] * 2)) ([int]([double]$p[1] * 1.5))
  } else {
    Press 438 411 120 28   # BTN_YTEXT
  }
  [void](State)
} while ((Get-Date) -lt $until)
$s = State
Report "It plays to the end" ($s.menu -eq "-" -and $messages -ge 4) "menu=$($s.menu) messages=$messages"
$moved = [Math]::Abs([int]$s.gold - $gold)
Report "The credits move by the wager" ($moved -eq 10) "gold $gold -> $($s.gold)"
Shot "pazaak_after"

FocusReport
Stop-Process -Id $v.Id -Force
Get-Content "$scratch\kotor_pazaak.err" -Tail 4
if ($script:failed) { exit 1 }
exit 0
