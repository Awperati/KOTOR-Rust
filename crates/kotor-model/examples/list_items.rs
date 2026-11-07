//! Item templates by base item: `list_items [prefix]` prints name and BaseItem.
use kotor_archive::Install;
use kotor_core::ResType;
fn main() {
    let prefix = std::env::args().nth(1).unwrap_or("g_w_".into());
    let mut inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
    for n in inst.list(ResType(2025)) {
        if !n.to_ascii_lowercase().starts_with(&prefix) {
            continue;
        }
        let Ok(b) = inst.read(&n, ResType(2025)) else {
            continue;
        };
        let Ok(g) = kotor_gff::Gff::parse(&b) else {
            continue;
        };
        println!("{n} base {:?}", g.root.int("BaseItem"));
    }
}
