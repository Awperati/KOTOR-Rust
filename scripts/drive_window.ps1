# Drive the viewer through its OWN window handle with posted messages. No global mouse or keyboard, and no focus.
# It sleeps on the wall clock between real key presses, so its checks hold only at 1x: see docs/FASTTEST.md.
param([string]$Size = $(if ($env:KOTOR_DRIVE_SIZE) { $env:KOTOR_DRIVE_SIZE } else { "1280x720" }), [string]$Shots, [int]$Speed = 1)
# Run from the repo root after building the viewer example. -Shots is a folder for pictures of the window.
# It writes one save to a folder of its own with --saves, deletes it through the screen, and never touches the player's saves.
. "$PSScriptRoot\drive_common.ps1"
$script:ClickScale = [double]($Size -split "x")[1] / 720.0
$savesDir = Join-Path $scratch "kotor_window_saves_$PID"
New-Item -ItemType Directory -Force $savesDir | Out-Null
$v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList @("--size",$Size,"--mute","--no-tutorials","--no-auto-pause","--no-chargen","--state-log","--saves",$savesDir,"--skip-menu","--near","end_locker01","--frames","200000","--speed","$Speed") -RedirectStandardOutput "$scratch\kotor_drive.out" -RedirectStandardError "$scratch\kotor_drive.err" -PassThru -NoNewWindow
$h = [IntPtr]::Zero
$h = WaitWindow $v; [Drv]::NeverActive($h)
"window $h"
$script:stateFile = "$scratch\kotor_drive.out"
$SPACE = @(0x20, 0x39); $RETURN = @(0x0D, 0x1C); $ONE = @(0x31, 0x02); $E = @(0x45, 0x12); $W = @(0x57, 0x11); $D = @(0x44, 0x20)

# Past the opening's beats with Space, up to Trask's reply list.
WaitLog
$s = State
for ($i = 0; $i -lt 400 -and [int]$s.replies -eq 0; $i++) { Key $SPACE[0] $SPACE[1]; $s = State }
Report "Space skips beats" ([int]$s.replies -gt 0) "replies=$($s.replies) said=$($s.said)"
if ($Shots) { [Drv]::Save($h, "$Shots\live_replies.png") }

# A click on the first reply row (layout y 378 of 480, in a 720 high client area).
$said = [int]$s.said
Click 220 578
# The next line can take a few frames to be said, and a state line can be from before the click.
$s = UntilSteps { param($o) [int]$o.said -gt $said } 300
Report "Reply click" ([int]$s.said -gt $said) "said $said -> $($s.said)"

# Through the rest of the talk with the 1 key and Space.
for ($i = 0; $i -lt 200 -and $s.dlg -eq "true"; $i++) { $s = Advance }
Report "Keys finish the talk" ($s.dlg -eq "false") "dlg=$($s.dlg) said=$($s.said)"

# The footlocker beside the player: E opens it, a click chooses the first item, Take Selected takes it, Space closes it.
$s = State
"before E: target=$($s.target) opened=$($s.opened)"
Key $E[0] $E[1]
$s = WaitFor "opened" "true"
Report "E opens the locker" ($s.opened -eq "true" -and [int]$s.items -gt 0) "opened=$($s.opened) items=$($s.items)"
if ($Shots) { [Drv]::Save($h, "$Shots\live_locker.png") }
$inv = [int]$s.inv
Click 648 222   # the first row of container.gui
Press 174 336 285 22   # Take Selected
$s = State
Report "Click takes an item" ([int]$s.inv -eq $inv + 1) "inv $inv -> $($s.inv)"
Key $SPACE[0] $SPACE[1]
$s = State
Report "Space closes it" ($s.opened -eq "false") "opened=$($s.opened)"

# W runs ahead; D runs to the right.
$a = State
$b = HoldUntilMoved $W $a 0.3 60
$moved = Moved $a $b
Report "W moves" ($moved -gt 0.3) "$($a.pc) -> $($b.pc)"
if ($Shots) { [Drv]::Save($h, "$Shots\live_wasd.png") }
$a = State
$b = HoldUntilMoved $D $a 0.3 60
$moved = Moved $a $b
Report "D moves" ($moved -gt 0.3) "$($a.pc) -> $($b.pc)"

# X pauses out of a conversation: W does nothing until X again.
$X = @(0x58, 0x2D)
Key $X[0] $X[1]
[void](State)
$a = State
Key $W[0] $W[1] 300
$b = State
Report "X pauses" ((Moved $a $b) -lt 0.05) "$($a.pc) -> $($b.pc)"
Key $X[0] $X[1]
[void](State)
$a = State
$b = HoldUntilMoved $W $a 0.3 60
Report "X goes on" ((Moved $a $b) -gt 0.3) "$($a.pc) -> $($b.pc)"

# A click on the floor walks there: look for one that lands.
$ok = $false; $detail = ""
foreach ($y in 600, 560, 520, 480, 640) {
  foreach ($x in 640, 400, 880) {
    $a = State
    for ($k = 0; $k -lt 80 -and $a.dlg -eq "true"; $k++) { $a = Advance }
    Click $x $y
    $b = UntilSteps { param($o) (Moved $a $o) -gt 0.3 -or $o.opened -eq "true" } 30
    $moved = Moved $a $b
    $detail = "click $x,$y $($a.pc) -> $($b.pc) clip=$($b.clip) opened=$($b.opened) dlg=$($b.dlg)"
    if ($b.opened -eq "true") { Key $SPACE[0] $SPACE[1] }
    if ($moved -gt 0.3) { $ok = $true; break }
  }
  if ($ok) { break }
}
Report "Click walks" $ok $detail

FocusReport
Stop-Process -Id $v.Id -Force
Get-Content "$scratch\kotor_drive.err" -Tail 4

# The second run: from the main menu, through a save and two loads, to Exit. A 640x480 screen fills this 1280x720
# window, so a control at layout (x, y) is at (2x, 1.5y).
$savesBefore = @(Get-ChildItem $savesDir -Filter *.sav -ErrorAction SilentlyContinue | ForEach-Object { $_.FullName })
$v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList @("--size",$Size,"--mute","--no-tutorials","--no-auto-pause","--no-chargen","--state-log","--saves",$savesDir,"--frames","200000") -RedirectStandardOutput "$scratch\kotor_drive2.out" -RedirectStandardError "$scratch\kotor_drive2.err" -PassThru -NoNewWindow
$h = [IntPtr]::Zero
$h = WaitWindow $v; [Drv]::NeverActive($h)
"window $h"
$script:stateFile = "$scratch\kotor_drive2.out"
$ESC = @(0x1B, 0x01); $ENTER = @(0x0D, 0x1C); $BACK = @(0x08, 0x0E); $KEY_I = @(0x49, 0x17); $KEY_U = @(0x55, 0x16); $KEY_C = @(0x43, 0x2E)

WaitLog
$s = WaitFor "menu" "main"
Report "Opens on the main menu" ($s.menu -eq "main" -and $s.module -eq "-") "menu=$($s.menu) module=$($s.module)"
Shot "live_main_menu"
Press 285 294 210 22   # BTN_OPTIONS
$s = WaitFor "menu" "options"
Report "Options opens" ($s.menu -eq "options") "menu=$($s.menu)"
Press 71 289 240 40    # BTN_SOUND
$s = WaitFor "menu" "sound"
Report "Sound page opens" ($s.menu -eq "sound") "menu=$($s.menu)"
Shot "live_sound"
Key $ESC[0] $ESC[1]
[void](WaitFor "menu" "options")
Key $ESC[0] $ESC[1]
[void](WaitFor "menu" "main")
Press 285 222 210 22   # BTN_NEWGAME
$s = WaitFor "module" "end_m01aa"
Report "New Game starts it" ($s.module -eq "end_m01aa" -and $s.menu -eq "-") "menu=$($s.menu) module=$($s.module)"

# Past the opening and the first talk, to where a save is allowed.
$quiet = 0
for ($i = 0; $i -lt 400 -and $quiet -lt 6; $i++) {
  $s = Advance
  if ($s.dlg -eq "false" -and [int]$s.said -gt 3) { $quiet++ } else { $quiet = 0 }
}
Report "Through the opening" ($quiet -ge 6) "dlg=$($s.dlg) said=$($s.said)"

Key $ESC[0] $ESC[1]
$s = WaitFor "menu" "pause"
Report "Esc pauses" ($s.menu -eq "pause") "menu=$($s.menu)"
Shot "live_pause"
Press 60 145 240 35    # BTN_SAVEGAME
$s = WaitFor "menu" "save"
Report "Save Game opens" ($s.menu -eq "save") "menu=$($s.menu) saves=$($s.saves) message from the screen if not"
$savesAt = [int]$s.saves
Press 253 410 134 28   # BTN_SAVELOAD
$s = WaitFor "menu" "name"
Report "Save asks a name" ($s.menu -eq "name") "menu=$($s.menu)"
for ($i = 0; $i -lt 34; $i++) { Key $BACK[0] $BACK[1] 0 }
foreach ($k in @(@(0x44, 0x20), @(0x52, 0x13), @(0x49, 0x17), @(0x56, 0x2F), @(0x45, 0x12))) { Key $k[0] $k[1] 5 }
Shot "live_save_name"
Key $ENTER[0] $ENTER[1]
$s = WaitFor "menu" "save"
Report "Enter writes the save" ($s.menu -eq "save" -and [int]$s.saves -eq $savesAt + 1) "saves $savesAt -> $($s.saves)"
$new = @(Get-ChildItem $savesDir -Filter *.sav -ErrorAction SilentlyContinue | Where-Object { $savesBefore -notcontains $_.FullName })
Report "It is in the saves folder" ($new.Count -eq 1) "$($new[0].FullName) $($new[0].Length) bytes"
Shot "live_save_list"
Press 389 410 138 28   # BTN_BACK
$s = WaitFor "menu" "pause"
Press 393 410 146 28   # BTN_EXIT
$saved = WaitFor "menu" "-"
Report "Back in the game" ($saved.menu -eq "-") "menu=$($saved.menu) pc=$($saved.pc)"

# Move away, then load: the player is back where it was saved.
Key $W[0] $W[1] 300
Key $D[0] $D[1] 200
$away = State
Report "Walked away" ((Moved $saved $away) -gt 0.3) "$($saved.pc) -> $($away.pc)"
Key $ESC[0] $ESC[1]
[void](WaitFor "menu" "pause")
Press 60 110 240 35    # BTN_LOADGAME
$s = WaitFor "menu" "load"
Press 69 88 246 50     # first row
[void](State)
Shot "live_load_list"
Press 253 410 134 28   # BTN_SAVELOAD
$s = WaitFor "menu" "-"
$s = State
Report "Load puts it back" ($s.menu -eq "-" -and (Moved $saved $s) -lt 0.3) "$($away.pc) -> $($s.pc), saved at $($saved.pc)"
Shot "live_loaded"

# The item screens.
Key $KEY_I[0] $KEY_I[1]
$s = WaitFor "menu" "inventory"
Report "I opens the inventory" ($s.menu -eq "inventory") "menu=$($s.menu)"
Shot "live_inventory"
Key $KEY_U[0] $KEY_U[1]
$s = WaitFor "menu" "equip"
Report "U opens the equipment" ($s.menu -eq "equip") "menu=$($s.menu)"
Shot "live_equip"
Press 207 182 64 64    # BTN_INV_BODY
[void](State)
Shot "live_equip_slot"
Key $ESC[0] $ESC[1]
[void](State)
$s = State
Report "Esc leaves a slot's list first" ($s.menu -eq "equip") "menu=$($s.menu)"
Key $KEY_C[0] $KEY_C[1]
$s = WaitFor "menu" "character"
Report "C opens the character" ($s.menu -eq "character") "menu=$($s.menu)"
Shot "live_character"
Key 0x4A 0x24
$s = WaitFor "menu" "journal"
Report "J opens the journal" ($s.menu -eq "journal") "menu=$($s.menu)"
Shot "live_journal"
Key 0x4D 0x32
$s = WaitFor "menu" "map"
Report "M opens the map" ($s.menu -eq "map") "menu=$($s.menu)"
Shot "live_map"
Key $ESC[0] $ESC[1]
$s = WaitFor "menu" "-"
Report "Esc closes it" ($s.menu -eq "-") "menu=$($s.menu)"

# Exit Game to the main menu, and load from there.
Key $ESC[0] $ESC[1]
[void](WaitFor "menu" "pause")
Press 60 364 240 35    # BTN_QUIT
$s = WaitFor "menu" "main"
Report "Exit Game goes to the menu" ($s.menu -eq "main" -and $s.module -eq "-") "menu=$($s.menu) module=$($s.module)"
Press 285 246 210 22   # BTN_LOADGAME
$s = WaitFor "menu" "load"
Press 69 88 246 50
[void](State)
Press 253 410 134 28
$s = WaitFor "module" "end_m01aa"
$s = State
Report "Load from the main menu" ($s.menu -eq "-" -and $s.module -eq "end_m01aa" -and (Moved $saved $s) -lt 0.3) "module=$($s.module) pc=$($s.pc)"

# Delete asks once, then the save is gone.
Key $ESC[0] $ESC[1]
[void](WaitFor "menu" "pause")
Press 60 110 240 35
$s = WaitFor "menu" "load"
$count = [int]$s.saves
Press 69 88 246 50
[void](State)
Press 113 410 138 28   # BTN_DELETE
$s = State
$asked = [int]$s.saves -eq $count
Press 113 410 138 28
$s = WaitFor "saves" "$($count - 1)"
$left = @(Get-ChildItem $savesDir -Filter *.sav -ErrorAction SilentlyContinue | Where-Object { $savesBefore -notcontains $_.FullName })
Report "Delete asks, then deletes" ($asked -and [int]$s.saves -eq $count - 1 -and $left.Count -eq 0) "saves $count -> $($s.saves), driver's files left $($left.Count)"

# Back out to the main menu, and Exit closes the window.
Press 389 410 138 28
[void](WaitFor "menu" "pause")
Press 60 364 240 35
[void](WaitFor "menu" "main")
FocusReport
Press 285 318 210 27   # BTN_EXIT
$gone = $v.WaitForExit(10000)
Report "Exit closes the window" $gone "exited=$gone"
if (-not $gone) { Stop-Process -Id $v.Id -Force }
foreach ($f in $left) { Remove-Item $f.FullName -Force }
Get-Content "$scratch\kotor_drive2.err" -Tail 4
# A failed check fails the script, so nothing that reads only its exit code takes it for a pass.
Remove-Item -Recurse -Force $savesDir -ErrorAction SilentlyContinue
if ($script:failed) { exit 1 }
exit 0
