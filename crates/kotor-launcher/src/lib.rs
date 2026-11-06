//! Start the newest build that works. `scripts/publish_build.ps1` puts builds where this looks:
//!
//! ```text
//! <root>\KotorRust.exe     this program
//! <root>\latest.txt        the newest build's short sha
//! <root>\history.txt       one build a line, oldest first: sha, a tab, its commit subject
//! <root>\builds\<sha>\kotor.exe
//! <root>\bad.txt           builds that crashed at start while an older one ran, one sha a line
//! <root>\last_run.txt      the build that last started well
//! <root>\launcher.log, game.log
//! ```
//!
//! A build that exits with an error within [`GRACE`] counts as crashing, and the next older one gets its turn. It is
//! only marked bad if an older one then starts: when every build fails, the fault is more likely the machine's, such as
//! a game drive that is not there, and nothing is marked.

use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

/// How long a build has to stay up, or exit cleanly, to count as started.
pub const GRACE: Duration = Duration::from_secs(10);
/// The game's name inside a build's folder.
pub const GAME_EXE: &str = "kotor.exe";
const POLL: Duration = Duration::from_millis(50);

/// What came of a start.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// This build ran, and ended with this exit code.
    Ran { sha: String, code: i32 },
    /// No build could be started. The text says why.
    Failed(String),
}

pub struct Launcher {
    pub root: PathBuf,
    pub grace: Duration,
}

fn read_lines(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

impl Launcher {
    pub fn new(root: impl Into<PathBuf>) -> Launcher {
        Launcher {
            root: root.into(),
            grace: GRACE,
        }
    }

    fn file(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn exe(&self, sha: &str) -> PathBuf {
        self.root.join("builds").join(sha).join(GAME_EXE)
    }

    fn log(&self, line: &str) {
        if let Ok(mut f) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.file("launcher.log"))
        {
            let _ = writeln!(f, "{line}");
        }
    }

    /// The commit subject of a build, from `history.txt`.
    fn subject(&self, sha: &str) -> String {
        read_lines(&self.file("history.txt"))
            .iter()
            .find_map(|l| {
                let (s, subject) = l.split_once('\t')?;
                (s == sha).then(|| subject.to_string())
            })
            .unwrap_or_default()
    }

    /// Builds to try in order: the newest first, then older ones, none marked bad, each with its exe there.
    pub fn candidates(&self) -> Vec<String> {
        let bad = read_lines(&self.file("bad.txt"));
        let mut order = read_lines(&self.file("latest.txt"));
        let history = read_lines(&self.file("history.txt"));
        order.extend(
            history
                .iter()
                .rev()
                .filter_map(|l| l.split('\t').next().map(str::to_string)),
        );
        let mut seen = Vec::new();
        for sha in order {
            if !seen.contains(&sha) && !bad.contains(&sha) && self.exe(&sha).is_file() {
                seen.push(sha);
            }
        }
        seen
    }

    /// What the window's title says about this start, if anything is new.
    fn note(&self, sha: &str, crashed: &[String]) -> Option<String> {
        if !crashed.is_empty() {
            return Some(format!(
                "Back on {sha}: {} would not start",
                crashed.join(", ")
            ));
        }
        let last = read_lines(&self.file("last_run.txt"));
        if last.first().is_some_and(|l| l == sha) {
            return None;
        }
        Some(format!("Updated to {sha}: {}", self.subject(sha)))
    }

    /// Run the newest build that works, passing `args`, until it ends.
    pub fn run(&self, args: &[OsString]) -> Outcome {
        let mut crashed: Vec<String> = Vec::new();
        let candidates = self.candidates();
        if candidates.is_empty() {
            let why = format!("No build in {}", self.root.display());
            self.log(&why);
            return Outcome::Failed(why);
        }
        for sha in candidates {
            let note = self.note(&sha, &crashed);
            if let Some(n) = &note {
                self.log(n);
            }
            let mut child = match self.start(&sha, args, note.as_deref()) {
                Ok(c) => c,
                Err(e) => {
                    self.log(&format!("{sha} would not start: {e}"));
                    crashed.push(sha);
                    continue;
                }
            };
            let mut waited = Duration::ZERO;
            let early = loop {
                match child.try_wait() {
                    Ok(Some(status)) => break Some(status),
                    Ok(None) if waited >= self.grace => break None,
                    Ok(None) => {
                        sleep(POLL.min(self.grace));
                        waited += POLL.min(self.grace);
                    }
                    Err(_) => break None,
                }
            };
            if let Some(status) = early.filter(|s| !s.success()) {
                self.log(&format!(
                    "{sha} crashed {} ms in, exit {:?}",
                    waited.as_millis(),
                    status.code()
                ));
                crashed.push(sha);
                continue;
            }
            // It started, so what crashed before it was the build's fault, not the machine's.
            self.mark_bad(&crashed);
            let _ = fs::write(self.file("last_run.txt"), format!("{sha}\n"));
            let code = match early {
                Some(status) => status.code().unwrap_or(0),
                None => child.wait().ok().and_then(|s| s.code()).unwrap_or(0),
            };
            self.log(&format!("{sha} ended, exit {code}"));
            return Outcome::Ran { sha, code };
        }
        let why = format!("No build would start: {}", crashed.join(", "));
        self.log(&why);
        Outcome::Failed(why)
    }

    fn mark_bad(&self, shas: &[String]) {
        if shas.is_empty() {
            return;
        }
        if let Ok(mut f) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.file("bad.txt"))
        {
            for sha in shas {
                let _ = writeln!(f, "{sha}");
                self.log(&format!("{sha} marked bad, and will be skipped"));
            }
        }
    }

    fn start(
        &self,
        sha: &str,
        args: &[OsString],
        note: Option<&str>,
    ) -> std::io::Result<std::process::Child> {
        let exe = self.exe(sha);
        let log = fs::File::create(self.file("game.log"))?;
        let mut c = Command::new(&exe);
        c.args(args)
            .current_dir(exe.parent().unwrap_or(&self.root))
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log);
        if let Some(n) = note {
            c.env("KOTOR_BUILD_NOTE", n);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // The game is a console program: no black window behind it.
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            c.creation_flags(CREATE_NO_WINDOW);
        }
        c.spawn()
    }
}
