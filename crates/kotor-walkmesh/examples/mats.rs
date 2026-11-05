use kotor_archive::Install;
use kotor_core::{Reader, ResType};
use std::collections::BTreeMap;
fn main() {
    let mut inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
    // Raw read: count materials per mesh and the adjacency count from the header.
    let mut ids: BTreeMap<u32, (usize, usize)> = BTreeMap::new(); // mat -> (meshes where present, total faces)
    let mut rows = Vec::new();
    for ty in [ResType::WOK] {
        for name in inst.list(ty) {
            let d = inst.read(&name, ty).unwrap();
            let mut r = Reader::at(&d, 8 + 4 + 12 * 5);
            let (_vn, _vo, fnum, _io, mo, _no, _do, _an, _ao, _u, adj_n) = (
                r.u32().unwrap(),
                r.u32().unwrap(),
                r.u32().unwrap(),
                r.u32().unwrap(),
                r.u32().unwrap(),
                r.u32().unwrap(),
                r.u32().unwrap(),
                r.u32().unwrap(),
                r.u32().unwrap(),
                r.u32().unwrap(),
                r.u32().unwrap(),
            );
            let mut m = Reader::at(&d, mo as usize);
            let mut hist: BTreeMap<u32, usize> = BTreeMap::new();
            for _ in 0..fnum {
                *hist.entry(m.u32().unwrap()).or_default() += 1;
            }
            for (k, v) in &hist {
                let e = ids.entry(*k).or_default();
                e.0 += 1;
                e.1 += v;
            }
            rows.push((name, adj_n as usize, hist));
        }
    }
    println!("material ids seen: {ids:?}");
    let n = all_ids(&ids);
    let mut best = (usize::MAX, 0u32);
    for mask in 0u32..(1 << n.len()) {
        let bad = rows
            .iter()
            .filter(|(_, adj, h)| {
                h.iter()
                    .filter(|(k, _)| {
                        n.iter()
                            .position(|x| x == *k)
                            .is_some_and(|i| mask >> i & 1 == 1)
                    })
                    .map(|(_, v)| *v)
                    .sum::<usize>()
                    != *adj
            })
            .count();
        if bad < best.0 {
            best = (bad, mask);
        }
    }
    let set: Vec<u32> = n
        .iter()
        .enumerate()
        .filter(|(i, _)| best.1 >> i & 1 == 1)
        .map(|(_, x)| *x)
        .collect();
    println!(
        "best walkable set {set:?} mismatches {} of {}",
        best.0,
        rows.len()
    );
}
fn all_ids(ids: &BTreeMap<u32, (usize, usize)>) -> Vec<u32> {
    ids.keys().copied().collect()
}
