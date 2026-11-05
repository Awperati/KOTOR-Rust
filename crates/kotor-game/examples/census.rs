//! Survey of the routines the whole game's scripts call: static call sites per routine over every script in the
//! BIFs and every module, with how the world handles each. Usage: census [top_n]

use kotor_archive::Install;
use kotor_core::ResType;
use kotor_game::{RoutineStatus, World};
use kotor_script::{disassemble, Op, Routines};
use std::collections::{BTreeMap, BTreeSet};

fn main() {
    let top: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(60);
    let dir = kotor_core::install_dir().unwrap();
    let mut base = Install::open(&dir).unwrap();
    let routines = Routines::parse(&kotor_core::latin1(
        &base.read("nwscript", ResType::NSS).unwrap(),
    ))
    .unwrap();
    let mut sites: BTreeMap<u16, usize> = BTreeMap::new();
    let mut users: BTreeMap<u16, BTreeSet<String>> = BTreeMap::new();
    let mut scripts = 0usize;
    let mut count = |name: &str, bytes: &[u8]| {
        if let Ok(p) = disassemble(bytes) {
            scripts += 1;
            for i in &p.instructions {
                if let Op::Action { routine, .. } = i.op {
                    *sites.entry(routine).or_default() += 1;
                    users.entry(routine).or_default().insert(name.to_string());
                }
            }
        }
    };
    for n in base.list(ResType::NCS) {
        count(&n, &base.read(&n, ResType::NCS).unwrap());
    }
    let mut modules: BTreeSet<String> = BTreeSet::new();
    for e in std::fs::read_dir(dir.join("modules")).unwrap().flatten() {
        let n = e.file_name().to_string_lossy().to_ascii_lowercase();
        if let Some(stem) = n.strip_suffix(".rim").or(n.strip_suffix(".mod")) {
            modules.insert(stem.trim_end_matches("_s").to_string());
        }
    }
    let mut module_scripts = 0;
    for m in &modules {
        let mut inst = Install::open(&dir).unwrap();
        if inst.load_module(m).is_err() {
            continue;
        }
        let names: Vec<String> = inst
            .overlay_resources()
            .filter(|(_, t, _)| *t == ResType::NCS)
            .map(|(n, _, _)| n.to_string())
            .collect();
        for n in names {
            if let Ok(b) = inst.read(&n, ResType::NCS) {
                module_scripts += 1;
                count(&format!("{m}/{n}"), &b);
            }
        }
    }
    println!(
        "{scripts} scripts ({module_scripts} inside {} modules)",
        modules.len()
    );
    let mut w = World::new(Install::open(&dir).unwrap()).unwrap();
    w.load_module("end_m01aa").unwrap();
    let total: usize = sites.values().sum();
    let mut by_status: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut rows = Vec::new();
    for (id, n) in &sites {
        let Some(r) = routines.get(*id) else { continue };
        let st = w.routine_status(&r.name);
        let e = by_status.entry(format!("{st:?}")).or_default();
        e.0 += 1;
        e.1 += n;
        rows.push((*n, r.name.clone(), users[id].len(), st));
    }
    println!(
        "{} distinct routines called, {total} call sites",
        rows.len()
    );
    for (k, (r, s)) in &by_status {
        println!(
            "  {k:<12} {r:4} routines, {s:6} call sites ({:.1}%)",
            *s as f64 * 100.0 / total as f64
        );
    }
    rows.sort_by_key(|a| std::cmp::Reverse(a.0));
    println!("most used routines that are not implemented:");
    for (n, name, u, st) in rows
        .iter()
        .filter(|r| matches!(r.3, RoutineStatus::Stub | RoutineStatus::Constant))
        .take(top)
    {
        println!("  {n:6} sites {u:5} scripts  {name} ({st:?})");
    }
    let unused = routines.len() - rows.len();
    println!("{unused} routines in nwscript.nss are never called by any script");
}
