//! A GPU call that never comes back should fail loudly. Once, a test sat for ten minutes at full speed on one core, and
//! nothing said where. Calls that wait on the GPU to make a device or read a frame back take a [`Watch`] while they run. If
//! one is still there after the limit, the program names everything being waited for, and on which thread, which is the
//! test's name in a test, and exits with a failure. The window's frames are not watched: a sleeping computer can hold one.

use std::collections::BTreeMap;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, Once};
use std::time::{Duration, Instant};

/// No GPU call here should take this long. `KOTOR_GPU_TIMEOUT`, in seconds, changes it.
const LIMIT_SECONDS: f64 = 120.0;
const FAILED: i32 = 101;

struct Wait {
    what: &'static str,
    thread: String,
    since: Instant,
}

static WAITS: Mutex<BTreeMap<u64, Wait>> = Mutex::new(BTreeMap::new());
static NEXT: AtomicU64 = AtomicU64::new(0);
static WATCHER: Once = Once::new();

/// While this lives, `what` is being waited for.
pub struct Watch(u64);

/// Say what is being waited for, until the result is dropped.
pub fn watch(what: &'static str) -> Watch {
    WATCHER.call_once(|| {
        let limit = std::env::var("KOTOR_GPU_TIMEOUT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(LIMIT_SECONDS);
        std::thread::spawn(move || watcher(Duration::from_secs_f64(limit)));
    });
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let thread = std::thread::current();
    let wait = Wait {
        what,
        thread: thread.name().unwrap_or("unnamed").to_string(),
        since: Instant::now(),
    };
    if let Ok(mut waits) = WAITS.lock() {
        waits.insert(id, wait);
    }
    Watch(id)
}

impl Drop for Watch {
    fn drop(&mut self) {
        if let Ok(mut waits) = WAITS.lock() {
            waits.remove(&self.0);
        }
    }
}

fn watcher(limit: Duration) {
    loop {
        std::thread::sleep(limit / 8);
        let Ok(waits) = WAITS.lock() else { return };
        if waits.values().any(|w| w.since.elapsed() >= limit) {
            // Straight to stderr: a test run keeps what `eprintln!` says, and it would go with the exit.
            let mut out = std::io::stderr();
            let _ = writeln!(
                out,
                "GPU TIMEOUT: nothing came back in {limit:?}. Waiting for:"
            );
            for w in waits.values() {
                let (what, thread, held) = (w.what, &w.thread, w.since.elapsed());
                let _ = writeln!(out, "  {what}, on thread {thread}, for {held:?}");
            }
            std::process::exit(FAILED);
        }
    }
}
