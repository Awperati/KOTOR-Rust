use kotor_archive::Install;
use kotor_core::ResType;
use kotor_script::{disassemble, Op};

#[test]
fn every_base_ncs_decodes_with_valid_branches() {
    let Some(dir) = kotor_core::install_dir() else { return };
    let mut inst = Install::open(dir).unwrap();
    let names = inst.list(ResType::NCS);
    assert!(names.len() > 1000);
    let mut max_routine = 0;
    let mut instrs = 0usize;
    for n in &names {
        let data = inst.read(n, ResType::NCS).unwrap();
        let p = disassemble(&data).unwrap_or_else(|e| panic!("{n}: {e}"));
        p.check_branches().unwrap_or_else(|e| panic!("{n}: {e}"));
        // Instructions tile the file exactly.
        let last = p.instructions.last().unwrap();
        assert_eq!(last.offset + last.len, p.size, "{n}");
        for i in &p.instructions {
            if let Op::Action { routine, .. } = i.op {
                max_routine = max_routine.max(routine);
            }
        }
        instrs += p.instructions.len();
    }
    println!("{} scripts, {instrs} instructions, highest ACTION id {max_routine}", names.len());
    assert!(max_routine > 700 && max_routine < 1200);
}
