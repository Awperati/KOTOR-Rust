# Light and dark in the running window: the drunks in the Upper City of Taris, whose conversation has both a light and a dark
# answer. The side moves and the sheet shows it. Like drive_window.ps1 it posts messages to the viewer's own handle only, and
# holds only at 1x: see docs/FASTTEST.md.
param([string]$Shots)
# Run from the repo root after building the viewer example. -Shots is a folder for pictures of the window. It writes no saves.
. "$PSScriptRoot\drive_common.ps1"
$script:stateFile = "$scratch\kotor_alignment.out"
$ENTER = @(0x0D, 0x1C); $ONE = @(0x31, 0x02); $THREE = @(0x33, 0x04); $E = @(0x45, 0x12); $ESC = @(0x1B, 0x01); $KEY_C = @(0x43, 0x2E)

function Sheet($name) {
  Key $KEY_C[0] $KEY_C[1]
  $s = WaitFor "menu" "character"
  Report "The sheet opens ($name)" ($s.menu -eq "character") "menu=$($s.menu) align=$($s.align)"
  Start-Sleep -Seconds 1
  [void](State)
  Shot $name
  Key $ESC[0] $ESC[1]
  [void](WaitFor "menu" "-")
}

# Starts a window at this alignment, answers the drunks with this key, and says what became of the side.
function Answer($start, $key, $name) {
  Remove-Item $script:stateFile -ErrorAction SilentlyContinue
  $v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList @("--mute","--no-tutorials","--no-auto-pause","--no-early-force","--module","tar_m02ab","--skip-menu","--state-log","--alignment","$start","--near","Drunk021","--frames","100000") -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_alignment.err" -PassThru -NoNewWindow
  for ($i = 0; $i -lt 600 -and $v.MainWindowTitle -notlike "kotor-rs*"; $i++) { Start-Sleep -Milliseconds 50; $v.Refresh() }
  $h = $v.MainWindowHandle; [Drv]::NeverActive($h)
  Set-Variable -Name h -Value $h -Scope Script
  "window $h"
  WaitLog
  $s = State
  Report "Starts at $start ($name)" ([int]$s.align -eq $start) "align=$($s.align) module=$($s.module)"

  # They speak first, as the player comes near. Up to the choice of three, then the answer.
  if ($s.dlg -ne "true") { Key $E[0] $E[1] }
  $until = (Get-Date).AddSeconds(40)
  do {
    $s = State
    if ([int]$s.replies -eq 3) { break }
    Key $ONE[0] $ONE[1]
  } while ((Get-Date) -lt $until)
  Report "The conversation gets to the answers" ([int]$s.replies -eq 3) "dlg=$($s.dlg) replies=$($s.replies)"
  Shot "alignment_${name}_answers"
  Key $key[0] $key[1]
  $until = (Get-Date).AddSeconds(10)
  do { $s = State } while ([int]$s.align -eq $start -and (Get-Date) -lt $until)
  Shot "alignment_${name}_flash"
  # Whatever is left of what they say.
  $until = (Get-Date).AddSeconds(30)
  while ($s.dlg -eq "true" -and (Get-Date) -lt $until) { Key $ONE[0] $ONE[1]; $s = State }
  $moved = [int]$s.align
  Sheet "alignment_${name}_after"
  FocusReport
  Stop-Process -Id $v.Id -Force
  Get-Content "$scratch\kotor_alignment.err" -Tail 2
  $script:moved = $moved
}

Answer 10 $ONE "light"
$light = $script:moved
Report "A kind answer moves it to the light" ([int]$light -gt 10) "10 -> $light"
Answer 90 $THREE "dark"
$dark = $script:moved
Report "A rough answer moves it to the dark" ([int]$dark -lt 90) "90 -> $dark"
if ($script:failed) { exit 1 }
exit 0
