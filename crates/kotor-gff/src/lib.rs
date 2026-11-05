//! GFF V3.2: the generic container behind areas, creatures, items, dialogue,
//! module info and save data. Reader and writer.

mod read;
mod write;

pub use read::parse;
pub use write::write;

use kotor_core::{Error, Result};

/// A localised string: an optional string-table reference plus inline text per language.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct LocString {
    /// Index into `dialog.tlk`, or `-1` (stored as `0xFFFFFFFF`) for none.
    pub strref: i32,
    /// `(language_id * 2 + gender, text)` pairs.
    pub strings: Vec<(u32, String)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Byte(u8),
    Char(i8),
    Word(u16),
    Short(i16),
    Dword(u32),
    Int(i32),
    Dword64(u64),
    Int64(i64),
    Float(f32),
    Double(f64),
    String(String),
    ResRef(String),
    LocString(LocString),
    Void(Vec<u8>),
    Struct(Struct),
    List(Vec<Struct>),
    /// Quaternion, stored w, x, y, z as in the file.
    Orientation([f32; 4]),
    Vector([f32; 3]),
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Struct {
    pub id: u32,
    pub fields: Vec<(String, Value)>,
}

impl Struct {
    pub fn get(&self, label: &str) -> Option<&Value> {
        self.fields.iter().find(|(l, _)| l == label).map(|(_, v)| v)
    }

    pub fn set(&mut self, label: &str, v: Value) {
        match self.fields.iter_mut().find(|(l, _)| l == label) {
            Some(f) => f.1 = v,
            None => self.fields.push((label.to_string(), v)),
        }
    }

    /// Any integer-typed field widened to i64.
    pub fn int(&self, label: &str) -> Option<i64> {
        Some(match self.get(label)? {
            Value::Byte(v) => *v as i64,
            Value::Char(v) => *v as i64,
            Value::Word(v) => *v as i64,
            Value::Short(v) => *v as i64,
            Value::Dword(v) => *v as i64,
            Value::Int(v) => *v as i64,
            Value::Dword64(v) => *v as i64,
            Value::Int64(v) => *v,
            _ => return None,
        })
    }

    pub fn float(&self, label: &str) -> Option<f64> {
        match self.get(label)? {
            Value::Float(v) => Some(*v as f64),
            Value::Double(v) => Some(*v),
            _ => None,
        }
    }

    /// A string or resref field.
    pub fn str(&self, label: &str) -> Option<&str> {
        match self.get(label)? {
            Value::String(s) | Value::ResRef(s) => Some(s),
            _ => None,
        }
    }

    pub fn loc(&self, label: &str) -> Option<&LocString> {
        match self.get(label)? {
            Value::LocString(l) => Some(l),
            _ => None,
        }
    }

    pub fn list(&self, label: &str) -> Option<&[Struct]> {
        match self.get(label)? {
            Value::List(l) => Some(l),
            _ => None,
        }
    }

    pub fn strukt(&self, label: &str) -> Option<&Struct> {
        match self.get(label)? {
            Value::Struct(s) => Some(s),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Gff {
    /// Four-character file type such as `"UTC "`, `"DLG "`, `"ARE "`.
    pub file_type: String,
    pub root: Struct,
}

impl Gff {
    pub fn parse(data: &[u8]) -> Result<Gff> {
        parse(data)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        write(self)
    }
}

pub(crate) fn too_big(what: &str) -> Error {
    Error::format(format!("gff {what} too large"))
}
