use kotor_core::{latin1, Error, Reader, ResRef, ResType, Result};

/// One resource listed in `chitin.key`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEntry {
    pub resref: ResRef,
    pub restype: ResType,
    /// Index into `Key::bifs`.
    pub bif_index: u32,
    /// Index of the resource inside that BIF.
    pub res_index: u32,
}

/// Parsed `chitin.key`: the list of BIF files and the master resource index.
#[derive(Clone, Debug)]
pub struct Key {
    /// BIF paths relative to the game folder, backslash separated as stored.
    pub bifs: Vec<String>,
    pub entries: Vec<KeyEntry>,
}

impl Key {
    pub fn parse(data: &[u8]) -> Result<Key> {
        let mut r = Reader::new(data);
        let tag = r.tag()?;
        let ver = r.tag()?;
        if &tag != b"KEY " || &ver != b"V1  " {
            return Err(Error::format(format!("not a KEY V1 file: {tag:?} {ver:?}")));
        }
        let bif_count = r.u32()? as usize;
        let key_count = r.u32()? as usize;
        let file_table = r.u32()? as usize;
        let key_table = r.u32()? as usize;

        // 12-byte file entries: size, name offset, name length (u16), drives (u16).
        let mut bifs = Vec::with_capacity(bif_count.min(4096));
        r.seek(file_table);
        let mut name_refs = Vec::new();
        for _ in 0..bif_count {
            let _size = r.u32()?;
            let name_off = r.u32()? as usize;
            let name_len = r.u16()? as usize;
            let _drives = r.u16()?;
            name_refs.push((name_off, name_len));
        }
        for (off, len) in name_refs {
            let mut nr = Reader::at(data, off);
            let raw = nr.bytes(len)?;
            let raw = raw.split(|&c| c == 0).next().unwrap_or(&[]);
            bifs.push(latin1(raw));
        }

        // 22-byte key entries: resref[16], type u16, id u32 (bif<<20 | index).
        r.seek(key_table);
        let mut entries = Vec::with_capacity(key_count.min(1 << 20));
        for _ in 0..key_count {
            let name = r.fixed_str(16)?;
            let restype = ResType(r.u16()?);
            let id = r.u32()?;
            let bif_index = id >> 20;
            if bif_index as usize >= bifs.len() {
                return Err(Error::format(format!(
                    "key entry {name} points at bif {bif_index}"
                )));
            }
            entries.push(KeyEntry {
                resref: ResRef::new(&name),
                restype,
                bif_index,
                res_index: id & 0xFFFFF,
            });
        }
        Ok(Key { bifs, entries })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_magic() {
        assert!(Key::parse(&[0u8; 64]).is_err());
    }

    #[test]
    fn real_chitin_key() {
        let Some(dir) = kotor_core::install_dir() else {
            return;
        };
        let data = std::fs::read(dir.join("chitin.key")).unwrap();
        let key = Key::parse(&data).unwrap();
        assert!(key.bifs.len() > 20, "bifs: {}", key.bifs.len());
        assert!(key.entries.len() > 10_000);
        assert!(key
            .bifs
            .iter()
            .any(|b| b.to_lowercase().contains("2da.bif")));
        assert!(key
            .entries
            .iter()
            .any(|e| e.resref.as_str() == "appearance" && e.restype == ResType::TWODA));
    }
}
