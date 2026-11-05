use kotor_core::{latin1, Error, Reader, Result};
use std::fmt;

/// Decoded operation with its operands. Branch targets are absolute offsets.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    CpDownSp { offset: i32, size: u16 },
    RsAdd,
    CpTopSp { offset: i32, size: u16 },
    ConstI(i32),
    ConstF(f32),
    ConstS(String),
    ConstO(i32),
    Action { routine: u16, args: u8 },
    LogAnd,
    LogOr,
    IncOr,
    ExcOr,
    BoolAnd,
    Equal { size: Option<u16> },
    NEqual { size: Option<u16> },
    Geq,
    Gt,
    Lt,
    Leq,
    ShLeft,
    ShRight,
    UShRight,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Neg,
    Comp,
    MovSp(i32),
    StoreStateAll,
    Jmp(u32),
    Jsr(u32),
    Jz(u32),
    Jnz(u32),
    Retn,
    Destruct { size: u16, keep_offset: i16, keep_size: u16 },
    Not,
    DecISp(i32),
    IncISp(i32),
    CpDownBp { offset: i32, size: u16 },
    CpTopBp { offset: i32, size: u16 },
    DecIBp(i32),
    IncIBp(i32),
    SaveBp,
    RestoreBp,
    StoreState { bp_size: u32, sp_size: u32 },
    Nop,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Instruction {
    pub offset: u32,
    pub len: u32,
    /// The type qualifier byte (e.g. 3 = int, 0x20 = int,int).
    pub qualifier: u8,
    pub op: Op,
}

#[derive(Clone, Debug)]
pub struct Program {
    pub instructions: Vec<Instruction>,
    pub size: u32,
}

fn type_name(q: u8) -> String {
    match q {
        0x00 => String::new(),
        0x01 => "stack".into(),
        0x03 => "I".into(),
        0x04 => "F".into(),
        0x05 => "S".into(),
        0x06 => "O".into(),
        0x10..=0x1F => format!("E{}", q - 0x10),
        0x20 => "II".into(),
        0x21 => "FF".into(),
        0x22 => "OO".into(),
        0x23 => "SS".into(),
        0x24 => "TT".into(),
        0x25 => "IF".into(),
        0x26 => "FI".into(),
        0x27 => "VV".into(),
        0x28 => "VF".into(),
        0x29 => "FV".into(),
        0x30..=0x3F => format!("E{0}E{0}", q - 0x30),
        _ => format!("?{q:#04x}"),
    }
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let t = type_name(self.qualifier);
        let sfx = if t.is_empty() { String::new() } else { format!(".{t}") };
        write!(f, "{:08x}  ", self.offset)?;
        match &self.op {
            Op::CpDownSp { offset, size } => write!(f, "CPDOWNSP{sfx} {offset}, {size}"),
            Op::RsAdd => write!(f, "RSADD{sfx}"),
            Op::CpTopSp { offset, size } => write!(f, "CPTOPSP{sfx} {offset}, {size}"),
            Op::ConstI(v) => write!(f, "CONST.I {v}"),
            Op::ConstF(v) => write!(f, "CONST.F {v}"),
            Op::ConstS(v) => write!(f, "CONST.S {v:?}"),
            Op::ConstO(v) => write!(f, "CONST.O {v:#x}"),
            Op::Action { routine, args } => write!(f, "ACTION {routine} ({args} args)"),
            Op::LogAnd => write!(f, "LOGAND{sfx}"),
            Op::LogOr => write!(f, "LOGOR{sfx}"),
            Op::IncOr => write!(f, "INCOR{sfx}"),
            Op::ExcOr => write!(f, "EXCOR{sfx}"),
            Op::BoolAnd => write!(f, "BOOLAND{sfx}"),
            Op::Equal { size } => write!(f, "EQUAL{sfx}{}", size.map(|s| format!(" {s}")).unwrap_or_default()),
            Op::NEqual { size } => write!(f, "NEQUAL{sfx}{}", size.map(|s| format!(" {s}")).unwrap_or_default()),
            Op::Geq => write!(f, "GEQ{sfx}"),
            Op::Gt => write!(f, "GT{sfx}"),
            Op::Lt => write!(f, "LT{sfx}"),
            Op::Leq => write!(f, "LEQ{sfx}"),
            Op::ShLeft => write!(f, "SHLEFT{sfx}"),
            Op::ShRight => write!(f, "SHRIGHT{sfx}"),
            Op::UShRight => write!(f, "USHRIGHT{sfx}"),
            Op::Add => write!(f, "ADD{sfx}"),
            Op::Sub => write!(f, "SUB{sfx}"),
            Op::Mul => write!(f, "MUL{sfx}"),
            Op::Div => write!(f, "DIV{sfx}"),
            Op::Mod => write!(f, "MOD{sfx}"),
            Op::Neg => write!(f, "NEG{sfx}"),
            Op::Comp => write!(f, "COMP{sfx}"),
            Op::MovSp(v) => write!(f, "MOVSP {v}"),
            Op::StoreStateAll => write!(f, "STORESTATEALL"),
            Op::Jmp(t) => write!(f, "JMP {t:08x}"),
            Op::Jsr(t) => write!(f, "JSR {t:08x}"),
            Op::Jz(t) => write!(f, "JZ {t:08x}"),
            Op::Jnz(t) => write!(f, "JNZ {t:08x}"),
            Op::Retn => write!(f, "RETN"),
            Op::Destruct { size, keep_offset, keep_size } => {
                write!(f, "DESTRUCT {size}, {keep_offset}, {keep_size}")
            }
            Op::Not => write!(f, "NOT{sfx}"),
            Op::DecISp(v) => write!(f, "DECISP{sfx} {v}"),
            Op::IncISp(v) => write!(f, "INCISP{sfx} {v}"),
            Op::CpDownBp { offset, size } => write!(f, "CPDOWNBP{sfx} {offset}, {size}"),
            Op::CpTopBp { offset, size } => write!(f, "CPTOPBP{sfx} {offset}, {size}"),
            Op::DecIBp(v) => write!(f, "DECIBP{sfx} {v}"),
            Op::IncIBp(v) => write!(f, "INCIBP{sfx} {v}"),
            Op::SaveBp => write!(f, "SAVEBP"),
            Op::RestoreBp => write!(f, "RESTOREBP"),
            Op::StoreState { bp_size, sp_size } => write!(f, "STORESTATE {bp_size}, {sp_size}"),
            Op::Nop => write!(f, "NOP"),
        }
    }
}

fn target(at: u32, rel: i32) -> Result<u32> {
    u32::try_from(at as i64 + rel as i64).map_err(|_| Error::format("branch target before start of program"))
}

/// Decode a whole NCS file into instructions.
pub fn disassemble(data: &[u8]) -> Result<Program> {
    let mut r = Reader::new(data);
    let tag = r.tag()?;
    let ver = r.tag()?;
    if &tag != b"NCS " || &ver != b"V1.0" {
        return Err(Error::format(format!("not an NCS V1.0 file: {tag:?} {ver:?}")));
    }
    let marker = r.u8()?;
    if marker != 0x42 {
        return Err(Error::format(format!("unexpected NCS size marker {marker:#x}")));
    }
    let size = r.u32_be()?;
    if size as usize != data.len() {
        return Err(Error::format(format!("NCS header says {size} bytes, file is {}", data.len())));
    }
    let mut instructions = Vec::new();
    while r.pos() < data.len() {
        let start = r.pos();
        let opcode = r.u8()?;
        let q = r.u8()?;
        let op = match opcode {
            0x01 => Op::CpDownSp { offset: r.i32_be()?, size: r.u16_be()? },
            0x02 => Op::RsAdd,
            0x03 => Op::CpTopSp { offset: r.i32_be()?, size: r.u16_be()? },
            0x04 => match q {
                0x03 => Op::ConstI(r.i32_be()?),
                0x04 => Op::ConstF(r.f32_be()?),
                0x05 => {
                    let n = r.u16_be()? as usize;
                    Op::ConstS(latin1(r.bytes(n)?))
                }
                0x06 => Op::ConstO(r.i32_be()?),
                _ => return Err(Error::format(format!("CONST of type {q:#x} at {start:#x}"))),
            },
            0x05 => Op::Action { routine: r.u16_be()?, args: r.u8()? },
            0x06 => Op::LogAnd,
            0x07 => Op::LogOr,
            0x08 => Op::IncOr,
            0x09 => Op::ExcOr,
            0x0A => Op::BoolAnd,
            0x0B => Op::Equal { size: (q == 0x24).then(|| r.u16_be()).transpose()? },
            0x0C => Op::NEqual { size: (q == 0x24).then(|| r.u16_be()).transpose()? },
            0x0D => Op::Geq,
            0x0E => Op::Gt,
            0x0F => Op::Lt,
            0x10 => Op::Leq,
            0x11 => Op::ShLeft,
            0x12 => Op::ShRight,
            0x13 => Op::UShRight,
            0x14 => Op::Add,
            0x15 => Op::Sub,
            0x16 => Op::Mul,
            0x17 => Op::Div,
            0x18 => Op::Mod,
            0x19 => Op::Neg,
            0x1A => Op::Comp,
            0x1B => Op::MovSp(r.i32_be()?),
            0x1C => Op::StoreStateAll,
            0x1D => Op::Jmp(target(start as u32, r.i32_be()?)?),
            0x1E => Op::Jsr(target(start as u32, r.i32_be()?)?),
            0x1F => Op::Jz(target(start as u32, r.i32_be()?)?),
            0x20 => Op::Retn,
            0x21 => Op::Destruct { size: r.u16_be()?, keep_offset: r.i16_be()?, keep_size: r.u16_be()? },
            0x22 => Op::Not,
            0x23 => Op::DecISp(r.i32_be()?),
            0x24 => Op::IncISp(r.i32_be()?),
            0x25 => Op::Jnz(target(start as u32, r.i32_be()?)?),
            0x26 => Op::CpDownBp { offset: r.i32_be()?, size: r.u16_be()? },
            0x27 => Op::CpTopBp { offset: r.i32_be()?, size: r.u16_be()? },
            0x28 => Op::DecIBp(r.i32_be()?),
            0x29 => Op::IncIBp(r.i32_be()?),
            0x2A => Op::SaveBp,
            0x2B => Op::RestoreBp,
            0x2C => Op::StoreState { bp_size: r.u32_be()?, sp_size: r.u32_be()? },
            0x2D => Op::Nop,
            other => {
                return Err(Error::format(format!("unknown opcode {other:#04x} at {start:#x}")));
            }
        };
        instructions.push(Instruction {
            offset: start as u32,
            len: (r.pos() - start) as u32,
            qualifier: q,
            op,
        });
    }
    Ok(Program { instructions, size })
}

impl Program {
    /// Check every branch lands on the start of an instruction.
    pub fn check_branches(&self) -> Result<()> {
        let starts: std::collections::HashSet<u32> = self.instructions.iter().map(|i| i.offset).collect();
        for i in &self.instructions {
            if let Op::Jmp(t) | Op::Jsr(t) | Op::Jz(t) | Op::Jnz(t) = i.op {
                if !starts.contains(&t) {
                    return Err(Error::format(format!("branch at {:#x} to {t:#x} is not an instruction", i.offset)));
                }
            }
        }
        Ok(())
    }

    pub fn listing(&self) -> String {
        self.instructions.iter().map(|i| format!("{i}\n")).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrap(body: &[u8]) -> Vec<u8> {
        let mut v = b"NCS V1.0B".to_vec();
        v.extend_from_slice(&((13 + body.len()) as u32).to_be_bytes());
        v.extend_from_slice(body);
        v
    }

    #[test]
    fn decodes_const_and_jump() {
        // CONST.I 5 ; JMP +6 (to RETN) ; RETN
        let body = [0x04, 0x03, 0, 0, 0, 5, 0x1D, 0x00, 0, 0, 0, 6, 0x20, 0x00];
        let p = disassemble(&wrap(&body)).unwrap();
        assert_eq!(p.instructions[0].op, Op::ConstI(5));
        assert_eq!(p.instructions[1].op, Op::Jmp(13 + 6 + 6));
        p.check_branches().unwrap();
        assert!(p.listing().contains("CONST.I 5"));
    }

    #[test]
    fn rejects_bad_size_and_opcode() {
        let mut v = wrap(&[0x20, 0x00]);
        v.push(0);
        assert!(disassemble(&v).is_err());
        assert!(disassemble(&wrap(&[0xEE, 0x00])).is_err());
    }

    #[test]
    fn truncated_never_panics() {
        let v = wrap(&[0x04, 0x05, 0, 3, b'a', b'b', b'c', 0x20, 0x00]);
        for n in 0..v.len() {
            let _ = disassemble(&v[..n]);
        }
    }
}
