use crate::Image;

fn crc32(chunks: &[&[u8]]) -> u32 {
    let mut table = [0u32; 256];
    for (i, t) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        *t = c;
    }
    let mut crc = 0xFFFF_FFFFu32;
    for chunk in chunks {
        for &b in *chunk {
            crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
        }
    }
    !crc
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    out.extend_from_slice(&crc32(&[kind, body]).to_be_bytes());
}

/// Encode as an uncompressed (stored-deflate) RGBA PNG. For inspection only.
pub fn to_png(img: &Image) -> Vec<u8> {
    let stride = img.width as usize * 4;
    let mut raw = Vec::with_capacity((stride + 1) * img.height as usize);
    for row in img.rgba.chunks(stride) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut z = vec![0x78, 0x01];
    let mut blocks = raw.chunks(65535).peekable();
    if blocks.peek().is_none() {
        z.extend_from_slice(&[1, 0, 0, 0xFF, 0xFF]);
    }
    while let Some(b) = blocks.next() {
        z.push(u8::from(blocks.peek().is_none()));
        z.extend_from_slice(&(b.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(b.len() as u16)).to_le_bytes());
        z.extend_from_slice(b);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());

    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&img.width.to_be_bytes());
    ihdr.extend_from_slice(&img.height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

/// Reads the bits of a deflate stream, low bit first.
struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    held: u32,
    count: u32,
}

impl Bits<'_> {
    fn take(&mut self, n: u32) -> Option<u32> {
        while self.count < n {
            self.held |= (*self.data.get(self.pos)? as u32) << self.count;
            self.pos += 1;
            self.count += 8;
        }
        let v = self.held & ((1u32 << n) - 1);
        self.held >>= n;
        self.count -= n;
        Some(v)
    }

    fn align(&mut self) {
        self.held = 0;
        self.count = 0;
    }
}

/// A canonical Huffman code: how many codes of each length, and the symbols in code order.
struct Code {
    counts: [u16; 16],
    symbols: Vec<u16>,
}

impl Code {
    fn new(lengths: &[u8]) -> Code {
        let mut counts = [0u16; 16];
        for &l in lengths {
            counts[l as usize] += 1;
        }
        counts[0] = 0;
        let mut offsets = [0u16; 16];
        for i in 1..15 {
            offsets[i + 1] = offsets[i] + counts[i];
        }
        let mut symbols = vec![0u16; lengths.len()];
        for (s, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbols[offsets[l as usize] as usize] = s as u16;
                offsets[l as usize] += 1;
            }
        }
        Code { counts, symbols }
    }

    fn read(&self, b: &mut Bits) -> Option<u16> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..16 {
            code |= b.take(1)? as i32;
            let count = self.counts[len] as i32;
            if code - count < first {
                return self.symbols.get((index + code - first) as usize).copied();
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        None
    }
}

const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DISTANCE_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DISTANCE_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

fn inflate_block(b: &mut Bits, out: &mut Vec<u8>, lit: &Code, dist: &Code) -> Option<()> {
    loop {
        let s = lit.read(b)? as usize;
        match s {
            0..=255 => out.push(s as u8),
            256 => return Some(()),
            _ => {
                let i = s - 257;
                let len = *LENGTH_BASE.get(i)? as usize + b.take(LENGTH_EXTRA[i] as u32)? as usize;
                let d = dist.read(b)? as usize;
                let back =
                    *DISTANCE_BASE.get(d)? as usize + b.take(DISTANCE_EXTRA[d] as u32)? as usize;
                let from = out.len().checked_sub(back)?;
                for k in 0..len {
                    out.push(out[from + k]);
                }
            }
        }
    }
}

/// Undo deflate (RFC 1951).
fn inflate(data: &[u8], size_hint: usize) -> Option<Vec<u8>> {
    let mut b = Bits {
        data,
        pos: 0,
        held: 0,
        count: 0,
    };
    let mut out = Vec::with_capacity(size_hint);
    loop {
        let last = b.take(1)?;
        match b.take(2)? {
            0 => {
                b.align();
                let len = *data.get(b.pos)? as usize | (*data.get(b.pos + 1)? as usize) << 8;
                b.pos += 4;
                out.extend_from_slice(data.get(b.pos..b.pos + len)?);
                b.pos += len;
            }
            1 => {
                let mut l = [8u8; 288];
                l[144..256].fill(9);
                l[256..280].fill(7);
                inflate_block(&mut b, &mut out, &Code::new(&l), &Code::new(&[5u8; 30]))?;
            }
            2 => {
                let (nlen, ndist, ncode) = (
                    b.take(5)? as usize + 257,
                    b.take(5)? as usize + 1,
                    b.take(4)? as usize + 4,
                );
                const ORDER: [usize; 19] = [
                    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
                ];
                let mut cl = [0u8; 19];
                for &o in ORDER.iter().take(ncode) {
                    cl[o] = b.take(3)? as u8;
                }
                let code = Code::new(&cl);
                let mut lengths = vec![0u8; nlen + ndist];
                let mut i = 0;
                while i < nlen + ndist {
                    let s = code.read(&mut b)?;
                    let (value, times) = match s {
                        0..=15 => (s as u8, 1),
                        16 => (*lengths.get(i.checked_sub(1)?)?, 3 + b.take(2)? as usize),
                        17 => (0, 3 + b.take(3)? as usize),
                        18 => (0, 11 + b.take(7)? as usize),
                        _ => return None,
                    };
                    for _ in 0..times {
                        *lengths.get_mut(i)? = value;
                        i += 1;
                    }
                }
                inflate_block(
                    &mut b,
                    &mut out,
                    &Code::new(&lengths[..nlen]),
                    &Code::new(&lengths[nlen..]),
                )?;
            }
            _ => return None,
        }
        if last == 1 {
            return Some(out);
        }
    }
}

/// Decode an 8- or 16-bit greyscale, RGB or RGBA PNG (not interlaced, not palette). 16-bit samples keep their high byte.
pub fn parse_png(data: &[u8]) -> Option<Image> {
    if data.get(..8)? != [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
        return None;
    }
    let (mut pos, mut idat, mut header) = (8, Vec::new(), None);
    while let Some(len) = data.get(pos..pos + 4) {
        let len = u32::from_be_bytes(len.try_into().ok()?) as usize;
        let kind = data.get(pos + 4..pos + 8)?;
        let body = data.get(pos + 8..pos + 8 + len)?;
        match kind {
            b"IHDR" => header = Some(body.to_vec()),
            b"IDAT" => idat.extend_from_slice(body),
            b"IEND" => break,
            _ => {}
        }
        pos += 12 + len;
    }
    let h = header?;
    let width = u32::from_be_bytes(h.get(0..4)?.try_into().ok()?);
    let height = u32::from_be_bytes(h.get(4..8)?.try_into().ok()?);
    let (depth, kind, interlace) = (*h.get(8)?, *h.get(9)?, *h.get(12)?);
    let channels = match kind {
        0 => 1,
        2 => 3,
        4 => 2,
        6 => 4,
        _ => return None,
    };
    if interlace != 0 || !(depth == 8 || depth == 16) {
        return None;
    }
    let sample = depth as usize / 8;
    let bpp = channels * sample;
    let stride = width as usize * bpp;
    let raw = inflate(idat.get(2..)?, (stride + 1) * height as usize)?;
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    let mut above = vec![0u8; stride];
    let mut row = vec![0u8; stride];
    for line in raw.chunks(stride + 1).take(height as usize) {
        let (filter, src) = (*line.first()?, line.get(1..)?);
        for i in 0..stride {
            let a = if i >= bpp { row[i - bpp] } else { 0 } as i32;
            let b = above[i] as i32;
            let c = if i >= bpp { above[i - bpp] } else { 0 } as i32;
            let predicted = match filter {
                0 => 0,
                1 => a,
                2 => b,
                3 => (a + b) / 2,
                4 => {
                    let p = a + b - c;
                    let (pa, pb, pc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());
                    if pa <= pb && pa <= pc {
                        a
                    } else if pb <= pc {
                        b
                    } else {
                        c
                    }
                }
                _ => return None,
            };
            row[i] = src.get(i)?.wrapping_add(predicted as u8);
        }
        for p in row.chunks(bpp) {
            let at = |k: usize| p[k * sample];
            rgba.extend_from_slice(&match channels {
                1 => [at(0), at(0), at(0), 255],
                2 => [at(0), at(0), at(0), at(1)],
                3 => [at(0), at(1), at(2), 255],
                _ => [at(0), at(1), at(2), at(3)],
            });
        }
        std::mem::swap(&mut above, &mut row);
    }
    (rgba.len() == width as usize * height as usize * 4).then_some(Image {
        width,
        height,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_to_png_writes_parse_png_reads() {
        let img = Image {
            width: 3,
            height: 2,
            rgba: (0..24).collect(),
        };
        assert_eq!(parse_png(&to_png(&img)), Some(img));
    }

    #[test]
    fn huffman_and_filters_are_undone() {
        // A 2x2 grey picture, deflated by zlib with a fixed code: filter 0 row, then filter 2 (up) row.
        // Raw rows: 00 0A 14, 02 01 01. Fixed-code deflate made by hand from those six bytes.
        let lit = |v: u32| -> (u32, u32) {
            // Literals 0-143 are 8-bit codes 0x30 + v, sent high bit first.
            (0x30 + v, 8)
        };
        let mut bits: Vec<u8> = vec![1, 1, 0];
        for v in [0u32, 10, 20, 2, 1, 1] {
            let (code, n) = lit(v);
            for k in (0..n).rev() {
                bits.push(((code >> k) & 1) as u8);
            }
        }
        bits.extend([0u8; 7]); // End of block: the 7-bit code 0.
        let mut z = vec![0x78, 0x01];
        for byte in bits.chunks(8) {
            let mut v = 0u8;
            for (i, b) in byte.iter().enumerate() {
                v |= b << i;
            }
            z.push(v);
        }
        z.extend_from_slice(&[0, 0, 0, 0]); // Checksum, which is not checked.
        let mut p = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&2u32.to_be_bytes());
        ihdr.extend_from_slice(&2u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 0, 0, 0, 0]);
        chunk(&mut p, b"IHDR", &ihdr);
        chunk(&mut p, b"IDAT", &z);
        chunk(&mut p, b"IEND", &[]);
        let got = parse_png(&p).unwrap();
        let greys: Vec<u8> = got.rgba.chunks(4).map(|c| c[0]).collect();
        assert_eq!(greys, vec![10, 20, 11, 21]);
    }

    #[test]
    fn crc_of_known_string() {
        assert_eq!(crc32(&[b"123456789"]), 0xCBF4_3926);
    }

    #[test]
    fn png_has_signature_and_end() {
        let img = Image {
            width: 2,
            height: 2,
            rgba: vec![255; 16],
        };
        let p = to_png(&img);
        assert_eq!(&p[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        assert_eq!(&p[p.len() - 8..p.len() - 4], b"IEND");
    }
}
