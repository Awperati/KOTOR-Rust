use crate::{too_big, Gff, Struct, Value};
use kotor_core::{Error, Result};

#[derive(Default)]
struct Builder {
    structs: Vec<[u32; 3]>,
    fields: Vec<[u32; 3]>,
    labels: Vec<String>,
    data: Vec<u8>,
    indices: Vec<u8>,
    lists: Vec<u8>,
}

fn u32_of(n: usize, what: &str) -> Result<u32> {
    u32::try_from(n).map_err(|_| too_big(what))
}

pub fn write(g: &Gff) -> Result<Vec<u8>> {
    let mut b = Builder::default();
    b.add_struct(&g.root)?;

    let mut tag = [b' '; 4];
    for (i, c) in g.file_type.bytes().take(4).enumerate() {
        tag[i] = c;
    }

    let struct_off = 56usize;
    let field_off = struct_off + b.structs.len() * 12;
    let label_off = field_off + b.fields.len() * 12;
    let data_off = label_off + b.labels.len() * 16;
    let indices_off = data_off + b.data.len();
    let lists_off = indices_off + b.indices.len();

    let mut out = Vec::with_capacity(lists_off + b.lists.len());
    out.extend_from_slice(&tag);
    out.extend_from_slice(b"V3.2");
    for v in [
        struct_off,
        b.structs.len(),
        field_off,
        b.fields.len(),
        label_off,
        b.labels.len(),
        data_off,
        b.data.len(),
        indices_off,
        b.indices.len(),
        lists_off,
        b.lists.len(),
    ] {
        out.extend_from_slice(&u32_of(v, "section")?.to_le_bytes());
    }
    for s in &b.structs {
        for v in s {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    for f in &b.fields {
        for v in f {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    for l in &b.labels {
        let mut buf = [0u8; 16];
        buf[..l.len()].copy_from_slice(l.as_bytes());
        out.extend_from_slice(&buf);
    }
    out.extend_from_slice(&b.data);
    out.extend_from_slice(&b.indices);
    out.extend_from_slice(&b.lists);
    Ok(out)
}

fn latin1_bytes(s: &str) -> Vec<u8> {
    s.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect()
}

impl Builder {
    fn label(&mut self, l: &str) -> Result<u32> {
        if l.len() > 16 || !l.is_ascii() {
            return Err(Error::format(format!("gff label '{l}' invalid")));
        }
        if let Some(i) = self.labels.iter().position(|x| x == l) {
            return Ok(i as u32);
        }
        self.labels.push(l.to_string());
        u32_of(self.labels.len() - 1, "labels")
    }

    /// Add a struct and everything below it; returns its index.
    fn add_struct(&mut self, s: &Struct) -> Result<u32> {
        let index = self.structs.len();
        self.structs.push([s.id, 0, 0]);
        // Fields first so children are laid out after this struct's slot.
        let mut field_ids = Vec::with_capacity(s.fields.len());
        for (label, v) in &s.fields {
            field_ids.push(self.add_field(label, v)?);
        }
        let data = match field_ids.len() {
            0 => 0,
            1 => field_ids[0],
            _ => {
                let off = u32_of(self.indices.len(), "indices")?;
                for f in &field_ids {
                    self.indices.extend_from_slice(&f.to_le_bytes());
                }
                off
            }
        };
        self.structs[index][1] = data;
        self.structs[index][2] = u32_of(field_ids.len(), "fields")?;
        u32_of(index, "structs")
    }

    fn add_field(&mut self, label: &str, v: &Value) -> Result<u32> {
        let label = self.label(label)?;
        let inline = |x: u32| x;
        let (ty, raw): (u32, u32) = match v {
            Value::Byte(x) => (0, inline(*x as u32)),
            Value::Char(x) => (1, *x as u8 as u32),
            Value::Word(x) => (2, *x as u32),
            Value::Short(x) => (3, *x as u16 as u32),
            Value::Dword(x) => (4, *x),
            Value::Int(x) => (5, *x as u32),
            Value::Float(x) => (8, x.to_bits()),
            Value::Dword64(x) => (6, self.push_data(&x.to_le_bytes())?),
            Value::Int64(x) => (7, self.push_data(&x.to_le_bytes())?),
            Value::Double(x) => (9, self.push_data(&x.to_le_bytes())?),
            Value::String(s) => {
                let bytes = latin1_bytes(s);
                let mut buf = u32_of(bytes.len(), "string")?.to_le_bytes().to_vec();
                buf.extend_from_slice(&bytes);
                (10, self.push_data(&buf)?)
            }
            Value::ResRef(s) => {
                let bytes = latin1_bytes(s);
                if bytes.len() > 255 {
                    return Err(too_big("resref"));
                }
                let mut buf = vec![bytes.len() as u8];
                buf.extend_from_slice(&bytes);
                (11, self.push_data(&buf)?)
            }
            Value::LocString(l) => {
                let mut body = Vec::new();
                body.extend_from_slice(&l.strref.to_le_bytes());
                body.extend_from_slice(&u32_of(l.strings.len(), "locstring")?.to_le_bytes());
                for (id, text) in &l.strings {
                    let bytes = latin1_bytes(text);
                    body.extend_from_slice(&id.to_le_bytes());
                    body.extend_from_slice(&u32_of(bytes.len(), "string")?.to_le_bytes());
                    body.extend_from_slice(&bytes);
                }
                let mut buf = u32_of(body.len(), "locstring")?.to_le_bytes().to_vec();
                buf.extend_from_slice(&body);
                (12, self.push_data(&buf)?)
            }
            Value::Void(bytes) => {
                let mut buf = u32_of(bytes.len(), "void")?.to_le_bytes().to_vec();
                buf.extend_from_slice(bytes);
                (13, self.push_data(&buf)?)
            }
            Value::Orientation(q) => {
                let buf: Vec<u8> = q.iter().flat_map(|f| f.to_le_bytes()).collect();
                (16, self.push_data(&buf)?)
            }
            Value::Vector(p) => {
                let buf: Vec<u8> = p.iter().flat_map(|f| f.to_le_bytes()).collect();
                (17, self.push_data(&buf)?)
            }
            Value::Struct(s) => (14, self.add_struct(s)?),
            Value::List(items) => {
                let mut ids = Vec::with_capacity(items.len());
                for it in items {
                    ids.push(self.add_struct(it)?);
                }
                let off = u32_of(self.lists.len(), "lists")?;
                self.lists.extend_from_slice(&u32_of(ids.len(), "list")?.to_le_bytes());
                for i in ids {
                    self.lists.extend_from_slice(&i.to_le_bytes());
                }
                (15, off)
            }
        };
        self.fields.push([ty, label, raw]);
        u32_of(self.fields.len() - 1, "fields")
    }

    fn push_data(&mut self, bytes: &[u8]) -> Result<u32> {
        let off = u32_of(self.data.len(), "data")?;
        self.data.extend_from_slice(bytes);
        Ok(off)
    }
}
