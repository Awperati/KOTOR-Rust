# What the scripts that drive the viewer share: posted messages to its own handle, its state log, and a count of failures.
# Each script sets $h, $v and $script:stateFile once its window is up.
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
function State {
  # Wait until the window has reported twice since this call: what was posted before it has then been handled.
  $start = (Get-Item $script:stateFile).Length
  $line = $null
  for ($t = 0; $t -lt 300; $t++) {
    $now = (Get-Item $script:stateFile).Length
    if ($now -gt $start) {
      $tail = @(Get-Content $script:stateFile -Tail 3 | Where-Object { $_ -like "state *" })
      if ($tail.Count) { $line = $tail[-1] }
      # This much is two lines.
      if ($line -and $now - $start -gt 2 * $line.Length) { break }
    }
    Start-Sleep -Milliseconds 5
  }
  if (-not $line) { $line = Get-Content $script:stateFile -Tail 2 | Where-Object { $_ -like "state *" } | Select-Object -Last 1 }
  $o = @{}
  foreach ($p in ($line -split ' ')) { if ($p -match '^(\w+)=(.*)$') { $o[$Matches[1]] = $Matches[2] } }
  $o
}
function Key([int]$vk, [int]$scan, [int]$ms = 10) {
  [void][Drv]::PostMessage($h, 0x100, [IntPtr]$vk, [IntPtr](1 -bor ($scan -shl 16)))
  if ($ms) { Start-Sleep -Milliseconds $ms }
  $up = [long]0xC0000001 -bor ([long]$scan -shl 16)
  [void][Drv]::PostMessage($h, 0x101, [IntPtr]$vk, [IntPtr][long]$up)
}
function Hover([int]$x, [int]$y) { [void][Drv]::PostMessage($h, 0x200, [IntPtr]0, [IntPtr](($y -shl 16) -bor $x)) }
function Click([int]$x, [int]$y) {
  Hover $x $y
  $l = [IntPtr](($y -shl 16) -bor $x)
  [void][Drv]::PostMessage($h, 0x201, [IntPtr]1, $l)
  [void][Drv]::PostMessage($h, 0x202, [IntPtr]0, $l)
}
$script:failed = 0
function Press([int]$x, [int]$y, [int]$w, [int]$h2) {
  # A 640x480 screen fills the 1280x720 window, so a control at layout (x, y) is at (2x, 1.5y).
  Click ([int](($x + $w / 2) * 2)) ([int](($y + $h2 / 2) * 1.5))
}
function Shot($name) { if ($Shots) { [void](State); [Drv]::Save($h, "$Shots\$name.png") } }
function Report($name, $ok, $detail) {
  if (-not $ok) { $script:failed++ }
  "{0,-22} {1}  {2}" -f $name, $(if ($ok) { "PASS" } else { "FAIL" }), $detail
}
function WaitFor([string]$name, [string]$want) {
  for ($t = 0; $t -lt 100; $t++) { $s = State; if ($s[$name] -eq $want) { return $s } }
  $s
}
function Moved($a, $b) { [Math]::Abs([double]($a.pc -split ',')[0] - [double]($b.pc -split ',')[0]) + [Math]::Abs([double]($a.pc -split ',')[1] - [double]($b.pc -split ',')[1]) }
function WaitLog {
  # The viewer writes its first state line once the window has drawn.
  for ($t = 0; $t -lt 600; $t++) {
    if ((Test-Path $script:stateFile) -and (Get-Item $script:stateFile).Length -gt 0) { return }
    Start-Sleep -Milliseconds 20
  }
}
