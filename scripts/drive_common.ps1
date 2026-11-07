# What the scripts that drive the viewer share: posted messages to its own handle, its state log, and a count of failures.
# Each script sets $h, $v and $script:stateFile once its window is up.
# Any size: set KOTOR_DRIVE_SIZE, as in 3840x2160. The viewer reads it too, and so do the drivers' -Size. Hover and Click multiply by
# ClickScale. With no -Size that is how much bigger than 1280x720 the window is. A driver with -Size works in the window's own pixels,
# unless it sets ClickScale itself to give its positions at 1280x720. Press always scales.
$script:windowSize = if ($Size) { $Size } elseif ($env:KOTOR_DRIVE_SIZE) { $env:KOTOR_DRIVE_SIZE } else { "1280x720" }
$script:realX = [double]($script:windowSize -split 'x')[0] / 1280
$script:realY = [double]($script:windowSize -split 'x')[1] / 720
$script:ClickScale = if ($Size) { 1.0 } else { $script:realY }
$scratch = $env:TEMP
Add-Type -AssemblyName System.Drawing
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @"
using System; using System.Drawing; using System.Runtime.InteropServices;
public static class Drv {
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] static extern IntPtr GetWindowLongPtr(IntPtr h, int i);
  [DllImport("user32.dll")] static extern IntPtr SetWindowLongPtr(IntPtr h, int i, IntPtr v);
  // Windows activates the next window when the active one closes, and that can be this one. WS_EX_NOACTIVATE stops it.
  // A click cannot activate it either, so only a driver sets it: the owner clicks the window to give it the keys.
  public static void NeverActive(IntPtr h) {
    SetWindowLongPtr(h, -20, (IntPtr)((long)GetWindowLongPtr(h, -20) | 0x08000000L));
  }
  public static string Who(IntPtr h) {
    uint pid; GetWindowThreadProcessId(h, out pid);
    try { return System.Diagnostics.Process.GetProcessById((int)pid).ProcessName + "/" + pid; } catch { return "gone/" + pid; }
  }
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
# The foreground window, watched at every action and every look at the state: who had it, and whether it was ever ours.
$script:focusChanges = New-Object System.Collections.ArrayList
$script:lastForeground = [IntPtr]::Zero
$script:ourForeground = 0
function WatchFocus {
  $f = [Drv]::GetForegroundWindow()
  if ($f -ne $script:lastForeground) {
    $script:lastForeground = $f
    [void]$script:focusChanges.Add((Get-Date).ToString("HH:mm:ss.fff") + " " + [Drv]::Who($f))
  }
  if ($f -eq $h) { $script:ourForeground++ }
}
# Another window taking the foreground is not ours to answer for; this window having it is.
function FocusReport {
  Report "Never took focus" ($script:ourForeground -eq 0) "seen in the foreground $($script:ourForeground) times; foreground changed: $($script:focusChanges -join ', ')"
}
function State {
  WatchFocus
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
  WatchFocus
  [void][Drv]::PostMessage($h, 0x100, [IntPtr]$vk, [IntPtr](1 -bor ($scan -shl 16)))
  if ($ms) { Start-Sleep -Milliseconds $ms }
  $up = [long]0xC0000001 -bor ([long]$scan -shl 16)
  [void][Drv]::PostMessage($h, 0x101, [IntPtr]$vk, [IntPtr][long]$up)
}
function PostHover([int]$x, [int]$y) { [void][Drv]::PostMessage($h, 0x200, [IntPtr]0, [IntPtr](($y -shl 16) -bor $x)) }
function PostClick([int]$x, [int]$y) {
  WatchFocus
  PostHover $x $y
  $l = [IntPtr](($y -shl 16) -bor $x)
  [void][Drv]::PostMessage($h, 0x201, [IntPtr]1, $l)
  [void][Drv]::PostMessage($h, 0x202, [IntPtr]0, $l)
}
function Hover([int]$x, [int]$y) { PostHover ([int]($x * $script:ClickScale)) ([int]($y * $script:ClickScale)) }
function Click([int]$x, [int]$y) { PostClick ([int]($x * $script:ClickScale)) ([int]($y * $script:ClickScale)) }
$script:failed = 0
function Press([int]$x, [int]$y, [int]$w, [int]$h2) {
  # A 640x480 screen fills the window, so in a 1280x720 one a control at layout (x, y) is at (2x, 1.5y).
  PostClick ([int](($x + $w / 2) * 2 * $script:realX)) ([int](($y + $h2 / 2) * 1.5 * $script:realY))
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
