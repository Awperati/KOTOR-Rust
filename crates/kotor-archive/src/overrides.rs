//! The `Override` folder of the player's own install: loose files that win over the game's archives, as the original engine
//! has it. Only ever read, at run time. What is used is the player's choice, kept for the whole of a run, and is off for
//! anything that has not chosen: tests and tools see the stock game unless `KOTOR_OVERRIDE` says otherwise.
use kotor_core::ResType;
use std::collections::{BTreeSet, HashMap};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

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

/// Mod packs the player keeps in a folder of their own (`KOTOR_MODS_ROOT`, else `Mods` beside the projects): each is a modder's
/// folder, read from there at run time like the `Override` folder and never copied into the repository or a build. The player
/// switches them on as a whole. The install's own `Override` files come before them, and the game's own files after.
static MOD_PACKS: AtomicBool = AtomicBool::new(false);

/// A pack: the start of its folder's name, the folder inside it that holds the files (empty for the folder itself), the types it
/// brings, and the starts of names it leaves alone. Later packs come before earlier ones where they hold the same name.
struct Pack {
    folder: &'static str,
    inside: &'static str,
    types: &'static [&'static str],
    leave: &'static [&'static str],
}

const PACKS: &[Pack] = &[
    // The skyboxes' models and the pictures made for them go together: the new pictures do not fit the old models.
    Pack {
        folder: "High Quality Skyboxes II",
        inside: "Override",
        types: &["tga", "tpc", "txi", "mdl", "mdx", "lyt", "vis", "are"],
        leave: &[],
    },
    Pack {
        folder: "High Resolution Lightsaber Beams",
        inside: "",
        types: &["tga", "txi"],
        leave: &[],
    },
    // The characters' pictures; the hero's own bodies and heads are painted by this game, so they stay.
    Pack {
        folder: "Ultimate Character Overhaul",
        inside: "KOTORII - Ultimate Character Overhaul 3.0 LITE - TPC",
        types: &["tpc"],
        leave: &["pfb", "pmb", "pfh", "pmh"],
    },
    Pack {
        folder: "Kotor Weapon Model Overhaul",
        inside: "KoTOR Weapon Model Overhaul/Override",
        types: &["mdl", "mdx", "tga", "txi"],
        leave: &[],
    },
    Pack {
        folder: "Weapon Model Overhaul Texture Rework",
        inside: "Weapon Model Overhaul Texture Rework/tslpatchdata",
        types: &["tga", "txi"],
        leave: &[],
    },
];

/// The player's choice, for every `Install` opened from now on.
pub fn set_mod_packs(on: bool) {
    MOD_PACKS.store(on, Ordering::Relaxed);
}

/// The folder that holds modders' folders: `KOTOR_MODS_ROOT` if set, else `Mods` beside the projects.
pub fn mods_root() -> PathBuf {
    std::env::var_os("KOTOR_MODS_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Users\User\Projects\Mods"))
}

/// The files of the packs this install uses, or none. `KOTOR_MOD_PACKS` (`on` or `off`) wins over the player's choice, for a
/// driver or the gate. Only the first game's install has them, and tests and tools see none unless they ask.
pub(crate) fn mod_pack_files(dir: &Path) -> Option<Arc<Loose>> {
    if kotor_core::install_dir().as_deref() != Some(dir) {
        return None;
    }
    let on = match std::env::var("KOTOR_MOD_PACKS").as_deref() {
        Ok("on") => true,
        Ok("off") => false,
        _ => MOD_PACKS.load(Ordering::Relaxed),
    };
    if !on {
        return None;
    }
    static FILES: OnceLock<Arc<Loose>> = OnceLock::new();
    Some(
        FILES
            .get_or_init(|| Arc::new(scan_packs(&mods_root())))
            .clone(),
    )
}

/// Every file the packs bring, by name and type, from the folders under `root`. A name too long for the game's files can never be
/// asked for.
pub(crate) fn scan_packs(root: &Path) -> Loose {
    let mut files = Loose::new();
    let Ok(folders) = std::fs::read_dir(root) else {
        return files;
    };
    let folders: Vec<_> = folders.flatten().map(|e| e.path()).collect();
    for pack in PACKS {
        let Some(folder) = folders.iter().find(|f| {
            f.is_dir()
                && f.file_name().is_some_and(|n| {
                    n.to_string_lossy()
                        .to_ascii_lowercase()
                        .starts_with(&pack.folder.to_ascii_lowercase())
                })
        }) else {
            continue;
        };
        let Ok(read) = std::fs::read_dir(folder.join(pack.inside)) else {
            continue;
        };
        for entry in read.flatten() {
            let path = entry.path();
            let (Some(stem), Some(ext)) = (path.file_stem(), path.extension()) else {
                continue;
            };
            let stem = stem.to_string_lossy().to_ascii_lowercase();
            let ext = ext.to_string_lossy().to_ascii_lowercase();
            if !pack.types.contains(&ext.as_str())
                || pack.leave.iter().any(|l| stem.starts_with(l))
                || stem.len() > 16
            {
                continue;
            }
            if let Some(restype) = ResType::from_extension(&ext) {
                files.insert((stem, restype), path);
            }
        }
    }
    files
}
