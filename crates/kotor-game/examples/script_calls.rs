//! Prints the routines a script calls, in order: `script_calls <script>`.
use kotor_archive::Install;
use kotor_core::ResType;
use kotor_script::{disassemble, Op, Routines};
fn main() {
    let name = std::env::args().nth(1).unwrap();
    let mut inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
    let r = Routines::parse(&kotor_core::latin1(
        &inst.read("nwscript", ResType::NSS).unwrap(),
    ))
    .unwrap();
    let p = disassemble(&inst.read(&name, ResType::NCS).unwrap()).unwrap();
    for i in &p.instructions {
        match i.op {
            Op::Action { routine, .. } => println!("{}", r.get(routine).unwrap().name),
            Op::ConstI(c) => println!("  const {c}"),
            _ => {}
        }
    }
}
