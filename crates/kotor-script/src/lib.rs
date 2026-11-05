//! NCS: compiled NWScript bytecode as used by KotOR. This crate currently
//! decodes and prints programs; the VM that runs them comes later.

mod disasm;

pub use disasm::{disassemble, Instruction, Op, Program};
