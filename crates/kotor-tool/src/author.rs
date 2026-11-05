//! Content authoring helpers. Output goes only where the caller says; never into the install.

use kotor_archive::{write_archive, ErfKind, Install, NewResource};
use kotor_core::{Error, ResType, Result};
use kotor_gff::dlg::Dialog;
use kotor_gff::{Gff, LocString, Struct, Value};
use std::path::Path;

/// Result of cloning a module.
pub struct Cloned {
    pub bytes: Vec<u8>,
    /// `old.ext -> new.ext` for every resource that was renamed or copied under the new area name.
    pub renamed: Vec<String>,
    /// Number of string fields rewritten inside IFO/ARE/GIT.
    pub string_edits: usize,
}

/// Replace every string or resref equal to `old` (case-insensitive) with `new`.
fn replace_strings(s: &mut Struct, old: &str, new: &str) -> usize {
    let mut n = 0;
    for (_, v) in &mut s.fields {
        match v {
            Value::String(t) | Value::ResRef(t) if t.eq_ignore_ascii_case(old) => {
                *t = new.to_string();
                n += 1;
            }
            Value::Struct(c) => n += replace_strings(c, old, new),
            Value::List(l) => {
                n += l
                    .iter_mut()
                    .map(|c| replace_strings(c, old, new))
                    .sum::<usize>()
            }
            _ => {}
        }
    }
    n
}

fn valid_name(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 16
        && n.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Build a `.mod` that holds every resource of module `src` with `Mod_Name` set to `display_name`.
///
/// With `new_area`, the entry area is renamed too so the copy can sit beside the original:
/// resources named after the area are renamed, its LYT/VIS are copied from the base data under
/// the new name, and IFO/ARE/GIT strings that name the area are rewritten. Room models and
/// scripts keep their original names, and compiled scripts that mention the old area by name
/// are not touched.
pub fn clone_module(
    inst: &mut Install,
    src: &str,
    display_name: &str,
    new_area: Option<&str>,
) -> Result<Cloned> {
    inst.load_module(src)?;
    let area = {
        let ifo = Gff::parse(&inst.read("module", ResType::IFO)?)?;
        ifo.root
            .str("Mod_Entry_Area")
            .ok_or_else(|| Error::format("module.ifo has no Mod_Entry_Area"))?
            .to_ascii_lowercase()
    };
    if let Some(new) = new_area {
        if !valid_name(new) {
            return Err(Error::format(
                "new area name must be 1-16 chars of a-z, 0-9 or _ (lower case)",
            ));
        }
        if new == area || inst.contains(new, ResType::LYT) || inst.contains(new, ResType::ARE) {
            return Err(Error::format(format!(
                "area name '{new}' is already in use"
            )));
        }
    }
    let listing: Vec<(String, ResType)> = inst
        .overlay_resources()
        .map(|(n, t, _)| (n.to_string(), t))
        .collect();
    let mut resources = Vec::with_capacity(listing.len() + 2);
    let (mut renamed, mut string_edits) = (Vec::new(), 0);
    for (name, ty) in listing {
        let mut data = inst.read(&name, ty)?;
        let is_area_res = name.eq_ignore_ascii_case(&area);
        let is_gff = ty == ResType::IFO || ty == ResType::ARE || ty == ResType::GIT;
        if ty == ResType::IFO && name == "module" {
            let mut g = Gff::parse(&data)?;
            g.root.set(
                "Mod_Name",
                Value::LocString(LocString {
                    strref: -1,
                    strings: vec![(0, display_name.to_string())],
                }),
            );
            if let Some(new) = new_area {
                string_edits += replace_strings(&mut g.root, &area, new);
            }
            data = g.to_bytes()?;
        } else if let (Some(new), true) = (new_area, is_gff) {
            let mut g = Gff::parse(&data)?;
            string_edits += replace_strings(&mut g.root, &area, new);
            data = g.to_bytes()?;
        }
        let out_name = match new_area {
            Some(new) if is_area_res => {
                renamed.push(format!("{name}.{ty} -> {new}.{ty}"));
                new.to_string()
            }
            _ => name,
        };
        resources.push(NewResource::new(&out_name, ty, data));
    }
    if let Some(new) = new_area {
        for ty in [ResType::LYT, ResType::VIS] {
            if let Ok(data) = inst.read(&area, ty) {
                renamed.push(format!("{area}.{ty} -> {new}.{ty} (copied from base data)"));
                resources.push(NewResource::new(new, ty, data));
            }
        }
    }
    Ok(Cloned {
        bytes: write_archive(ErfKind::Mod, &resources)?,
        renamed,
        string_edits,
    })
}

/// Turn a dialogue authoring text into DLG file bytes.
pub fn dialog_from_text(src: &str) -> Result<Vec<u8>> {
    Dialog::from_text(src)?.to_gff().to_bytes()
}

/// Refuse to write inside the game folder.
pub fn check_output_path(inst: &Install, out: &Path) -> Result<()> {
    let install = std::fs::canonicalize(inst.dir())?;
    let parent = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = std::fs::canonicalize(parent)?;
    if parent.starts_with(&install) {
        return Err(Error::format(
            "refusing to write inside the game folder; choose another output folder",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kotor_archive::{Erf, Source};

    #[test]
    fn clone_keeps_resources_and_renames() {
        let Some(dir) = kotor_core::install_dir() else {
            return;
        };
        let mut inst = Install::open(&dir).unwrap();
        let c = clone_module(&mut inst, "end_m01aa", "Test Spire", None).unwrap();
        let new = Erf::open(Source::memory(c.bytes)).unwrap();
        assert_eq!(new.kind, ErfKind::Mod);

        let mut base = Install::open(&dir).unwrap();
        base.load_module("end_m01aa").unwrap();
        let n = base.overlay_resources().count();
        assert_eq!(new.entries.len(), n);
        let ifo = Gff::parse(&new.get("module", ResType::IFO).unwrap()).unwrap();
        assert_eq!(ifo.root.loc("Mod_Name").unwrap().strings[0].1, "Test Spire");
        assert_eq!(ifo.root.str("Mod_Entry_Area"), Some("m01aa"));
        let are = base.read("m01aa", ResType::ARE).unwrap();
        assert_eq!(new.get("m01aa", ResType::ARE).unwrap(), are);
    }

    #[test]
    fn area_rename_sits_beside_the_original() {
        let Some(dir) = kotor_core::install_dir() else {
            return;
        };
        let mut inst = Install::open(&dir).unwrap();
        let c = clone_module(&mut inst, "end_m01aa", "Spire Copy", Some("zz_spire")).unwrap();
        assert!(c.string_edits >= 2, "only {} edits", c.string_edits);
        let new = Erf::open(Source::memory(c.bytes)).unwrap();
        let has = |n: &str, t| new.find(&kotor_core::ResRef::new(n), t).is_some();
        assert!(has("zz_spire", ResType::ARE));
        assert!(has("zz_spire", ResType::GIT));
        assert!(has("zz_spire", ResType::LYT));
        assert!(has("zz_spire", ResType::VIS));
        assert!(!has("m01aa", ResType::ARE));
        assert!(!has("m01aa", ResType::GIT));

        // No IFO/ARE/GIT string still names the old area.
        fn mentions(s: &Struct, name: &str) -> bool {
            s.fields.iter().any(|(_, v)| match v {
                Value::String(t) | Value::ResRef(t) => t.eq_ignore_ascii_case(name),
                Value::Struct(c) => mentions(c, name),
                Value::List(l) => l.iter().any(|c| mentions(c, name)),
                _ => false,
            })
        }
        for ty in [ResType::IFO, ResType::ARE, ResType::GIT] {
            let name = if ty == ResType::IFO {
                "module"
            } else {
                "zz_spire"
            };
            let g = Gff::parse(&new.get(name, ty).unwrap()).unwrap();
            assert!(!mentions(&g.root, "m01aa"), "{ty} still names m01aa");
        }
        let ifo = Gff::parse(&new.get("module", ResType::IFO).unwrap()).unwrap();
        assert_eq!(ifo.root.str("Mod_Entry_Area"), Some("zz_spire"));
        // The layout is a byte copy of the original's.
        let lyt = inst.read("m01aa", ResType::LYT).unwrap();
        assert_eq!(new.get("zz_spire", ResType::LYT).unwrap(), lyt);
    }

    #[test]
    fn bad_or_taken_area_names_are_refused() {
        let Some(dir) = kotor_core::install_dir() else {
            return;
        };
        for bad in [
            "m01aa",
            "M01AA",
            "has space",
            "this_is_far_too_long_a_name",
            "m02aa",
        ] {
            let mut inst = Install::open(&dir).unwrap();
            assert!(
                clone_module(&mut inst, "end_m01aa", "x", Some(bad)).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn dialogue_text_builds_a_dlg() {
        let bytes = dialog_from_text(
            "start E0\nentry E0 speaker=Carth: Hi.\n  link R0\nreply R0: Hello.\n",
        )
        .unwrap();
        let d = Dialog::from_gff(&Gff::parse(&bytes).unwrap()).unwrap();
        assert_eq!(d.entries.len(), 1);
        assert_eq!(d.entries[0].links[0].index, 0);
    }

    #[test]
    fn output_inside_install_is_refused() {
        let Some(dir) = kotor_core::install_dir() else {
            return;
        };
        let inst = Install::open(&dir).unwrap();
        assert!(check_output_path(&inst, &dir.join("modules").join("x.mod")).is_err());
        assert!(check_output_path(&inst, &std::env::temp_dir().join("x.mod")).is_ok());
    }
}
