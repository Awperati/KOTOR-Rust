# The gate: formatting, lints, tests. Run before every commit with no switches.
# Tests that read the game skip themselves when the install is missing; set KOTOR_DIR to override the path.
#   -Fast        no fmt, no clippy, no doc tests: for the inner loop, not for commits
#   -Only NAME   only this crate's tests, for example kotor-game, or gpu for everything in gpu\ (implies -Fast)
#   -Jobs N      test binaries at once (default: half the logical processors, at least 2)
#   -BinaryLimit S  seconds a test binary may run (default 600). Past that it is killed, named, and counts as failed.
#                   autopilot gets 1500 s: 72 s alone, and 450 to over 600 s among the others on a loaded machine.
# A binary that uses the GPU is marked [GPU] in what it prints, so that a hang can be placed.
# It prints a last line "gate: P passed, F failed" and exits 0 only when F is 0 and nothing else failed.
param([switch]$Fast, [string]$Only = '', [int]$Jobs = 0, [int]$BinaryLimit = 600)
$ErrorActionPreference = 'Stop'
if ($Only) { $Fast = $true }
if ($Jobs -le 0) { $Jobs = [Math]::Max(2, [int]([Environment]::ProcessorCount / 2)) }
$started = [Diagnostics.Stopwatch]::StartNew()
$root = (Get-Location).Path
$gpu = Test-Path gpu\Cargo.lock

function Fail($why) { "gate: $why"; exit 1 }

# Seconds this test binary may run.
function Limit-Of($bin) { if ($bin.name -eq 'autopilot') { [Math]::Max($BinaryLimit, 1500) } else { $BinaryLimit } }

# Build the tests of a cargo workspace, and return each test binary with the folder cargo would run it in.
function Build-Tests([string[]]$CargoArgs) {
    $lines = cmd /c "cargo test --no-run --message-format=json-render-diagnostics $($CargoArgs -join ' ') 2>&1"
    $code = $LASTEXITCODE
    $bins = @()
    foreach ($l in $lines) {
        if ($l -notlike '{*') {
            # What cargo says for itself, errors included.
            if ($l -match '^error|^warning: unused') { $l }
            continue
        }
        if ($l -notlike '*"compiler-artifact"*' -or $l -notlike '*"executable":"*') { continue }
        $m = $l | ConvertFrom-Json
        if ($m.executable -and $m.profile.test) {
            $bins += [pscustomobject]@{ exe = $m.executable; cwd = Split-Path $m.manifest_path; name = $m.target.name }
        }
    }
    if ($code) { Fail "tests did not build: cargo test --no-run $($CargoArgs -join ' ')" }
    $script:built += $bins
}

# ' [GPU]' for a test binary whose source uses the GPU.
function Gpu-Mark($bin) {
  $source = Join-Path $bin.cwd "tests\$($bin.name).rs"
  if ((Test-Path $source) -and (Select-String -Path $source -Pattern 'GpuRenderer|Presenter' -Quiet)) { ' [GPU]' } else { '' }
}

# Run every test binary, $Jobs at a time, and add up what they say.
function Run-Tests($bins) {
    $out = Join-Path $env:TEMP "kotor_check_$PID"
    New-Item -ItemType Directory -Force $out | Out-Null
    $queue = [Collections.Queue]::new()
    $n = 0
    foreach ($b in $bins) { $n++; $queue.Enqueue(@{ bin = $b; log = "$out\$n.txt"; err = "$out\$n.err" }) }
    $running = @()
    $results = @()
    while ($queue.Count -or $running.Count) {
        while ($queue.Count -and $running.Count -lt $Jobs) {
            $t = $queue.Dequeue()
            $t.proc = Start-Process -FilePath $t.bin.exe -WorkingDirectory $t.bin.cwd -NoNewWindow -PassThru `
                -RedirectStandardOutput $t.log -RedirectStandardError $t.err
            $null = $t.proc.Handle
            $t.watch = [Diagnostics.Stopwatch]::StartNew()
            $running += $t
        }
        Start-Sleep -Milliseconds 20
        $still = @()
        foreach ($t in $running) {
            if ($t.proc.HasExited) {
                $t.proc.WaitForExit()
                $t.code = $t.proc.ExitCode
                $t.seconds = $t.watch.Elapsed.TotalSeconds
                $results += $t
            } elseif ($t.watch.Elapsed.TotalSeconds -gt (Limit-Of $t.bin)) {
                # A hang must not stop the gate for good, and must not pass.
                Stop-Process -Id $t.proc.Id -Force -ErrorAction SilentlyContinue
                $t.proc.WaitForExit()
                $t.code = 'timeout'
                $t.timedOut = $true
                $t.seconds = $t.watch.Elapsed.TotalSeconds
                $results += $t
            } else { $still += $t }
        }
        $running = $still
    }
    $passed = 0; $failed = 0; $broken = 0
    foreach ($t in ($results | Sort-Object { $_.seconds } -Descending)) {
        $text = Get-Content $t.log -Raw -ErrorAction SilentlyContinue
        $m = [regex]::Match([string]$text, 'test result: \w+\. (\d+) passed; (\d+) failed')
        if ($m.Success) {
            $passed += [int]$m.Groups[1].Value
            $failed += [int]$m.Groups[2].Value
        }
        if ($t.timedOut) {
            $failed++
            "--- $($t.bin.name)$(Gpu-Mark $t.bin) TIMED OUT: killed after $(Limit-Of $t.bin) s"
        }
        if ($t.code -ne 0 -or -not $m.Success) {
            $broken++
            "--- $($t.bin.name)$(Gpu-Mark $t.bin) exit $($t.code)"
            $text
            Get-Content $t.err -Tail 20 -ErrorAction SilentlyContinue
        }
        if ($t.seconds -ge 2) { "{0,7:n1} s  {1}{2}" -f $t.seconds, $t.bin.name, (Gpu-Mark $t.bin) }
    }
    Remove-Item $out -Recurse -Force -ErrorAction SilentlyContinue
    $script:passed += $passed
    $script:failed += $failed
    $script:broken += $broken
}

$script:passed = 0; $script:failed = 0; $script:broken = 0; $script:built = @()
$wantMain = $Only -ne 'gpu'
$wantGpu = $gpu -and ((-not $Only) -or $Only -eq 'gpu')

if (-not $Fast) {
    cargo fmt --all -- --check
    if ($LASTEXITCODE) { Fail 'fmt' }
    cargo clippy --workspace --all-targets -- -D warnings
    if ($LASTEXITCODE) { Fail 'clippy' }
    if ($gpu) {
        # The GPU package is standalone; once its crates have been fetched (gpu/Cargo.lock exists) it must build and lint too.
        cargo fmt --manifest-path gpu\Cargo.toml --all -- --check
        if ($LASTEXITCODE) { Fail 'fmt gpu' }
        cargo clippy --manifest-path gpu\Cargo.toml --offline --workspace --all-targets -- -D warnings
        if ($LASTEXITCODE) { Fail 'clippy gpu' }
    }
}
if ($wantMain) {
    if ($Only) { Build-Tests @('-p', $Only) } else { Build-Tests @('--workspace') }
}
if ($wantGpu) { Build-Tests @('--manifest-path', 'gpu\Cargo.toml', '--offline', '--workspace', '--release') }
Run-Tests $script:built

if (-not $Fast) {
    # No crate has doc tests today. They stay in the full gate so that one added later runs.
    cmd /c "cargo test --workspace --doc > `"$env:TEMP\kotor_check_doc_$PID.txt`" 2>&1"
    if ($LASTEXITCODE) { Get-Content "$env:TEMP\kotor_check_doc_$PID.txt"; $script:broken++ }
    Remove-Item "$env:TEMP\kotor_check_doc_$PID.txt" -ErrorAction SilentlyContinue
}

"{0:n1} s, {1} test binaries, {2} at once" -f $started.Elapsed.TotalSeconds, $script:built.Count, $Jobs
"gate: $($script:passed) passed, $($script:failed) failed"
if ($script:failed -or $script:broken -or $script:passed -eq 0) { exit 1 }
exit 0
