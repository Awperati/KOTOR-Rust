use crate::ErfKind;
use kotor_core::{Error, ResRef, ResType, Result};

/// One resource to store in a new archive.
#[derive(Clone, Debug)]
pub struct NewResource {
    pub resref: ResRef,
    pub restype: ResType,
    pub data: Vec<u8>,
}

impl NewResource {
    pub fn new(resref: &str, restype: ResType, data: Vec<u8>) -> NewResource {
        NewResource {
            resref: ResRef::new(resref),
            restype,
            data,
        }
    }
}

fn name16(r: &ResRef) -> Result<[u8; 16]> {
    let s = r.as_str();
    if s.len() > 16 || !s.is_ascii() {
        return Err(Error::format(format!(
            "resource name '{s}' is not 16 ASCII chars or fewer"
        )));
    }
    let mut b = [0u8; 16];
    b[..s.len()].copy_from_slice(s.as_bytes());
    Ok(b)
}

fn u32_of(n: usize) -> Result<u32> {
    u32::try_from(n).map_err(|_| Error::format("archive too large"))
}

/// Build an ERF, MOD, SAV or RIM archive in memory. The result reads back through `Erf::open`.
/// ERF-family archives are written without localized strings.
pub fn write_archive(kind: ErfKind, resources: &[NewResource]) -> Result<Vec<u8>> {
    let n = resources.len();
    let mut out = Vec::new();
    if kind == ErfKind::Rim {
        const HEADER: usize = 120;
        let data_start = HEADER + n * 32;
        out.extend_from_slice(b"RIM V1.0");
        for v in [0, u32_of(n)?, HEADER as u32] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.resize(HEADER, 0);
        let mut offset = data_start;
        for (i, r) in resources.iter().enumerate() {
            out.extend_from_slice(&name16(&r.resref)?);
            for v in [
                r.restype.0 as u32,
                i as u32,
                u32_of(offset)?,
                u32_of(r.data.len())?,
            ] {
                out.extend_from_slice(&v.to_le_bytes());
            }
            offset += r.data.len();
        }
    } else {
        const HEADER: usize = 160;
        let tag: &[u8; 4] = match kind {
            ErfKind::Mod => b"MOD ",
            ErfKind::Sav => b"SAV ",
            _ => b"ERF ",
        };
        let key_off = HEADER;
        let res_off = key_off + n * 24;
        let data_start = res_off + n * 8;
        out.extend_from_slice(tag);
        out.extend_from_slice(b"V1.0");
        // languages, localized size, entries, localized off, key off, res off, year, day, desc strref
        for v in [
            0,
            0,
            u32_of(n)?,
            HEADER as u32,
            key_off as u32,
            u32_of(res_off)?,
            0,
            0,
            0xFFFF_FFFF,
        ] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.resize(HEADER, 0);
        for (i, r) in resources.iter().enumerate() {
            out.extend_from_slice(&name16(&r.resref)?);
            out.extend_from_slice(&u32_of(i)?.to_le_bytes());
            out.extend_from_slice(&r.restype.0.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
        }
        let mut offset = data_start;
        for r in resources {
            out.extend_from_slice(&u32_of(offset)?.to_le_bytes());
            out.extend_from_slice(&u32_of(r.data.len())?.to_le_bytes());
            offset += r.data.len();
        }
    }
    for r in resources {
        out.extend_from_slice(&r.data);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Erf, Source};

    fn sample() -> Vec<NewResource> {
        vec![
            NewResource::new("alpha", ResType(2017), b"2DA V2.b".to_vec()),
            NewResource::new("Beta", ResType(2014), vec![]),
            NewResource::new("gamma", ResType(2010), vec![1, 2, 3]),
        ]
    }

    #[test]
    fn all_kinds_round_trip() {
        for kind in [ErfKind::Erf, ErfKind::Mod, ErfKind::Sav, ErfKind::Rim] {
            let bytes = write_archive(kind, &sample()).unwrap();
            let erf = Erf::open(Source::memory(bytes)).unwrap();
            assert_eq!(erf.kind, kind);
            assert_eq!(erf.entries.len(), 3);
            assert_eq!(erf.get("beta", ResType(2014)).unwrap(), b"");
            assert_eq!(erf.get("gamma", ResType(2010)).unwrap(), [1, 2, 3]);
        }
    }

    #[test]
    fn rejects_long_names() {
        let r = [NewResource::new(
            "this_name_is_too_long",
            ResType(2010),
            vec![],
        )];
        assert!(write_archive(ErfKind::Mod, &r).is_err());
    }

    #[test]
    fn real_module_survives_rewrite() {
        let Some(dir) = kotor_core::install_dir() else {
            return;
        };
        let src = Erf::open(Source::file(dir.join("modules").join("end_m01aa.rim"))).unwrap();
        let res: Vec<NewResource> = src
            .entries
            .iter()
            .map(|e| NewResource {
                resref: e.resref.clone(),
                restype: e.restype,
                data: src.read(e).unwrap(),
            })
            .collect();
        for kind in [ErfKind::Mod, ErfKind::Rim] {
            let copy = Erf::open(Source::memory(write_archive(kind, &res).unwrap())).unwrap();
            assert_eq!(copy.entries.len(), src.entries.len());
            for (a, b) in src.entries.iter().zip(&copy.entries) {
                assert_eq!((&a.resref, a.restype), (&b.resref, b.restype));
                assert_eq!(src.read(a).unwrap(), copy.read(b).unwrap());
            }
        }
    }
}
