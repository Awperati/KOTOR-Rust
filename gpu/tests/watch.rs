//! A GPU wait that does not come back fails the program, and says what it was waiting for.
use std::process::Command;
use std::time::Duration;

/// Only for the test below, in a second run of this program with a short limit: a wait that outlasts it.
#[test]
fn a_wait_that_does_not_come_back() {
    if std::env::var_os("KOTOR_GPU_TIMEOUT").is_some() {
        let _wait = kotor_gpu::watch("a made-up wait");
        std::thread::sleep(Duration::from_secs(60));
    }
}

#[test]
fn a_stuck_wait_fails_loudly_and_names_what_it_waited_for() {
    let out = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "a_wait_that_does_not_come_back"])
        .env("KOTOR_GPU_TIMEOUT", "0.5")
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(101), "{said}");
    assert!(said.contains("GPU TIMEOUT"), "{said}");
    assert!(said.contains("a made-up wait"), "{said}");
    assert!(said.contains("a_wait_that_does_not_come_back"), "{said}");
}

#[test]
fn a_wait_that_ends_in_time_says_nothing() {
    let wait = kotor_gpu::watch("a short wait");
    drop(wait);
}
