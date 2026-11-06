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

# A running launcher can be renamed but not overwritten.
$launcher = Join-Path $Root 'KotorRust.exe'
$old = Join-Path $Root 'KotorRust.old.exe'
if (Test-Path $old) { Remove-Item -Force $old -ErrorAction SilentlyContinue }
$new = "$launcher.new"
Copy-Item $LauncherExe $new -Force
if (Test-Path $launcher) { Move-Item -Force $launcher $old }
Move-Item -Force $new $launcher

# The oldest beyond -Keep go, but never what last ran well.
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
        if (Test-Path $folder) { Remove-Item -Recurse -Force $folder }
        "removed old build $s"
    } else {
        $kept += $line
    }
}
Set-Whole $historyFile $kept
"published $Sha to $dest, $($kept.Count) builds kept, $([math]::Round((Get-Item (Join-Path $dest 'kotor.exe')).Length / 1MB, 1)) MB"
