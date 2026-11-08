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

/// A pack: the start of its folder's name, the folder inside it that holds the files (empty for the folder itself; each step
/// the start of a name, as modders' download folders carry a date), the types it brings, the starts of names it leaves alone,
/// when not empty the starts of the only names it brings, and new starts for names that would otherwise take the place of the
/// game's own. Later packs come before earlier ones where they hold the same name.
struct Pack {
    folder: &'static str,
    inside: &'static str,
    types: &'static [&'static str],
    leave: &'static [&'static str],
    only: &'static [&'static str],
    rename: &'static [(&'static str, &'static str)],
}

const PACKS: &[Pack] = &[
    // The skyboxes' models and the pictures made for them go together: the new pictures do not fit the old models.
    Pack {
        folder: "High Quality Skyboxes II",
        inside: "Override",
        types: &["tga", "tpc", "txi", "mdl", "mdx", "lyt", "vis", "are"],
        leave: &[],
        only: &[],
        rename: &[],
    },
    Pack {
        folder: "High Resolution Lightsaber Beams",
        inside: "",
        types: &["tga", "txi"],
        leave: &[],
        only: &[],
        rename: &[],
    },
    // The characters' pictures; the hero's own bodies and heads are painted by this game, so they stay.
    Pack {
        folder: "Ultimate Character Overhaul",
        inside: "KOTORII - Ultimate Character Overhaul 3.0 LITE - TPC",
        types: &["tpc"],
        leave: &["pfb", "pmb", "pfh", "pmh"],
        only: &[],
        rename: &[],
    },
    Pack {
        folder: "Kotor Weapon Model Overhaul",
        inside: "KoTOR Weapon Model Overhaul/Override",
        types: &["mdl", "mdx", "tga", "txi"],
        leave: &[],
        only: &[],
        rename: &[],
    },
    Pack {
        folder: "Weapon Model Overhaul Texture Rework",
        inside: "Weapon Model Overhaul Texture Rework/tslpatchdata",
        types: &["tga", "txi"],
        leave: &[],
        only: &[],
        rename: &[],
    },
    // Playable Twi'lek women: only the new heads, their pictures and their portraits. The pack's characters, bodies and tables
    // are for its installer, which changes the game's own; the Race row puts these heads on the player's body instead.
    Pack {
        folder: "K1 HD Female Twilek Diversity",
        inside: "tslpatchdata/Default",
        types: &["mdl", "mdx", "tpc", "tga", "txi"],
        leave: &[],
        only: &["p_twi", "po_ptwi"],
        rename: &[],
    },
    // The playable Jawa: a Jawa's body on the hero's skeleton, made to take the place of the first man's body, and its voice,
    // made to take a hero's. Here they keep names of their own, for the Race row, and the game's own stay. Its empty head, a
    // neck with no picture, is left: the hood is the head.
    Pack {
        folder: "playable Jawa",
        inside: "JAWA PROTOTYPE/JAWA PROTOTYPE",
        types: &["mdl", "mdx"],
        leave: &[],
        only: &["pmbal"],
        rename: &[("pmbal", "p_jawab")],
    },
    Pack {
        folder: "playable Jawa",
        inside: "Jawa Player Sounds/Jawa Player sounds",
        types: &["wav"],
        leave: &[],
        only: &["p_plyermw_"],
        rename: &[("p_plyermw_", "p_jawamw_")],
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
        let Some(read) = inside(folder, pack.inside).and_then(|d| std::fs::read_dir(d).ok()) else {
            continue;
        };
        for entry in read.flatten() {
            let path = entry.path();
            let (Some(stem), Some(ext)) = (path.file_stem(), path.extension()) else {
                continue;
            };
            let mut stem = stem.to_string_lossy().to_ascii_lowercase();
            let ext = ext.to_string_lossy().to_ascii_lowercase();
            if !pack.types.contains(&ext.as_str())
                || pack.leave.iter().any(|l| stem.starts_with(l))
                || (!pack.only.is_empty() && !pack.only.iter().any(|o| stem.starts_with(o)))
                || stem.len() > 16
            {
                continue;
            }
            if let Some((from, to)) = pack.rename.iter().find(|(f, _)| stem.starts_with(f)) {
                stem = format!("{to}{}", &stem[from.len()..]);
            }
            if let Some(restype) = ResType::from_extension(&ext) {
                files.insert((stem, restype), path);
            }
        }
    }
    files
}

/// The folder a pack keeps its files in: each step of `path` the folder of that name, else the first whose name starts so.
fn inside(folder: &Path, path: &str) -> Option<PathBuf> {
    let mut at = folder.to_path_buf();
    for step in path.split('/').filter(|s| !s.is_empty()) {
        let exact = at.join(step);
        if exact.is_dir() {
            at = exact;
            continue;
        }
        let step = step.to_ascii_lowercase();
        let mut found: Vec<PathBuf> = std::fs::read_dir(&at)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.is_dir()
                    && p.file_name().is_some_and(|n| {
                        n.to_string_lossy().to_ascii_lowercase().starts_with(&step)
                    })
            })
            .collect();
        found.sort();
        at = found.into_iter().next()?;
    }
    Some(at)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pack_finds_its_dated_folder_and_renames_what_would_replace_the_games_own() {
        let root = std::env::temp_dir().join(format!("kotor-packs-{}", std::process::id()));
        let models = root.join("playable Jawa/JAWA PROTOTYPE 1837 1 2026-07-21/JAWA PROTOTYPE");
        let sounds =
            root.join("playable Jawa/Jawa Player Sounds 1837 2 2026-07-24/Jawa Player sounds");
        std::fs::create_dir_all(&models).unwrap();
        std::fs::create_dir_all(&sounds).unwrap();
        for f in [
            "pmbal.mdl",
            "pmbal.mdx",
            "pmha01.mdl",
            "pmbala01.tga",
            "appearance.2da",
        ] {
            std::fs::write(models.join(f), b"").unwrap();
        }
        std::fs::write(sounds.join("P_PLYERMW_ATK1.wav"), b"").unwrap();
        let files = scan_packs(&root);
        let _ = std::fs::remove_dir_all(&root);
        let mut names: Vec<String> = files.keys().map(|(n, _)| n.clone()).collect();
        names.sort();
        assert_eq!(names, ["p_jawab", "p_jawab", "p_jawamw_atk1"]);
        assert!(files.contains_key(&("p_jawamw_atk1".to_string(), ResType::WAV)));
    }
}
