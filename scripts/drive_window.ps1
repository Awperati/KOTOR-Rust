# Drive the viewer through its OWN window handle with posted messages. No global mouse or keyboard, and no focus.
param([string]$Shots)
# Run from the repo root after building the viewer example. -Shots is a folder for pictures of the window.
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
$v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList @("--mute","--state-log","--near","end_locker01","--frames","200000") -RedirectStandardOutput "$scratch\kotor_drive.out" -RedirectStandardError "$scratch\kotor_drive.err" -PassThru -NoNewWindow
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
Click 220 578
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

