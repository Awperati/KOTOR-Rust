# The launcher's tests, many copies at once on a busy machine: nothing about them may depend on timing.
# Not part of the gate, as it takes the whole processor for a while. Prints a line per failure and a total.
param(
    [int]$Runs = 50,
    # A test program built earlier, for showing what an older version did.
    [string]$TestExe = ""
)
$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
if (-not $TestExe) {
    # Through cmd, as cargo writes to stderr, which PowerShell would take for a failure.
    $line = cmd /c "cargo test --manifest-path `"$repo\Cargo.toml`" -p kotor-launcher --test launcher --no-run --message-format=short 2>&1" |
        Where-Object { $_ -match "Executable.*\((.*)\)" } | Select-Object -Last 1
    if ($line -notmatch "\((.*)\)") { throw "no test program" }
    $TestExe = Join-Path $repo $Matches[1]
}
$work = Join-Path $env:TEMP "kotor-launcher-stress-$PID"
New-Item -ItemType Directory -Force $work | Out-Null

$burners = 1..[Environment]::ProcessorCount | ForEach-Object {
    Start-Process powershell -NoNewWindow -PassThru -ArgumentList "-NoProfile", "-Command", "while(`$true){}"
}
$failed = 0
try {
    $tests = 1..$Runs | ForEach-Object {
        $p = Start-Process $TestExe -NoNewWindow -PassThru `
            -RedirectStandardOutput "$work\$_.out" -RedirectStandardError "$work\$_.err"
        $null = $p.Handle
        @{ Run = $_; Process = $p }
    }
    foreach ($t in $tests) {
        $t.Process.WaitForExit()
        if ($t.Process.ExitCode -ne 0) {
            $failed++
            $why = Select-String -Path "$work\$($t.Run).out" -Pattern "^test .*FAILED|panicked" | Select-Object -First 3
            "run $($t.Run) failed, exit $($t.Process.ExitCode): $($why.Line -join ' | ')"
        }
    }
}
finally {
    $burners | ForEach-Object { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue }
}
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
"stress: $($Runs - $failed) passed, $failed failed"
if ($failed) { exit 1 }
