use crate::{dxt, Image};
use kotor_core::{latin1, Error, Reader, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TpcFormat {
    Gray8,
    Rgb8,
    Rgba8,
    Dxt1,
    Dxt5,
}

/// A parsed TPC texture. Mip level 0 is decoded to RGBA; `txi` holds trailing
/// texture-info text if present.
#[derive(Clone, Debug)]
pub struct Tpc {
    pub width: u32,
    pub height: u32,
    pub format: TpcFormat,
    pub mip_count: u32,
    /// Six stacked square faces (`height == 6 * width` in the header).
    pub cube_map: bool,
    pub image: Image,
    pub txi: String,
}

fn level_size(f: TpcFormat, w: usize, h: usize) -> usize {
    match f {
        TpcFormat::Gray8 => w * h,
        TpcFormat::Rgb8 => w * h * 3,
        TpcFormat::Rgba8 => w * h * 4,
        TpcFormat::Dxt1 => w.div_ceil(4).max(1) * h.div_ceil(4).max(1) * 8,
        TpcFormat::Dxt5 => w.div_ceil(4).max(1) * h.div_ceil(4).max(1) * 16,
    }
}

pub fn parse_tpc(data: &[u8]) -> Result<Tpc> {
    let mut r = Reader::new(data);
    let data_size = r.u32()? as usize;
    let _alpha_test = r.f32()?;
    let width = r.u16()? as usize;
    let mut height = r.u16()? as usize;
    let encoding = r.u8()?;
    let mips = r.u8()? as u32;
    r.skip(114);
    if width == 0 || height == 0 {
        return Err(Error::format("empty TPC"));
    }
    let compressed = data_size != 0;
    let format = match (compressed, encoding) {
        (false, 1) => TpcFormat::Gray8,
        (false, 2) => TpcFormat::Rgb8,
        (false, 4) => TpcFormat::Rgba8,
        (true, 2) => TpcFormat::Dxt1,
        (true, 4) => TpcFormat::Dxt5,
        _ => {
            return Err(Error::format(format!(
                "unsupported TPC encoding {encoding} (compressed={compressed})"
            )))
        }
    };
    let cube_map = height == width * 6;
    if cube_map {
        height = width; // decode the first face only
    }
    let faces = if cube_map { 6 } else { 1 };
    let first = level_size(format, width, height);
    let body = r.bytes(first)?;
    let rgba = match format {
        TpcFormat::Dxt1 => dxt::decode(body, width, height, false),
        TpcFormat::Dxt5 => dxt::decode(body, width, height, true),
        TpcFormat::Gray8 => body.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        TpcFormat::Rgb8 => body
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        TpcFormat::Rgba8 => body.to_vec(),
    };

    // Walk the rest of the chain so the trailing TXI text can be located.
    let (mut w, mut h) = (width, height);
    for level in 1..mips.max(1) {
        w = (w / 2).max(1);
        h = (h / 2).max(1);
        let _ = level;
        r.skip(level_size(format, w, h) * faces);
    }
    if faces > 1 {
        r.skip(first * (faces - 1));
    }
    let tail = if r.pos() <= data.len() {
        &data[r.pos()..]
    } else {
        &[][..]
    };
    let txi = latin1(tail.split(|&c| c == 0).next().unwrap_or(&[]));
    Ok(Tpc {
        width: width as u32,
        height: height as u32,
        format,
        mip_count: mips,
        cube_map,
        image: Image {
            width: width as u32,
            height: height as u32,
            rgba,
        },
        txi,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(size: u32, w: u16, h: u16, enc: u8, mips: u8) -> Vec<u8> {
        let mut v = size.to_le_bytes().to_vec();
        v.extend_from_slice(&1f32.to_le_bytes());
        v.extend_from_slice(&w.to_le_bytes());
        v.extend_from_slice(&h.to_le_bytes());
        v.push(enc);
        v.push(mips);
        v.resize(128, 0);
        v
    }

    #[test]
    fn uncompressed_rgb() {
        let mut d = header(0, 1, 1, 2, 1);
        d.extend_from_slice(&[10, 20, 30]);
        let t = parse_tpc(&d).unwrap();
        assert_eq!(t.format, TpcFormat::Rgb8);
        assert_eq!(t.image.rgba, [10, 20, 30, 255]);
    }

    #[test]
    fn dxt1_with_txi() {
        let mut d = header(8, 4, 4, 2, 1);
        d.extend_from_slice(&[0x00, 0xF8, 0, 0, 0, 0, 0, 0]);
        d.extend_from_slice(b"envmaptexture x\0");
        let t = parse_tpc(&d).unwrap();
        assert_eq!(t.format, TpcFormat::Dxt1);
        assert_eq!(&t.image.rgba[..4], &[255, 0, 0, 255]);
        assert_eq!(t.txi, "envmaptexture x");
    }

    #[test]
    fn truncated_is_error() {
        let d = header(8, 4, 4, 2, 1);
        assert!(parse_tpc(&d).is_err());
    }
}
