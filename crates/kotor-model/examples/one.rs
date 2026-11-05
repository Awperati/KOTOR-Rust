use kotor_core::ResType;
fn main() {
    let n = std::env::args().nth(1).unwrap();
    let mut inst = kotor_archive::Install::open(kotor_core::install_dir().unwrap()).unwrap();
    let mdl = inst.read(&n, ResType::MDL).unwrap();
    let mdx = inst.read(&n, ResType::MDX).unwrap_or_default();
    match kotor_model::parse_model(&mdl, &mdx) {
        Ok(m) => println!("ok {}", m.root.name),
        Err(e) => println!("err {e}"),
    }
}
