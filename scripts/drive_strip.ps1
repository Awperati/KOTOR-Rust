# A strip of frames of the running window, for a live look at animation: the pilot plays on its own and, when a foe is in
# sight, a frame is taken every -Interval milliseconds, -Count times. Like drive_window.ps1 it posts messages to the
# viewer's own handle only and never takes focus. Run from the repo root after building the viewer example.
# -Shots is a folder (made if missing) for the pictures; -Size is the window's, like 1280x720 or 3840x2160.
# -Scenario fight: wait for a foe on the screen; conversation: wait for a conversation to open (the opening one).
param([string]$Shots, [string]$Size = $(if ($env:KOTOR_DRIVE_SIZE) { $env:KOTOR_DRIVE_SIZE } else { "1280x720" }), [int]$Count = 12, [int]$Interval = 250, [string]$Scenario = "fight", [int]$Appearance = -1, [string]$Underwear = "")
. "$PSScriptRoot\drive_common.ps1"
if ($Shots) { [void](New-Item -ItemType Directory -Force -Path $Shots) }
$script:stateFile = "$scratch\kotor_strip_$PID.out"
$look = if ($Appearance -ge 0) { @("--appearance", "$Appearance") } else { @() }
if ($Underwear) { $look += @("--underwear", "`"$Underwear`"") }
$mode = if ($Scenario -eq "fight") { @("--autopilot", "--no-auto-pause", "--no-early-force", "--grenadier") } else { @("--no-chargen", "--skip-menu") }
$v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList (@("--mute", "--no-tutorials", "--state-log", "--size", $Size, "--frames", "400000") + $mode + $look) -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_strip_$PID.err" -PassThru -NoNewWindow
for ($i = 0; $i -lt 600 -and $v.MainWindowTitle -notlike "kotor-rs*"; $i++) { Start-Sleep -Milliseconds 50; $v.Refresh() }
$h = $v.MainWindowHandle; [Drv]::NeverActive($h)
"window $h"
WaitLog
$until = (Get-Date).AddSeconds(300)
do {
  $s = State
  $ready = if ($Scenario -eq "fight") { $s.foe -and $s.foe -ne "-" } else { $s.dlg -eq "true" }
  Start-Sleep -Milliseconds 100
} while (-not $ready -and (Get-Date) -lt $until)
Report "The scene is up" $ready "foe=$($s.foe) dlg=$($s.dlg) hp=$($s.hp)"
for ($n = 0; $n -lt $Count; $n++) {
  Shot ("{0}_{1:D2}" -f $Scenario, $n)
  Start-Sleep -Milliseconds $Interval
}
FocusReport
Stop-Process -Id $v.Id -Force
if ($script:failed) { exit 1 }
exit 0
