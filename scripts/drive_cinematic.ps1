# The opening conversation in the running window, for frames of a character up close, as the owner's frames of the bare skin are.
# Like drive_window.ps1 it posts messages to the viewer's own handle only. Run from the repo root after building the viewer example.
param([string]$Shots, [int]$Appearance = -1, [string]$Size = "1280x720", [int]$Lines = 6, [switch]$Original)
. "$PSScriptRoot\drive_common.ps1"
$script:stateFile = "$scratch\kotor_cinematic_$PID.out"
$look = if ($Appearance -ge 0) { @("--appearance", "$Appearance") } else { @() }
$style = if ($Original) { @("--original-underwear") } else { @() }
$v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList (@("--mute", "--no-tutorials", "--state-log", "--no-chargen", "--skip-menu", "--size", $Size, "--frames", "40000") + $look + $style) -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_cinematic_$PID.err" -PassThru -NoNewWindow
for ($i = 0; $i -lt 600 -and $v.MainWindowTitle -notlike "kotor-rs*"; $i++) { Start-Sleep -Milliseconds 50; $v.Refresh() }
$h = $v.MainWindowHandle; [Drv]::NeverActive($h)
"window $h"
$SPACE = @(0x20, 0x39)
WaitLog
$s = WaitFor "dlg" "true"
Report "A conversation opens" ($s.dlg -eq "true") "dlg=$($s.dlg) module=$($s.module)"
for ($n = 0; $n -lt $Lines; $n++) {
  # Let the camera settle on the line.
  Start-Sleep -Milliseconds 700
  Shot ("line{0}" -f $n)
  Key $SPACE[0] $SPACE[1]
}
FocusReport
Stop-Process -Id $v.Id -Force
if ($script:failed) { exit 1 }
exit 0
