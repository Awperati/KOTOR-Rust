use kotor_launcher::{Launcher, Outcome, GAME_EXE};
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

/// A folder like `C:\KotorRust` with fake builds, oldest first, each `(sha, behaviour)`, and the last one the newest.
fn root(name: &str, builds: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kotor-launcher-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("builds")).unwrap();
    for (sha, behaviour) in builds {
        push(&dir, sha, behaviour);
    }
    dir
}

/// What a push to master does, as `scripts/publish_build.ps1` does it.
fn push(dir: &std::path::Path, sha: &str, behaviour: &str) {
    let build = dir.join("builds").join(sha);
    fs::create_dir_all(&build).unwrap();
    fs::copy(env!("CARGO_BIN_EXE_kotor-fake-game"), build.join(GAME_EXE)).unwrap();
    fs::write(build.join("behaviour.txt"), behaviour).unwrap();
    let mut history = fs::read_to_string(dir.join("history.txt")).unwrap_or_default();
    history.push_str(&format!("{sha}\tsubject of {sha}\n"));
    fs::write(dir.join("history.txt"), history).unwrap();
    fs::write(dir.join("latest.txt"), format!("{sha}\n")).unwrap();
}

/// With time for a crash to be seen on a machine under load: 400 ms once failed a CI run, as a build that had not yet
/// crashed counted as started. A build that exits comes back at once, whatever this is.
fn launcher(dir: &std::path::Path) -> Launcher {
    Launcher {
        root: dir.to_path_buf(),
        grace: Duration::from_secs(30),
    }
}

fn ran(dir: &std::path::Path) -> Vec<String> {
    fs::read_to_string(dir.join("builds").join("ran.txt"))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

fn ran_build(sha: &str, code: i32) -> Outcome {
    Outcome::Ran {
        sha: sha.to_string(),
        code,
    }
}

#[test]
fn the_newest_build_runs_and_says_it_is_new_once() {
    let dir = root("newer", &[("aaa", "0")]);
    let l = launcher(&dir);
    assert_eq!(l.run(&[]), ran_build("aaa", 0));
    push(&dir, "bbb", "0");
    let args = [OsString::from("--mute")];
    assert_eq!(l.run(&args), ran_build("bbb", 0));
    assert_eq!(l.run(&args), ran_build("bbb", 0));
    let lines = ran(&dir);
    assert_eq!(lines[0], "aaa [] Updated to aaa: subject of aaa");
    assert_eq!(lines[1], r#"bbb ["--mute"] Updated to bbb: subject of bbb"#);
    assert_eq!(
        lines[2].trim_end(),
        r#"bbb ["--mute"]"#,
        "nothing new to say"
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_build_that_crashes_at_start_is_passed_over_and_then_skipped() {
    let dir = root("crash", &[("good", "0")]);
    let l = launcher(&dir);
    assert_eq!(l.run(&[]), ran_build("good", 0));
    push(&dir, "broken", "crash");
    assert_eq!(l.run(&[]), ran_build("good", 0));
    assert_eq!(
        fs::read_to_string(dir.join("bad.txt")).unwrap().trim(),
        "broken"
    );
    assert!(ran(&dir)[2].ends_with("Back on good: broken would not start"));
    // Skipped from now on, with no second try.
    let before = ran(&dir).len();
    assert_eq!(l.run(&[]), ran_build("good", 0));
    assert_eq!(ran(&dir).len(), before + 1);
    assert_eq!(l.candidates(), ["good"]);
    // The next push is tried as usual.
    push(&dir, "fixed", "0");
    assert_eq!(l.run(&[]), ran_build("fixed", 0));
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_crash_after_the_grace_period_is_not_held_against_a_build() {
    let dir = root("late", &[("old", "0")]);
    let l = Launcher {
        grace: Duration::from_millis(100),
        ..launcher(&dir)
    };
    // Up past the grace, then ends well enough: the game's own affair from there.
    push(&dir, "new", "700");
    assert_eq!(l.run(&[]), ran_build("new", 0));
    assert!(!dir.join("bad.txt").exists());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn when_every_build_fails_none_is_marked_bad() {
    // A missing game drive would look like this, and must not condemn good builds.
    let dir = root("all", &[("one", "crash"), ("two", "crash")]);
    let l = launcher(&dir);
    assert!(matches!(l.run(&[]), Outcome::Failed(_)));
    assert!(!dir.join("bad.txt").exists());
    assert_eq!(l.candidates(), ["two", "one"]);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn no_builds_is_said_plainly() {
    let dir = root("none", &[]);
    assert!(matches!(launcher(&dir).run(&[]), Outcome::Failed(_)));
    fs::remove_dir_all(&dir).unwrap();
}
