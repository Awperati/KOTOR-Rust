use kotor_core::{cp1252, Error, Reader, Result};

#[derive(Clone, Debug, PartialEq)]
pub struct TlkEntry {
    pub flags: u32,
    pub sound_resref: String,
    pub sound_length: f32,
    offset: u32,
    size: u32,
}

impl TlkEntry {
    pub fn has_text(&self) -> bool {
        self.flags & 1 != 0
    }
    pub fn has_sound(&self) -> bool {
        self.flags & 2 != 0
    }
}

/// A talk table (`dialog.tlk`). Text is decoded on demand.
pub struct Tlk {
    pub language: u32,
    pub entries: Vec<TlkEntry>,
    text: Vec<u8>,
}

impl Tlk {
    pub fn parse(data: &[u8]) -> Result<Tlk> {
        let mut r = Reader::new(data);
        let tag = r.tag()?;
        let ver = r.tag()?;
        if &tag != b"TLK " || &ver != b"V3.0" {
            return Err(Error::format(format!(
                "not a TLK V3.0 file: {tag:?} {ver:?}"
            )));
        }
        let language = r.u32()?;
        let count = r.u32()? as usize;
        let text_off = r.u32()? as usize;
        // 40-byte entries.
        let mut entries = Vec::with_capacity(count.min(1 << 20));
        for _ in 0..count {
            let flags = r.u32()?;
            let sound_resref = r.fixed_str(16)?;
            let _volume_variance = r.u32()?;
            let _pitch_variance = r.u32()?;
            let offset = r.u32()?;
            let size = r.u32()?;
            let sound_length = r.f32()?;
            entries.push(TlkEntry {
                flags,
                sound_resref,
                sound_length,
                offset,
                size,
            });
        }
        let text = data
            .get(text_off..)
            .ok_or_else(|| Error::format("tlk text offset past end"))?
            .to_vec();
        for (i, e) in entries.iter().enumerate() {
            if e.offset as usize + e.size as usize > text.len() {
                return Err(Error::format(format!("tlk entry {i} text past end")));
            }
        }
        Ok(Tlk {
            language,
            entries,
            text,
        })
    }

    /// An empty table, for building a custom `dialog.tlk` or a mod string table.
    pub fn new(language: u32) -> Tlk {
        Tlk {
            language,
            entries: Vec::new(),
            text: Vec::new(),
        }
    }

    /// Append a string (and optional voice-over resref) and return its strref.
    pub fn push(&mut self, text: Option<&str>, sound_resref: &str) -> u32 {
        let bytes = text.map(kotor_core::cp1252_encode).unwrap_or_default();
        let mut flags = 0;
        if text.is_some() {
            flags |= 1;
        }
        if !sound_resref.is_empty() {
            flags |= 2;
        }
        self.entries.push(TlkEntry {
            flags,
            sound_resref: sound_resref.to_string(),
            sound_length: 0.0,
            offset: self.text.len() as u32,
            size: bytes.len() as u32,
        });
        self.text.extend_from_slice(&bytes);
        (self.entries.len() - 1) as u32
    }

    /// Serialise as TLK V3.0. Volume and pitch variance are written as 0.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let n = u32::try_from(self.entries.len()).map_err(|_| Error::format("too many strings"))?;
        let mut out = b"TLK V3.0".to_vec();
        for v in [self.language, n, 20 + 40 * n] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for e in &self.entries {
            if e.sound_resref.len() > 16 || !e.sound_resref.is_ascii() {
                return Err(Error::format("tlk sound resref longer than 16 chars"));
            }
            let mut name = [0u8; 16];
            name[..e.sound_resref.len()].copy_from_slice(e.sound_resref.as_bytes());
            out.extend_from_slice(&e.flags.to_le_bytes());
            out.extend_from_slice(&name);
            out.extend_from_slice(&[0u8; 8]);
            out.extend_from_slice(&e.offset.to_le_bytes());
            out.extend_from_slice(&e.size.to_le_bytes());
            out.extend_from_slice(&e.sound_length.to_le_bytes());
        }
        out.extend_from_slice(&self.text);
        Ok(out)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Text for a string reference, or `None` if out of range or the entry has no text.
    pub fn get(&self, strref: u32) -> Option<String> {
        let e = self.entries.get(strref as usize)?;
        if !e.has_text() {
            return None;
        }
        let s = e.offset as usize;
        Some(cp1252(&self.text[s..s + e.size as usize]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny() -> Vec<u8> {
        let mut v = b"TLK V3.0".to_vec();
        for x in [0u32, 2, 20 + 80] {
            v.extend_from_slice(&x.to_le_bytes());
        }
        for (flags, off, size) in [(1u32, 0u32, 2u32), (0, 2, 0)] {
            v.extend_from_slice(&flags.to_le_bytes());
            v.extend_from_slice(&[0u8; 16]);
            v.extend_from_slice(&[0u8; 8]);
            v.extend_from_slice(&off.to_le_bytes());
            v.extend_from_slice(&size.to_le_bytes());
            v.extend_from_slice(&0f32.to_le_bytes());
        }
        v.extend_from_slice(b"hi");
        v
    }

    #[test]
    fn synthetic() {
        let t = Tlk::parse(&tiny()).unwrap();
        assert_eq!(t.get(0).as_deref(), Some("hi"));
        assert_eq!(t.get(1), None);
        assert_eq!(t.get(99), None);
    }

    #[test]
    fn built_table_round_trips() {
        let mut t = Tlk::new(0);
        let a = t.push(Some("Hello \u{2019}there\u{2019}"), "");
        let b = t.push(None, "vo_line_01");
        let c = t.push(Some(""), "");
        let back = Tlk::parse(&t.to_bytes().unwrap()).unwrap();
        assert_eq!(back.len(), 3);
        assert_eq!(back.get(a).as_deref(), Some("Hello \u{2019}there\u{2019}"));
        assert_eq!(back.get(b), None);
        assert!(back.entries[b as usize].has_sound());
        assert_eq!(back.entries[b as usize].sound_resref, "vo_line_01");
        assert_eq!(back.get(c).as_deref(), Some(""));
    }

    #[test]
    fn real_dialog_tlk_survives_rewrite() {
        let Some(dir) = kotor_core::install_dir() else {
            return;
        };
        let t = Tlk::parse(&std::fs::read(dir.join("dialog.tlk")).unwrap()).unwrap();
        let back = Tlk::parse(&t.to_bytes().unwrap()).unwrap();
        assert_eq!(back.len(), t.len());
        assert_eq!(back.language, t.language);
        for i in 0..t.len() as u32 {
            assert_eq!(back.get(i), t.get(i), "strref {i}");
            let (a, b) = (&back.entries[i as usize], &t.entries[i as usize]);
            assert_eq!(
                (a.flags, &a.sound_resref),
                (b.flags, &b.sound_resref),
                "strref {i}"
            );
        }
    }

    #[test]
    fn truncated_never_panics() {
        let v = tiny();
        for n in 0..v.len() {
            let _ = Tlk::parse(&v[..n]);
        }
    }
}
