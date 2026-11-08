//! Which soundsets (`soundset.2da` rows) give a creature a pain grunt and a dying line whose wave file is in the install.
//!
//!   cargo run --release -p kotor-game --example soundset_scan
use kotor_archive::Install;
use kotor_core::ResType;
use kotor_game::sound_file;
use kotor_tables::{Tlk, TwoDa};

fn main() {
    let dir = kotor_core::install_dir().expect("no install");
    let mut inst = Install::open(&dir).unwrap();
    let tlk = Tlk::parse(&std::fs::read(dir.join("dialog.tlk")).unwrap()).unwrap();
    let table = TwoDa::parse(&inst.read("soundset", ResType::TWODA).unwrap()).unwrap();
    let (mut rows, mut with_ssf, mut pain_ok, mut dead_ok) = (0, 0, 0, 0);
    let mut no_dead: Vec<String> = Vec::new();
    for row in 1..table.rows.len() {
        let name = table.get(row, "resref").unwrap_or_default().to_string();
        if name.is_empty() {
            continue;
        }
        rows += 1;
        let Some(b) = inst
            .read(&name, ResType(2060))
            .ok()
            .filter(|b| b.len() >= 12 && &b[..4] == b"SSF ")
        else {
            continue;
        };
        with_ssf += 1;
        let off = u32::from_le_bytes([b[8], b[9], b[10], b[11]]) as usize;
        let slots: Vec<u32> = b[off..]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| u32::from_le_bytes(*c))
            .collect();
        let heard = |slot: usize| {
            let s = *slots.get(slot).unwrap_or(&u32::MAX);
            s != u32::MAX
                && tlk
                    .entries
                    .get(s as usize)
                    .filter(|e| e.has_sound() && !e.sound_resref.is_empty())
                    .is_some_and(|e| sound_file(&dir, &e.sound_resref).is_some())
        };
        pain_ok += (heard(12) || heard(13)) as usize;
        if heard(15) {
            dead_ok += 1;
        } else {
            no_dead.push(name);
        }
    }
    println!("soundset rows: {rows}; with an .ssf: {with_ssf}; with a playable pain grunt: {pain_ok}; with a playable dying line: {dead_ok}");
    println!("no dying line: {}", no_dead.join(" "));
}
