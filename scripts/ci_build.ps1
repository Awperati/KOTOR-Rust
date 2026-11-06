# CI's second job: build the game for release, start it twice with no window on a throwaway home, then publish it.
# Reads the owner's install, read-only, wherever kotor_core::install_dir finds it. Nothing of the game goes in a build.
param([string]$Root = 'C:\KotorRust')
$ErrorActionPreference = 'Stop'

$sha = (git rev-parse --short HEAD).Trim()
$subject = (git log -1 --format=%s).Trim()
$env:KOTOR_BUILD_SHA = $sha
$env:KOTOR_BUILD_DATE = Get-Date -Format 'yyyy-MM-dd'

cmd /c "cargo build --release --offline --manifest-path gpu\Cargo.toml --example viewer 2>&1"
if ($LASTEXITCODE) { throw 'the game did not build' }
cmd /c "cargo build --release --offline -p kotor-launcher --bin KotorRust 2>&1"
if ($LASTEXITCODE) { throw 'the launcher did not build' }

$target = $env:CARGO_TARGET_DIR
$game = if ($target) { Join-Path $target 'release\examples\viewer.exe' } else { 'gpu\target\release\examples\viewer.exe' }
$launcher = if ($target) { Join-Path $target 'release\KotorRust.exe' } else { 'target\release\KotorRust.exe' }

# The smoke: the menu, then the first room. Hidden, muted, on a home of its own, never the owner's settings and saves.
$scratch = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { $env:TEMP }
$home2 = Join-Path $scratch "kotor-smoke-$sha"
New-Item -ItemType Directory -Force $home2 | Out-Null
$realHome = $env:LOCALAPPDATA
$env:LOCALAPPDATA = $home2
try {
    foreach ($how in @('--hidden --mute --frames 120', '--hidden --mute --frames 120 --skip-menu --no-tutorials')) {
        $log = Join-Path $home2 'smoke.log'
        $p = Start-Process -FilePath $game -ArgumentList $how -NoNewWindow -PassThru -RedirectStandardOutput $log -RedirectStandardError "$log.err"
        # Holding the handle is what keeps the exit code readable afterwards.
        $null = $p.Handle
        if (-not $p.WaitForExit(300000)) {
            Stop-Process -Id $p.Id -Force
            throw "smoke timed out: $how"
        }
        $p.WaitForExit()
        if ($p.ExitCode -ne 0) {
            Get-Content "$log.err" -Tail 20
            throw "smoke failed with exit $($p.ExitCode): $how"
        }
        "smoke passed: $how"
    }
} finally {
    $env:LOCALAPPDATA = $realHome
    Remove-Item -Recurse -Force $home2 -ErrorAction SilentlyContinue
}

powershell -NoProfile -ExecutionPolicy Bypass -File "$PSScriptRoot\publish_build.ps1" -GameExe $game -LauncherExe $launcher -Sha $sha -Subject $subject -Root $Root
if ($LASTEXITCODE) { throw 'publishing failed' }
