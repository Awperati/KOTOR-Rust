use crate::Source;
use kotor_core::{Error, Reader, ResType, Result};

#[derive(Clone, Debug)]
struct BifEntry {
    offset: u32,
    size: u32,
    restype: ResType,
}

/// A BIF data file. Only the header and table are loaded; payloads are read on demand.
#[derive(Clone, Debug)]
pub struct Bif {
    source: Source,
    entries: Vec<BifEntry>,
}

impl Bif {
    pub fn open(source: Source) -> Result<Bif> {
        // 20-byte header, then 16 bytes per variable resource.
        let head = source.read_at(0, 20)?;
        let mut r = Reader::new(&head);
        let tag = r.tag()?;
        let ver = r.tag()?;
        if &tag != b"BIFF" || &ver != b"V1  " {
            return Err(Error::format(format!(
                "not a BIFF V1 file: {tag:?} {ver:?}"
            )));
        }
        let var_count = r.u32()? as usize;
        let _fixed_count = r.u32()?;
        let table_off = r.u32()? as u64;
        let table_len = var_count
            .checked_mul(16)
            .ok_or_else(|| Error::format("bif resource count overflow"))?;
        let table = source.read_at(table_off, table_len)?;
        let mut t = Reader::new(&table);
        let mut entries = Vec::with_capacity(var_count);
        for i in 0..var_count {
            let id = t.u32()?;
            let offset = t.u32()?;
            let size = t.u32()?;
            let restype = ResType(t.u32()? as u16);
            if (id & 0xFFFFF) as usize != i {
                return Err(Error::format(format!(
                    "bif entry {i} has id index {}",
                    id & 0xFFFFF
                )));
            }
            entries.push(BifEntry {
                offset,
                size,
                restype,
            });
        }
        Ok(Bif { source, entries })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn restype(&self, index: u32) -> Option<ResType> {
        self.entries.get(index as usize).map(|e| e.restype)
    }

    pub fn size(&self, index: u32) -> Option<u32> {
        self.entries.get(index as usize).map(|e| e.size)
    }

    pub fn read(&self, index: u32) -> Result<Vec<u8>> {
        let e = self
            .entries
            .get(index as usize)
            .ok_or_else(|| Error::NotFound(format!("bif resource {index}")))?;
        self.source.read_at(e.offset as u64, e.size as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_garbage() {
        assert!(Bif::open(Source::memory(vec![0; 64])).is_err());
    }

    #[test]
    fn real_2da_bif() {
        let Some(dir) = kotor_core::install_dir() else {
            return;
        };
        let bif = Bif::open(Source::file(dir.join("data").join("2da.bif"))).unwrap();
        assert!(bif.len() > 50);
        let first = bif.read(0).unwrap();
        assert_eq!(first.len() as u32, bif.size(0).unwrap());
        assert_eq!(bif.restype(0), Some(ResType::TWODA));
        assert_eq!(&first[..8], b"2DA V2.b");
    }
}
