//! NCS: compiled NWScript bytecode as used by KotOR: a disassembler, the engine routine table read from
//! the game's own `nwscript.nss`, and a stack virtual machine that calls the game through a `Host` trait.

mod disasm;
mod host;
mod routines;
mod vm;

pub use disasm::{disassemble, Instruction, Op, Program};
pub use host::HeadlessHost;
pub use routines::{Param, Routine, Routines, Ty};
pub use vm::{Host, Outcome, SavedState, Value, Vm, VmError, OBJECT_INVALID, OBJECT_SELF};
