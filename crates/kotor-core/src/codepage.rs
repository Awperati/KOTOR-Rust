use crate::cp1252;

/// Encode text as Windows-1252; characters with no mapping become `?`.
pub fn cp1252_encode(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| match c as u32 {
            0..=0x7F | 0xA0..=0xFF => c as u8,
            _ => (0x80u8..=0x9F)
                .find(|&b| cp1252(&[b]).starts_with(c))
                .unwrap_or(b'?'),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_all_bytes() {
        let all: Vec<u8> = (0..=255).collect();
        assert_eq!(cp1252_encode(&cp1252(&all)), all);
        assert_eq!(cp1252_encode("a\u{4e2d}"), b"a?");
    }
}
