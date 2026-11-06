# The number keys in the running window: each reply list of the opening, answered with its last reply, by the number row
# and the keypad in turn. A key past the end of the list does nothing, and a key while a line waits does nothing.
# Like drive_window.ps1 it posts messages to the viewer's own handle only, and holds only at 1x: see docs/FASTTEST.md.
param([string]$Shots)
# Run from the repo root after building the viewer example. -Shots is a folder for pictures of the window. It writes no saves.
. "$PSScriptRoot\drive_common.ps1"
$script:stateFile = "$scratch\kotor_keys.out"
$v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList @("--mute","--no-tutorials","--no-auto-pause","--no-chargen","--no-early-force","--skip-menu","--state-log","--frames","100000") -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_keys.err" -PassThru -NoNewWindow
for ($i = 0; $i -lt 600 -and $v.MainWindowTitle -notlike "kotor-rs*"; $i++) { Start-Sleep -Milliseconds 50; $v.Refresh() }
$h = $v.MainWindowHandle; [Drv]::NeverActive($h)
"window $h"
# Virtual key and scan code: the number row, and the keypad's as it reports them with Num Lock on.
$ROW = @(@(0x31,0x02),@(0x32,0x03),@(0x33,0x04),@(0x34,0x05),@(0x35,0x06),@(0x36,0x07),@(0x37,0x08),@(0x38,0x09),@(0x39,0x0A))
$PAD = @(@(0x61,0x4F),@(0x62,0x50),@(0x63,0x51),@(0x64,0x4B),@(0x65,0x4C),@(0x66,0x4D),@(0x67,0x47),@(0x68,0x48),@(0x69,0x49))
$ENTER = @(0x0D, 0x1C)

WaitLog
$lists = 0; $fromRow = 0; $fromPad = 0; $refused = 0
$seen = @{}
$until = (Get-Date).AddSeconds(90)
while ($lists -lt 6 -and (Get-Date) -lt $until) {
  $s = State
  if ($s.dlg -ne "true") {
    if ($lists -gt 0 -and [int]$s.said -gt 3) { break }
    continue
  }
  $n = [int]$s.replies
  if ($n -eq 0) { Key $ENTER[0] $ENTER[1]; continue }
  # A number past the end does nothing.
  if ($n -lt 9) {
    $said = [int]$s.said
    Key $ROW[$n][0] $ROW[$n][1]
    $s = State
    if ([int]$s.said -eq $said -and [int]$s.replies -eq $n) { $refused++ }
  }
  $said = [int]$s.said
  $set = if ($lists % 2 -eq 0) { $ROW } else { $PAD }
  Key $set[$n-1][0] $set[$n-1][1]
  $moved = $false
  $wait = (Get-Date).AddSeconds(3)
  do { $s = State; $moved = [int]$s.said -gt $said } while (-not $moved -and (Get-Date) -lt $wait)
  if ($moved) { if ($lists % 2 -eq 0) { $fromRow++ } else { $fromPad++ } }
  $seen["$n"] = $true
  "list $lists of $n replies: " + $(if ($lists % 2 -eq 0) { "row" } else { "pad" }) + " $n moved=$moved said $said -> $($s.said)"
  $lists++
}
Report "The number row answers" ($fromRow -gt 0 -and $fromRow -eq [Math]::Ceiling($lists / 2)) "$fromRow of $([Math]::Ceiling($lists / 2)) lists"
Report "The keypad answers" ($fromPad -gt 0 -and $fromPad -eq [Math]::Floor($lists / 2)) "$fromPad of $([Math]::Floor($lists / 2)) lists"
Report "A number past the end does nothing" ($refused -eq $lists) "$refused of $lists lists"

FocusReport
Stop-Process -Id $v.Id -Force
Get-Content "$scratch\kotor_keys.err" -Tail 4
if ($script:failed) { exit 1 }
exit 0
