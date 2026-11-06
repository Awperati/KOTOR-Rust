# Put a built game where KotorRust.exe finds it, so that it appears all at once: nothing half-copied is ever named.
#   publish_build.ps1 -GameExe viewer.exe -LauncherExe KotorRust.exe -Sha 2e3c874 -Subject "..." [-Root C:\KotorRust] [-Keep 5]
# Keeps the newest -Keep builds. Deletes nothing but older folders it published itself, named in history.txt.
param(
    [Parameter(Mandatory)][string]$GameExe,
    [Parameter(Mandatory)][string]$LauncherExe,
    [Parameter(Mandatory)][string]$Sha,
    [Parameter(Mandatory)][string]$Subject,
    [string]$Root = 'C:\KotorRust',
    [int]$Keep = 5
)
$ErrorActionPreference = 'Stop'

# Write a whole file beside its name, then swap it in.
function Set-Whole([string]$Path, [string[]]$Lines) {
    $tmp = "$Path.tmp"
    [IO.File]::WriteAllLines($tmp, $Lines, (New-Object Text.UTF8Encoding $false))
    if (Test-Path $Path) { [IO.File]::Replace($tmp, $Path, [NullString]::Value) } else { [IO.File]::Move($tmp, $Path) }
}

$builds = Join-Path $Root 'builds'
New-Item -ItemType Directory -Force $builds | Out-Null
$dest = Join-Path $builds $Sha
if (-not (Test-Path $dest)) {
    $tmp = Join-Path $builds ".$Sha.tmp"
    if (Test-Path $tmp) { Remove-Item -Recurse -Force $tmp }
    New-Item -ItemType Directory $tmp | Out-Null
    Copy-Item $GameExe (Join-Path $tmp 'kotor.exe')
    Set-Content -Encoding utf8 (Join-Path $tmp 'build.txt') "$Sha $(Get-Date -Format s) $Subject"
    [IO.Directory]::Move($tmp, $dest)
}

$historyFile = Join-Path $Root 'history.txt'
$history = @()
if (Test-Path $historyFile) {
    $history = @(Get-Content -Encoding utf8 $historyFile | Where-Object { $_ -and -not $_.StartsWith("$Sha`t") })
}
$history += "$Sha`t$Subject"
Set-Whole $historyFile $history
# latest.txt goes last: once it names a build, that build is whole.
Set-Whole (Join-Path $Root 'latest.txt') @($Sha)

# The build is out now. Nothing from here on may fail the job: a launcher left running holds its file.
# A running launcher can be renamed but not overwritten, and once renamed it cannot be removed until it stops.
$launcher = Join-Path $Root 'KotorRust.exe'
$new = "$launcher.new"
try {
    Get-ChildItem $Root -Filter 'KotorRust.old*.exe' | ForEach-Object { Remove-Item -Force $_.FullName -ErrorAction SilentlyContinue }
    $same = (Test-Path $launcher) -and ((Get-FileHash $launcher).Hash -eq (Get-FileHash $LauncherExe).Hash)
    if ($same) {
        "launcher unchanged"
    } else {
        Copy-Item $LauncherExe $new -Force
        $old = Join-Path $Root 'KotorRust.old.exe'
        if (Test-Path $old) { $old = Join-Path $Root "KotorRust.old.$Sha.exe" }
        if (Test-Path $launcher) { Move-Item -Force $launcher $old }
        Move-Item -Force $new $launcher
    }
} catch {
    "launcher not replaced, the build is published all the same: $_"
    if (-not (Test-Path $launcher) -and (Test-Path $old)) { Move-Item -Force $old $launcher -ErrorAction SilentlyContinue }
}
Remove-Item -Force $new -ErrorAction SilentlyContinue

# The oldest beyond -Keep go, but never what last ran well. A build still running stays for next time.
$lastRun = ''
$lastFile = Join-Path $Root 'last_run.txt'
if (Test-Path $lastFile) { $lastRun = (Get-Content $lastFile -TotalCount 1).Trim() }
$kept = @()
$extra = $history.Count - $Keep
foreach ($line in $history) {
    $s = $line.Split("`t")[0]
    if ($extra -gt 0 -and $s -ne $lastRun -and $s -ne $Sha) {
        $extra--
        $folder = Join-Path $builds $s
        try {
            if (Test-Path $folder) { Remove-Item -Recurse -Force $folder }
            "removed old build $s"
        } catch {
            "old build $s kept: $_"
            $kept += $line
        }
    } else {
        $kept += $line
    }
}
try { Set-Whole $historyFile $kept } catch { "history not trimmed: $_" }
"published $Sha to $dest, $($kept.Count) builds kept, $([math]::Round((Get-Item (Join-Path $dest 'kotor.exe')).Length / 1MB, 1)) MB"
