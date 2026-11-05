//! S3TC block decoders (DXT1 and DXT5) to RGBA8.

fn rgb565(c: u16) -> [u8; 3] {
    let r = ((c >> 11) & 31) as u32;
    let g = ((c >> 5) & 63) as u32;
    let b = (c & 31) as u32;
    [
        ((r * 255 + 15) / 31) as u8,
        ((g * 255 + 31) / 63) as u8,
        ((b * 255 + 15) / 31) as u8,
    ]
}

fn color_block(b: &[u8], allow_alpha: bool) -> [[u8; 4]; 16] {
    let c0 = u16::from_le_bytes([b[0], b[1]]);
    let c1 = u16::from_le_bytes([b[2], b[3]]);
    let (p0, p1) = (rgb565(c0), rgb565(c1));
    let mix = |a: u8, b: u8, wa: u32, wb: u32, d: u32| ((a as u32 * wa + b as u32 * wb) / d) as u8;
    let mut pal = [[0u8; 4]; 4];
    pal[0] = [p0[0], p0[1], p0[2], 255];
    pal[1] = [p1[0], p1[1], p1[2], 255];
    if c0 > c1 || !allow_alpha {
        for i in 0..3 {
            pal[2][i] = mix(p0[i], p1[i], 2, 1, 3);
            pal[3][i] = mix(p0[i], p1[i], 1, 2, 3);
        }
        pal[2][3] = 255;
        pal[3][3] = 255;
    } else {
        for i in 0..3 {
            pal[2][i] = mix(p0[i], p1[i], 1, 1, 2);
        }
        pal[2][3] = 255;
        pal[3] = [0, 0, 0, 0];
    }
    let bits = u32::from_le_bytes([b[4], b[5], b[6], b[7]]);
    let mut out = [[0u8; 4]; 16];
    for (i, px) in out.iter_mut().enumerate() {
        *px = pal[((bits >> (2 * i)) & 3) as usize];
    }
    out
}

fn alpha_block(b: &[u8]) -> [u8; 16] {
    let a0 = b[0] as u32;
    let a1 = b[1] as u32;
    let mut pal = [0u8; 8];
    pal[0] = a0 as u8;
    pal[1] = a1 as u8;
    if a0 > a1 {
        for i in 1..7u32 {
            pal[1 + i as usize] = (((7 - i) * a0 + i * a1) / 7) as u8;
        }
    } else {
        for i in 1..5u32 {
            pal[1 + i as usize] = (((5 - i) * a0 + i * a1) / 5) as u8;
        }
        pal[6] = 0;
        pal[7] = 255;
    }
    let mut bits = 0u64;
    for (i, &byte) in b[2..8].iter().enumerate() {
        bits |= (byte as u64) << (8 * i);
    }
    let mut out = [0u8; 16];
    for (i, a) in out.iter_mut().enumerate() {
        *a = pal[((bits >> (3 * i)) & 7) as usize];
    }
    out
}

/// Decode a `w` x `h` DXT1 (`alpha_blocks == false`) or DXT5 image.
pub fn decode(data: &[u8], w: usize, h: usize, dxt5: bool) -> Vec<u8> {
    let block = if dxt5 { 16 } else { 8 };
    let bw = w.div_ceil(4);
    let bh = h.div_ceil(4);
    let mut out = vec![0u8; w * h * 4];
    for by in 0..bh {
        for bx in 0..bw {
            let off = (by * bw + bx) * block;
            let Some(b) = data.get(off..off + block) else {
                return out;
            };
            let (alpha, color) = if dxt5 {
                (Some(alpha_block(&b[..8])), color_block(&b[8..], false))
            } else {
                (None, color_block(b, true))
            };
            for i in 0..16 {
                let (x, y) = (bx * 4 + i % 4, by * 4 + i / 4);
                if x < w && y < h {
                    let mut px = color[i];
                    if let Some(a) = &alpha {
                        px[3] = a[i];
                    }
                    let d = (y * w + x) * 4;
                    out[d..d + 4].copy_from_slice(&px);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dxt1_solid_red() {
        // c0 = pure red (0xF800), c1 = 0, all indices 0.
        let blk = [0x00, 0xF8, 0, 0, 0, 0, 0, 0];
        let img = decode(&blk, 4, 4, false);
        assert_eq!(&img[..4], &[255, 0, 0, 255]);
        assert_eq!(&img[60..64], &[255, 0, 0, 255]);
    }

    #[test]
    fn dxt1_transparent_index() {
        // c0 <= c1 -> index 3 is transparent.
        let blk = [0, 0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        let img = decode(&blk, 4, 4, false);
        assert_eq!(img[3], 0);
    }

    #[test]
    fn dxt5_alpha_ramp_endpoints() {
        let mut blk = [0u8; 16];
        blk[0] = 255;
        blk[1] = 0;
        let img = decode(&blk, 4, 4, true);
        assert_eq!(img[3], 255);
    }

    #[test]
    fn short_data_does_not_panic() {
        let _ = decode(&[0; 5], 8, 8, true);
    }
}
