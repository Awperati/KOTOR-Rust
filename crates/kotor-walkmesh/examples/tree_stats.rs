use kotor_core::ResType;
fn main() {
    let mut inst = kotor_archive::Install::open(kotor_core::install_dir().unwrap()).unwrap();
    let (mut bad, mut total) = (0, 0);
    for name in inst.list(ResType::WOK) {
        let b = kotor_walkmesh::parse_bwm(&inst.read(&name, ResType::WOK).unwrap()).unwrap();
        total += 1;
        if let Err(e) = b.check_tree() {
            bad += 1;
            if bad <= 12 {
                println!(
                    "{name}: {e}  (faces {}, aabbs {})",
                    b.faces.len(),
                    b.aabbs.len()
                );
            }
        }
    }
    println!("{bad} of {total} meshes fail check_tree");
}
