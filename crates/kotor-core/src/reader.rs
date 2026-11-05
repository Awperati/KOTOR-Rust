use crate::{Error, Result};

/// Bounds-checked little-endian cursor over a byte slice.
#[derive(Clone)]
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

macro_rules! num {
    ($name:ident, $t:ty, $from:ident) => {
        pub fn $name(&mut self) -> Result<$t> {
            const N: usize = std::mem::size_of::<$t>();
            let b = self.bytes(N)?;
            let mut a = [0u8; N];
            a.copy_from_slice(b);
            Ok(<$t>::$from(a))
        }
    };
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Reader { data, pos: 0 }
    }

    pub fn at(data: &'a [u8], pos: usize) -> Self {
        Reader { data, pos }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn seek(&mut self, pos: usize) {
        self.pos = pos;
    }

    pub fn skip(&mut self, n: usize) {
        self.pos = self.pos.saturating_add(n);
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        match self.pos.checked_add(n).filter(|&e| e <= self.data.len()) {
            Some(end) => {
                let s = &self.data[self.pos..end];
                self.pos = end;
                Ok(s)
            }
            None => Err(Error::Eof {
                offset: self.pos,
                wanted: n,
                len: self.data.len(),
            }),
        }
    }

    num!(u8, u8, from_le_bytes);
    num!(i8, i8, from_le_bytes);
    num!(u16, u16, from_le_bytes);
    num!(i16, i16, from_le_bytes);
    num!(u32, u32, from_le_bytes);
    num!(i32, i32, from_le_bytes);
    num!(u64, u64, from_le_bytes);
    num!(i64, i64, from_le_bytes);
    num!(f32, f32, from_le_bytes);
    num!(f64, f64, from_le_bytes);

    // Big-endian reads: the script VM stores its operands big-endian.
    num!(u16_be, u16, from_be_bytes);
    num!(i16_be, i16, from_be_bytes);
    num!(u32_be, u32, from_be_bytes);
    num!(i32_be, i32, from_be_bytes);
    num!(f32_be, f32, from_be_bytes);

    /// Fixed-width, NUL-padded Latin-1 string field.
    pub fn fixed_str(&mut self, n: usize) -> Result<String> {
        let b = self.bytes(n)?;
        Ok(latin1(b.split(|&c| c == 0).next().unwrap_or(&[])))
    }

    pub fn tag(&mut self) -> Result<[u8; 4]> {
        let b = self.bytes(4)?;
        Ok([b[0], b[1], b[2], b[3]])
    }

    pub fn vec3(&mut self) -> Result<[f32; 3]> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }
}

/// Decode Latin-1 bytes (the closest match to the game's code page).
pub fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_bounds() {
        let mut r = Reader::new(&[1, 0, 0, 0, 0xff, 0xff]);
        assert_eq!(r.u32().unwrap(), 1);
        assert_eq!(r.i16().unwrap(), -1);
        assert!(r.u8().is_err());
    }

    #[test]
    fn fixed_string_stops_at_nul() {
        let mut r = Reader::new(b"abc\0\0\0xx");
        assert_eq!(r.fixed_str(6).unwrap(), "abc");
        assert_eq!(r.pos(), 6);
    }

    #[test]
    fn huge_length_does_not_overflow() {
        let mut r = Reader::new(&[0; 4]);
        r.skip(2);
        assert!(r.bytes(usize::MAX).is_err());
    }

    #[test]
    fn big_endian() {
        let mut r = Reader::new(&[0, 0, 1, 0]);
        assert_eq!(r.u32_be().unwrap(), 256);
    }
}
