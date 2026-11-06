# Who works on what

The owner asked for two Builders on this project. This keeps them from colliding.

| Builder | Session | Worktree and branch | Owns |
|---|---|---|---|
| Game | `local_17acac93-742a-47dc-9099-2be4f61a024e` (may not resolve; use `ListAgents`) | `the repository's folder`, `master` | game loop, rendering, audio, particles, rules: `crates/kotor-game` except below, `crates/kotor-render`, `crates/kotor-audio`, `gpu/src/lib.rs`, `gpu/src/effects.rs`, `gpu/audio`, `gpu/src/stage.rs`, `gpu/examples/viewer.rs` |
| Menus | `local_14ac18c8-5da4-41e5-a6fd-4f5a607027b7` | `../kotor-rs-menus`, `menus` | menus, inventory and equip, loot tables, save and load screens: `crates/kotor-ui`, `crates/kotor-game/src/save.rs`, `gpu/src/menus.rs` |

## Rules
1. **Commit only your own files**, by path. Never `git add -A`.
2. **Your own target folder.** Each worktree builds in its own, or set `CARGO_TARGET_DIR` to a folder of your own.
3. **Small hooks in the other's files are fine.** The Menus Builder has hooks in `stage.rs` (`Stage::open` split into boot and start, early-outs in `update`, `hover`, `click`, `number_key`, `continue_line`, `refresh_overlay`, `dynamic_scene`), `viewer.rs` (`--skip-menu`, I, U, Esc) and `combat.rs`, `play.rs`, `world.rs`. Tell the owner of a file before changing more than a hook.
4. **Shared files go through a message first:** `Cargo.toml`, `gpu/Cargo.toml`, `docs/STATUS.md`, `docs/PLAY.md`, `scripts/check.ps1`.
5. **Handover is rebase and fast-forward, after telling each other.** The Menus Builder rebases `menus` onto `master` and fixes its own conflicts. `master` takes it by fast-forward only, never a merge commit or a force. The owner decides when that merge happens. Top Dog's words for how: "it tells you, you rebase onto the current master tip, rerun check.ps1 and drive_window.ps1 on the rebased tip, fast-forward master, tell it".
6. **Green only:** `scripts/check.ps1` exits 0 with zero failures on the exact commit.
7. **One Gigapixel job at a time**, through `scripts/gigapixel_guard.ps1`.
8. **Pushing to `origin` (the private repository):** only after `scripts/check.ps1` shows 0 failures on the exact commit, rebased on `origin/master`, as a fast-forward. Never force. If `origin/master` is red, only its fix or a revert goes on it.
9. Facts and coordination go between Builders directly. Decisions and finished-work reports go to Top Dog.
