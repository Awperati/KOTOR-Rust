# Make upscaled copies of the interface pictures with Topaz Gigapixel AI. Run from the repo root.
# Everything lands in %LOCALAPPDATA%\kotor-rs\hd: these are game pictures, so never in the repo or the install.
# Pictures already upscaled are skipped, so a stopped run can be started again.
param(
    [string]$Gigapixel = "C:\Program Files\Topaz Labs LLC\Topaz Gigapixel AI\gigapixel.exe",
    [int]$Scale = 4,
    [int]$Limit = 0
)
$hd = Join-Path $env:LOCALAPPDATA "kotor-rs\hd"
$src = Join-Path $env:LOCALAPPDATA "kotor-rs\hd_source"
New-Item -ItemType Directory -Force $hd, $src | Out-Null
cargo run --release -q -p kotor-ui --example hd_export -- $src
if ($LASTEXITCODE) { exit 1 }
$inputs = @(Get-ChildItem $src -Filter *.png | Where-Object { -not (Test-Path (Join-Path $hd $_.Name)) })
if ($Limit -gt 0) { $inputs = @($inputs | Select-Object -First $Limit) }
"$($inputs.Count) pictures to upscale"
if ($inputs.Count -eq 0) { exit 0 }
$batch = Join-Path $env:TEMP "kotor_gp_batch"
if (Test-Path $batch) { Remove-Item -Recurse -Force $batch }
New-Item -ItemType Directory $batch | Out-Null
$inputs | ForEach-Object { Copy-Item $_.FullName $batch }
# The default model is used: -m std makes this version's engine fail.
$log = Join-Path $env:TEMP "kotor_gp_batch.log"
$p = Start-Process -FilePath $Gigapixel -ArgumentList @("-i", $batch, "-o", $hd, "--cf", "--scale", "$Scale", "-f", "png", "--bd", "8", "--se", "-q") -RedirectStandardOutput $log -RedirectStandardError "$log.err" -PassThru -NoNewWindow
$p.WaitForExit()
Remove-Item -Recurse -Force $batch
"$((Get-ChildItem $hd -Filter *.png).Count) of $((Get-ChildItem $src -Filter *.png).Count) upscaled pictures in $hd"
Get-Content "$log.err" -Tail 3
