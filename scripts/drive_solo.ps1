# Solo Mode in the running window: the key asks, Yes turns it on and shows its button on the HUD, No leaves things as they are,
# and the button turns it off. Like drive_window.ps1 it posts messages to the viewer's own handle only, and holds only at 1x: see
# docs/FASTTEST.md.
param([string]$Shots)
# Run from the repo root after building the viewer example. -Shots is a folder for pictures of the window. It writes no saves.
. "$PSScriptRoot\drive_common.ps1"
$script:stateFile = "$scratch\kotor_solo.out"
$v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList @("--mute","--no-tutorials","--no-auto-pause","--no-early-force","--no-chargen","--skip-menu","--state-log","--near","end_locker01","--frames","100000") -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_solo.err" -PassThru -NoNewWindow
for ($i = 0; $i -lt 600 -and $v.MainWindowTitle -notlike "kotor-rs*"; $i++) { Start-Sleep -Milliseconds 50; $v.Refresh() }
$h = $v.MainWindowHandle; [Drv]::NeverActive($h)
"window $h"
$ENTER = @(0x0D, 0x1C); $ONE = @(0x31, 0x02); $ESC = @(0x1B, 0x01); $KEY_V = @(0x56, 0x2F)

WaitLog
# The opening's lines come first, and a line can start a moment after the last: quiet means quiet for three seconds.
$until = (Get-Date).AddSeconds(90)
$quiet = (Get-Date)
do {
  $s = State
  if ($s.dlg -eq "true") {
    Key $ENTER[0] $ENTER[1]; Key $ONE[0] $ONE[1]
    $quiet = (Get-Date)
  }
} while (((Get-Date) - $quiet).TotalSeconds -lt 3 -and (Get-Date) -lt $until)
Report "In the game, with the party following" ($s.dlg -eq "false" -and $s.solo -eq "false") "dlg=$($s.dlg) solo=$($s.solo)"
Shot "solo_off"

# Esc: nothing changes.
Key $KEY_V[0] $KEY_V[1]
$s = WaitFor "menu" "solo"
Report "The key asks" ($s.menu -eq "solo") "menu=$($s.menu)"
Shot "solo_ask"
Key $ESC[0] $ESC[1]
$s = WaitFor "menu" "-"
Report "Esc leaves it off" ($s.menu -eq "-" -and $s.solo -eq "false") "menu=$($s.menu) solo=$($s.solo)"

# Yes.
Key $KEY_V[0] $KEY_V[1]
$s = WaitFor "menu" "solo"
$p = $s.ok -split ','
Click ([int]([double]$p[0] * 2)) ([int]([double]$p[1] * 1.5))
$s = WaitFor "menu" "-"
Report "Yes turns it on" ($s.solo -eq "true") "menu=$($s.menu) solo=$($s.solo)"
Start-Sleep -Milliseconds 300
[void](State)
Shot "solo_on"

# And off.
Key $KEY_V[0] $KEY_V[1]
$s = WaitFor "menu" "solo"
Shot "solo_ask_off"
$p = $s.ok -split ','
Click ([int]([double]$p[0] * 2)) ([int]([double]$p[1] * 1.5))
$s = WaitFor "menu" "-"
Report "Yes again turns it off" ($s.solo -eq "false") "menu=$($s.menu) solo=$($s.solo)"

FocusReport
Stop-Process -Id $v.Id -Force
Get-Content "$scratch\kotor_solo.err" -Tail 4
if ($script:failed) { exit 1 }
exit 0
