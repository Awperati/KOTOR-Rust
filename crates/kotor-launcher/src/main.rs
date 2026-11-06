//! `KotorRust.exe`: start the newest build beside this program. See the library for how it chooses.
#![cfg_attr(windows, windows_subsystem = "windows")]

use kotor_launcher::{Launcher, Outcome};

fn main() {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let root = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| ".".into());
    match Launcher::new(root).run(&args) {
        Outcome::Ran { code, .. } => std::process::exit(code),
        Outcome::Failed(why) => {
            tell(&why);
            std::process::exit(1);
        }
    }
}

#[cfg(windows)]
fn tell(text: &str) {
    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(window: isize, text: *const u16, title: *const u16, kind: u32) -> i32;
    }
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let (text, title) = (wide(text), wide("KotorRust"));
    // SAFETY: both strings end in a zero and outlive the call, and no window owns the box.
    unsafe {
        MessageBoxW(0, text.as_ptr(), title.as_ptr(), 0);
    }
}

#[cfg(not(windows))]
fn tell(text: &str) {
    eprintln!("{text}");
}
