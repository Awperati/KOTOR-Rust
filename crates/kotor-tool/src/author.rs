//! Content authoring helpers. The smallest end-to-end proof: copy a base module into a new `.mod`
//! with its display name changed. Output goes only where the caller says; never into the install.

use kotor_archive::{write_archive, ErfKind, Install, NewResource};
use kotor_core::{Error, ResType, Result};
use kotor_gff::{Gff, LocString, Value};
use std::path::Path;

/// Build the bytes of a `.mod` that holds every resource of module `src`, with `Mod_Name` set to `display_name`.
pub fn clone_module(inst: &mut Install, src: &str, display_name: &str) -> Result<Vec<u8>> {
    inst.load_module(src)?;
    let listing: Vec<(String, ResType)> = inst
        .overlay_resources()
        .map(|(n, t, _)| (n.to_string(), t))
        .collect();
    let mut resources = Vec::with_capacity(listing.len());
    for (name, ty) in listing {
        let mut data = inst.read(&name, ty)?;
        if ty == ResType::IFO && name == "module" {
            let mut g = Gff::parse(&data)?;
            g.root.set(
                "Mod_Name",
                Value::LocString(LocString {
                    strref: -1,
                    strings: vec![(0, display_name.to_string())],
                }),
            );
            data = g.to_bytes()?;
        }
        resources.push(NewResource::new(&name, ty, data));
    }
    write_archive(ErfKind::Mod, &resources)
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
        let bytes = clone_module(&mut inst, "end_m01aa", "Test Spire").unwrap();
        let new = Erf::open(Source::memory(bytes)).unwrap();
        assert_eq!(new.kind, ErfKind::Mod);

        let mut base = Install::open(&dir).unwrap();
        base.load_module("end_m01aa").unwrap();
        let n = base.overlay_resources().count();
        assert_eq!(new.entries.len(), n);
        let ifo = Gff::parse(&new.get("module", ResType::IFO).unwrap()).unwrap();
        assert_eq!(ifo.root.loc("Mod_Name").unwrap().strings[0].1, "Test Spire");
        assert_eq!(ifo.root.str("Mod_Entry_Area"), Some("m01aa"));
        // Everything else is byte-identical to the source.
        let are = base.read("m01aa", ResType::ARE).unwrap();
        assert_eq!(new.get("m01aa", ResType::ARE).unwrap(), are);
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
