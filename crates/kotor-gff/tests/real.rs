use kotor_archive::{Erf, Install, Source};
use kotor_core::ResType;
use kotor_gff::{Gff, Value};

const GFF_TYPES: &[(u16, &str)] = &[
    (2015, "BIC "),
    (2025, "UTI "),
    (2027, "UTC "),
    (2029, "DLG "),
    (2030, "ITP "),
    (2032, "UTT "),
    (2040, "UTE "),
    (2042, "UTD "),
    (2044, "UTP "),
    (2047, "GUI "),
    (2056, "JRL "),
    (2058, "UTW "),
];

fn round_trip(name: &str, data: &[u8]) -> Gff {
    let g = Gff::parse(data).unwrap_or_else(|e| panic!("{name}: {e}"));
    let out = g.to_bytes().unwrap();
    let g2 = Gff::parse(&out).unwrap_or_else(|e| panic!("{name} rewritten: {e}"));
    assert_eq!(g, g2, "{name} did not round-trip");
    g
}

#[test]
fn every_gff_in_base_data_round_trips() {
    let Some(dir) = kotor_core::install_dir() else { return };
    let mut inst = Install::open(dir).unwrap();
    let mut count = 0;
    for (ty, magic) in GFF_TYPES {
        let ty = ResType(*ty);
        for name in inst.list(ty) {
            let data = inst.read(&name, ty).unwrap();
            let g = round_trip(&format!("{name}.{ty}"), &data);
            assert_eq!(g.file_type, *magic, "{name}.{ty}");
            count += 1;
        }
    }
    assert!(count > 1000, "only {count} gff files checked");
}

#[test]
fn module_areas_and_info() {
    let Some(dir) = kotor_core::install_dir() else { return };
    let rim = Erf::open(Source::file(dir.join("modules").join("end_m01aa.rim"))).unwrap();
    let ifo = round_trip("end_m01aa ifo", &rim.get("module", ResType::IFO).unwrap());
    assert_eq!(ifo.file_type, "IFO ");
    assert_eq!(ifo.root.str("Mod_Entry_Area"), Some("m01aa"));
    let are = round_trip("m01aa are", &rim.get("m01aa", ResType::ARE).unwrap());
    assert_eq!(are.file_type, "ARE ");
    assert!(are.root.get("Name").is_some());
    let _ = round_trip("m01aa git", &rim.get("m01aa", ResType::GIT).unwrap());
}

#[test]
fn creature_template_fields() {
    let Some(dir) = kotor_core::install_dir() else { return };
    let mut inst = Install::open(dir).unwrap();
    let g = Gff::parse(&inst.read("c_bantha", ResType(2027)).unwrap()).unwrap();
    assert_eq!(g.root.str("Tag"), Some("Bantha"));
    assert!(matches!(g.root.get("FirstName"), Some(Value::LocString(_))));
    assert!(g.root.int("Str").is_some());
}
