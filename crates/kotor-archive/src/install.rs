use crate::{Bif, Erf, Key, Source};
use kotor_core::{Error, ResRef, ResType, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
        Ok(Install {
            dir,
            key,
            index,
            bifs,
            overlays: Vec::new(),
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn key(&self) -> &Key {
        &self.key
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
        if m.is_file() {
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
        self.overlays.iter().any(|o| o.find(&rr, restype).is_some())
            || self.index.contains_key(&ResourceId {
                resref: rr,
                restype,
            })
    }

    /// Read a resource: overlays first, then the BIFs named by the key file.
    pub fn read(&mut self, resref: &str, restype: ResType) -> Result<Vec<u8>> {
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
