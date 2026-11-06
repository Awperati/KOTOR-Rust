# Seamless areas, live: the player walks through a door into the next area and back, with no teleport and no loading screen.
# Unlike the other drive_*.ps1 scripts it opens no window at all: the stage runs in this process and its frames are drawn
# offscreen, so it cannot take focus and never captures a screen.
#   -Module tar_m02ac -Door Apartments   the module to start in and the tag of the link door (the default is Upper City South to the Apartments)
#   -Out docs\shots\seamless             where the frames go (docs\shots is not committed: it holds the game's textures)
#   -Sizes 1280x720,3840x2160            frames of the door opening onto the next area, at each size
# Prints PASS or FAIL for each check and exits 1 if any fails. Run from the repository root.
param(
    [string]$Module = 'tar_m02ac',
    [string]$Door = 'Apartments',
    [string]$Out = 'docs\shots\seamless',
    [string[]]$Sizes = @('1280x720', '3840x2160')
)
$ErrorActionPreference = 'Stop'
Push-Location gpu
try {
    $build = cmd /c "cargo build --release --example seamless_drive 2>&1"
    if ($LASTEXITCODE) { $build | Select-Object -Last 20; 'FAIL  the driver did not build'; exit 1 }
} finally { Pop-Location }
$failed = 0
foreach ($size in $Sizes) {
    $w, $h = $size -split 'x'
    "== $size"
    $lines = & gpu\target\release\examples\seamless_drive.exe $Module $Door (Join-Path (Get-Location) $Out) $w $h
    if ($LASTEXITCODE) { $failed++ }
    $lines | Where-Object { $_ -match 'PASS|FAIL|frame times' }
}
if ($failed) { exit 1 }
exit 0
