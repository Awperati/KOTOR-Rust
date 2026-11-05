use kotor_archive::Install;
use kotor_core::ResType;
use kotor_script::{disassemble, Op, Routines};

fn table(inst: &mut Install) -> Routines {
    let text = kotor_core::latin1(&inst.read("nwscript", ResType::NSS).unwrap());
    Routines::parse(&text).unwrap()
}

#[test]
fn routine_table_matches_every_action_in_every_base_script() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    let routines = table(&mut inst);
    assert!(routines.len() >= 770, "{} routines", routines.len());
    assert_eq!(routines.get(0).unwrap().name, "Random");
    assert_eq!(routines.get(1).unwrap().name, "PrintString");

    // The ACTION operand `argc` is how many arguments the compiler pushed: it must fit the prototype
    // (at least the parameters without defaults, at most all of them). This ties the table's ids to the
    // bytecode in every one of the base game's scripts.
    let (mut calls, mut seen) = (0usize, std::collections::BTreeSet::new());
    let mut mismatches: Vec<String> = Vec::new();
    for name in inst.list(ResType::NCS) {
        let prog = disassemble(&inst.read(&name, ResType::NCS).unwrap()).unwrap();
        for i in &prog.instructions {
            if let Op::Action { routine, args } = i.op {
                let r = routines
                    .get(routine)
                    .unwrap_or_else(|| panic!("{name}: no routine {routine}"));
                let required = r.params.iter().filter(|p| p.default.is_none()).count();
                if !(required..=r.params.len()).contains(&(args as usize)) {
                    mismatches.push(format!(
                        "{name}: {} called with {args} args, prototype takes {required}..={}",
                        r.name,
                        r.params.len()
                    ));
                }
                calls += 1;
                seen.insert(routine);
            }
        }
    }
    println!(
        "{calls} routine calls over {} distinct routines checked",
        seen.len()
    );
    println!("mismatches: {mismatches:?}");
    // One stock script (nw_s0_lghtnbolt) passes an extra argument to EffectBlasterDeflectionIncrease; the
    // other 20,937 calls agree with the table, which would not happen if any id were misaligned.
    assert!(mismatches.len() <= 1, "{mismatches:?}");
    assert!(
        mismatches.iter().all(|m| m.starts_with("nw_s0_lghtnbolt")),
        "{mismatches:?}"
    );
    assert!(calls > 20_000 && seen.len() > 300);
}
