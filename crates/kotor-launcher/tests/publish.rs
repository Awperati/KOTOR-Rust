//! `scripts/publish_build.ps1` run for real on a throwaway folder, with its files held as a running launcher holds them.
#![cfg(windows)]

use std::fs::{self, File, OpenOptions};
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

fn publish(root: &Path, launcher: &Path, game: &Path, sha: &str) -> String {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/publish_build.ps1");
    let out = Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(script)
        .arg("-GameExe")
        .arg(game)
        .arg("-LauncherExe")
        .arg(launcher)
        .args(["-Sha", sha, "-Subject", "a test", "-Root"])
        .arg(root)
        .output()
        .unwrap();
    let said =
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{sha}: {said}");
    assert_eq!(
        fs::read_to_string(root.join("latest.txt")).unwrap().trim(),
        sha
    );
    said
}

/// Nobody else may open it, the way a running program holds its own file against being replaced.
fn hold(path: &Path) -> File {
    OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(path)
        .unwrap()
}

fn folder() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kotor-publish-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_launcher_left_running_never_fails_a_publish() {
    let dir = folder();
    let root = dir.join("root");
    let (game, launcher) = (dir.join("game.exe"), dir.join("new-launcher.exe"));
    fs::write(&game, "game").unwrap();
    fs::write(&launcher, "first").unwrap();
    let installed = root.join("KotorRust.exe");
    publish(&root, &launcher, &game, "aaa");
    assert_eq!(fs::read_to_string(&installed).unwrap(), "first");

    // The same launcher is left alone.
    let said = publish(&root, &launcher, &game, "bbb");
    assert!(said.contains("launcher unchanged"), "{said}");
    assert!(!root.join("KotorRust.exe.new").exists());

    // A new one, with the last swap's old file still running: as CI failed three times.
    fs::write(root.join("KotorRust.old.exe"), "running").unwrap();
    let running = hold(&root.join("KotorRust.old.exe"));
    fs::write(&launcher, "second").unwrap();
    publish(&root, &launcher, &game, "ccc");
    assert_eq!(fs::read_to_string(&installed).unwrap(), "second");
    assert_eq!(
        fs::read_to_string(root.join("KotorRust.old.ccc.exe")).unwrap(),
        "first"
    );
    drop(running);

    // Its own file held so that it cannot even be renamed: the build still goes out, and the launcher stays whole.
    fs::write(&launcher, "third").unwrap();
    let running = hold(&installed);
    let said = publish(&root, &launcher, &game, "ddd");
    assert!(said.contains("launcher not replaced"), "{said}");
    drop(running);
    assert_eq!(fs::read_to_string(&installed).unwrap(), "second");
    assert!(!root.join("KotorRust.exe.new").exists());

    // Once it has stopped, the next push replaces it and clears what was left.
    publish(&root, &launcher, &game, "eee");
    assert_eq!(fs::read_to_string(&installed).unwrap(), "third");
    let olds = fs::read_dir(&root)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("KotorRust.old"))
        .count();
    assert_eq!(olds, 1, "only the last one");
    let _ = fs::remove_dir_all(&dir);
}
