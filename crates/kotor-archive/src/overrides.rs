//! The `Override` folder of the player's own install: loose files that win over the game's archives, as the original engine
//! has it. Only ever read, at run time. What is used is the player's choice, kept for the whole of a run, and is off for
//! anything that has not chosen: tests and tools see the stock game unless `KOTOR_OVERRIDE` says otherwise.
use kotor_core::ResType;
use std::collections::{BTreeSet, HashMap};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// What of the folder is used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct OverrideUse {
    pub folder: bool,
    /// Compiled scripts: a mod's can change rules this engine implements itself.
    pub scripts: bool,
    /// Tables: so can a mod's rows, and they may assume a patched program.
    pub tables: bool,
    /// Screens: the owner's are laid out for a patched program's wide window, and this game lays out its own.
    pub screens: bool,
    /// A `dialog.tlk` in the folder in place of the one beside the game. The original engine does not look there.
    pub talk: bool,
}

impl OverrideUse {
    pub const NONE: OverrideUse = OverrideUse {
        folder: false,
        scripts: false,
        tables: false,
        screens: false,
        talk: false,
    };
    /// All but the talk table, which is off until asked for by name.
    pub const ALL: OverrideUse = OverrideUse {
        folder: true,
        scripts: true,
        tables: true,
        screens: true,
        talk: false,
    };

    fn allows(self, restype: ResType) -> bool {
        self.folder
            && match restype {
                ResType::NCS => self.scripts,
                ResType::TWODA => self.tables,
                GUI => self.screens,
                ResType::TLK => false,
                _ => true,
            }
    }

    pub(crate) fn would_read(self, restype: ResType) -> bool {
        self.allows(restype)
    }
}

const GUI: ResType = ResType(2047);

/// The name of a loose file and its type, to where it is.
pub(crate) type Loose = HashMap<(String, ResType), PathBuf>;

static CHOSEN: Mutex<Option<OverrideUse>> = Mutex::new(None);
static FOLDERS: Mutex<Vec<(PathBuf, Arc<Loose>)>> = Mutex::new(Vec::new());
static NOTES: Mutex<Notes> = Mutex::new(Notes {
    used: BTreeSet::new(),
    problems: BTreeSet::new(),
    problems_file: None,
});

struct Notes {
    /// Each file that was read, and whether it took the place of a stock one.
    used: BTreeSet<(String, bool)>,
    problems: BTreeSet<String>,
    problems_file: Option<PathBuf>,
}

/// The player's choice, for every `Install` opened from now on.
pub fn set_override_use(chosen: OverrideUse) {
    if let Ok(mut c) = CHOSEN.lock() {
        *c = Some(chosen);
    }
}

/// What an install in this folder uses. `KOTOR_OVERRIDE` wins, for a driver or the gate: `all`, `on` for all but screens,
/// `data` for all but screens, scripts and tables, or `off`. Only the first game's install has this: the second game's folder is read for its stock rows.
pub(crate) fn override_use_for(dir: &Path) -> OverrideUse {
    if kotor_core::install_dir().as_deref() != Some(dir) {
        return OverrideUse::NONE;
    }
    match std::env::var("KOTOR_OVERRIDE").as_deref() {
        Ok("all") => return OverrideUse::ALL,
        Ok("on") => {
            return OverrideUse {
                screens: false,
                ..OverrideUse::ALL
            }
        }
        Ok("data") => {
            return OverrideUse {
                folder: true,
                ..OverrideUse::NONE
            }
        }
        Ok("off") => return OverrideUse::NONE,
        _ => {}
    }
    CHOSEN
        .lock()
        .ok()
        .and_then(|c| *c)
        .unwrap_or(OverrideUse::NONE)
}

/// The folder's files, listed once a run. A name too long for the game's files can never be asked for.
pub(crate) fn loose_files(dir: &Path) -> Arc<Loose> {
    if let Ok(known) = FOLDERS.lock() {
        if let Some((_, l)) = known.iter().find(|(d, _)| d == dir) {
            return l.clone();
        }
    }
    let mut files = Loose::new();
    if let Ok(read) = std::fs::read_dir(dir.join("Override")) {
        for entry in read.flatten() {
            let path = entry.path();
            let (Some(stem), Some(ext)) = (path.file_stem(), path.extension()) else {
                continue;
            };
            let stem = stem.to_string_lossy().to_ascii_lowercase();
            let Some(restype) = ResType::from_extension(&ext.to_string_lossy()) else {
                continue;
            };
            if stem.len() <= 16 {
                files.insert((stem, restype), path);
            }
        }
    }
    let files = Arc::new(files);
    if let Ok(mut known) = FOLDERS.lock() {
        known.push((dir.to_path_buf(), files.clone()));
    }
    files
}

/// Do these bytes begin as a file of this type should? A file that does not is passed over for the stock one.
pub fn looks_right(restype: ResType, bytes: &[u8]) -> bool {
    let starts = |magic: &[u8]| bytes.starts_with(magic);
    match restype {
        ResType::TWODA => starts(b"2DA V2"),
        ResType::NCS => starts(b"NCS V1.0"),
        ResType::WOK | ResType::BWM | ResType::DWK | ResType::PWK => starts(b"BWM V1.0"),
        // A compiled model starts with four zero bytes. A text one, which no reader here takes, does not.
        ResType::MDL => starts(&[0, 0, 0, 0]),
        ResType::TGA => bytes.len() > 18,
        ResType::TPC => bytes.len() > 128,
        r => match r.extension().filter(|e| is_gff(e)) {
            Some(ext) => {
                let want = format!("{:<4}", ext.to_ascii_uppercase());
                starts(want.as_bytes())
            }
            None => true,
        },
    }
}

/// The types kept in the game's general file format.
pub fn is_gff(ext: &str) -> bool {
    [
        "are", "ifo", "git", "uti", "utc", "dlg", "itp", "utt", "uts", "fac", "ute", "utd", "utp",
        "gui", "utm", "jrl", "utw", "ptm", "ptt",
    ]
    .contains(&ext)
}

/// A file of the folder was read: for the list of what changed a load, and the driver that asks for it.
pub(crate) fn note_used(name: String, replaces: bool) {
    let Ok(mut n) = NOTES.lock() else { return };
    if n.used.insert((name.clone(), replaces)) {
        if let Some(path) = std::env::var_os("KOTOR_OVERRIDE_USED") {
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                let what = if replaces { "replaces" } else { "new" };
                let _ = writeln!(f, "{name} {what}");
            }
        }
    }
}

/// Every file of the folder read in this run, and whether it took the place of a stock one.
pub fn override_used() -> Vec<(String, bool)> {
    NOTES
        .lock()
        .map_or(Vec::new(), |n| n.used.iter().cloned().collect())
}

/// Where problems are kept from now on. The folder is never written to: this is beside the settings.
pub fn set_override_problems_file(path: PathBuf) {
    if let Ok(mut n) = NOTES.lock() {
        // What it says already is not said again.
        let known = std::fs::read_to_string(&path).unwrap_or_default();
        n.problems.extend(known.lines().map(str::to_string));
        n.problems_file = Some(path);
    }
}

/// A file was skipped or failed to parse. The game goes on with the stock one.
pub fn note_override_problem(line: String) {
    let Ok(mut n) = NOTES.lock() else { return };
    if n.problems.insert(line.clone()) {
        if let Some(path) = &n.problems_file {
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                let _ = writeln!(f, "{line}");
            }
        }
    }
}

/// The problems noted in this run.
pub fn override_problems() -> Vec<String> {
    NOTES
        .lock()
        .map_or(Vec::new(), |n| n.problems.iter().cloned().collect())
}
