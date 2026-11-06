//! Every creature appearance the pilot's modules use, its model and supermodels, and which clips each has.
use kotor_archive::Install;
use kotor_core::ResType;
use kotor_gff::Gff;
use kotor_tables::TwoDa;
use std::collections::{BTreeMap, BTreeSet};

const MODULES: [&str; 21] = [
    "end_m01aa",
    "end_m01ab",
    "STUNT_00",
    "tar_m02aa",
    "tar_m02ab",
    "tar_m02ac",
    "tar_m02ad",
    "tar_m02ae",
    "tar_m02af",
    "tar_m03aa",
    "tar_m03ab",
    "tar_m03ad",
    "tar_m03ae",
    "tar_m04aa",
    "tar_m05aa",
    "tar_m05ab",
    "tar_m09aa",
    "tar_m09ab",
    "tar_m10aa",
    "tar_m10ac",
    "tar_m11aa",
];

fn main() {
    let dir = kotor_core::install_dir().unwrap();
    let utc = ResType(2027);
    let mut rows: BTreeMap<usize, BTreeSet<String>> = BTreeMap::new();
    for m in MODULES {
        let mut inst = Install::open(&dir).unwrap();
        inst.set_prefer_rim(true);
        inst.load_module(m).unwrap();
        let names: Vec<String> = inst
            .overlay_resources()
            .filter(|(_, t, _)| *t == utc)
            .map(|(n, _, _)| n.to_string())
            .collect();
        for n in names {
            let Ok(b) = inst.read(&n, utc) else { continue };
            let Ok(g) = Gff::parse(&b) else { continue };
            if let Some(r) = g.root.int("Appearance_Type") {
                rows.entry(r as usize)
                    .or_default()
                    .insert(format!("{m}/{n}"));
            }
        }
    }
    let mut inst = Install::open(&dir).unwrap();
    let app = TwoDa::parse(&inst.read("appearance", ResType::TWODA).unwrap()).unwrap();
    for (row, users) in &rows {
        let label = app.get(*row, "label").unwrap_or("?").to_string();
        let kind = app.get(*row, "modeltype").unwrap_or("?").to_string();
        let race = app.get(*row, "race").unwrap_or("").to_string();
        let rig = kotor_render::rig_for_appearance(&mut inst, *row, 1);
        match rig {
            Ok(rig) => {
                let clip = |want: &str| rig.clip_name(want).unwrap_or("-").to_string();
                println!(
                    "{row} {label} model {race} supermodels {:?} idle {} moved {:.3} walk {} run {} die {} ready {} attack {} ranged {} hit {} dodge {} used {}",
                    rig.chain(),
                    clip("pause1"),
                    rig.distance_from_rest("pause1"),
                    clip("walk"),
                    clip("run"),
                    clip("die"),
                    clip("g8r1"),
                    clip("g8a1"),
                    clip("b5a1"),
                    clip("g8g1"),
                    clip("g8d1"),
                    users.iter().next().unwrap(),
                );
            }
            Err(e) => println!("{row} {label} type {kind} race {race} NO RIG {e}"),
        }
    }
}
