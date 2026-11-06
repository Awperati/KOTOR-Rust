# Simulates pushes in a throwaway folder, with a stand-in for the game, through the real publish script and launcher.
$ErrorActionPreference = 'Stop'
Set-Location "$PSScriptRoot\.."
cmd /c "cargo build --release --offline -p kotor-launcher 2>&1" | Select-Object -Last 1
$bin = if ($env:CARGO_TARGET_DIR) { Join-Path $env:CARGO_TARGET_DIR 'release' } else { 'target\release' }
$fake = (Resolve-Path "$bin\kotor-fake-game.exe").Path
$launcherExe = (Resolve-Path "$bin\KotorRust.exe").Path
$root = Join-Path $env:TEMP "kotor-launcher-demo-$PID"
New-Item -ItemType Directory -Force $root | Out-Null

function Publish($sha, $subject, $behaviour) {
    powershell -NoProfile -ExecutionPolicy Bypass -File "$PSScriptRoot\publish_build.ps1" -GameExe $fake -LauncherExe $launcherExe -Sha $sha -Subject $subject -Root $root | Select-Object -Last 1
    Set-Content (Join-Path $root "builds\$sha\behaviour.txt") $behaviour
}
function Start-Game($say) {
    $p = Start-Process -FilePath (Join-Path $root 'KotorRust.exe') -ArgumentList '--mute' -PassThru -Wait
    $ran = @(Get-Content (Join-Path $root 'builds\ran.txt'))
    "$say -> exit $($p.ExitCode), last ran: $($ran[-1])"
}

Publish 'aaa1111' 'First push' '0'
Start-Game 'first push'
Publish 'bbb2222' 'Second push' '0'
Start-Game 'second push, restart'
Start-Game 'restart, nothing new'
Publish 'ccc3333' 'Broken push' 'crash'
"--- the launcher waits out nothing here: a crash ends at once"
Start-Game 'broken push, restart'
"bad.txt: $(Get-Content (Join-Path $root 'bad.txt'))"
Start-Game 'restart, the bad build is skipped'
Publish 'ddd4444' 'Fixed push' '0'
Start-Game 'fixed push, restart'
foreach ($n in 'eee5555', 'fff6666', 'ggg7777') { Publish $n "Push $n" '0' | Out-Null }
"builds kept: $((Get-ChildItem (Join-Path $root 'builds') -Directory -Name) -join ' ')"
"--- launcher.log"
Get-Content (Join-Path $root 'launcher.log')
Remove-Item -Recurse -Force $root
