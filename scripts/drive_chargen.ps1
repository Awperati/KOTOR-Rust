# A new character made in the running window, custom from the class to the first state of the game, then a quick one.
# Like drive_window.ps1 it posts messages to the viewer's own handle only, and holds only at 1x: see docs/FASTTEST.md.
param([string]$Shots)
# Run from the repo root after building the viewer example. -Shots is a folder for pictures of the window. It writes no saves.
. "$PSScriptRoot\drive_common.ps1"
$BACK = @(0x08, 0x0E); $ESC = @(0x1B, 0x01)
function Start-Viewer {
  $script:stateFile = "$scratch\kotor_chargen.out"
  $script:v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList @("--mute","--no-tutorials","--no-auto-pause","--state-log","--frames","200000") -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_chargen.err" -PassThru -NoNewWindow
  $script:h = WaitWindow $script:v; [Drv]::NeverActive($script:h)
  "window $($script:h)"
  WaitLog
}
function Step($name, [int]$x, [int]$y, [int]$w, [int]$h2, [string]$want) {
  Press $x $y $w $h2
  $s = WaitFor "menu" $want
  Report $name ($s.menu -eq $want) "menu=$($s.menu)"
}
# The steps' panel, at its place over the screen: the steps of custpnl.gui and of quickpnl.gui.
function Custom([int]$n) { Press 377 (87 + 43 + 31 * ($n - 1) + [int]($n -gt 2)) 195 31 }
function Quick([int]$n) { Press 377 (87 + 59 + 30 * ($n - 1)) 195 31 }

Start-Viewer
$s = WaitFor "menu" "main"
Report "Opens on the main menu" ($s.menu -eq "main") "menu=$($s.menu)"
Step "New Game asks the class" 285 222 210 22 "chargen_class"
Hover 553 336          # BTN_SEL3: the words about the class
[void](State)
Shot "cg_class"
Hover 80 230           # BTN_SEL1: the big figure goes where the mouse is
[void](State)
Shot "cg_class_big_first"
Hover 700 230          # the last
[void](State)
Shot "cg_class_big_last"
Press 239 128 75 193
$s = WaitFor "menu" "chargen"
Report "A figure is chosen" ($s.menu -eq "chargen") "menu=$($s.menu)"
Shot "cg_quick_or_custom"
Press 339 143 233 24   # CUST_CHAR_BTN of qorcpnl.gui, at the panel's place
[void](State)
Shot "cg_custom"

# Appearance.
Custom 1
$s = WaitFor "menu" "chargen_look"
Report "Step 1 is the appearance" ($s.menu -eq "chargen_look") "menu=$($s.menu)"
Press 298 100 18 24    # The arrow on the Head row
[void](State)
Shot "cg_look"
Press 113 410 146 28   # BTN_ACCEPT
$s = WaitFor "menu" "chargen"
Report "OK keeps it" ($s.menu -eq "chargen") "menu=$($s.menu)"

# Attributes.
Custom 2
$s = WaitFor "menu" "chargen_attributes"
Report "Step 2 is attributes" ($s.menu -eq "chargen_attributes") "menu=$($s.menu)"
Press 292 143 27 32    # STR_PLUS_BTN
[void](State)
Press 112 410 138 28   # BTN_RECOMMENDED
[void](State)
Shot "cg_attributes"
Press 253 410 134 28   # BTN_ACCEPT
$s = WaitFor "menu" "chargen"
Report "Attributes are spent" ($s.menu -eq "chargen") "menu=$($s.menu)"

# Skills and feats.
Custom 3
$s = WaitFor "menu" "level_skills"
Report "Step 3 is skills" ($s.menu -eq "level_skills") "menu=$($s.menu)"
Press 113 410 138 28   # BTN_RECOMMENDED
[void](State)
Shot "cg_skills"
Press 253 410 134 28   # BTN_ACCEPT
$s = WaitFor "menu" "chargen"
Report "Skills are spent" ($s.menu -eq "chargen") "menu=$($s.menu)"
Custom 4
$s = WaitFor "menu" "level_feats"
Report "Step 4 is feats" ($s.menu -eq "level_feats") "menu=$($s.menu)"
Press 100 410 146 28   # BTN_RECOMMENDED
[void](State)
Shot "cg_feats"
Press 247 410 146 28   # BTN_ACCEPT
$s = WaitFor "menu" "chargen"
Report "Feats are chosen" ($s.menu -eq "chargen") "menu=$($s.menu)"

# The name: Random first, then typed.
Custom 5
$s = WaitFor "menu" "chargen_name"
Report "Step 5 is the name" ($s.menu -eq "chargen_name") "menu=$($s.menu)"
Press 218 376 203 28   # BTN_RANDOM
[void](State)
Shot "cg_name_random"
for ($i = 0; $i -lt 34; $i++) { Key $BACK[0] $BACK[1] 0 }
foreach ($k in @(@(0x52, 0x13), @(0x45, 0x12), @(0x56, 0x2F), @(0x41, 0x1E), @(0x4E, 0x31))) { Key $k[0] $k[1] 5 }
[void](State)
Shot "cg_name"
Press 113 410 158 28   # END_BTN
$s = WaitFor "menu" "chargen"
Report "OK keeps the name" ($s.menu -eq "chargen") "menu=$($s.menu)"
Shot "cg_ready"

# Play.
Custom 6
$s = WaitFor "module" "end_m01aa"
$s = State
Report "Play starts the game" ($s.module -eq "end_m01aa" -and $s.menu -eq "-") "menu=$($s.menu) module=$($s.module) level=$($s.level) hp=$($s.hp) xp=$($s.xp)"
Report "Level 1, with a Soldier's HP" ([int]$s.level -eq 1 -and [int]$s.hp -gt 0) "level=$($s.level) hp=$($s.hp)"
$quiet = 0
for ($i = 0; $i -lt 400 -and $quiet -lt 6; $i++) {
  Key 0x0D 0x1C; Key 0x31 0x02; $s = State
  if ($s.dlg -eq "false" -and [int]$s.said -gt 3) { $quiet++ } else { $quiet = 0 }
}
Report "Through the opening" ($quiet -ge 6) "dlg=$($s.dlg) said=$($s.said)"
Key 0x4D 0x32
$s = WaitFor "menu" "map"
Press 113 410 278 13   # BTN_PRTYSLCT
$s = WaitFor "menu" "party"
Report "The map's button has the party" ($s.menu -eq "party") "menu=$($s.menu)"
Shot "cg_party"
Key $ESC[0] $ESC[1]
[void](WaitFor "menu" "map")
Key $ESC[0] $ESC[1]
[void](WaitFor "menu" "-")
Key 0x43 0x2E
$s = WaitFor "menu" "character"
Report "C shows the character" ($s.menu -eq "character") "menu=$($s.menu)"
Shot "cg_in_game"
FocusReport
Stop-Process -Id $v.Id -Force

# A quick one: portrait, name, play.
Start-Viewer
$s = WaitFor "menu" "main"
Step "New Game asks the class" 285 222 210 22 "chargen_class"
Press 500 128 75 193   # BTN_SEL6
$s = WaitFor "menu" "chargen"
Report "A figure is chosen" ($s.menu -eq "chargen") "menu=$($s.menu)"
Press 339 114 233 24   # QUICK_CHAR_BTN
[void](State)
Shot "cg_quick"
Quick 1
$s = WaitFor "menu" "chargen_portrait"
Press 113 410 142 28
[void](WaitFor "menu" "chargen")
Quick 2
$s = WaitFor "menu" "chargen_name"
foreach ($k in @(@(0x4D, 0x32), @(0x49, 0x17), @(0x52, 0x13), @(0x41, 0x1E))) { Key $k[0] $k[1] 5 }
[void](State)
Press 113 410 158 28
[void](WaitFor "menu" "chargen")
Quick 3
$s = WaitFor "module" "end_m01aa"
$s = State
Report "Quick Character plays" ($s.module -eq "end_m01aa" -and $s.menu -eq "-") "menu=$($s.menu) module=$($s.module) hp=$($s.hp)"
FocusReport
Stop-Process -Id $v.Id -Force
Get-Content "$scratch\kotor_chargen.err" -Tail 4
if ($script:failed) { exit 1 }
exit 0
