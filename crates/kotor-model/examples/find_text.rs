//! Which 2DA files mention a text: `find_text <text>`.
use kotor_archive::Install;
use kotor_core::ResType;
fn main() {
    let want = std::env::args().nth(1).unwrap().to_ascii_lowercase();
    let mut inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
    for n in inst.list(ResType::TWODA) {
        let Ok(b) = inst.read(&n, ResType::TWODA) else {
            continue;
        };
        let t = kotor_core::latin1(&b).to_ascii_lowercase();
        let hits: Vec<&str> = t.lines().filter(|l| l.contains(&want)).take(3).collect();
        if !hits.is_empty() {
            println!("{n}: {:?}", hits);
        }
    }
}
