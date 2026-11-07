//! Resource names containing a word: `list_names <restype 3 letters> <word>`, for example `wav fall`.
use kotor_archive::Install;
use kotor_core::ResType;
fn main() {
    let (ty, word) = (
        std::env::args().nth(1).unwrap(),
        std::env::args().nth(2).unwrap(),
    );
    let ty = match ty.as_str() {
        "wav" => ResType::WAV,
        "mdl" => ResType::MDL,
        _ => ResType::TWODA,
    };
    let inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
    let mut n: Vec<String> = inst
        .list(ty)
        .into_iter()
        .filter(|n| n.to_ascii_lowercase().contains(&word))
        .collect();
    n.sort();
    println!("{}", n.join(" "));
}
