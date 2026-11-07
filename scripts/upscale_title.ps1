# Make the upscaled copy of the owner's title card with Topaz Gigapixel AI. Run from the repo root.
# It lands in %LOCALAPPDATA%\kotor-rs\hd: third-party artwork, so never in the repo or the install.
param(
    [string]$Gigapixel = "C:\Program Files\Topaz Labs LLC\Topaz Gigapixel AI\gigapixel.exe",
    [int]$Scale = 4
)
. (Join-Path $PSScriptRoot "gigapixel_guard.ps1")
$root = Join-Path $env:LOCALAPPDATA "kotor-rs"
$card = Join-Path $root "assets\title-card.png"
$hd = Join-Path $root "hd"
if (-not (Test-Path $card)) { "No card at $card"; exit 1 }
$batch = Join-Path $env:TEMP "kotor_gp_title"
if (Test-Path $batch) { Remove-Item -Recurse -Force $batch }
New-Item -ItemType Directory -Force $batch, $hd | Out-Null
Copy-Item $card (Join-Path $batch "kotorrs_title.png")
$log = Join-Path $env:TEMP "kotor_gp_title.log"
$r = Invoke-GuardedGigapixel -Exe $Gigapixel -Log $log -Arguments @("-i", $batch, "-o", $hd, "--cf", "--scale", "$Scale", "-f", "png", "--bd", "8", "--se", "-q")
$r
Remove-Item -Recurse -Force $batch
Get-Content "$log.err" -Tail 3
