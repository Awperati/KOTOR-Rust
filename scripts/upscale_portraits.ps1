# Make sharper copies of the portraits with Topaz Gigapixel AI, over those upscale_ui.ps1 made. Run from the repo root after it.
# A portrait is 64 by 64 in the game: at 4 times with no face recovery it is still soft at 1440p. See bug 4 in docs/STATUS.md.
# Everything lands in %LOCALAPPDATA%\kotor-rs\hd: these are game pictures, so never in the repo or the install.
# Portraits already this size are skipped, so a stopped run can be started again.
param(
    [string]$Gigapixel = "C:\Program Files\Topaz Labs LLC\Topaz Gigapixel AI\gigapixel.exe",
    [int]$Scale = 8,
    [int]$FaceRecovery = 80
)
. (Join-Path $PSScriptRoot "gigapixel_guard.ps1")
Add-Type -AssemblyName System.Drawing
$hd = Join-Path $env:LOCALAPPDATA "kotor-rs\hd"
$src = Join-Path $env:LOCALAPPDATA "kotor-rs\hd_source"
function WidthOf($file) { $i = [Drawing.Image]::FromFile($file); try { $i.Width } finally { $i.Dispose() } }
$inputs = @(Get-ChildItem $src -Filter po_*.png | Where-Object {
    $done = Join-Path $hd $_.Name
    -not (Test-Path $done) -or (WidthOf $done) -lt (WidthOf $_.FullName) * $Scale
})
"$($inputs.Count) portraits to upscale"
if ($inputs.Count -eq 0) { exit 0 }
$batch = Join-Path $env:TEMP "kotor_gp_portraits"
if (Test-Path $batch) { Remove-Item -Recurse -Force $batch }
New-Item -ItemType Directory $batch | Out-Null
$inputs | ForEach-Object { Copy-Item $_.FullName $batch }
$log = Join-Path $env:TEMP "kotor_gp_portraits.log"
# Into a folder of its own and then over the old ones, so that a stopped run leaves what was there.
$made = Join-Path $env:TEMP "kotor_gp_portraits_out"
if (Test-Path $made) { Remove-Item -Recurse -Force $made }
$r = Invoke-GuardedGigapixel -Exe $Gigapixel -Log $log -Arguments @("-i", $batch, "-o", $made, "--cf", "--scale", "$Scale", "--fr", "$FaceRecovery", "--frv", "2", "-f", "png", "--bd", "8", "-q")
$r
Get-ChildItem $made -Filter *.png | Copy-Item -Destination $hd -Force
"$((Get-ChildItem $made -Filter *.png).Count) portraits made"
Remove-Item -Recurse -Force $batch, $made
Get-Content "$log.err" -Tail 3
