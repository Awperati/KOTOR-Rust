//! A look through the install's `Override` folder, on a thread at the start: what this game has no reader for, and what does
//! not parse, go in `override_problems.txt` beside the settings. The result is kept until the folder changes. Pictures are
//! not opened here, as they are nearly all of its 3 GB: one that fails is written down when it is first drawn.
use kotor_archive::is_gff;
use std::path::{Path, PathBuf};

/// What a type this game does not read would have changed.
fn no_reader(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "lip" => "lip sync for a spoken line; mouths here follow the voice's loudness",
        "ssf" => "a creature's sound set; this game has no reader for it",
        "vis" => "which rooms can be seen from which; this game draws what the camera sees",
        "txt" => "not a game file",
        _ => return None,
    })
}

fn parses(path: &Path, ext: &str) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let e = |e: kotor_core::Error| e.to_string();
    match ext {
        "2da" => kotor_tables::TwoDa::parse(&bytes).map(|_| ()).map_err(e),
        "wok" => kotor_walkmesh::parse_bwm(&bytes).map(|_| ()).map_err(e),
        "mdl" => {
            let mdx = std::fs::read(path.with_extension("mdx")).map_err(|e| e.to_string())?;
            kotor_model::parse_model(&bytes, &mdx)
                .map(|_| ())
                .map_err(e)
        }
        x if is_gff(x) => kotor_gff::Gff::parse(&bytes).map(|_| ()).map_err(e),
        _ => Ok(()),
    }
}

/// One line for each file of the folder that cannot be used, sorted. A parser that panics counts as one that failed.
pub fn scan_override_folder(folder: &Path) -> Vec<String> {
    let Ok(read) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in read.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let stem = path.file_stem().map_or(0, |s| s.len());
        let ext = path
            .extension()
            .map_or(String::new(), |e| e.to_string_lossy().to_ascii_lowercase());
        if stem > 16 {
            out.push(format!(
                "{name}: skipped, a name of over 16 letters can never be asked for"
            ));
        } else if let Some(what) = no_reader(&ext) {
            out.push(format!("{name}: skipped, {what}"));
        } else if ext == "gui" {
            out.push(format!(
                "{name}: used only with Override screens on: laid out for a patched program's window"
            ));
        } else if ext == "tlk" {
            out.push(format!(
                "{name}: used only with Override talk table on, which is off unless you turn it on"
            ));
        } else {
            let p = path.clone();
            let e = ext.clone();
            match std::panic::catch_unwind(move || parses(&p, &e)) {
                Ok(Ok(())) => {}
                Ok(Err(why)) => out.push(format!(
                    "{name}: failed to parse ({why}), so what uses it goes without"
                )),
                Err(_) => out.push(format!(
                    "{name}: its parser gave up, so what uses it goes without"
                )),
            }
        }
    }
    out.sort();
    out
}

/// How the folder stands: the first line of the problems file, to tell whether it needs looking through again.
fn folder_stamp(folder: &Path) -> Option<String> {
    let (mut files, mut bytes) = (0u64, 0u64);
    for entry in std::fs::read_dir(folder).ok()?.flatten() {
        files += 1;
        bytes += entry.metadata().map_or(0, |m| m.len());
    }
    Some(format!("# Override folder: {files} files, {bytes} bytes"))
}

/// Look through the folder on a thread, unless the problems file already says how it stands.
pub fn scan_override_in_background(folder: PathBuf, problems: PathBuf) {
    let Some(stamp) = folder_stamp(&folder) else {
        return;
    };
    let known = std::fs::read_to_string(&problems).unwrap_or_default();
    if known.lines().next() == Some(stamp.as_str()) {
        return;
    }
    if let Some(parent) = problems.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::thread::Builder::new()
        .name("override scan".to_string())
        .spawn(move || {
            let lines = scan_override_folder(&folder);
            // The stamp goes in only with what it stands for: a game closed before this is done looks again next time. What was
            // noted while the look went on stays after it.
            let noted = std::fs::read_to_string(&problems).unwrap_or_default();
            let noted: Vec<&str> = noted
                .lines()
                .filter(|l| !l.starts_with("# Override folder:") && !known.contains(l))
                .collect();
            let text = format!("{stamp}\n{}\n{}\n", lines.join("\n"), noted.join("\n"));
            let _ = std::fs::write(&problems, text);
        });
}
