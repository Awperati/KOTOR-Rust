use kotor_archive::Install;
use kotor_core::ResType;
use kotor_script::{disassemble, HeadlessHost, Routines, Value, Vm, VmError};
use std::collections::BTreeMap;

fn setup(module: &str) -> Option<(Install, Routines, Vec<String>)> {
    let dir = kotor_core::install_dir()?;
    let mut inst = Install::open(dir).unwrap();
    let text = kotor_core::latin1(&inst.read("nwscript", ResType::NSS).unwrap());
    let routines = Routines::parse(&text).unwrap();
    inst.load_module(module).unwrap();
    let names = inst
        .overlay_resources()
        .filter(|(_, t, _)| *t == ResType::NCS)
        .map(|(n, _, _)| n.to_string())
        .collect();
    Some((inst, routines, names))
}

fn world() -> HeadlessHost {
    let mut h = HeadlessHost::default();
    h.add_object(2, "end_trask", [20.0, 20.0, -1.0]);
    h.add_object(3, "end_carth", [30.0, 20.0, -1.0]);
    h
}

/// Run every script of the Endar Spire module against the headless world. A script either finishes with
/// a balanced stack, or stops because it calls a routine the headless host does not implement. Any other
/// failure (stack underflow, type error, bad offset, bad jump) would be a VM bug.
#[test]
fn endar_spire_scripts_run_or_stop_at_an_unimplemented_routine() {
    let Some((mut inst, routines, names)) = setup("end_m01aa") else {
        return;
    };
    let (mut finished, mut stopped) = (0, 0);
    let mut missing: BTreeMap<String, usize> = BTreeMap::new();
    let mut bad: Vec<String> = Vec::new();
    for n in &names {
        let program = disassemble(&inst.read(n, ResType::NCS).unwrap()).unwrap();
        let mut host = world();
        match Vm::new(&program, &routines).run(2, &mut host) {
            Ok(out) => {
                finished += 1;
                if out.stack.len() > 1 {
                    bad.push(format!("{n}: {} cells left on the stack", out.stack.len()));
                }
                if let Some(Value::Int(v)) = out.stack.first() {
                    if !(0..=1).contains(v)
                        && out.stack.len() == 1
                        && n.starts_with("k_pend_traskdl")
                    {
                        bad.push(format!("{n}: conditional returned {v}"));
                    }
                }
            }
            Err(VmError::Host(m)) if m.contains("is not implemented") => {
                stopped += 1;
                let name = m.split_whitespace().nth(1).unwrap_or("?").to_string();
                *missing.entry(name).or_default() += 1;
            }
            Err(e) => bad.push(format!("{n}: {e}")),
        }
    }
    let mut top: Vec<_> = missing.iter().collect();
    top.sort_by(|a, b| b.1.cmp(a.1));
    println!(
        "{} scripts: {finished} finished cleanly, {stopped} stopped at an unimplemented routine; first missing: {:?}",
        names.len(),
        top.iter().take(12).collect::<Vec<_>>()
    );
    assert!(bad.is_empty(), "VM faults: {bad:#?}");
    assert!(finished >= 40, "only {finished} scripts ran to completion");
}

#[test]
fn trask_conditions_follow_the_global_they_read() {
    let Some((mut inst, routines, names)) = setup("end_m01aa") else {
        return;
    };
    // The k_pend_traskdl* scripts are dialogue conditions that read one global number. For every one that
    // uses nothing else: the answer must be 0 or 1, and must change with the global (else it is not a test).
    let mut varied = 0;
    for n in names
        .iter()
        .filter(|n| n.starts_with("k_pend_traskdl") || n.starts_with("k_pend_trask0"))
    {
        let program = disassemble(&inst.read(n, ResType::NCS).unwrap()).unwrap();
        let mut results = Vec::new();
        for value in 0..8 {
            let mut host = world();
            // Every global the script might read gets the same value: the scripts read one name each.
            for name in [
                "END_TRASK_DLG",
                "K_PEND_TRASK",
                "END_TRASK",
                "END_TRASK_DIALOG",
            ] {
                host.global_numbers.insert(name.to_string(), value);
            }
            match Vm::new(&program, &routines).run(2, &mut host) {
                Ok(out) => results.push(out.int_result()),
                Err(VmError::Host(m)) if m.contains("is not implemented") => continue,
                Err(e) => panic!("{n}: {e}"),
            }
        }
        // Void scripts leave no result; conditionals leave 0 or 1.
        assert!(
            results
                .iter()
                .all(|r| matches!(r, None | Some(0) | Some(1))),
            "{n}: {results:?}"
        );
        if results
            .iter()
            .flatten()
            .any(|r| Some(r) != results.iter().flatten().next())
        {
            varied += 1;
        }
    }
    println!("{varied} Trask condition scripts change their answer with the global");
    assert!(
        varied >= 5,
        "only {varied} condition scripts reacted to the global"
    );
}

#[test]
fn an_action_script_writes_its_effect_to_the_world() {
    let Some((mut inst, routines, _names)) = setup("end_m01aa") else {
        return;
    };
    // k_pend_aion calls only SetLocalBoolean: after running as object 2, object 2 has a local flag set.
    let program = disassemble(&inst.read("k_pend_aion", ResType::NCS).unwrap()).unwrap();
    let mut host = world();
    let out = Vm::new(&program, &routines).run(2, &mut host).unwrap();
    assert!(out.stack.len() <= 1, "{:?}", out.stack);
    assert_eq!(host.local_booleans.len(), 1, "{:?}", host.local_booleans);
    let ((who, _index), _value) = host.local_booleans.iter().next().unwrap();
    assert_eq!(*who, 2, "OBJECT_SELF must resolve to the caller");
}

/// The same check over every script in the base data: only "routine not implemented" may stop a run.
#[test]
fn no_base_script_faults_the_vm() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    let text = kotor_core::latin1(&inst.read("nwscript", ResType::NSS).unwrap());
    let routines = Routines::parse(&text).unwrap();
    let (mut finished, mut stopped) = (0, 0);
    let mut bad: Vec<String> = Vec::new();
    for n in inst.list(ResType::NCS) {
        let program = disassemble(&inst.read(&n, ResType::NCS).unwrap()).unwrap();
        match Vm::new(&program, &routines).run(2, &mut world()) {
            Ok(_) => finished += 1,
            Err(VmError::Host(m)) if m.contains("is not implemented") => stopped += 1,
            Err(e) => bad.push(format!("{n}: {e}")),
        }
    }
    println!(
        "{} base scripts: {finished} finished, {stopped} stopped at an unimplemented routine, {} faults",
        finished + stopped + bad.len(),
        bad.len()
    );
    assert!(bad.is_empty(), "VM faults: {bad:#?}");
}
