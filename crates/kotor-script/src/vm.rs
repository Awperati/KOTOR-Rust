//! The NCS virtual machine: a stack machine over 4-byte cells.
//!
//! Every value occupies cells: ints, floats, strings, objects and engine types one cell each, a vector
//! three float cells (x, y, z with z on top). Offsets in the bytecode are in bytes, so they are divided
//! by four to index cells. Engine routines are called through the [`Host`] trait; the VM knows nothing
//! about the game world.

use crate::routines::{Routine, Routines, Ty};
use crate::{Instruction, Op, Program};
use std::collections::HashMap;
use std::fmt;

/// `OBJECT_INVALID` as the engine encodes it.
pub const OBJECT_INVALID: u32 = 0x7F00_0000;
/// `OBJECT_SELF`: the object running the script.
pub const OBJECT_SELF: u32 = 0;

/// A script value as seen by the host.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(i32),
    Float(f32),
    Str(String),
    Object(u32),
    /// `effect`, `event`, `location` or `talent` (kind 0..=3) with a host-defined handle.
    Engine(u8, u32),
    /// A deferred action: the id of the state saved by the last `STORESTATE`.
    Action(u32),
    Vector([f32; 3]),
}

#[derive(Debug, PartialEq)]
pub enum VmError {
    StackUnderflow(usize),
    BadOffset(usize),
    Type(String),
    DivideByZero(usize),
    UnknownRoutine(u16),
    Host(String),
    /// A branch target that is not an instruction, or a jump out of the program.
    BadJump(u32),
    StepLimit,
}

impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmError::StackUnderflow(at) => write!(f, "stack underflow at {at:#x}"),
            VmError::BadOffset(at) => write!(f, "stack offset out of range at {at:#x}"),
            VmError::Type(m) => write!(f, "type error: {m}"),
            VmError::DivideByZero(at) => write!(f, "division by zero at {at:#x}"),
            VmError::UnknownRoutine(id) => write!(f, "unknown routine {id}"),
            VmError::Host(m) => write!(f, "{m}"),
            VmError::BadJump(t) => write!(f, "bad jump target {t:#x}"),
            VmError::StepLimit => write!(f, "step limit reached"),
        }
    }
}

impl std::error::Error for VmError {}

/// The game side of a script: implements the engine routines.
pub trait Host {
    /// Run `routine` for the script running on `caller`. `args` are the arguments the script pushed
    /// (fewer than the prototype when it relies on defaults), first parameter first. Return `None` for `void`.
    fn call(
        &mut self,
        caller: u32,
        routine: &Routine,
        args: &[Value],
    ) -> Result<Option<Value>, String>;
}

/// What a finished script left behind.
#[derive(Debug, PartialEq)]
pub struct Outcome {
    /// Cells left on the stack when the script returned. A `StartingConditional` or an int-returning
    /// script leaves its result in the first one.
    pub stack: Vec<Value>,
    pub steps: u64,
    /// Number of `STORESTATE` snapshots taken (deferred actions the host was handed).
    pub saved_states: u32,
}

impl Outcome {
    /// The script's integer result (`StartingConditional` or `int main()`), if it left one.
    pub fn int_result(&self) -> Option<i32> {
        match self.stack.first() {
            Some(Value::Int(i)) => Some(*i),
            _ => None,
        }
    }
}

pub struct Vm<'a> {
    program: &'a Program,
    routines: &'a Routines,
    index: HashMap<u32, usize>,
    stack: Vec<Value>,
    bp: usize,
    calls: Vec<usize>,
    saved_states: u32,
    /// Safety net against runaway scripts.
    pub step_limit: u64,
}

fn default_for(q: u8) -> Value {
    match q {
        0x04 => Value::Float(0.0),
        0x05 => Value::Str(String::new()),
        0x06 => Value::Object(OBJECT_INVALID),
        0x10..=0x1F => Value::Engine(q - 0x10, 0),
        _ => Value::Int(0),
    }
}

impl<'a> Vm<'a> {
    pub fn new(program: &'a Program, routines: &'a Routines) -> Vm<'a> {
        let index = program
            .instructions
            .iter()
            .enumerate()
            .map(|(i, ins)| (ins.offset, i))
            .collect();
        Vm {
            program,
            routines,
            index,
            stack: Vec::new(),
            bp: 0,
            calls: Vec::new(),
            saved_states: 0,
            step_limit: 2_000_000,
        }
    }

    fn pop(&mut self, at: u32) -> Result<Value, VmError> {
        self.stack.pop().ok_or(VmError::StackUnderflow(at as usize))
    }

    fn pop_int(&mut self, at: u32) -> Result<i32, VmError> {
        match self.pop(at)? {
            Value::Int(i) => Ok(i),
            other => Err(VmError::Type(format!(
                "expected int at {at:#x}, found {other:?}"
            ))),
        }
    }

    fn pop_float(&mut self, at: u32) -> Result<f32, VmError> {
        match self.pop(at)? {
            Value::Float(f) => Ok(f),
            other => Err(VmError::Type(format!(
                "expected float at {at:#x}, found {other:?}"
            ))),
        }
    }

    fn pop_vec(&mut self, at: u32) -> Result<[f32; 3], VmError> {
        let z = self.pop_float(at)?;
        let y = self.pop_float(at)?;
        let x = self.pop_float(at)?;
        Ok([x, y, z])
    }

    fn push_vec(&mut self, v: [f32; 3]) {
        self.stack.extend(v.map(Value::Float));
    }

    /// Cell index for a byte offset relative to `base` (negative offsets reach down into the stack).
    fn cell(&self, base: usize, off: i32, at: u32) -> Result<usize, VmError> {
        let idx = base as i64 + (off as i64).div_euclid(4);
        if idx < 0 || idx as usize > self.stack.len() {
            return Err(VmError::BadOffset(at as usize));
        }
        Ok(idx as usize)
    }

    /// Run the program from its first instruction until the outermost `RETN`.
    pub fn run(mut self, caller: u32, host: &mut dyn Host) -> Result<Outcome, VmError> {
        let mut pc = 0usize;
        let mut steps = 0u64;
        loop {
            steps += 1;
            if steps > self.step_limit {
                return Err(VmError::StepLimit);
            }
            let Some(ins) = self.program.instructions.get(pc) else {
                return Err(VmError::BadJump(0));
            };
            let at = ins.offset;
            let mut next = pc + 1;
            match self.exec(ins, caller, host, &mut next)? {
                Flow::Continue => {}
                Flow::Return => match self.calls.pop() {
                    Some(r) => next = r,
                    None => {
                        return Ok(Outcome {
                            stack: self.stack,
                            steps,
                            saved_states: self.saved_states,
                        })
                    }
                },
            }
            let _ = at;
            pc = next;
        }
    }

    fn jump(&self, target: u32) -> Result<usize, VmError> {
        self.index
            .get(&target)
            .copied()
            .ok_or(VmError::BadJump(target))
    }

    fn exec(
        &mut self,
        ins: &Instruction,
        caller: u32,
        host: &mut dyn Host,
        next: &mut usize,
    ) -> Result<Flow, VmError> {
        let at = ins.offset;
        let q = ins.qualifier;
        match &ins.op {
            Op::RsAdd => self.stack.push(default_for(q)),
            Op::ConstI(v) => self.stack.push(Value::Int(*v)),
            Op::ConstF(v) => self.stack.push(Value::Float(*v)),
            Op::ConstS(s) => self.stack.push(Value::Str(s.clone())),
            Op::ConstO(o) => self.stack.push(Value::Object(*o as u32)),
            Op::CpTopSp { offset, size } => {
                let n = (*size as usize) / 4;
                let start = self.cell(self.stack.len(), *offset, at)?;
                let copy: Vec<Value> = self
                    .stack
                    .get(start..start + n)
                    .ok_or(VmError::BadOffset(at as usize))?
                    .to_vec();
                self.stack.extend(copy);
            }
            Op::CpDownSp { offset, size } => {
                let n = (*size as usize) / 4;
                let len = self.stack.len();
                if n > len {
                    return Err(VmError::StackUnderflow(at as usize));
                }
                let dst = self.cell(len, *offset, at)?;
                if dst + n > len {
                    return Err(VmError::BadOffset(at as usize));
                }
                let src: Vec<Value> = self.stack[len - n..].to_vec();
                self.stack[dst..dst + n].clone_from_slice(&src);
            }
            Op::CpTopBp { offset, size } => {
                let n = (*size as usize) / 4;
                let start = self.cell(self.bp, *offset, at)?;
                let copy: Vec<Value> = self
                    .stack
                    .get(start..start + n)
                    .ok_or(VmError::BadOffset(at as usize))?
                    .to_vec();
                self.stack.extend(copy);
            }
            Op::CpDownBp { offset, size } => {
                let n = (*size as usize) / 4;
                let len = self.stack.len();
                if n > len {
                    return Err(VmError::StackUnderflow(at as usize));
                }
                let dst = self.cell(self.bp, *offset, at)?;
                if dst + n > len {
                    return Err(VmError::BadOffset(at as usize));
                }
                let src: Vec<Value> = self.stack[len - n..].to_vec();
                self.stack[dst..dst + n].clone_from_slice(&src);
            }
            Op::MovSp(off) => {
                if *off > 0 {
                    return Err(VmError::BadOffset(at as usize));
                }
                let n = (-*off as usize) / 4;
                if n > self.stack.len() {
                    return Err(VmError::StackUnderflow(at as usize));
                }
                self.stack.truncate(self.stack.len() - n);
            }
            Op::Destruct {
                size,
                keep_offset,
                keep_size,
            } => {
                let (total, off, keep) = (
                    *size as usize / 4,
                    *keep_offset as usize / 4,
                    *keep_size as usize / 4,
                );
                let len = self.stack.len();
                if total > len || off + keep > total {
                    return Err(VmError::StackUnderflow(at as usize));
                }
                let kept: Vec<Value> =
                    self.stack[len - total + off..len - total + off + keep].to_vec();
                self.stack.truncate(len - total);
                self.stack.extend(kept);
            }
            Op::IncISp(off) | Op::DecISp(off) | Op::IncIBp(off) | Op::DecIBp(off) => {
                let base = if matches!(ins.op, Op::IncIBp(_) | Op::DecIBp(_)) {
                    self.bp
                } else {
                    self.stack.len()
                };
                let i = self.cell(base, *off, at)?;
                let delta = if matches!(ins.op, Op::IncISp(_) | Op::IncIBp(_)) {
                    1
                } else {
                    -1
                };
                match self.stack.get_mut(i) {
                    Some(Value::Int(v)) => *v = v.wrapping_add(delta),
                    _ => return Err(VmError::Type(format!("inc/dec of a non-int at {at:#x}"))),
                }
            }
            Op::SaveBp => {
                // BP points at the saved-BP cell itself, just above the global variables.
                let old = self.bp as i32;
                self.bp = self.stack.len();
                self.stack.push(Value::Int(old));
            }
            Op::RestoreBp => {
                let old = self.pop_int(at)?;
                self.bp = old.max(0) as usize;
            }
            Op::StoreState { .. } | Op::StoreStateAll => self.saved_states += 1,
            Op::Nop => {}
            Op::Jmp(t) => *next = self.jump(*t)?,
            Op::Jsr(t) => {
                self.calls.push(*next);
                *next = self.jump(*t)?;
            }
            Op::Jz(t) => {
                if self.pop_int(at)? == 0 {
                    *next = self.jump(*t)?;
                }
            }
            Op::Jnz(t) => {
                if self.pop_int(at)? != 0 {
                    *next = self.jump(*t)?;
                }
            }
            Op::Retn => return Ok(Flow::Return),
            Op::Action { routine, args } => {
                self.action(*routine, *args as usize, caller, host, at)?
            }
            Op::Not => {
                let v = self.pop_int(at)?;
                self.stack.push(Value::Int((v == 0) as i32));
            }
            Op::Comp => {
                let v = self.pop_int(at)?;
                self.stack.push(Value::Int(!v));
            }
            Op::Neg => match self.pop(at)? {
                Value::Int(v) => self.stack.push(Value::Int(v.wrapping_neg())),
                Value::Float(v) => self.stack.push(Value::Float(-v)),
                other => return Err(VmError::Type(format!("cannot negate {other:?}"))),
            },
            Op::LogAnd
            | Op::LogOr
            | Op::BoolAnd
            | Op::IncOr
            | Op::ExcOr
            | Op::ShLeft
            | Op::ShRight
            | Op::UShRight => {
                let b = self.pop_int(at)?;
                let a = self.pop_int(at)?;
                let r = match ins.op {
                    Op::LogAnd | Op::BoolAnd => (a != 0 && b != 0) as i32,
                    Op::LogOr => (a != 0 || b != 0) as i32,
                    Op::IncOr => a | b,
                    Op::ExcOr => a ^ b,
                    Op::ShLeft => a.wrapping_shl(b as u32),
                    Op::ShRight => a.wrapping_shr(b as u32),
                    _ => ((a as u32).wrapping_shr(b as u32)) as i32,
                };
                self.stack.push(Value::Int(r));
            }
            Op::Equal { size } | Op::NEqual { size } => {
                let eq = self.equal(q, *size, at)?;
                let want = matches!(ins.op, Op::Equal { .. });
                self.stack.push(Value::Int((eq == want) as i32));
            }
            Op::Geq | Op::Gt | Op::Lt | Op::Leq => {
                let ord = match q {
                    0x20 => {
                        let b = self.pop_int(at)?;
                        let a = self.pop_int(at)?;
                        a.cmp(&b)
                    }
                    0x21 => {
                        let b = self.pop_float(at)?;
                        let a = self.pop_float(at)?;
                        a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal)
                    }
                    _ => {
                        return Err(VmError::Type(format!(
                            "comparison of qualifier {q:#x} at {at:#x}"
                        )))
                    }
                };
                let r = match ins.op {
                    Op::Geq => ord.is_ge(),
                    Op::Gt => ord.is_gt(),
                    Op::Lt => ord.is_lt(),
                    _ => ord.is_le(),
                };
                self.stack.push(Value::Int(r as i32));
            }
            Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Mod => self.arith(&ins.op, q, at)?,
            // Pushing a CPDOWNSP-style default for the remaining (obsolete) forms is not needed.
        }
        Ok(Flow::Continue)
    }

    fn equal(&mut self, q: u8, size: Option<u16>, at: u32) -> Result<bool, VmError> {
        Ok(match q {
            0x20 => {
                let b = self.pop_int(at)?;
                self.pop_int(at)? == b
            }
            0x21 => {
                let b = self.pop_float(at)?;
                self.pop_float(at)? == b
            }
            0x22 | 0x23 | 0x30..=0x3F => {
                let b = self.pop(at)?;
                self.pop(at)? == b
            }
            0x24 => {
                let n = size.unwrap_or(0) as usize / 4;
                let len = self.stack.len();
                if 2 * n > len {
                    return Err(VmError::StackUnderflow(at as usize));
                }
                let b: Vec<Value> = self.stack.split_off(len - n);
                let a: Vec<Value> = self.stack.split_off(len - 2 * n);
                a == b
            }
            _ => {
                return Err(VmError::Type(format!(
                    "equality of qualifier {q:#x} at {at:#x}"
                )))
            }
        })
    }

    fn arith(&mut self, op: &Op, q: u8, at: u32) -> Result<(), VmError> {
        let zero = || VmError::DivideByZero(at as usize);
        let bad = || VmError::Type(format!("{op:?} with qualifier {q:#x} at {at:#x}"));
        match q {
            0x20 => {
                let b = self.pop_int(at)?;
                let a = self.pop_int(at)?;
                let r = match op {
                    Op::Add => a.wrapping_add(b),
                    Op::Sub => a.wrapping_sub(b),
                    Op::Mul => a.wrapping_mul(b),
                    Op::Div => a.checked_div(b).ok_or_else(zero)?,
                    _ => a.checked_rem(b).ok_or_else(zero)?,
                };
                self.stack.push(Value::Int(r));
            }
            0x21 | 0x25 | 0x26 => {
                // FF, IF (int left), FI (int right): all computed in float.
                let b = if q == 0x26 {
                    self.pop_int(at)? as f32
                } else {
                    self.pop_float(at)?
                };
                let a = if q == 0x25 {
                    self.pop_int(at)? as f32
                } else {
                    self.pop_float(at)?
                };
                let r = match op {
                    Op::Add => a + b,
                    Op::Sub => a - b,
                    Op::Mul => a * b,
                    Op::Div if b == 0.0 => return Err(zero()),
                    Op::Div => a / b,
                    _ => return Err(bad()),
                };
                self.stack.push(Value::Float(r));
            }
            0x23 => {
                let (b, a) = (self.pop(at)?, self.pop(at)?);
                match (op, a, b) {
                    (Op::Add, Value::Str(a), Value::Str(b)) => self.stack.push(Value::Str(a + &b)),
                    _ => return Err(bad()),
                }
            }
            0x27 => {
                let b = self.pop_vec(at)?;
                let a = self.pop_vec(at)?;
                let r = match op {
                    Op::Add => [0, 1, 2].map(|i| a[i] + b[i]),
                    Op::Sub => [0, 1, 2].map(|i| a[i] - b[i]),
                    _ => return Err(bad()),
                };
                self.push_vec(r);
            }
            0x28 => {
                // vector (left) and float (right, on top)
                let f = self.pop_float(at)?;
                let v = self.pop_vec(at)?;
                match op {
                    Op::Mul => self.push_vec(v.map(|x| x * f)),
                    Op::Div if f == 0.0 => return Err(zero()),
                    Op::Div => self.push_vec(v.map(|x| x / f)),
                    _ => return Err(bad()),
                }
            }
            0x29 => {
                // float (left) and vector (right, on top)
                let v = self.pop_vec(at)?;
                let f = self.pop_float(at)?;
                match op {
                    Op::Mul => self.push_vec(v.map(|x| x * f)),
                    _ => return Err(bad()),
                }
            }
            _ => return Err(bad()),
        }
        Ok(())
    }

    fn action(
        &mut self,
        id: u16,
        argc: usize,
        caller: u32,
        host: &mut dyn Host,
        at: u32,
    ) -> Result<(), VmError> {
        let routine = self.routines.get(id).ok_or(VmError::UnknownRoutine(id))?;
        let mut args = Vec::with_capacity(argc);
        for p in routine.params.iter().take(argc) {
            let v = match p.ty {
                // A deferred action is not on the stack: it is the state saved by the last STORESTATE.
                Ty::Action => Value::Action(self.saved_states.saturating_sub(1)),
                Ty::Vector => Value::Vector(self.pop_vec(at)?),
                Ty::Void => continue,
                _ => self.pop(at)?,
            };
            args.push(v);
        }
        match (
            host.call(caller, routine, &args).map_err(VmError::Host)?,
            routine.ret,
        ) {
            (None, Ty::Void) => {}
            (Some(Value::Vector(v)), Ty::Vector) => self.push_vec(v),
            (Some(v), t) if t != Ty::Void && t != Ty::Vector => self.stack.push(v),
            (got, want) => {
                return Err(VmError::Host(format!(
                    "{} returned {got:?}, prototype says {want:?}",
                    routine.name
                )))
            }
        }
        Ok(())
    }
}

enum Flow {
    Continue,
    Return,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{disassemble, HeadlessHost};

    /// Routine table of 800 dummies, with `PrintString` at id 1 and `GetGlobalNumber` at id 2.
    fn routines() -> Routines {
        let mut src = String::new();
        for i in 0..800 {
            match i {
                1 => src.push_str("// 1: p\nvoid PrintString(string s);\n"),
                2 => src.push_str("// 2: g\nint GetGlobalNumber(string s);\n"),
                _ => src.push_str(&format!("// {i}: x\nint Fn{i}();\n")),
            }
        }
        Routines::parse(&src).unwrap()
    }

    fn ncs(body: &[u8]) -> Vec<u8> {
        let mut v = b"NCS V1.0B".to_vec();
        v.extend_from_slice(&((13 + body.len()) as u32).to_be_bytes());
        v.extend_from_slice(body);
        v
    }

    fn consti(v: i32) -> Vec<u8> {
        let mut b = vec![0x04, 0x03];
        b.extend_from_slice(&v.to_be_bytes());
        b
    }
    fn consts(s: &str) -> Vec<u8> {
        let mut b = vec![0x04, 0x05];
        b.extend_from_slice(&(s.len() as u16).to_be_bytes());
        b.extend_from_slice(s.as_bytes());
        b
    }
    fn op(code: u8, q: u8) -> Vec<u8> {
        vec![code, q]
    }
    fn cpdownsp(off: i32, size: u16) -> Vec<u8> {
        let mut b = vec![0x01, 0x01];
        b.extend_from_slice(&off.to_be_bytes());
        b.extend_from_slice(&size.to_be_bytes());
        b
    }
    fn movsp(off: i32) -> Vec<u8> {
        let mut b = vec![0x1B, 0x00];
        b.extend_from_slice(&off.to_be_bytes());
        b
    }
    fn action(id: u16, argc: u8) -> Vec<u8> {
        let mut b = vec![0x05, 0x00];
        b.extend_from_slice(&id.to_be_bytes());
        b.push(argc);
        b
    }

    /// `int main() { <body> }`-shaped program: return slot, call main, return; main runs `body` and returns.
    fn program(body: Vec<u8>) -> Program {
        // 13: RSADD.I (2) | 15: JSR +8 (6) | 21: RETN (2) | 23: main
        let mut b = op(0x02, 0x03);
        b.extend_from_slice(&[0x1E, 0x00, 0, 0, 0, 8]);
        b.extend(op(0x20, 0));
        b.extend(body);
        b.extend(op(0x20, 0));
        disassemble(&ncs(&b)).unwrap()
    }

    fn run(p: &Program, host: &mut HeadlessHost) -> Result<Outcome, VmError> {
        let r = routines();
        // Leak the table so the borrow outlives the helper; fine in tests.
        let r: &'static Routines = Box::leak(Box::new(r));
        Vm::new(p, r).run(OBJECT_SELF, host)
    }

    /// Store the int on top of the stack into the return slot (the cell below main's frame).
    fn store_result_and_pop() -> Vec<u8> {
        [cpdownsp(-8, 4), movsp(-4)].concat()
    }

    #[test]
    fn adds_two_ints_into_the_return_slot() {
        let body = [consti(2), consti(3), op(0x14, 0x20), store_result_and_pop()].concat();
        let out = run(&program(body), &mut HeadlessHost::default()).unwrap();
        assert_eq!(out.int_result(), Some(5));
        assert_eq!(
            out.stack.len(),
            1,
            "stack must be balanced: {:?}",
            out.stack
        );
    }

    #[test]
    fn division_and_comparison() {
        // (7 / 2 == 3) && (7 % 2 == 1)
        let body = [
            consti(7),
            consti(2),
            op(0x17, 0x20),
            consti(3),
            op(0x0B, 0x20),
            consti(7),
            consti(2),
            op(0x18, 0x20),
            consti(1),
            op(0x0B, 0x20),
            op(0x06, 0x20),
            store_result_and_pop(),
        ]
        .concat();
        let out = run(&program(body), &mut HeadlessHost::default()).unwrap();
        assert_eq!(out.int_result(), Some(1));
    }

    #[test]
    fn branch_skips_code() {
        // if (0) result = 9; else result = 4;   JZ jumps to the else part.
        // layout from main (offset 23): CONSTI 0 (6) | JZ +20 | CONSTI 9 (6) | JMP +14 | CONSTI 4 (6) | store...
        let mut then = consti(9);
        then.extend(store_result_and_pop());
        let mut els = consti(4);
        els.extend(store_result_and_pop());
        let then_len = then.len() as i32 + 6; // plus the JMP over the else part
        let mut body = consti(0);
        let mut jz = vec![0x1F, 0x00];
        jz.extend_from_slice(&(6 + then_len).to_be_bytes());
        body.extend(jz);
        body.extend(then);
        let mut jmp = vec![0x1D, 0x00];
        jmp.extend_from_slice(&(6 + els.len() as i32).to_be_bytes());
        body.extend(jmp);
        body.extend(els);
        let out = run(&program(body), &mut HeadlessHost::default()).unwrap();
        assert_eq!(out.int_result(), Some(4));
    }

    #[test]
    fn strings_concatenate_and_actions_reach_the_host() {
        let body = [
            consts("a"),
            consts("b"),
            op(0x14, 0x23),
            action(1, 1),
            consti(1),
            store_result_and_pop(),
        ]
        .concat();
        let mut host = HeadlessHost::default();
        let out = run(&program(body), &mut host).unwrap();
        assert_eq!(host.log, ["ab"]);
        assert_eq!(out.int_result(), Some(1));
    }

    #[test]
    fn errors_are_reported_not_panics() {
        let div0 = [consti(1), consti(0), op(0x17, 0x20)].concat();
        assert!(matches!(
            run(&program(div0), &mut HeadlessHost::default()),
            Err(VmError::DivideByZero(_))
        ));
        let underflow = op(0x14, 0x20);
        assert!(matches!(
            run(&program(underflow), &mut HeadlessHost::default()),
            Err(VmError::StackUnderflow(_)) | Err(VmError::Type(_))
        ));
        let wrong_type = [consts("x"), consti(1), op(0x14, 0x20)].concat();
        assert!(matches!(
            run(&program(wrong_type), &mut HeadlessHost::default()),
            Err(VmError::Type(_))
        ));
        let unknown = action(799, 0);
        assert!(matches!(
            run(&program(unknown), &mut HeadlessHost::default()),
            Err(VmError::Host(_))
        ));
        // An endless loop hits the step limit instead of hanging: JMP to itself.
        let mut spin = vec![0x1D, 0x00];
        spin.extend_from_slice(&0i32.to_be_bytes());
        assert!(matches!(
            run(&program(spin), &mut HeadlessHost::default()),
            Err(VmError::StepLimit)
        ));
    }
}
