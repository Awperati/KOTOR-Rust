//! A stand-in for the game. `behaviour.txt` beside it says `crash`, `wait` to stay up until `release.txt` appears and then
//! crash, or how many milliseconds to stay up; it notes in `ran.txt`, one level up beside the other builds, which build
//! ran, its arguments and its update note.

fn main() {
    let exe = std::env::current_exe().unwrap();
    let dir = exe.parent().unwrap();
    let sha = dir.file_name().unwrap().to_string_lossy().to_string();
    let behaviour = std::fs::read_to_string(dir.join("behaviour.txt")).unwrap_or_default();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let note = std::env::var("KOTOR_BUILD_NOTE").unwrap_or_default();
    if let Some(up) = dir.parent() {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(up.join("ran.txt"))
        {
            let _ = writeln!(f, "{sha} {args:?} {note}");
        }
    }
    let behaviour = behaviour.trim();
    if behaviour == "crash" {
        std::process::exit(3);
    }
    if behaviour == "wait" {
        while !dir.join("release.txt").exists() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        std::process::exit(3);
    }
    let ms = behaviour.parse().unwrap_or(0);
    std::thread::sleep(std::time::Duration::from_millis(ms));
}
