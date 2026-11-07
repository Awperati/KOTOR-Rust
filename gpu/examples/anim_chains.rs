//! Which animation sets each body of `appearance.2da` has: its supermodel chain, and what the game's two-handed (3) and
//! dual-wield (4) ready clips resolve to.
//!
//!   cargo run --release --example anim_chains
use std::collections::BTreeMap;

fn main() {
    let mut inst = kotor_archive::Install::open(kotor_core::install_dir().unwrap()).unwrap();
    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for row in 0..700 {
        let Ok(rig) = kotor_render::rig_for_outfit(&mut inst, row, 1, None) else {
            continue;
        };
        let r = |n: &str| rig.clip_name(n).unwrap_or("-").to_string();
        let key = format!(
            "chain {:?}: g1r1->{} g2r1->{} g3r1->{} g4r1->{} g5r1->{} g7r1->{} g9r1->{}",
            rig.chain(),
            r("g1r1"),
            r("g2r1"),
            r("g3r1"),
            r("g4r1"),
            r("g5r1"),
            r("g7r1"),
            r("g9r1")
        );
        groups.entry(key).or_default().push(row);
    }
    for (k, rows) in groups {
        println!(
            "{} rows, e.g. {:?}\n   {k}",
            rows.len(),
            &rows[..rows.len().min(8)]
        );
    }
}
