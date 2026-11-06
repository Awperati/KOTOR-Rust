# Make upscaled copies of the player's female body textures with Topaz Gigapixel AI. Run from the repo root.
# Everything lands in %LOCALAPPDATA%\kotor-rs\hd3d: these are game pictures, so never in the repo or the install.
# The underwear style is applied to them as they load, at their own size. Pictures already upscaled are skipped.
param(
    [string]$Gigapixel = "C:\Program Files\Topaz Labs LLC\Topaz Gigapixel AI\gigapixel.exe",
    [int]$Scale = 2
)
. (Join-Path $PSScriptRoot "gigapixel_guard.ps1")
$hd = Join-Path $env:LOCALAPPDATA "kotor-rs\hd3d"
$src = Join-Path $env:LOCALAPPDATA "kotor-rs\hd3d_source\bodies"
New-Item -ItemType Directory -Force $hd, $src | Out-Null
cargo run --release -q -p kotor-render --example hd_export_bodies -- $src
if ($LASTEXITCODE) { exit 1 }
$inputs = @(Get-ChildItem $src -Filter *.png | Where-Object { -not (Test-Path (Join-Path $hd $_.Name)) })
"$($inputs.Count) pictures to upscale"
if ($inputs.Count -eq 0) { exit 0 }
$batch = Join-Path $env:TEMP "kotor_gp_bodies"
if (Test-Path $batch) { Remove-Item -Recurse -Force $batch }
New-Item -ItemType Directory $batch | Out-Null
$inputs | ForEach-Object { Copy-Item $_.FullName $batch }
$log = Join-Path $env:TEMP "kotor_gp_bodies.log"
# The default model is used: -m std makes this version's engine fail.
$r = Invoke-GuardedGigapixel -Exe $Gigapixel -Log $log -Arguments @("-i", $batch, "-o", $hd, "--cf", "--scale", "$Scale", "-f", "png", "--bd", "8", "--se", "-q")
$r
Remove-Item -Recurse -Force $batch
$files = Get-ChildItem $hd -Filter pfba*.png
"$($r.Seconds) s; $($files.Count) body files in $hd"
Get-Content "$log.err" -Tail 3
