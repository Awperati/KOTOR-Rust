# The game that updates itself

Every green push to master becomes a build on this PC. The next start of `C:\KotorRust\KotorRust.exe` runs it.

To play, run this:

```
C:\KotorRust\KotorRust.exe
```

Arguments go on to the game, such as `--fullscreen`. A desktop or Start shortcut is the owner's to make, if wanted. Nothing here installs anything, changes a Windows setting, or adds a service, task or startup entry.

## What happens

1. CI's `check` job runs the gate with no game files.
2. If that is green, its `publish` job runs `scripts/ci_build.ps1` on the same runner. It builds the game for release, and starts it twice with no window, muted and on a throwaway home: the menu, then the first room. It reads the install read-only. Then `scripts/publish_build.ps1` copies the game to `C:\KotorRust\builds\<sha>\kotor.exe`, and names it in `latest.txt` last, so that nothing half-copied is ever named.
3. The launcher reads `latest.txt` and starts that build.

The newest five builds stay for going back, and the one that last ran well as well. Only older builds named in `history.txt` are ever removed. A build holds no game files or textures: it finds the install as the viewer does, through `KOTOR_DIR` or the default.

## What the launcher does

| Case | What it does |
|---|---|
| A build newer than the one last run | The window's title and `launcher.log` say `Updated to <sha>: <commit subject>` |
| Exits with an error within 10 s of starting | Tries the next older build. `bad.txt` gets the crashed one if an older one then starts, and it is skipped from then on |
| Every build fails | Marks nothing bad, as a missing game drive would look the same, and says so in a box |
| A crash after 10 s | The game's own affair: no build is marked bad |

To try a bad build again, take its line out of `C:\KotorRust\bad.txt`. To go back by hand, put an older sha in `latest.txt`. The game's output goes to `C:\KotorRust\game.log`.

## How it was checked

`crates/kotor-launcher/tests/launcher.rs` covers each case above with a stand-in for the game. `scripts/launcher_demo.ps1` runs the real publish script and launcher through seven fake pushes in a temporary folder.

This is one PC only. Other PCs would need a private release feed and a token, which is a separate decision.
