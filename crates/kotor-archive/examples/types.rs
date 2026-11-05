use kotor_archive::Install;
use kotor_core::ResType;
use std::collections::BTreeMap;
fn main() {
    let dir = kotor_core::install_dir().unwrap();
    let mut inst = Install::open(&dir).unwrap();
    let mut by: BTreeMap<u16, (usize, String)> = BTreeMap::new();
    let entries: Vec<_> = inst
        .key()
        .entries
        .iter()
        .map(|e| (e.restype.0, e.resref.to_string()))
        .collect();
    for (t, n) in entries {
        let e = by.entry(t).or_insert((0, n));
        e.0 += 1;
    }
    for (t, (c, n)) in by {
        let d = inst.read(&n, ResType(t)).unwrap();
        let m: String = d
            .iter()
            .take(8)
            .map(|&b| {
                if (32..127).contains(&b) {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        println!("{t}\t{c}\t{n}\t{m}");
    }
}
