use crate::Image;
use kotor_core::{Error, Reader, Result};

/// Decode a TGA file (types 2/3 raw and 10/11 run-length, 8/24/32 bits).
pub fn parse_tga(data: &[u8]) -> Result<Image> {
    let mut r = Reader::new(data);
    let id_len = r.u8()? as usize;
    let cmap_type = r.u8()?;
    let ty = r.u8()?;
    r.skip(5); // colour map spec
    r.skip(4); // x/y origin
    let width = r.u16()? as usize;
    let height = r.u16()? as usize;
    let bpp = r.u8()?;
    let desc = r.u8()?;
    if cmap_type != 0 {
        return Err(Error::format("colour-mapped TGA not supported"));
    }
    let (rle, gray) = match ty {
        2 => (false, false),
        3 => (false, true),
        10 => (true, false),
        11 => (true, true),
        _ => return Err(Error::format(format!("unsupported TGA type {ty}"))),
    };
    let bytes_pp = match (bpp, gray) {
        (8, true) => 1,
        (24, false) => 3,
        (32, false) => 4,
        _ => return Err(Error::format(format!("unsupported TGA depth {bpp}"))),
    };
    if width == 0 || height == 0 {
        return Err(Error::format("empty TGA"));
    }
    r.skip(id_len);
    let pixels = width * height;
    let mut raw = Vec::with_capacity(pixels * bytes_pp);
    if rle {
        while raw.len() < pixels * bytes_pp {
            let head = r.u8()?;
            let n = (head & 0x7F) as usize + 1;
            if head & 0x80 != 0 {
                let px = r.bytes(bytes_pp)?;
                for _ in 0..n {
                    raw.extend_from_slice(px);
                }
            } else {
                raw.extend_from_slice(r.bytes(n * bytes_pp)?);
            }
        }
        raw.truncate(pixels * bytes_pp);
    } else {
        raw.extend_from_slice(r.bytes(pixels * bytes_pp)?);
    }
    let bottom_up = desc & 0x20 == 0;
    let mut rgba = vec![0u8; pixels * 4];
    for y in 0..height {
        let src_y = if bottom_up { height - 1 - y } else { y };
        for x in 0..width {
            let s = (src_y * width + x) * bytes_pp;
            let d = (y * width + x) * 4;
            match bytes_pp {
                1 => rgba[d..d + 4].copy_from_slice(&[raw[s], raw[s], raw[s], 255]),
                3 => rgba[d..d + 4].copy_from_slice(&[raw[s + 2], raw[s + 1], raw[s], 255]),
                _ => rgba[d..d + 4].copy_from_slice(&[raw[s + 2], raw[s + 1], raw[s], raw[s + 3]]),
            }
        }
    }
    Ok(Image {
        width: width as u32,
        height: height as u32,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(ty: u8, w: u16, h: u16, bpp: u8, desc: u8) -> Vec<u8> {
        let mut v = vec![0, 0, ty, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        v[12 - 4..].iter_mut().for_each(|b| *b = 0);
        v.extend_from_slice(&w.to_le_bytes());
        v.extend_from_slice(&h.to_le_bytes());
        v.push(bpp);
        v.push(desc);
        v
    }

    #[test]
    fn raw_bottom_up_bgr() {
        let mut d = header(2, 1, 2, 24, 0);
        d.extend_from_slice(&[0, 0, 255]); // bottom row: red (BGR)
        d.extend_from_slice(&[255, 0, 0]); // top row: blue
        let img = parse_tga(&d).unwrap();
        assert_eq!(&img.rgba[..4], &[0, 0, 255, 255]);
        assert_eq!(&img.rgba[4..], &[255, 0, 0, 255]);
    }

    #[test]
    fn rle_run() {
        let mut d = header(10, 3, 1, 32, 0x20);
        d.push(0x82); // run of 3
        d.extend_from_slice(&[1, 2, 3, 4]);
        let img = parse_tga(&d).unwrap();
        assert_eq!(&img.rgba[8..12], &[3, 2, 1, 4]);
    }

    #[test]
    fn truncated_is_error() {
        let d = header(2, 4, 4, 32, 0);
        assert!(parse_tga(&d).is_err());
    }
}
