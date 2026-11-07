# One game from the main menu, in the running window: new game, level up, equip, journal, map, save, load.
# Like drive_window.ps1 it posts messages to the viewer's own handle only, and holds only at 1x: see docs/FASTTEST.md.
param([string]$Shots)
# Run from the repo root after building the viewer example. -Shots is a folder for pictures of the window.
# --xp gives the level, since an earned one lies past the first fights. --near puts the player by the footlocker.
# It writes one save to %LOCALAPPDATA%\kotor-rs\saves, deletes it through the screen, and leaves other saves alone.
. "$PSScriptRoot\drive_common.ps1"
$savesDir = Join-Path $env:LOCALAPPDATA "kotor-rs\saves"
$savesBefore = @(Get-ChildItem $savesDir -Filter *.sav -ErrorAction SilentlyContinue | ForEach-Object { $_.FullName })
$script:stateFile = "$scratch\kotor_play.out"
$v = Start-Process -FilePath "gpu\target\release\examples\viewer.exe" -ArgumentList @("--mute","--no-tutorials","--no-auto-pause","--no-chargen","--no-early-force","--state-log","--near","end_locker01","--xp","1000","--frames","200000") -RedirectStandardOutput $script:stateFile -RedirectStandardError "$scratch\kotor_play.err" -PassThru -NoNewWindow
for ($i = 0; $i -lt 600 -and $v.MainWindowTitle -notlike "kotor-rs*"; $i++) { Start-Sleep -Milliseconds 50; $v.Refresh() }
$h = $v.MainWindowHandle; [Drv]::NeverActive($h)
"window $h"
$ESC = @(0x1B, 0x01); $ENTER = @(0x0D, 0x1C); $BACK = @(0x08, 0x0E); $SPACE = @(0x20, 0x39); $ONE = @(0x31, 0x02); $W = @(0x57, 0x11); $D = @(0x44, 0x20)
$E = @(0x45, 0x12); $KEY_U = @(0x55, 0x16); $KEY_C = @(0x43, 0x2E); $KEY_J = @(0x4A, 0x24); $KEY_M = @(0x4D, 0x32); $KEY_K = @(0x4B, 0x25)

# New game.
WaitLog
$s = WaitFor "menu" "main"
Report "Opens on the main menu" ($s.menu -eq "main") "menu=$($s.menu)"
Shot "play_main_menu"
Press 285 222 210 22   # BTN_NEWGAME
$s = WaitFor "module" "end_m01aa"
Report "New Game starts it" ($s.module -eq "end_m01aa" -and $s.menu -eq "-") "menu=$($s.menu) module=$($s.module)"
$quiet = 0
for ($i = 0; $i -lt 400 -and $quiet -lt 6; $i++) {
  Key $ENTER[0] $ENTER[1]; Key $ONE[0] $ONE[1]; $s = State
  if ($s.dlg -eq "false" -and [int]$s.said -gt 3) { $quiet++ } else { $quiet = 0 }
}
Report "Through the opening" ($quiet -ge 6) "dlg=$($s.dlg) said=$($s.said)"
$start = State
Shot "play_start"

# The footlocker holds what the player puts on: take it all.
Key $E[0] $E[1]
$s = WaitFor "opened" "true"
Report "E opens the locker" ($s.opened -eq "true" -and [int]$s.items -gt 0) "opened=$($s.opened) items=$($s.items)"
Shot "play_locker"
$items = [int]$s.items
Press 174 311 285 22   # Take All
$s = WaitFor "opened" "false"
# Its two Medpacs make one stack in the bag.
Report "Takes all of it" ([int]$s.items -eq 0 -and [int]$s.inv -eq [int]$start.inv + $items - 1) "items=$($s.items) inv $($start.inv) -> $($s.inv)"
Report "Take All closes it" ($s.opened -eq "false") "opened=$($s.opened)"

# Level up: the sheet's button, then the panel's steps. Level 2 asks for skills and feats.
Key $KEY_C[0] $KEY_C[1]
$s = WaitFor "menu" "character"
Report "C opens the character" ($s.menu -eq "character") "menu=$($s.menu) level=$($s.level)"
Shot "play_character_before"
Press 126 267 127 40   # BTN_LEVELUP
$s = WaitFor "menu" "levelup"
Report "Level Up opens the panel" ($s.menu -eq "levelup") "menu=$($s.menu)"
Shot "play_level_panel"
Press 377 177 195 31   # BTN_STEPNAME2
$s = WaitFor "menu" "level_skills"
Report "Skills step opens" ($s.menu -eq "level_skills") "menu=$($s.menu)"
Press 113 410 138 28   # BTN_RECOMMENDED
[void](State)
Shot "play_level_skills"
Press 253 410 134 28   # BTN_ACCEPT
$s = WaitFor "menu" "levelup"
Report "Skills step accepted" ($s.menu -eq "levelup") "menu=$($s.menu)"
Press 377 209 195 31   # BTN_STEPNAME3
$s = WaitFor "menu" "level_feats"
Report "Feats step opens" ($s.menu -eq "level_feats") "menu=$($s.menu)"
Press 100 410 146 28   # BTN_RECOMMENDED
[void](State)
Shot "play_level_feats"
Press 247 410 146 28   # BTN_ACCEPT
$s = WaitFor "menu" "levelup"
Report "Feats step accepted" ($s.menu -eq "levelup") "menu=$($s.menu)"
Press 377 271 195 31   # BTN_STEPNAME5
$s = WaitFor "menu" "character"
Report "Accept gives the level" ($s.menu -eq "character" -and [int]$s.level -eq [int]$start.level + 1) "menu=$($s.menu) level $($start.level) -> $($s.level)"
Shot "play_character_after"

# Equip what came from the locker: the second row is the first after None, when nothing is worn there.
Key $KEY_U[0] $KEY_U[1]
$s = WaitFor "menu" "equip"
Report "U opens the equipment" ($s.menu -eq "equip") "menu=$($s.menu) worn=$($s.worn)"
$worn = [int]$s.worn
Press 207 182 64 64    # BTN_INV_BODY
[void](State)
Shot "play_equip_slot"
Press 316 151 270 39   # the second row of LB_ITEMS
[void](State)
Shot "play_equip_chosen"
Press 247 410 146 28   # BTN_EQUIP
$until = (Get-Date).AddSeconds(3)
do { $s = State } while ([int]$s.worn -eq $worn -and (Get-Date) -lt $until)
Report "OK puts it on" ([int]$s.worn -eq $worn + 1) "worn $worn -> $($s.worn)"
Shot "play_equip_after"

# Journal and map.
Key $KEY_J[0] $KEY_J[1]
$s = WaitFor "menu" "journal"
Report "J opens the journal" ($s.menu -eq "journal") "menu=$($s.menu)"
Shot "play_journal"
Press 263 43 52 40     # BTN_ABI
$s = WaitFor "menu" "abilities"
Report "The tab opens Abilities" ($s.menu -eq "abilities") "menu=$($s.menu)"
Shot "play_abilities"
Press 389 88 138 18    # BTN_FEATS
[void](State)
Shot "play_abilities_feats"
Press 325 43 52 40     # BTN_MSG
$s = WaitFor "menu" "messages"
Report "The tab opens Messages" ($s.menu -eq "messages") "menu=$($s.menu)"
Shot "play_messages"
Press 113 410 279 28   # BTN_SHOW
[void](State)
Shot "play_messages_feedback"
Key $KEY_K[0] $KEY_K[1]
$s = WaitFor "menu" "abilities"
Report "K opens Abilities" ($s.menu -eq "abilities") "menu=$($s.menu)"
Key $KEY_M[0] $KEY_M[1]
$s = WaitFor "menu" "map"
Report "M opens the map" ($s.menu -eq "map") "menu=$($s.menu)"
Shot "play_map"
Key $ESC[0] $ESC[1]
$s = WaitFor "menu" "-"
Report "Esc closes it" ($s.menu -eq "-") "menu=$($s.menu)"

# Save.
Key $ESC[0] $ESC[1]
[void](WaitFor "menu" "pause")
Press 60 145 240 35    # BTN_SAVEGAME
$s = WaitFor "menu" "save"
$savesAt = [int]$s.saves
Press 253 410 134 28   # BTN_SAVELOAD
$s = WaitFor "menu" "name"
for ($i = 0; $i -lt 34; $i++) { Key $BACK[0] $BACK[1] 0 }
foreach ($k in @(@(0x50, 0x19), @(0x4C, 0x26), @(0x41, 0x1E), @(0x59, 0x15))) { Key $k[0] $k[1] 5 }
Key $ENTER[0] $ENTER[1]
$s = WaitFor "saves" "$($savesAt + 1)"
Report "Save writes a file" ($s.menu -eq "save" -and [int]$s.saves -eq $savesAt + 1) "menu=$($s.menu) saves $savesAt -> $($s.saves)"
Shot "play_save_list"
Press 389 410 138 28   # BTN_BACK
[void](WaitFor "menu" "pause")
Press 393 410 146 28   # BTN_EXIT
$saved = WaitFor "menu" "-"

# Change things, leave for the main menu, and load: all of it comes back.
Key $W[0] $W[1] 300
Key $D[0] $D[1] 200
$away = State
Report "Walked away" ((Moved $saved $away) -gt 0.3) "$($saved.pc) -> $($away.pc)"
Key $ESC[0] $ESC[1]
[void](WaitFor "menu" "pause")
Press 60 364 240 35    # BTN_QUIT
$s = WaitFor "menu" "main"
Report "Exit Game goes to the menu" ($s.menu -eq "main" -and $s.module -eq "-") "menu=$($s.menu) module=$($s.module)"
Press 285 246 210 22   # BTN_LOADGAME
$s = WaitFor "menu" "load"
Press 69 88 246 50     # first row
[void](State)
Shot "play_load_list"
Press 253 410 134 28   # BTN_SAVELOAD
$s = WaitFor "module" "end_m01aa"
$s = State
$same = $s.level -eq $saved.level -and $s.worn -eq $saved.worn -and $s.inv -eq $saved.inv -and $s.gold -eq $saved.gold
Report "Load brings it back" ($s.menu -eq "-" -and $same -and (Moved $saved $s) -lt 0.3) "level=$($s.level) worn=$($s.worn) inv=$($s.inv) gold=$($s.gold) pc=$($s.pc), saved level=$($saved.level) worn=$($saved.worn) inv=$($saved.inv) gold=$($saved.gold) pc=$($saved.pc)"
Shot "play_loaded"
Key $KEY_C[0] $KEY_C[1]
$s = WaitFor "menu" "character"
Shot "play_character_loaded"
Key $ESC[0] $ESC[1]
[void](WaitFor "menu" "-")

# Delete asks once, then the save is gone. Then the main menu, and Exit closes the window.
Key $ESC[0] $ESC[1]
[void](WaitFor "menu" "pause")
Press 60 110 240 35    # BTN_LOADGAME
$s = WaitFor "menu" "load"
$count = [int]$s.saves
Press 69 88 246 50
[void](State)
Press 113 410 138 28   # BTN_DELETE
[void](State)
Press 113 410 138 28
$s = WaitFor "saves" "$($count - 1)"
$left = @(Get-ChildItem $savesDir -Filter *.sav -ErrorAction SilentlyContinue | Where-Object { $savesBefore -notcontains $_.FullName })
Report "The save is deleted" ([int]$s.saves -eq $count - 1 -and $left.Count -eq 0) "saves $count -> $($s.saves), left $($left.Count)"
Press 389 410 138 28
[void](WaitFor "menu" "pause")
Press 60 364 240 35
[void](WaitFor "menu" "main")
FocusReport
Press 285 318 210 27   # BTN_EXIT
$gone = $v.WaitForExit(10000)
Report "Exit closes the window" $gone "exited=$gone"
if (-not $gone) { Stop-Process -Id $v.Id -Force }
foreach ($f in $left) { Remove-Item $f.FullName -Force }
Get-Content "$scratch\kotor_play.err" -Tail 4
if ($script:failed) { exit 1 }
exit 0
