use kotor_core::{latin1, Error, Reader, Result};

/// A 2DA table. Cells are strings; `"****"` marks an empty cell.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TwoDa {
    pub columns: Vec<String>,
    /// Row labels as stored (normally "0", "1", ...).
    pub labels: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl TwoDa {
    /// Parse binary `V2.b` or text `V2.0` data.
    pub fn parse(data: &[u8]) -> Result<TwoDa> {
        if data.starts_with(b"2DA V2.b") {
            Self::parse_binary(data)
        } else if data.starts_with(b"2DA V2.0") {
            Self::parse_text(data)
        } else {
            Err(Error::format("not a 2DA V2.b or V2.0 file"))
        }
    }

    fn parse_binary(data: &[u8]) -> Result<TwoDa> {
        // "2DA V2.b" + LF, then tab-terminated column names, then a NUL.
        let mut r = Reader::at(data, 9);
        let mut columns = Vec::new();
        loop {
            let start = r.pos();
            let mut end = start;
            loop {
                let b = r.u8()?;
                if b == b'\t' || b == 0 {
                    if b == 0 && end == start {
                        // NUL right at the start of a name: end of header.
                        end = usize::MAX;
                    }
                    break;
                }
                end += 1;
            }
            if end == usize::MAX {
                break;
            }
            columns.push(latin1(&data[start..end]));
            if data.get(r.pos() - 1) == Some(&0) {
                break;
            }
        }
        let rows_n = r.u32()? as usize;
        let cols_n = columns.len();
        // Row labels: tab-terminated strings.
        let mut labels = Vec::with_capacity(rows_n.min(1 << 16));
        for _ in 0..rows_n {
            let start = r.pos();
            while r.u8()? != b'\t' {}
            labels.push(latin1(&data[start..r.pos() - 1]));
        }
        let cells = rows_n
            .checked_mul(cols_n)
            .ok_or_else(|| Error::format("2da size overflow"))?;
        let mut offsets = Vec::with_capacity(cells.min(1 << 20));
        for _ in 0..cells {
            offsets.push(r.u16()? as usize);
        }
        let _data_size = r.u16()?;
        let base = r.pos();
        let mut rows = Vec::with_capacity(rows_n);
        for row in 0..rows_n {
            let mut cols = Vec::with_capacity(cols_n);
            for col in 0..cols_n {
                let start = base + offsets[row * cols_n + col];
                let tail = data
                    .get(start..)
                    .ok_or_else(|| Error::format("2da cell offset past end"))?;
                let len = tail
                    .iter()
                    .position(|&b| b == 0)
                    .ok_or_else(|| Error::format("2da cell not terminated"))?;
                cols.push(latin1(&tail[..len]));
            }
            rows.push(cols);
        }
        Ok(TwoDa {
            columns,
            labels,
            rows,
        })
    }

    fn parse_text(data: &[u8]) -> Result<TwoDa> {
        let text = latin1(data);
        let mut lines = text.lines().skip(1).filter(|l| !l.trim().is_empty());
        let header = lines
            .next()
            .ok_or_else(|| Error::format("2da text has no header"))?;
        let columns: Vec<String> = split_cells(header);
        let mut labels = Vec::new();
        let mut rows = Vec::new();
        for line in lines {
            let mut cells = split_cells(line);
            if cells.len() < columns.len() + 1 {
                cells.resize(columns.len() + 1, "****".to_string());
            }
            labels.push(cells.remove(0));
            cells.truncate(columns.len());
            rows.push(cells);
        }
        Ok(TwoDa {
            columns,
            labels,
            rows,
        })
    }

    /// Append a row; missing cells become `****`. The label is the new row number.
    pub fn push_row(&mut self, cells: &[&str]) -> usize {
        let mut row: Vec<String> = cells.iter().map(|c| c.to_string()).collect();
        row.resize(self.columns.len(), "****".to_string());
        row.truncate(self.columns.len());
        self.labels.push(self.rows.len().to_string());
        self.rows.push(row);
        self.rows.len() - 1
    }

    /// Serialise as binary `V2.b`. Identical cell strings are stored once.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut out = b"2DA V2.b\n".to_vec();
        for c in &self.columns {
            out.extend_from_slice(c.as_bytes());
            out.push(b'\t');
        }
        out.push(0);
        out.extend_from_slice(&(self.rows.len() as u32).to_le_bytes());
        for l in &self.labels {
            out.extend_from_slice(l.as_bytes());
            out.push(b'\t');
        }
        let mut data: Vec<u8> = Vec::new();
        let mut seen: std::collections::HashMap<&str, u16> = std::collections::HashMap::new();
        for row in &self.rows {
            if row.len() != self.columns.len() {
                return Err(Error::format("2da row length differs from column count"));
            }
            for cell in row {
                let off = match seen.get(cell.as_str()) {
                    Some(&o) => o,
                    None => {
                        let o = u16::try_from(data.len())
                            .map_err(|_| Error::format("2da cell data over 64 KB"))?;
                        data.extend(cell.chars().map(|c| {
                            if (c as u32) < 256 {
                                c as u8
                            } else {
                                b'?'
                            }
                        }));
                        data.push(0);
                        seen.insert(cell.as_str(), o);
                        o
                    }
                };
                out.extend_from_slice(&off.to_le_bytes());
            }
        }
        let size =
            u16::try_from(data.len()).map_err(|_| Error::format("2da cell data over 64 KB"))?;
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&data);
        Ok(out)
    }

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns
            .iter()
            .position(|c| c.eq_ignore_ascii_case(name))
    }

    /// Cell text; `None` if the row or column is missing or the cell is `****`.
    pub fn get(&self, row: usize, column: &str) -> Option<&str> {
        let c = self.column_index(column)?;
        let v = self.rows.get(row)?.get(c)?;
        (v != "****").then_some(v.as_str())
    }

    pub fn int(&self, row: usize, column: &str) -> Option<i64> {
        let v = self.get(row, column)?;
        match v.strip_prefix("0x").or_else(|| v.strip_prefix("0X")) {
            Some(h) => i64::from_str_radix(h, 16).ok(),
            None => v.parse().ok(),
        }
    }

    pub fn float(&self, row: usize, column: &str) -> Option<f64> {
        self.get(row, column)?.parse().ok()
    }
}

/// Split a text row on whitespace, keeping "quoted cells" whole.
fn split_cells(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = line.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '"' {
            chars.next();
            let mut s = String::new();
            for ch in chars.by_ref() {
                if ch == '"' {
                    break;
                }
                s.push(ch);
            }
            out.push(s);
        } else {
            let mut s = String::new();
            while let Some(&ch) = chars.peek() {
                if ch.is_whitespace() {
                    break;
                }
                s.push(ch);
                chars.next();
            }
            out.push(s);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_table() {
        let t = TwoDa::parse(b"2DA V2.0\n\nname val\n0 foo \"a b\"\n1 **** 0x10\n").unwrap();
        assert_eq!(t.columns, ["name", "val"]);
        assert_eq!(t.get(0, "val"), Some("a b"));
        assert_eq!(t.get(1, "name"), None);
        assert_eq!(t.int(1, "val"), Some(16));
    }

    #[test]
    fn built_table_round_trips() {
        let mut t = TwoDa {
            columns: vec!["label".into(), "cost".into()],
            ..TwoDa::default()
        };
        t.push_row(&["sword", "100"]);
        t.push_row(&["shield"]);
        t.push_row(&["axe", "100"]);
        let back = TwoDa::parse(&t.to_bytes().unwrap()).unwrap();
        assert_eq!(back, t);
        assert_eq!(back.get(1, "cost"), None);
        assert_eq!(back.int(2, "cost"), Some(100));
    }

    #[test]
    fn rejects_garbage() {
        assert!(TwoDa::parse(b"nope").is_err());
        assert!(TwoDa::parse(b"2DA V2.b\n").is_err());
    }

    #[test]
    fn truncated_binary_never_panics() {
        let Some(dir) = kotor_core::install_dir() else {
            return;
        };
        let mut inst = kotor_archive::Install::open(dir).unwrap();
        let data = inst.read("appearance", kotor_core::ResType::TWODA).unwrap();
        for n in (0..data.len()).step_by(97) {
            let _ = TwoDa::parse(&data[..n]);
        }
    }
}
