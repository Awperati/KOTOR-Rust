//! The engine routine table: id, name and signature of every function the game exposes to scripts.
//!
//! The table is read at runtime from the installed game's own `nwscript.nss`, where each prototype
//! is preceded by a `// <id>: ...` comment. Nothing from that file is copied into this repo.

use kotor_core::{Error, Result};

/// A script value type as it appears in a prototype.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Void,
    Int,
    Float,
    String,
    Object,
    Vector,
    /// `effect`, `event`, `location` or `talent`.
    Engine(u8),
    /// A deferred action argument (`action`).
    Action,
}

impl Ty {
    fn parse(s: &str) -> Option<Ty> {
        // A few prototypes spell types in capitals or use `object_id` for `object`.
        Some(match s.to_ascii_lowercase().as_str() {
            "void" => Ty::Void,
            "int" => Ty::Int,
            "float" => Ty::Float,
            "string" => Ty::String,
            "object" | "object_id" => Ty::Object,
            "vector" => Ty::Vector,
            "effect" => Ty::Engine(0),
            "event" => Ty::Engine(1),
            "location" => Ty::Engine(2),
            "talent" => Ty::Engine(3),
            "action" => Ty::Action,
            _ => return None,
        })
    }

    /// Stack cells (4 bytes each) a value of this type occupies.
    pub fn cells(self) -> usize {
        match self {
            Ty::Void => 0,
            Ty::Vector => 3,
            _ => 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    pub ty: Ty,
    pub name: String,
    /// Source text of the default value, if the parameter has one.
    pub default: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Routine {
    pub id: u16,
    pub name: String,
    pub ret: Ty,
    pub params: Vec<Param>,
}

/// All routines, indexed by id.
#[derive(Clone, Debug, Default)]
pub struct Routines {
    list: Vec<Option<Routine>>,
}

impl Routines {
    /// Parse the text of `nwscript.nss`.
    pub fn parse(src: &str) -> Result<Routines> {
        let mut list: Vec<Option<Routine>> = Vec::new();
        let mut pending: Option<u16> = None;
        for line in src.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("//") {
                // A numbered comment marks the next prototype with that id.
                let digits: String = rest
                    .trim_start()
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                // Most ids are written "// 12: text", a few "// 12. Name".
                let after = &rest.trim_start()[digits.len()..];
                if !digits.is_empty()
                    && (after.starts_with(':') || after == "." || after.starts_with(". "))
                {
                    pending = digits.parse().ok();
                }
                continue;
            }
            let Some(id) = pending else { continue };
            if t.is_empty() {
                continue;
            }
            pending = None;
            let routine = parse_prototype(id, t)?;
            if list.len() <= id as usize {
                list.resize(id as usize + 1, None);
            }
            list[id as usize] = Some(routine);
        }
        if list.len() < 700 {
            return Err(Error::format(format!(
                "only {} routines found; is this nwscript.nss?",
                list.len()
            )));
        }
        Ok(Routines { list })
    }

    pub fn get(&self, id: u16) -> Option<&Routine> {
        self.list.get(id as usize)?.as_ref()
    }

    pub fn by_name(&self, name: &str) -> Option<&Routine> {
        self.list.iter().flatten().find(|r| r.name == name)
    }

    /// Number of routines with a prototype.
    pub fn len(&self) -> usize {
        self.list.iter().flatten().count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// `int GetLocalNumber(object oObject, string sVarName);` and friends.
fn parse_prototype(id: u16, line: &str) -> Result<Routine> {
    let bad = || Error::format(format!("cannot read prototype for routine {id}: {line}"));
    let open = line.find('(').ok_or_else(bad)?;
    let close = line.rfind(')').ok_or_else(bad)?;
    let mut head = line[..open].split_whitespace();
    let ret = Ty::parse(head.next().ok_or_else(bad)?).ok_or_else(bad)?;
    let name = head.next().ok_or_else(bad)?.to_string();
    let mut params = Vec::new();
    // Split on commas that are not inside a vector literal like `[0.0,0.0,0.0]`.
    let mut depth = 0;
    let mut in_string = false;
    let mut cur = String::new();
    let mut parts = Vec::new();
    for c in line[open + 1..close].chars() {
        match c {
            '"' => in_string = !in_string,
            '[' | '(' if !in_string => depth += 1,
            ']' | ')' if !in_string => depth -= 1,
            ',' if depth == 0 && !in_string => {
                parts.push(std::mem::take(&mut cur));
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    parts.push(cur);
    for p in parts.iter().map(|p| p.trim()).filter(|p| !p.is_empty()) {
        let (decl, default) = match p.split_once('=') {
            Some((d, v)) => (d.trim(), Some(v.trim().to_string())),
            None => (p, None),
        };
        let mut w = decl.split_whitespace();
        let ty = Ty::parse(w.next().ok_or_else(bad)?).ok_or_else(bad)?;
        params.push(Param {
            ty,
            name: w.next().unwrap_or("").to_string(),
            default,
        });
    }
    Ok(Routine {
        id,
        name,
        ret,
        params,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prototype_with_defaults_and_vectors() {
        let r = parse_prototype(
            7,
            "float GetDistanceBetween(object oObjectA, object oObjectB);",
        )
        .unwrap();
        assert_eq!((r.ret, r.params.len()), (Ty::Float, 2));
        let r = parse_prototype(
            9,
            "void Foo(vector v=[0.0,0.0,0.0], int n=3, string s=\"a,b\");",
        )
        .unwrap();
        assert_eq!(r.params.len(), 3);
        assert_eq!(r.params[0].ty, Ty::Vector);
        assert_eq!(r.params[0].default.as_deref(), Some("[0.0,0.0,0.0]"));
        assert!(parse_prototype(1, "garbage").is_err());
    }

    #[test]
    fn comment_ids_attach_to_the_next_prototype() {
        let mut src = String::new();
        for i in 0..800 {
            src.push_str(&format!("// {i}: doc\nint Fn{i}(int a);\n\n"));
        }
        let r = Routines::parse(&src).unwrap();
        assert_eq!(r.len(), 800);
        assert_eq!(r.get(5).unwrap().name, "Fn5");
        assert_eq!(r.by_name("Fn799").unwrap().id, 799);
        assert!(Routines::parse("int X();").is_err());
    }
}
