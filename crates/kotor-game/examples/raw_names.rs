//! Every item, creature, placeable and door template whose name is a designer's tag and not words: bug B2, "End_onedam". Over
//! the BIFs and every module.
//!
//!   cargo run --release -p kotor-game --example raw_names
use kotor_archive::Install;
use kotor_core::ResType;
use kotor_game::name_is_a_tag;
use kotor_gff::Gff;
use kotor_tables::Tlk;
use std::collections::BTreeMap;

const KINDS: [(ResType, &str, &str); 4] = [
    (ResType(2025), "item", "LocalizedName"),
    (ResType(2027), "creature", "FirstName"),
    (ResType(2044), "placeable", "LocName"),
    (ResType(2042), "door", "LocName"),
];

fn main() {
    let dir = kotor_core::install_dir().expect("no install");
    let tlk = Tlk::parse(&std::fs::read(dir.join("dialog.tlk")).unwrap()).unwrap();
    let mut inst = Install::open(&dir).unwrap();
    let mut found: BTreeMap<(&str, String, i64, String), Vec<String>> = BTreeMap::new();
    let (mut seen, mut empty) = (0, 0);
    let mut look = |inst: &mut Install, names: Vec<(String, ResType)>, place: &str| {
        for (n, t) in names {
            let Some((_, kind, label)) = KINDS.iter().find(|k| k.0 == t) else {
                continue;
            };
            let Some(g) = inst.read(&n, t).ok().and_then(|b| Gff::parse(&b).ok()) else {
                continue;
            };
            seen += 1;
            let Some(l) = g.root.loc(label) else {
                continue;
            };
            let text = if l.strref >= 0 {
                tlk.get(l.strref as u32).unwrap_or_default()
            } else {
                l.strings.first().map(|s| s.1.clone()).unwrap_or_default()
            };
            empty += text.is_empty() as usize;
            if name_is_a_tag(&text) {
                found
                    .entry((
                        *kind,
                        n.to_ascii_lowercase(),
                        g.root.int("Useable").unwrap_or(-1),
                        text,
                    ))
                    .or_default()
                    .push(place.to_string());
            }
        }
    };
    let base: Vec<(String, ResType)> = inst
        .key()
        .entries
        .iter()
        .map(|e| (e.resref.as_str().to_string(), e.restype))
        .collect();
    look(&mut inst, base, "bif");
    let mut modules: Vec<String> = std::fs::read_dir(dir.join("modules"))
        .unwrap()
        .filter_map(|e| {
            let n = e.ok()?.file_name().to_string_lossy().to_ascii_lowercase();
            n.strip_suffix(".rim")
                .filter(|s| !s.ends_with("_s"))
                .map(str::to_string)
        })
        .collect();
    modules.sort();
    for m in &modules {
        let mut one = Install::open(&dir).unwrap();
        if one.load_module(m).is_err() {
            continue;
        }
        let names: Vec<(String, ResType)> = one
            .overlay_resources()
            .map(|(r, t, _)| (r.as_str().to_string(), t))
            .collect();
        look(&mut one, names, m);
    }
    println!(
        "{seen} templates read, {empty} with no name, {} raw:",
        found.len()
    );
    for ((kind, template, useable, text), places) in &found {
        let first = places.first().map_or("", String::as_str);
        println!(
            "{kind}\t{template}\tuseable {useable}\t{text}\t{first}\t{}",
            places.len()
        );
    }
}
