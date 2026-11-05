use std::fmt;

/// Resource type id as stored in KEY/ERF/RIM files.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct ResType(pub u16);

const TABLE: &[(u16, &str)] = &[
    (1, "bmp"),
    (3, "tga"),
    (4, "wav"),
    (6, "plt"),
    (7, "ini"),
    (9, "txt"),
    (2002, "mdl"),
    (2009, "nss"),
    (2010, "ncs"),
    (2011, "mod"),
    (2012, "are"),
    (2013, "set"),
    (2014, "ifo"),
    (2015, "bic"),
    (2016, "wok"),
    (2017, "2da"),
    (2018, "tlk"),
    (2022, "txi"),
    (2023, "git"),
    (2024, "bti"),
    (2025, "uti"),
    (2026, "btc"),
    (2027, "utc"),
    (2029, "dlg"),
    (2030, "itp"),
    (2032, "utt"),
    (2033, "dds"),
    (2035, "uts"),
    (2036, "ltr"),
    (2037, "gff"),
    (2038, "fac"),
    (2040, "ute"),
    (2042, "utd"),
    (2044, "utp"),
    (2045, "dft"),
    (2046, "gic"),
    (2047, "gui"),
    (2048, "css"),
    (2049, "ccs"),
    (2050, "btm"),
    (2051, "utm"),
    (2052, "dwk"),
    (2053, "pwk"),
    (2054, "btg"),
    (2055, "utg"),
    (2056, "jrl"),
    (2057, "sav"),
    (2058, "utw"),
    (2059, "4pc"),
    (2060, "ssf"),
    (2061, "hak"),
    (2062, "nwm"),
    (2063, "bik"),
    (2064, "ndb"),
    (2065, "ptm"),
    (2066, "ptt"),
    (3000, "lyt"),
    (3001, "vis"),
    (3002, "rim"),
    (3003, "pth"),
    (3004, "lip"),
    (3005, "bwm"),
    (3006, "txb"),
    (3007, "tpc"),
    (3008, "mdx"),
    (3009, "rsv"),
    (3010, "sig"),
    (3011, "mab"),
    (3012, "qst"),
    (3013, "sto"),
    (3014, "hex"),
    (9997, "erf"),
    (9998, "bif"),
    (9999, "key"),
];

impl ResType {
    pub const TGA: ResType = ResType(3);
    pub const WAV: ResType = ResType(4);
    pub const MDL: ResType = ResType(2002);
    pub const NSS: ResType = ResType(2009);
    pub const NCS: ResType = ResType(2010);
    pub const MOD: ResType = ResType(2011);
    pub const ARE: ResType = ResType(2012);
    pub const IFO: ResType = ResType(2014);
    pub const WOK: ResType = ResType(2016);
    pub const TWODA: ResType = ResType(2017);
    pub const TLK: ResType = ResType(2018);
    pub const TXI: ResType = ResType(2022);
    pub const GIT: ResType = ResType(2023);
    pub const DLG: ResType = ResType(2029);
    pub const DWK: ResType = ResType(2052);
    pub const PWK: ResType = ResType(2053);
    pub const LYT: ResType = ResType(3000);
    pub const VIS: ResType = ResType(3001);
    pub const BWM: ResType = ResType(3005);
    pub const TPC: ResType = ResType(3007);
    pub const MDX: ResType = ResType(3008);

    /// Lower-case file extension, or `None` for an id this crate does not know.
    pub fn extension(self) -> Option<&'static str> {
        TABLE.iter().find(|(id, _)| *id == self.0).map(|(_, e)| *e)
    }

    pub fn from_extension(ext: &str) -> Option<ResType> {
        let ext = ext.to_ascii_lowercase();
        TABLE
            .iter()
            .find(|(_, e)| *e == ext)
            .map(|(id, _)| ResType(*id))
    }
}

impl fmt::Display for ResType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.extension() {
            Some(e) => f.write_str(e),
            None => write!(f, "type{}", self.0),
        }
    }
}

/// Resource name: up to 16 characters, case-insensitive, stored lower-case.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Default)]
pub struct ResRef(String);

impl ResRef {
    pub fn new(s: &str) -> ResRef {
        ResRef(s.to_ascii_lowercase())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for ResRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_round_trip() {
        for (id, ext) in TABLE {
            assert_eq!(ResType::from_extension(ext), Some(ResType(*id)));
            assert_eq!(ResType(*id).extension(), Some(*ext));
        }
        assert_eq!(ResType(12345).to_string(), "type12345");
    }

    #[test]
    fn resref_is_lowercase() {
        assert_eq!(ResRef::new("END_m01aa").as_str(), "end_m01aa");
    }
}
