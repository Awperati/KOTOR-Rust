//! Every creature the pilot's 21 modules can field has an idle that moves it off its bind pose.

use kotor_archive::Install;
use kotor_core::ResType;
use kotor_gff::Gff;
use std::collections::BTreeSet;

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

/// How far some part of a body has to move, in the numbers of its matrices, not to count as standing in its bind pose.
const MOVED: f32 = 0.01;

fn appearances(dir: &std::path::Path) -> BTreeSet<usize> {
    let creature = ResType(2027);
    let mut rows = BTreeSet::new();
    for m in MODULES {
        let mut inst = Install::open(dir).unwrap();
        inst.set_prefer_rim(true);
        inst.load_module(m).unwrap();
        let names: Vec<String> = inst
            .overlay_resources()
            .filter(|(_, t, _)| *t == creature)
            .map(|(n, _, _)| n.to_string())
            .collect();
        for n in names {
            let g = Gff::parse(&inst.read(&n, creature).unwrap()).unwrap();
            rows.extend(g.root.int("Appearance_Type").map(|r| r as usize));
        }
    }
    rows
}

#[test]
fn no_creature_of_the_pilots_modules_stands_in_its_bind_pose() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let rows = appearances(&dir);
    assert!(rows.len() > 100, "{} appearances", rows.len());
    let mut inst = Install::open(&dir).unwrap();
    let mut still = Vec::new();
    let mut walkless = Vec::new();
    for row in rows {
        let rig = kotor_render::rig_for_appearance(&mut inst, row, 1).unwrap();
        if rig.distance_from_rest("pause1") < MOVED {
            still.push(row);
        }
        if rig.clip_name("walk").is_none() {
            walkless.push(row);
        }
    }
    assert!(still.is_empty(), "appearances in a bind pose: {still:?}");
    // Two Hutts and two turrets: their models have no walk.
    assert_eq!(walkless, [70, 182, 183, 228]);
}

#[test]
fn a_creature_answers_to_a_persons_clip_names() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(&dir).unwrap();
    let clips = |rig: &kotor_render::Rig| {
        [
            "pause1", "walk", "run", "die", "g8r1", "g8a1", "g8g1", "g8d1",
        ]
        .map(|want| rig.clip_name(want).map(str::to_string))
    };
    let names = |list: [&str; 8]| list.map(|n| Some(n.to_string()));
    // The rakghoul carries them in its own model, as do droids. The protocol droid in `tar_m02ab` has a supermodel.
    for row in [79, 317] {
        let rig = kotor_render::rig_for_appearance(&mut inst, row, 1).unwrap();
        assert_eq!(
            clips(&rig),
            names(["cpause1", "cwalk", "crun", "cdie", "creadyr", "g0a1", "cdamages", "cdodgeg"]),
            "row {row}"
        );
    }
    // T3-M4 has a person's names, with a pistol's digit.
    let rig = kotor_render::rig_for_appearance(&mut inst, 2, 1).unwrap();
    assert_eq!(
        clips(&rig),
        names(["pause1", "walk", "run", "die", "g5r1", "b5a1", "g5g1", "g5d1"])
    );
}
