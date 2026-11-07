//! Counts the trapped things in every module's GIT: triggers, doors and placeables with a TrapFlag, by trap type.
use kotor_archive::Install;
use kotor_core::ResType;
use kotor_gff::Gff;
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
    let mut by_type: BTreeMap<(String, i32), usize> = BTreeMap::new();
    let mut shown = 0;
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
        for n in names {
            let Ok(b) = inst.read(&n, ResType::GIT) else {
                continue;
            };
            let Ok(git) = Gff::parse(&b) else { continue };
            for (list, ty, kind) in [
                ("TriggerList", ResType(2032), "trigger"),
                ("Door List", ResType(2042), "door"),
                ("Placeable List", ResType(2044), "placeable"),
            ] {
                for s in git.root.list(list).unwrap_or_default() {
                    let Some(t) = s.str("TemplateResRef") else {
                        continue;
                    };
                    let Ok(bytes) = inst.read(t, ty) else {
                        continue;
                    };
                    let Ok(g) = Gff::parse(&bytes) else { continue };
                    if g.root.int("TrapFlag").unwrap_or(0) == 0 {
                        continue;
                    }
                    let tt = g.root.int("TrapType").unwrap_or(-1) as i32;
                    *by_type.entry((kind.to_string(), tt)).or_default() += 1;
                    if shown < 6 {
                        shown += 1;
                        println!(
                            "{m} {kind} {t}: type {tt} detectable {:?} detectdc {:?} disarmable {:?} disarmdc {:?} oneshot {:?} onTrap {:?}",
                            g.root.int("TrapDetectable"),
                            g.root.int("TrapDetectDC"),
                            g.root.int("TrapDisarmable"),
                            g.root.int("DisarmDC"),
                            g.root.int("TrapOneShot"),
                            g.root.str("OnTrapTriggered"),
                        );
                    }
                }
            }
        }
    }
    for ((kind, t), n) in by_type {
        println!("{kind} type {t}: {n}");
    }
}
