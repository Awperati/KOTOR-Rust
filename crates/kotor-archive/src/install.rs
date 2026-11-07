use crate::overrides::{self, Loose, OverrideUse};
use crate::{Bif, Erf, Key, Source};
use kotor_core::{Error, ResRef, ResType, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Identifies a resource in the base game data.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ResourceId {
    pub resref: ResRef,
    pub restype: ResType,
}

/// A read-only view of an installed game: `chitin.key` + BIFs, with
/// optional module archives layered on top. The folder is never written to.
pub struct Install {
    dir: PathBuf,
    key: Key,
    index: HashMap<ResourceId, (u32, u32)>,
    bifs: Vec<Option<Bif>>,
    /// Archives searched before the BIFs, newest first (modules, patch.erf).
    overlays: Vec<Erf>,
    /// What is read from `<install>/Override` first. None, unless the player has chosen it.
    override_use: OverrideUse,
    loose: Option<Arc<Loose>>,
    /// Load a module from its original `.rim` pair even when a `.mod` of the same name exists. Off by default.
    prefer_rim: bool,
}

impl Install {
    pub fn open(dir: impl AsRef<Path>) -> Result<Install> {
        let dir = dir.as_ref().to_path_buf();
        let key = Key::parse(&std::fs::read(dir.join("chitin.key"))?)?;
        let mut index = HashMap::with_capacity(key.entries.len());
        for e in &key.entries {
            let id = ResourceId {
                resref: e.resref.clone(),
                restype: e.restype,
            };
            index.insert(id, (e.bif_index, e.res_index));
        }
        let bifs = vec![None; key.bifs.len()];
        let override_use = overrides::override_use_for(&dir);
        let loose = override_use.folder.then(|| overrides::loose_files(&dir));
        Ok(Install {
            dir,
            key,
            index,
            bifs,
            overlays: Vec::new(),
            override_use,
            loose,
            prefer_rim: false,
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn key(&self) -> &Key {
        &self.key
    }

    /// A `.mod` beside a module's `.rim` pair is an installed mod. On: ignore it and load the original.
    pub fn set_prefer_rim(&mut self, on: bool) {
        self.prefer_rim = on;
    }

    /// Whether loose files in the `Override` folder win over everything else.
    /// Off by default so the engine sees the stock game; the folder is only ever read.
    pub fn set_honour_override(&mut self, on: bool) {
        self.override_use = if on {
            OverrideUse::ALL
        } else {
            OverrideUse::NONE
        };
        self.loose = on.then(|| overrides::loose_files(&self.dir));
    }

    /// The same, by part.
    pub fn set_override_use(&mut self, chosen: OverrideUse) {
        self.override_use = chosen;
        self.loose = chosen.folder.then(|| overrides::loose_files(&self.dir));
    }

    fn override_file(&self, resref: &str, restype: ResType) -> Option<&PathBuf> {
        if !self.override_use.would_read(restype) {
            return None;
        }
        let key = (resref.to_ascii_lowercase(), restype);
        self.loose.as_ref()?.get(&key)
    }

    /// Is this in the `Override` folder, and used? A picture there wins whatever type the game's own is.
    pub fn in_override(&self, resref: &str, restype: ResType) -> bool {
        self.override_file(resref, restype).is_some()
    }

    /// The talk table in use: the one beside the game, or the folder's if the player has asked for that.
    pub fn talk_table_file(&self) -> PathBuf {
        let theirs = self.dir.join("Override").join("dialog.tlk");
        if self.override_use.folder && self.override_use.talk && theirs.is_file() {
            overrides::note_used("dialog.tlk".to_string(), true);
            return theirs;
        }
        self.dir.join("dialog.tlk")
    }

    /// Add an archive that takes priority over everything added before it.
    pub fn push_overlay(&mut self, erf: Erf) {
        self.overlays.insert(0, erf);
    }

    /// Load `modules/<name>.rim`, `<name>_s.rim` or `<name>.mod` as overlays.
    /// A `.mod` file replaces the RIMs, as in the game.
    pub fn load_module(&mut self, name: &str) -> Result<()> {
        let modules = self.dir.join("modules");
        let m = modules.join(format!("{name}.mod"));
        let has_rim = modules.join(format!("{name}.rim")).is_file();
        if m.is_file() && !(self.prefer_rim && has_rim) {
            self.push_overlay(Erf::open(Source::file(m))?);
            return Ok(());
        }
        let base = modules.join(format!("{name}.rim"));
        if !base.is_file() {
            return Err(Error::NotFound(format!("module {name}")));
        }
        self.push_overlay(Erf::open(Source::file(base))?);
        let s = modules.join(format!("{name}_s.rim"));
        if s.is_file() {
            self.push_overlay(Erf::open(Source::file(s))?);
        }
        Ok(())
    }

    /// Add the texture pack archives (`TexturePacks/*.erf`) as overlays, with the
    /// highest-resolution pack taking priority.
    pub fn load_texture_packs(&mut self) -> Result<()> {
        for pack in [
            "swpc_tex_gui",
            "swpc_tex_tpc",
            "swpc_tex_tpb",
            "swpc_tex_tpa",
        ] {
            let path = self.dir.join("TexturePacks").join(format!("{pack}.erf"));
            if path.is_file() {
                self.push_overlay(Erf::open(Source::file(path))?);
            }
        }
        Ok(())
    }

    /// Resources of one type in the overlays (modules, texture packs, ...), unsorted.
    pub fn overlay_resources(&self) -> impl Iterator<Item = (&kotor_core::ResRef, ResType, u32)> {
        self.overlays
            .iter()
            .flat_map(|o| o.entries.iter().map(|e| (&e.resref, e.restype, e.size)))
    }

    fn bif(&mut self, i: u32) -> Result<&Bif> {
        let i = i as usize;
        if self.bifs[i].is_none() {
            let rel = self.key.bifs[i].replace('\\', "/");
            let path = self.dir.join(rel);
            self.bifs[i] = Some(Bif::open(Source::file(path))?);
        }
        Ok(self.bifs[i].as_ref().expect("just loaded"))
    }

    pub fn contains(&self, resref: &str, restype: ResType) -> bool {
        let rr = ResRef::new(resref);
        self.override_file(resref, restype).is_some()
            || self.overlays.iter().any(|o| o.find(&rr, restype).is_some())
            || self.index.contains_key(&ResourceId {
                resref: rr,
                restype,
            })
    }

    /// Read a resource: overlays first, then the BIFs named by the key file.
    pub fn read(&mut self, resref: &str, restype: ResType) -> Result<Vec<u8>> {
        if let Some(p) = self.override_file(resref, restype).cloned() {
            let file = format!("{resref}.{restype}").to_ascii_lowercase();
            match std::fs::read(&p) {
                Ok(bytes) if overrides::looks_right(restype, &bytes) => {
                    overrides::note_used(file, self.stock_has(resref, restype));
                    return Ok(bytes);
                }
                Ok(_) => overrides::note_override_problem(format!(
                    "{file}: not a {restype} file, so the stock one is used"
                )),
                Err(e) => overrides::note_override_problem(format!(
                    "{file}: cannot be read ({e}), so the stock one is used"
                )),
            }
        }
        self.read_stock(resref, restype)
    }

    fn stock_has(&self, resref: &str, restype: ResType) -> bool {
        let rr = ResRef::new(resref);
        self.overlays.iter().any(|o| o.find(&rr, restype).is_some())
            || self.index.contains_key(&ResourceId {
                resref: rr,
                restype,
            })
    }

    /// Read a resource past the `Override` folder: for one there that turned out not to parse.
    pub fn read_stock(&mut self, resref: &str, restype: ResType) -> Result<Vec<u8>> {
        let rr = ResRef::new(resref);
        for o in &self.overlays {
            if let Some(e) = o.find(&rr, restype) {
                return o.read(e);
            }
        }
        let id = ResourceId {
            resref: rr,
            restype,
        };
        let (bif, idx) = *self
            .index
            .get(&id)
            .ok_or_else(|| Error::NotFound(format!("{resref}.{restype}")))?;
        self.bif(bif)?.read(idx)
    }

    /// All resource names in the base data of one type, sorted.
    pub fn list(&self, restype: ResType) -> Vec<String> {
        let mut v: Vec<String> = self
            .key
            .entries
            .iter()
            .filter(|e| e.restype == restype)
            .map(|e| e.resref.to_string())
            .collect();
        v.sort();
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_through_key_and_bif() {
        let Some(dir) = kotor_core::install_dir() else {
            return;
        };
        let mut inst = Install::open(dir).unwrap();
        let data = inst.read("appearance", ResType::TWODA).unwrap();
        assert_eq!(&data[..8], b"2DA V2.b");
        assert!(inst.read("no_such_thing", ResType::TWODA).is_err());
        assert!(inst.list(ResType::TWODA).len() > 50);
    }

    #[test]
    fn override_is_ignored_unless_enabled() {
        let Some(dir) = kotor_core::install_dir() else {
            return;
        };
        // The owner's Override holds a loose model (CP_CommF02.mdl); the BIFs do not.
        let mut inst = Install::open(dir).unwrap();
        let in_override = inst.dir().join("Override").join("CP_CommF02.mdl").is_file();
        assert!(!inst.contains("cp_commf02", ResType::MDL));
        inst.set_honour_override(true);
        assert_eq!(inst.contains("cp_commf02", ResType::MDL), in_override);
    }

    #[test]
    fn module_overlay_wins() {
        let Some(dir) = kotor_core::install_dir() else {
            return;
        };
        let mut inst = Install::open(dir).unwrap();
        inst.load_module("end_m01aa").unwrap();
        let ifo = inst.read("module", ResType::IFO).unwrap();
        assert_eq!(&ifo[..4], b"IFO ");
    }
}
