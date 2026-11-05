use crate::{Gff, LocString, Struct, Value};
use kotor_core::{latin1, Error, Reader, Result};

struct Header {
    struct_off: usize,
    struct_count: usize,
    field_off: usize,
    field_count: usize,
    label_off: usize,
    label_count: usize,
    data_off: usize,
    data_len: usize,
    indices_off: usize,
    indices_len: usize,
    lists_off: usize,
    lists_len: usize,
}

struct Ctx<'a> {
    d: &'a [u8],
    h: Header,
}

pub fn parse(data: &[u8]) -> Result<Gff> {
    let mut r = Reader::new(data);
    let tag = r.tag()?;
    let ver = r.tag()?;
    if &ver != b"V3.2" {
        return Err(Error::format(format!("unsupported GFF version {ver:?}")));
    }
    let mut n = || r.u32().map(|v| v as usize);
    let h = Header {
        struct_off: n()?,
        struct_count: n()?,
        field_off: n()?,
        field_count: n()?,
        label_off: n()?,
        label_count: n()?,
        data_off: n()?,
        data_len: n()?,
        indices_off: n()?,
        indices_len: n()?,
        lists_off: n()?,
        lists_len: n()?,
    };
    let ctx = Ctx { d: data, h };
    if ctx.h.struct_count == 0 {
        return Err(Error::format("gff has no root struct"));
    }
    let root = ctx.read_struct(0, 0)?;
    Ok(Gff {
        file_type: latin1(&tag),
        root,
    })
}

/// Nesting limit; real files stay well under this, and it stops cyclic data.
const MAX_DEPTH: usize = 64;

impl Ctx<'_> {
    fn at(&self, off: usize) -> Reader<'_> {
        Reader::at(self.d, off)
    }

    fn read_struct(&self, index: usize, depth: usize) -> Result<Struct> {
        if depth > MAX_DEPTH {
            return Err(Error::format("gff nesting too deep"));
        }
        if index >= self.h.struct_count {
            return Err(Error::format(format!("struct index {index} out of range")));
        }
        let mut r = self.at(self.h.struct_off + index * 12);
        let id = r.u32()?;
        let data = r.u32()? as usize;
        let count = r.u32()? as usize;
        let mut fields = Vec::with_capacity(count.min(1024));
        if count == 1 {
            fields.push(self.read_field(data, depth)?);
        } else if count > 1 {
            let mut ir = self.at(self.h.indices_off + data);
            if data + count * 4 > self.h.indices_len {
                return Err(Error::format("field index list past end"));
            }
            for _ in 0..count {
                let fi = ir.u32()? as usize;
                fields.push(self.read_field(fi, depth)?);
            }
        }
        Ok(Struct { id, fields })
    }

    fn label(&self, index: usize) -> Result<String> {
        if index >= self.h.label_count {
            return Err(Error::format(format!("label index {index} out of range")));
        }
        self.at(self.h.label_off + index * 16).fixed_str(16)
    }

    fn read_field(&self, index: usize, depth: usize) -> Result<(String, Value)> {
        if index >= self.h.field_count {
            return Err(Error::format(format!("field index {index} out of range")));
        }
        let mut r = self.at(self.h.field_off + index * 12);
        let ty = r.u32()?;
        let label = self.label(r.u32()? as usize)?;
        let raw = r.u32()?;
        let inline = raw.to_le_bytes();
        let v = match ty {
            0 => Value::Byte(inline[0]),
            1 => Value::Char(inline[0] as i8),
            2 => Value::Word(u16::from_le_bytes([inline[0], inline[1]])),
            3 => Value::Short(i16::from_le_bytes([inline[0], inline[1]])),
            4 => Value::Dword(raw),
            5 => Value::Int(raw as i32),
            8 => Value::Float(f32::from_le_bytes(inline)),
            14 => Value::Struct(self.read_struct(raw as usize, depth + 1)?),
            15 => {
                let off = self.h.lists_off + raw as usize;
                if raw as usize + 4 > self.h.lists_len {
                    return Err(Error::format("list offset past end"));
                }
                let mut lr = self.at(off);
                let n = lr.u32()? as usize;
                if raw as usize + 4 + n * 4 > self.h.lists_len {
                    return Err(Error::format("list past end"));
                }
                let mut items = Vec::with_capacity(n);
                for _ in 0..n {
                    items.push(self.read_struct(lr.u32()? as usize, depth + 1)?);
                }
                Value::List(items)
            }
            6 | 7 | 9 | 10 | 11 | 12 | 13 | 16 | 17 => self.read_data(ty, raw as usize)?,
            other => return Err(Error::format(format!("unknown GFF field type {other}"))),
        };
        Ok((label, v))
    }

    /// Complex values live in the field-data block at `off`.
    fn read_data(&self, ty: u32, off: usize) -> Result<Value> {
        if off > self.h.data_len {
            return Err(Error::format("field data offset past end"));
        }
        let mut r = self.at(self.h.data_off + off);
        let limit = self.h.data_off + self.h.data_len;
        let check = |r: &Reader, n: usize| -> Result<()> {
            if r.pos().saturating_add(n) > limit {
                Err(Error::format("field data runs past block"))
            } else {
                Ok(())
            }
        };
        Ok(match ty {
            6 => Value::Dword64(r.u64()?),
            7 => Value::Int64(r.i64()?),
            9 => Value::Double(r.f64()?),
            10 => {
                let n = r.u32()? as usize;
                check(&r, n)?;
                Value::String(latin1(r.bytes(n)?))
            }
            11 => {
                let n = r.u8()? as usize;
                Value::ResRef(latin1(r.bytes(n)?))
            }
            12 => {
                let total = r.u32()? as usize;
                check(&r, total)?;
                let strref = r.i32()?;
                let count = r.u32()? as usize;
                let mut strings = Vec::new();
                for _ in 0..count {
                    let id = r.u32()?;
                    let n = r.u32()? as usize;
                    check(&r, n)?;
                    strings.push((id, latin1(r.bytes(n)?)));
                }
                Value::LocString(LocString { strref, strings })
            }
            13 => {
                let n = r.u32()? as usize;
                check(&r, n)?;
                Value::Void(r.bytes(n)?.to_vec())
            }
            16 => Value::Orientation([r.f32()?, r.f32()?, r.f32()?, r.f32()?]),
            17 => Value::Vector([r.f32()?, r.f32()?, r.f32()?]),
            _ => unreachable!("caller restricts types"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garbage_is_rejected_not_panicking() {
        assert!(parse(&[]).is_err());
        assert!(parse(b"GFF V3.2").is_err());
        let mut bad = b"GFF V3.2".to_vec();
        bad.extend_from_slice(&[0xff; 48]);
        assert!(parse(&bad).is_err());
    }
}
