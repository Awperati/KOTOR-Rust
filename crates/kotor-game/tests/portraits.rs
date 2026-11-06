//! Every creature of the pilot's 21 modules has the portrait its file names.

use kotor_archive::Install;
use kotor_core::ResType;
use kotor_game::World;
use kotor_gff::Gff;
use kotor_tables::TwoDa;

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
const CREATURE: ResType = ResType(2027);

#[test]
fn every_creature_has_the_portrait_its_file_names() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(&dir).unwrap();
    let table = TwoDa::parse(&inst.read("portraits", ResType(2017)).unwrap()).unwrap();
    let name = |row: usize| {
        table
            .get(row, "baseresref")
            .unwrap_or("")
            .to_ascii_lowercase()
    };
    let mut seen = 0;
    let mut wrong = Vec::new();
    for m in MODULES {
        let mut files = Install::open(&dir).unwrap();
        files.set_prefer_rim(true);
        files.load_module(m).unwrap();
        let names: Vec<String> = files
            .overlay_resources()
            .filter(|(_, t, _)| *t == CREATURE)
            .map(|(n, _, _)| n.to_string())
            .collect();
        let mut w = World::new(Install::open(&dir).unwrap()).unwrap();
        w.load_module(m).unwrap();
        for n in names {
            let file = Gff::parse(&files.read(&n, CREATURE).unwrap()).unwrap();
            let id = file.root.int("PortraitId").unwrap_or(0) as usize;
            let creature = w.spawn_creature(&n, [0.0; 3]);
            let got = w.object(creature).unwrap().portrait.clone();
            seen += 1;
            let player =
                (0..table.row_count()).any(|r| table.int(r, "forpc") == Some(1) && name(r) == got);
            if id != 0 && got != name(id) {
                wrong.push(format!("{m} {n}: row {id} is {} but got {got}", name(id)));
            }
            if id == 0 && player {
                wrong.push(format!(
                    "{m} {n}: names no portrait but got the player's {got}"
                ));
            }
        }
    }
    assert!(seen > 300, "{seen} creatures");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn trask_and_carth_have_their_own_faces() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut w = World::new(Install::open(&dir).unwrap()).unwrap();
    w.load_module("end_m01aa").unwrap();
    let trask = w.spawn_creature("end_trask", [0.0; 3]);
    assert_eq!(w.object(trask).unwrap().portrait, "po_ptrask");
    let carth = w.spawn_creature("p_carth", [0.0; 3]);
    assert_eq!(w.object(carth).unwrap().portrait, "po_pcarth");
    // Row 0, with a body the table has no row for, finds nothing.
    let droid = w.spawn_creature("c_drdastro001", [0.0; 3]);
    assert_eq!(w.object(droid).unwrap().portrait, "");

    // A save from before this has them blank, and loading it fills them in.
    w.object_mut(trask).unwrap().portrait.clear();
    let dir_out = std::env::temp_dir().join(format!("kotor_rs_portraits_{}", std::process::id()));
    std::fs::create_dir_all(&dir_out).unwrap();
    let file = dir_out.join("old.sav");
    w.save_to(&file, "old").unwrap();
    let mut again = World::new(Install::open(&dir).unwrap()).unwrap();
    let loaded = again.load_from(&file);
    let _ = std::fs::remove_dir_all(&dir_out);
    loaded.unwrap();
    let found = again.object(trask).unwrap();
    assert_eq!(
        (found.template.as_str(), found.portrait.as_str()),
        ("end_trask", "po_ptrask")
    );
}
