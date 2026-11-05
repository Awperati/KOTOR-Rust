use crate::Source;
use kotor_core::{Error, Reader, ResRef, ResType, Result};

/// Which container layout a file uses. ERF, MOD and SAV share one layout; RIM differs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErfKind {
    Erf,
    Mod,
    Sav,
    Rim,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErfEntry {
    pub resref: ResRef,
    pub restype: ResType,
    pub offset: u32,
    pub size: u32,
}

/// An ERF, MOD, SAV or RIM archive: a flat list of named resources.
#[derive(Clone, Debug)]
pub struct Erf {
    pub kind: ErfKind,
    pub entries: Vec<ErfEntry>,
    source: Source,
}

impl Erf {
    pub fn open(source: Source) -> Result<Erf> {
        let head = source.read_at(0, 160.min(source.len()? as usize))?;
        let mut r = Reader::new(&head);
        let tag = r.tag()?;
        let ver = r.tag()?;
        match &tag {
            b"RIM " => Self::open_rim(source, &mut r, &ver),
            b"ERF " | b"MOD " | b"SAV " | b"HAK " => Self::open_erf(source, &mut r, &tag, &ver),
            _ => Err(Error::format(format!("unknown archive tag {tag:?}"))),
        }
    }

    fn open_erf(source: Source, r: &mut Reader, tag: &[u8; 4], ver: &[u8; 4]) -> Result<Erf> {
        if ver != b"V1.0" {
            return Err(Error::format(format!("unsupported ERF version {ver:?}")));
        }
        let _lang_count = r.u32()?;
        let _loc_size = r.u32()?;
        let entry_count = r.u32()? as usize;
        let _loc_off = r.u32()?;
        let key_off = r.u32()? as u64;
        let res_off = r.u32()? as u64;
        // 24-byte key entries: resref[16], resid u32, type u16, unused u16.
        let keys = source.read_at(key_off, entry_count.checked_mul(24).ok_or_else(over)?)?;
        // 8-byte resource entries: offset, size.
        let ress = source.read_at(res_off, entry_count.checked_mul(8).ok_or_else(over)?)?;
        let mut kr = Reader::new(&keys);
        let mut rr = Reader::new(&ress);
        let mut entries = Vec::with_capacity(entry_count);
        for _ in 0..entry_count {
            let name = kr.fixed_str(16)?;
            let _id = kr.u32()?;
            let restype = ResType(kr.u16()?);
            let _unused = kr.u16()?;
            let offset = rr.u32()?;
            let size = rr.u32()?;
            entries.push(ErfEntry {
                resref: ResRef::new(&name),
                restype,
                offset,
                size,
            });
        }
        let kind = match tag {
            b"MOD " => ErfKind::Mod,
            b"SAV " => ErfKind::Sav,
            _ => ErfKind::Erf,
        };
        Ok(Erf {
            kind,
            entries,
            source,
        })
    }

    fn open_rim(source: Source, r: &mut Reader, ver: &[u8; 4]) -> Result<Erf> {
        if ver != b"V1.0" {
            return Err(Error::format(format!("unsupported RIM version {ver:?}")));
        }
        let _reserved = r.u32()?;
        let count = r.u32()? as usize;
        let table_off = r.u32()? as u64;
        // 32-byte entries: resref[16], type u32, id u32, offset u32, size u32.
        let table = source.read_at(table_off, count.checked_mul(32).ok_or_else(over)?)?;
        let mut t = Reader::new(&table);
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            let name = t.fixed_str(16)?;
            let restype = ResType(t.u32()? as u16);
            let _id = t.u32()?;
            let offset = t.u32()?;
            let size = t.u32()?;
            entries.push(ErfEntry {
                resref: ResRef::new(&name),
                restype,
                offset,
                size,
            });
        }
        Ok(Erf {
            kind: ErfKind::Rim,
            entries,
            source,
        })
    }

    pub fn find(&self, resref: &ResRef, restype: ResType) -> Option<&ErfEntry> {
        self.entries
            .iter()
            .find(|e| e.restype == restype && &e.resref == resref)
    }

    pub fn read(&self, entry: &ErfEntry) -> Result<Vec<u8>> {
        self.source
            .read_at(entry.offset as u64, entry.size as usize)
    }

    pub fn get(&self, resref: &str, restype: ResType) -> Result<Vec<u8>> {
        let rr = ResRef::new(resref);
        let e = self
            .find(&rr, restype)
            .ok_or_else(|| Error::NotFound(format!("{resref}.{restype}")))?;
        self.read(e)
    }
}

fn over() -> Error {
    Error::format("archive entry count overflow")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a tiny ERF in memory to test layout handling without game files.
    fn tiny_erf() -> Vec<u8> {
        let payload = b"hello";
        let mut v = Vec::new();
        v.extend_from_slice(b"MOD V1.0");
        for x in [0u32, 0, 1, 160, 160, 160 + 24, 0, 0, 0xFFFF_FFFF] {
            v.extend_from_slice(&x.to_le_bytes());
        }
        v.resize(160, 0);
        let mut name = [0u8; 16];
        name[..3].copy_from_slice(b"abc");
        v.extend_from_slice(&name);
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&2009u16.to_le_bytes());
        v.extend_from_slice(&0u16.to_le_bytes());
        v.extend_from_slice(&(160u32 + 24 + 8).to_le_bytes());
        v.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        v.extend_from_slice(payload);
        v
    }

    #[test]
    fn parses_synthetic_mod() {
        let erf = Erf::open(Source::memory(tiny_erf())).unwrap();
        assert_eq!(erf.kind, ErfKind::Mod);
        assert_eq!(erf.get("ABC", ResType(2009)).unwrap(), b"hello");
        assert!(erf.get("nope", ResType(2009)).is_err());
    }

    #[test]
    fn truncated_does_not_panic() {
        let v = tiny_erf();
        for n in 0..v.len() {
            let _ = Erf::open(Source::memory(v[..n].to_vec()));
        }
    }

    #[test]
    fn real_module_and_rim() {
        let Some(dir) = kotor_core::install_dir() else {
            return;
        };
        let m = Erf::open(Source::file(dir.join("modules").join("end_m01aa.rim"))).unwrap();
        assert_eq!(m.kind, ErfKind::Rim);
        assert!(!m.entries.is_empty());
        for e in &m.entries {
            let data = m.read(e).unwrap();
            assert_eq!(data.len() as u32, e.size);
        }
        let patch = Erf::open(Source::file(dir.join("patch.erf"))).unwrap();
        assert_eq!(patch.kind, ErfKind::Erf);
        assert!(!patch.entries.is_empty());
        let mod_path = dir.join("lips").join("localization.mod");
        let l = Erf::open(Source::file(mod_path)).unwrap();
        assert_eq!(l.kind, ErfKind::Mod);
        assert!(!l.entries.is_empty());
    }
}
