//! Which supermodels carry which weapon animation sets: every model whose name starts `s_`, its supermodel, and the stance
//! digits of its fighting clips (`g1`..`g9`, `c2`..`c4`, `b5`..`b9`, `m2`, `f2`, `f5`).
//!
//!   cargo run --release -p kotor-model --example anim_sets
use kotor_archive::Install;
use kotor_core::ResType;
use std::collections::BTreeSet;

fn main() {
    let mut inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
    let names: Vec<String> = inst
        .list(ResType::MDL)
        .into_iter()
        .filter(|n| n.to_ascii_lowercase().starts_with("s_"))
        .collect();
    for n in names {
        let (Ok(mdl), mdx) = (inst.read(&n, ResType::MDL), inst.read(&n, ResType::MDX)) else {
            continue;
        };
        let Ok(m) = kotor_model::parse_model(&mdl, &mdx.unwrap_or_default()) else {
            continue;
        };
        let sets: BTreeSet<String> = m
            .animations
            .iter()
            .filter(|a| {
                let b = a.name.as_bytes();
                b.len() == 4
                    && b[1].is_ascii_digit()
                    && matches!(b[0], b'g' | b'c' | b'b' | b'm' | b'f')
            })
            .map(|a| a.name[..2].to_string())
            .collect();
        println!("{n}: super {:?}: {:?}", m.supermodel, sets);
    }
    for m in ["S_Male02", "S_Female02", "S_Female01", "S_Male01"] {
        hands(&mut inst, m);
    }
}

/// How far apart the hands are in the ready clip of each set, to tell a two-handed grip from a one-handed one.
#[allow(dead_code)]
pub fn hands(inst: &mut Install, model: &str) {
    let mdl = inst.read(model, ResType::MDL).unwrap();
    let mdx = inst.read(model, ResType::MDX).unwrap_or_default();
    let m = kotor_model::parse_model(&mdl, &mdx).unwrap();
    for a in &m.animations {
        let b = a.name.as_bytes();
        if b.len() == 4 && (a.name.ends_with("r1") || a.name.ends_with("a1")) {
            let t = a.length * 0.5;
            let w = m.world_matrices(Some(&a.sample(&m, t)));
            let p = |n: &str| {
                w.iter()
                    .find(|(x, _)| x.eq_ignore_ascii_case(n))
                    .map(|(_, mm)| [mm[0][3], mm[1][3], mm[2][3]])
            };
            if let (Some(r), Some(l)) = (p("rhand"), p("lhand")) {
                let d =
                    ((r[0] - l[0]).powi(2) + (r[1] - l[1]).powi(2) + (r[2] - l[2]).powi(2)).sqrt();
                println!("{model} {} (at {t:.2} s): hands {d:.2} m apart", a.name);
            }
        }
    }
}
