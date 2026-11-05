use kotor_archive::Install;
use kotor_core::ResType;
use kotor_tables::{Tlk, TwoDa};

#[test]
fn every_base_2da_parses() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    let names = inst.list(ResType::TWODA);
    assert!(names.len() > 100);
    for n in &names {
        let t = TwoDa::parse(&inst.read(n, ResType::TWODA).unwrap())
            .unwrap_or_else(|e| panic!("{n}: {e}"));
        assert!(!t.columns.is_empty(), "{n} has no columns");
        for (i, r) in t.rows.iter().enumerate() {
            assert_eq!(r.len(), t.columns.len(), "{n} row {i}");
        }
    }
}

#[test]
fn known_2da_values() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    let t = TwoDa::parse(&inst.read("appearance", ResType::TWODA).unwrap()).unwrap();
    println!("appearance cols: {:?}, rows {}", t.columns, t.row_count());
    assert!(t.row_count() > 400);
    assert!(t.column_index("label").is_some());
    assert!(t.column_index("race").is_some());
    // Row labels are the row numbers.
    for (i, l) in t.labels.iter().enumerate() {
        assert_eq!(l, &i.to_string());
    }
    let classes = TwoDa::parse(&inst.read("classes", ResType::TWODA).unwrap()).unwrap();
    assert_eq!(classes.get(0, "label"), Some("Soldier"));
    assert_eq!(classes.int(0, "hitdie"), Some(10));
}

#[test]
fn dialog_tlk() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let tlk = Tlk::parse(&std::fs::read(dir.join("dialog.tlk")).unwrap()).unwrap();
    assert!(tlk.len() > 40_000, "{} strings", tlk.len());
    let texts = (0..tlk.len() as u32).filter_map(|i| tlk.get(i)).count();
    assert!(texts > 30_000);
    // Every string decodes; no stray control bytes besides whitespace.
    for i in 0..tlk.len() as u32 {
        if let Some(s) = tlk.get(i) {
            assert!(
                !s.chars().any(|c| c.is_control()
                    && !c.is_whitespace()
                    && c as u32 != 0x81
                    && !(0x8D..=0x9F).contains(&(c as u32))),
                "strref {i}"
            );
        }
    }
}
