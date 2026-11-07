//! Which conversation lines have a voice, and which should but find no file: every dialogue of the BIFs and of every module.
//! A line's voice is its node's `VO_ResRef`, else its `Sound`; the talk table's own sound name for the line's words is the
//! third place to look.
//!
//!   cargo run --release -p kotor-game --example voice_scan
use kotor_archive::Install;
use kotor_core::ResType;
use kotor_game::sound_file;
use kotor_gff::dlg::Dialog;
use kotor_gff::Gff;
use kotor_tables::Tlk;
use std::collections::BTreeMap;

fn main() {
    let dir = kotor_core::install_dir().expect("no install");
    let tlk = Tlk::parse(&std::fs::read(dir.join("dialog.tlk")).unwrap()).unwrap();
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
    let mut totals: BTreeMap<&str, usize> = BTreeMap::new();
    let mut missing: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut tlk_only: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut seen = std::collections::HashSet::new();
    let mut one = |inst: &mut Install, place: &str| {
        let names: Vec<String> = inst
            .overlay_resources()
            .filter(|(_, t, _)| *t == ResType(2029))
            .map(|(r, _, _)| r.as_str().to_string())
            .collect();
        let mut names = names;
        if place == "bif" {
            names = inst
                .key()
                .entries
                .iter()
                .filter(|e| e.restype == ResType(2029))
                .map(|e| e.resref.as_str().to_string())
                .collect();
        }
        for n in names {
            if !seen.insert(n.to_ascii_lowercase()) {
                continue;
            }
            let Some(g) = inst
                .read(&n, ResType(2029))
                .ok()
                .and_then(|b| Gff::parse(&b).ok())
            else {
                continue;
            };
            let Ok(d) = Dialog::from_gff(&g) else {
                continue;
            };
            for node in d.entries.iter() {
                let Some(text) = node.text.as_ref() else {
                    continue;
                };
                let words = text.strref >= 0
                    && tlk.get(text.strref as u32).is_some_and(|t| !t.is_empty())
                    || text.strings.iter().any(|s| !s.1.is_empty());
                if !words {
                    continue;
                }
                let sound = node
                    .extra
                    .iter()
                    .find_map(|(l, v)| match (l.as_str(), v) {
                        ("Sound", kotor_gff::Value::ResRef(s) | kotor_gff::Value::String(s)) => {
                            Some(s.clone())
                        }
                        _ => None,
                    })
                    .unwrap_or_default();
                let vo = match node.vo.clone().unwrap_or_default() {
                    v if v.is_empty() => sound,
                    v => v,
                };
                let tsound = (text.strref >= 0)
                    .then(|| tlk.entries.get(text.strref as usize))
                    .flatten()
                    .map(|e| e.sound_resref.clone())
                    .unwrap_or_default();
                let speaker = node.speaker.clone().unwrap_or_default();
                let found = |r: &str| !r.is_empty() && sound_file(&dir, r).is_some();
                let kind = if found(&vo) {
                    "voiced by VO_ResRef"
                } else if !vo.is_empty() {
                    missing
                        .entry(format!("{n} / {speaker}"))
                        .or_default()
                        .push(vo.clone());
                    "VO_ResRef named, file missing"
                } else if found(&tsound) {
                    tlk_only
                        .entry(format!("{n} / {speaker}"))
                        .or_default()
                        .push(tsound.clone());
                    "no VO_ResRef, the talk table's sound is a file"
                } else if !tsound.is_empty() {
                    "talk table names a sound, no file"
                } else {
                    "no voice at all"
                };
                *totals.entry(kind).or_default() += 1;
            }
        }
    };
    let mut inst = Install::open(&dir).unwrap();
    one(&mut inst, "bif");
    for m in &modules {
        let mut i = Install::open(&dir).unwrap();
        if i.load_module(m).is_ok() {
            one(&mut i, m);
        }
    }
    println!("{totals:#?}");
    println!(
        "-- VO_ResRef named, file missing: {} dialogue/speaker pairs",
        missing.len()
    );
    for (k, v) in missing.iter() {
        println!("{k}: {} lines, e.g. {}", v.len(), v[0]);
    }
    println!(
        "-- no VO_ResRef but the talk table's sound is a file: {} pairs",
        tlk_only.len()
    );
    for (k, v) in tlk_only.iter() {
        println!("{k}: {} lines, e.g. {}", v.len(), v[0]);
    }
}
