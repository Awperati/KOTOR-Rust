use kotor_tables::TwoDa;
fn main() {
    let name = std::env::args().nth(1).unwrap();
    let mut inst = kotor_archive::Install::open(kotor_core::install_dir().unwrap()).unwrap();
    let t = TwoDa::parse(&inst.read(&name, kotor_core::ResType::TWODA).unwrap()).unwrap();
    println!("{}", t.columns.join("\t"));
    for r in &t.rows {
        println!("{}", r.join("\t"));
    }
}
