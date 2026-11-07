//! Who stands in each module: its creatures by faction (the row of `repute.2da`) and, for each faction, a few of their templates.
//! For the table of authorities in `docs/law.tsv`.
//!
//!   cargo run --release -p kotor-game --example law_survey
use kotor_archive::Install;
use kotor_core::ResType;
use kotor_gff::Gff;
use kotor_tables::TwoDa;
use std::collections::{BTreeMap, BTreeSet};

fn main() {
    let dir = kotor_core::install_dir().unwrap();
    let mut modules: BTreeSet<String> = BTreeSet::new();
    for e in std::fs::read_dir(dir.join("modules")).unwrap().flatten() {
        let n = e.file_name().to_string_lossy().to_ascii_lowercase();
        if let Some(stem) = n.strip_suffix(".rim").or(n.strip_suffix(".mod")) {
            modules.insert(stem.trim_end_matches("_s").to_string());
        }
    }
    let mut inst = Install::open(&dir).unwrap();
    let repute = TwoDa::parse(&inst.read("repute", ResType::TWODA).unwrap()).unwrap();
    let label = |f: i64| {
        repute
            .get(f.max(0) as usize, "label")
            .unwrap_or("?")
            .to_string()
    };
    for m in &modules {
        let mut inst = Install::open(&dir).unwrap();
        if inst.load_module(m).is_err() {
            continue;
        }
        let names: Vec<String> = inst
            .overlay_resources()
            .filter(|(_, t, _)| *t == ResType::GIT)
            .map(|(n, _, _)| n.to_string())
            .collect();
        let mut by: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut count: BTreeMap<String, usize> = BTreeMap::new();
        for n in names {
            let Ok(b) = inst.read(&n, ResType::GIT) else {
                continue;
            };
            let Ok(git) = Gff::parse(&b) else { continue };
            for s in git.root.list("Creature List").unwrap_or_default() {
                let Some(t) = s.str("TemplateResRef") else {
                    continue;
                };
                let Ok(bytes) = inst.read(t, ResType(2027)) else {
                    continue;
                };
                let Ok(g) = Gff::parse(&bytes) else { continue };
                let f = label(g.root.int("FactionID").unwrap_or(0));
                *count.entry(f.clone()).or_default() += 1;
                let e = by.entry(f).or_default();
                if e.len() < 4 {
                    e.insert(t.to_string());
                }
            }
        }
        if count.is_empty() {
            continue;
        }
        let line: Vec<String> = count
            .iter()
            .map(|(f, n)| {
                format!(
                    "{f}:{n} [{}]",
                    by[f].iter().cloned().collect::<Vec<_>>().join(",")
                )
            })
            .collect();
        println!("{m}: {}", line.join("; "));
    }
}
