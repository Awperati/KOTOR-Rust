# Drive the viewer through its OWN window handle with posted messages. No global mouse or keyboard, and no focus.
param([string]$Shots)
# Run from the repo root after building the viewer example. -Shots is a folder for pictures of the window.
# It writes one save to %LOCALAPPDATA%\kotor-rs\saves, deletes it through the screen, and leaves other saves alone.
$scratch = $env:TEMP
Add-Type -AssemblyName System.Drawing
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @"
using System; using System.Drawing; using System.Runtime.InteropServices;
public static class Drv {
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L,T,R,B; }
  public static void Save(IntPtr h, string path) {
    RECT r; GetWindowRect(h, out r);
    using (var b = new Bitmap(r.R - r.L, r.B - r.T)) {
      using (var g = Graphics.FromImage(b)) { var dc = g.GetHdc(); PrintWindow(h, dc, 2); g.ReleaseHdc(dc); }
      b.Save(path, System.Drawing.Imaging.ImageFormat.Png);
    }
  }
}
"@
$v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList @("--mute","--state-log","--skip-menu","--near","end_locker01","--frames","200000") -RedirectStandardOutput "$scratch\kotor_drive.out" -RedirectStandardError "$scratch\kotor_drive.err" -PassThru -NoNewWindow
$h = [IntPtr]::Zero
for ($i = 0; $i -lt 60 -and $h -eq [IntPtr]::Zero; $i++) { Start-Sleep -Milliseconds 500; $v.Refresh(); $h = $v.MainWindowHandle }
"window $h"
$before = [Drv]::GetForegroundWindow()
function State {
  Start-Sleep -Milliseconds 150
  $line = Get-Content "$scratch\kotor_drive.out" -Tail 2 | Where-Object { $_ -like "state *" } | Select-Object -Last 1
  $o = @{}
  foreach ($p in ($line -split ' ')) { if ($p -match '^(\w+)=(.*)$') { $o[$Matches[1]] = $Matches[2] } }
  $o
}
function Key([int]$vk, [int]$scan, [int]$ms = 60) {
  [void][Drv]::PostMessage($h, 0x100, [IntPtr]$vk, [IntPtr](1 -bor ($scan -shl 16)))
  Start-Sleep -Milliseconds $ms
  $up = [long]0xC0000001 -bor ([long]$scan -shl 16)
  [void][Drv]::PostMessage($h, 0x101, [IntPtr]$vk, [IntPtr][long]$up)
  Start-Sleep -Milliseconds 80
}
function Hover([int]$x, [int]$y) { [void][Drv]::PostMessage($h, 0x200, [IntPtr]0, [IntPtr](($y -shl 16) -bor $x)); Start-Sleep -Milliseconds 80 }
function Click([int]$x, [int]$y) {
  Hover $x $y
  $l = [IntPtr](($y -shl 16) -bor $x)
  [void][Drv]::PostMessage($h, 0x201, [IntPtr]1, $l)
  Start-Sleep -Milliseconds 50
  [void][Drv]::PostMessage($h, 0x202, [IntPtr]0, $l)
  Start-Sleep -Milliseconds 120
}
function Report($name, $ok, $detail) { "{0,-22} {1}  {2}" -f $name, $(if ($ok) { "PASS" } else { "FAIL" }), $detail }
$SPACE = @(0x20, 0x39); $ONE = @(0x31, 0x02); $E = @(0x45, 0x12); $W = @(0x57, 0x11); $D = @(0x44, 0x20)

# Past the opening's beats with Space, up to Trask's reply list.
$s = State
for ($i = 0; $i -lt 120 -and [int]$s.replies -eq 0; $i++) { Key $SPACE[0] $SPACE[1]; Start-Sleep -Milliseconds 120; $s = State }
Report "Space skips beats" ([int]$s.replies -gt 0) "replies=$($s.replies) said=$($s.said)"
if ($Shots) { [Drv]::Save($h, "$Shots\live_replies.png") }

# A click on the first reply row (layout y 378 of 480, in a 720 high client area).
$said = [int]$s.said
Click 220 578
$s = State
Report "Reply click" ([int]$s.said -gt $said) "said $said -> $($s.said)"

# Through the rest of the talk with the 1 key and Space.
for ($i = 0; $i -lt 200 -and $s.dlg -eq "true"; $i++) { Key $ONE[0] $ONE[1]; Key $SPACE[0] $SPACE[1]; $s = State }
Report "Keys finish the talk" ($s.dlg -eq "false") "dlg=$($s.dlg) said=$($s.said)"
Start-Sleep -Seconds 1

# The footlocker beside the player: E opens it, a click takes the first item, Space closes it.
$s = State
"before E: target=$($s.target) opened=$($s.opened)"
Key $E[0] $E[1]
Start-Sleep -Milliseconds 400
$s = State
Report "E opens the locker" ($s.opened -eq "true" -and [int]$s.items -gt 0) "opened=$($s.opened) items=$($s.items)"
if ($Shots) { [Drv]::Save($h, "$Shots\live_locker.png") }
$inv = [int]$s.inv
Click 645 222   # the first row of container.gui
$s = State
Report "Click takes an item" ([int]$s.inv -eq $inv + 1) "inv $inv -> $($s.inv)"
Key $SPACE[0] $SPACE[1]
$s = State
Report "Space closes it" ($s.opened -eq "false") "opened=$($s.opened)"

# W runs ahead; D runs to the right.
$a = State
Key $W[0] $W[1] 900
$b = State
$moved = [Math]::Abs([double]($a.pc -split ',')[0] - [double]($b.pc -split ',')[0]) + [Math]::Abs([double]($a.pc -split ',')[1] - [double]($b.pc -split ',')[1])
Report "W moves" ($moved -gt 0.3) "$($a.pc) -> $($b.pc)"
if ($Shots) { [Drv]::Save($h, "$Shots\live_wasd.png") }
$a = State
Key $D[0] $D[1] 900
$b = State
$moved = [Math]::Abs([double]($a.pc -split ',')[0] - [double]($b.pc -split ',')[0]) + [Math]::Abs([double]($a.pc -split ',')[1] - [double]($b.pc -split ',')[1])
Report "D moves" ($moved -gt 0.3) "$($a.pc) -> $($b.pc)"

# A click on the floor walks there: look for one that lands.
$ok = $false; $detail = ""
foreach ($y in 600, 560, 520, 480, 640) {
  foreach ($x in 640, 400, 880) {
    $a = State
    for ($k = 0; $k -lt 80 -and $a.dlg -eq "true"; $k++) { Key $ONE[0] $ONE[1]; Key $SPACE[0] $SPACE[1]; $a = State }
    Click $x $y
    Start-Sleep -Milliseconds 700
    $b = State
    $moved = [Math]::Abs([double]($a.pc -split ',')[0] - [double]($b.pc -split ',')[0]) + [Math]::Abs([double]($a.pc -split ',')[1] - [double]($b.pc -split ',')[1])
    $detail = "click $x,$y $($a.pc) -> $($b.pc) clip=$($b.clip) opened=$($b.opened) dlg=$($b.dlg)"
    if ($b.opened -eq "true") { Key $SPACE[0] $SPACE[1] }
    if ($moved -gt 0.3) { $ok = $true; break }
  }
  if ($ok) { break }
}
Report "Click walks" $ok $detail

$during = [Drv]::GetForegroundWindow()
Report "Never took focus" ($during -ne $h) "foreground before=$before now=$during viewer=$h"
Stop-Process -Id $v.Id -Force
Get-Content "$scratch\kotor_drive.err" -Tail 4

# The second run: from the main menu, through a save and two loads, to Exit. Layout units are 853 wide on this
# 1280x720 window, and a 640 wide screen is centred, so a control at layout x is at (x + 106) * 1.5.
$savesDir = Join-Path $env:LOCALAPPDATA "kotor-rs\saves"
$savesBefore = @(Get-ChildItem $savesDir -Filter *.sav -ErrorAction SilentlyContinue | ForEach-Object { $_.FullName })
$v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList @("--mute","--state-log","--frames","200000") -RedirectStandardOutput "$scratch\kotor_drive2.out" -RedirectStandardError "$scratch\kotor_drive2.err" -PassThru -NoNewWindow
$h = [IntPtr]::Zero
for ($i = 0; $i -lt 60 -and $h -eq [IntPtr]::Zero; $i++) { Start-Sleep -Milliseconds 500; $v.Refresh(); $h = $v.MainWindowHandle }
"window $h"
$before = [Drv]::GetForegroundWindow()
function State {
  Start-Sleep -Milliseconds 150
  $line = Get-Content "$scratch\kotor_drive2.out" -Tail 2 | Where-Object { $_ -like "state *" } | Select-Object -Last 1
  $o = @{}
  foreach ($p in ($line -split ' ')) { if ($p -match '^(\w+)=(.*)$') { $o[$Matches[1]] = $Matches[2] } }
  $o
}
function Press([int]$x, [int]$y, [int]$w, [int]$h2) {
  Click ([int](($x + $w / 2 + 106) * 1.5)) ([int](($y + $h2 / 2) * 1.5))
  Start-Sleep -Milliseconds 300
}
function WaitFor([string]$name, [string]$want) {
  for ($t = 0; $t -lt 60; $t++) { $s = State; if ($s[$name] -eq $want) { return $s }; Start-Sleep -Milliseconds 300 }
  $s
}
function Moved($a, $b) { [Math]::Abs([double]($a.pc -split ',')[0] - [double]($b.pc -split ',')[0]) + [Math]::Abs([double]($a.pc -split ',')[1] - [double]($b.pc -split ',')[1]) }
function Shot($name) { if ($Shots) { Start-Sleep -Milliseconds 300; [Drv]::Save($h, "$Shots\$name.png") } }
$ESC = @(0x1B, 0x01); $ENTER = @(0x0D, 0x1C); $BACK = @(0x08, 0x0E); $KEY_I = @(0x49, 0x17); $KEY_U = @(0x55, 0x16); $KEY_C = @(0x43, 0x2E)

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
  Key $SPACE[0] $SPACE[1] 20; Key $ONE[0] $ONE[1] 20; $s = State
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
for ($i = 0; $i -lt 34; $i++) { Key $BACK[0] $BACK[1] 15 }
foreach ($k in @(@(0x44, 0x20), @(0x52, 0x13), @(0x49, 0x17), @(0x56, 0x2F), @(0x45, 0x12))) { Key $k[0] $k[1] 30 }
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
Key $W[0] $W[1] 900
Key $D[0] $D[1] 500
Start-Sleep -Milliseconds 300
$away = State
Report "Walked away" ((Moved $saved $away) -gt 0.3) "$($saved.pc) -> $($away.pc)"
Key $ESC[0] $ESC[1]
[void](WaitFor "menu" "pause")
Press 60 110 240 35    # BTN_LOADGAME
$s = WaitFor "menu" "load"
Press 69 88 246 50     # first row
Shot "live_load_list"
Press 253 410 134 28   # BTN_SAVELOAD
$s = WaitFor "menu" "-"
Start-Sleep -Milliseconds 400
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
Key $KEY_C[0] $KEY_C[1]
$s = WaitFor "menu" "character"
Report "C opens the character" ($s.menu -eq "character") "menu=$($s.menu)"
Shot "live_character"
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
Press 253 410 134 28
$s = WaitFor "module" "end_m01aa"
Start-Sleep -Milliseconds 400
$s = State
Report "Load from the main menu" ($s.menu -eq "-" -and $s.module -eq "end_m01aa" -and (Moved $saved $s) -lt 0.3) "module=$($s.module) pc=$($s.pc)"

# Delete asks once, then the save is gone.
Key $ESC[0] $ESC[1]
[void](WaitFor "menu" "pause")
Press 60 110 240 35
$s = WaitFor "menu" "load"
$count = [int]$s.saves
Press 69 88 246 50
Press 113 410 138 28   # BTN_DELETE
$s = State
$asked = [int]$s.saves -eq $count
Press 113 410 138 28
$s = State
$left = @(Get-ChildItem $savesDir -Filter *.sav -ErrorAction SilentlyContinue | Where-Object { $savesBefore -notcontains $_.FullName })
Report "Delete asks, then deletes" ($asked -and [int]$s.saves -eq $count - 1 -and $left.Count -eq 0) "saves $count -> $($s.saves), driver's files left $($left.Count)"

# Back out to the main menu, and Exit closes the window.
Press 389 410 138 28
[void](WaitFor "menu" "pause")
Press 60 364 240 35
[void](WaitFor "menu" "main")
$during = [Drv]::GetForegroundWindow()
Report "Never took focus" ($during -ne $h) "foreground before=$before now=$during viewer=$h"
Press 285 318 210 27   # BTN_EXIT
$gone = $v.WaitForExit(8000)
Report "Exit closes the window" $gone "exited=$gone"
if (-not $gone) { Stop-Process -Id $v.Id -Force }
foreach ($f in $left) { Remove-Item $f.FullName -Force }
Get-Content "$scratch\kotor_drive2.err" -Tail 4
