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
            return Err(Error::format(format!("not a TLK V3.0 file: {tag:?} {ver:?}")));
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
    fn truncated_never_panics() {
        let v = tiny();
        for n in 0..v.len() {
            let _ = Tlk::parse(&v[..n]);
        }
    }
}
