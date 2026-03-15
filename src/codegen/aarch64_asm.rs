//! ARM64 (AArch64) instruction encoder for bare-metal inline assembly.
//!
//! Encodes ARM64 instructions as raw 32-bit words for AOT compilation.
//! Used when `asm!()` contains ARM64-specific instructions that cannot
//! be lowered to Cranelift IR (e.g., `mrs`, `msr`, `isb`, `eret`).

use crate::codegen::CodegenError;

// ═══════════════════════════════════════════════════════════════════════
// System register constants
// ═══════════════════════════════════════════════════════════════════════

/// System register encoding: (op0, op1, CRn, CRm, op2)
pub fn sysreg_encoding(name: &str) -> Option<(u32, u32, u32, u32, u32)> {
    match name.to_lowercase().as_str() {
        // EL1 control registers
        "sctlr_el1" => Some((3, 0, 1, 0, 0)),
        "tcr_el1" => Some((3, 0, 2, 0, 2)),
        "mair_el1" => Some((3, 0, 10, 2, 0)),
        "ttbr0_el1" => Some((3, 0, 2, 0, 0)),
        "ttbr1_el1" => Some((3, 0, 2, 0, 1)),
        "vbar_el1" => Some((3, 0, 12, 0, 0)),
        "esr_el1" => Some((3, 0, 5, 2, 0)),
        "far_el1" => Some((3, 0, 6, 0, 0)),
        "elr_el1" => Some((3, 0, 4, 0, 1)),
        "spsr_el1" => Some((3, 0, 4, 0, 0)),
        "sp_el0" => Some((3, 0, 4, 1, 0)),
        "daif" => Some((3, 3, 4, 2, 1)),
        "currentel" => Some((3, 0, 4, 2, 2)),
        // GIC (interrupt controller)
        "icc_sre_el1" => Some((3, 0, 12, 12, 5)),
        "icc_pmr_el1" => Some((3, 0, 4, 6, 0)),
        "icc_iar1_el1" => Some((3, 0, 12, 12, 0)),
        "icc_eoir1_el1" => Some((3, 0, 12, 12, 1)),
        "icc_ctrl_el1" => Some((3, 0, 12, 12, 4)),
        // Timers
        "cntfrq_el0" => Some((3, 3, 14, 0, 0)),
        "cntp_tval_el0" => Some((3, 3, 14, 2, 0)),
        "cntp_ctl_el0" => Some((3, 3, 14, 2, 1)),
        _ => None,
    }
}

/// Maps a register name to its 5-bit number.
pub fn reg_number(name: &str) -> Option<u32> {
    let name = name.to_lowercase();
    // Check special names first (xzr starts with 'x' but isn't x0-x30)
    match name.as_str() {
        "sp" | "xzr" | "wzr" => return Some(31),
        "lr" => return Some(30),
        _ => {}
    }
    if let Some(n) = name.strip_prefix('x') {
        n.parse::<u32>().ok().filter(|&n| n <= 30)
    } else if let Some(n) = name.strip_prefix('w') {
        n.parse::<u32>().ok().filter(|&n| n <= 30)
    } else {
        None
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Instruction encoders
// ═══════════════════════════════════════════════════════════════════════

/// Encodes `mrs Xd, <sysreg>` → reads system register into Xd.
pub fn encode_mrs(rd: u32, sysreg: &str) -> Result<u32, CodegenError> {
    let (op0, op1, crn, crm, op2) = sysreg_encoding(sysreg).ok_or_else(|| {
        CodegenError::NotImplemented(format!("unknown ARM64 system register: {sysreg}"))
    })?;
    // MRS: 1101_0101_0011_<op0-2><op1><CRn><CRm><op2><Rt>
    Ok(0xD5300000 | ((op0 & 1) << 19) | (op1 << 16) | (crn << 12) | (crm << 8) | (op2 << 5) | rd)
}

/// Encodes `msr <sysreg>, Xt` → writes Xt to system register.
pub fn encode_msr(sysreg: &str, rt: u32) -> Result<u32, CodegenError> {
    let (op0, op1, crn, crm, op2) = sysreg_encoding(sysreg).ok_or_else(|| {
        CodegenError::NotImplemented(format!("unknown ARM64 system register: {sysreg}"))
    })?;
    // MSR: 1101_0101_0001_<op0-2><op1><CRn><CRm><op2><Rt>
    Ok(0xD5100000 | ((op0 & 1) << 19) | (op1 << 16) | (crn << 12) | (crm << 8) | (op2 << 5) | rt)
}

/// Encodes barrier instructions.
pub fn encode_barrier(kind: &str) -> Result<u32, CodegenError> {
    match kind.to_lowercase().as_str() {
        "isb" => Ok(0xD5033FDF),            // ISB SY
        "dsb sy" | "dsb" => Ok(0xD503309F), // DSB SY
        "dmb sy" | "dmb" => Ok(0xD50330BF), // DMB SY
        "dsb ish" => Ok(0xD5033B9F),        // DSB ISH
        "dmb ish" => Ok(0xD5033B9F),        // DMB ISH (same encoding pattern)
        _ => Err(CodegenError::NotImplemented(format!(
            "unknown barrier: {kind}"
        ))),
    }
}

/// Encodes exception/control instructions.
pub fn encode_exception(mnemonic: &str) -> Result<u32, CodegenError> {
    match mnemonic.to_lowercase().as_str() {
        "eret" => Ok(0xD69F03E0),
        "wfi" => Ok(0xD503207F),
        "wfe" => Ok(0xD503205F),
        "nop" => Ok(0xD503201F),
        "ret" => Ok(0xD65F03C0), // RET x30
        _ => Err(CodegenError::NotImplemented(format!(
            "unknown exception instruction: {mnemonic}"
        ))),
    }
}

/// Encodes `svc #imm16` → supervisor call.
pub fn encode_svc(imm: u32) -> u32 {
    0xD4000001 | ((imm & 0xFFFF) << 5)
}

/// Encodes `mov Xd, #imm16` (MOVZ, shift=0).
pub fn encode_movz(rd: u32, imm16: u32) -> u32 {
    // MOVZ: 1_10_100101_00_<imm16>_<Rd>
    0xD2800000 | ((imm16 & 0xFFFF) << 5) | rd
}

/// Encodes `movk Xd, #imm16, lsl #shift` (MOVK).
pub fn encode_movk(rd: u32, imm16: u32, shift: u32) -> u32 {
    let hw = (shift / 16) & 3;
    0xF2800000 | (hw << 21) | ((imm16 & 0xFFFF) << 5) | rd
}

/// Encodes `ldr Xt, [Xn, #imm12]` (unsigned offset, 8-byte aligned).
pub fn encode_ldr(rt: u32, rn: u32, imm12: u32) -> u32 {
    // LDR (immediate, unsigned offset): 11_111_00_110_<imm12>_<Rn>_<Rt>
    let scaled = (imm12 / 8) & 0xFFF;
    0xF9400000 | (scaled << 10) | (rn << 5) | rt
}

/// Encodes `str Xt, [Xn, #imm12]` (unsigned offset, 8-byte aligned).
pub fn encode_str(rt: u32, rn: u32, imm12: u32) -> u32 {
    // STR (immediate, unsigned offset): 11_111_00_100_<imm12>_<Rn>_<Rt>
    let scaled = (imm12 / 8) & 0xFFF;
    0xF9000000 | (scaled << 10) | (rn << 5) | rt
}

/// Encodes `br Xn` → branch to register.
pub fn encode_br(rn: u32) -> u32 {
    0xD61F0000 | (rn << 5)
}

/// Encodes `blr Xn` → branch with link to register.
pub fn encode_blr(rn: u32) -> u32 {
    0xD63F0000 | (rn << 5)
}

/// Encodes `ret Xn` → return to address in Xn (default x30).
pub fn encode_ret(rn: u32) -> u32 {
    0xD65F0000 | (rn << 5)
}

/// Encodes TLBI operations.
pub fn encode_tlbi(op: &str, rt: u32) -> Result<u32, CodegenError> {
    match op.to_lowercase().as_str() {
        "alle1" => Ok(0xD508871F),
        "vae1" => Ok(0xD5088760 | rt),
        "vmalle1" => Ok(0xD508871F),
        _ => Err(CodegenError::NotImplemented(format!(
            "unknown TLBI operation: {op}"
        ))),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// High-level assembly encoding
// ═══════════════════════════════════════════════════════════════════════

/// List of ARM64-specific mnemonics (not representable in Cranelift IR).
pub const ARM64_SPECIFIC: &[&str] = &[
    "mrs", "msr", "ldr", "str", "stp", "ldp", "eret", "svc", "wfi", "wfe", "isb", "dsb", "dmb",
    "tlbi", "at", "dc", "ic", "movz", "movk", "br", "blr", "b", "cbz", "cbnz", "adr", "adrp",
    "ret",
];

/// Returns true if the mnemonic is ARM64-specific (cannot be lowered to Cranelift IR).
pub fn is_arm64_specific(mnemonic: &str) -> bool {
    ARM64_SPECIFIC.contains(&mnemonic.to_lowercase().as_str())
}

/// Encodes a single ARM64 instruction from its mnemonic and operands.
///
/// Returns the 32-bit instruction word.
pub fn encode_instruction(mnemonic: &str, operands: &[&str]) -> Result<u32, CodegenError> {
    match mnemonic.to_lowercase().as_str() {
        "mrs" => {
            if operands.len() != 2 {
                return Err(CodegenError::NotImplemented(
                    "mrs requires 2 operands: mrs Xd, sysreg".into(),
                ));
            }
            let rd = reg_number(operands[0]).ok_or_else(|| {
                CodegenError::NotImplemented(format!("invalid register: {}", operands[0]))
            })?;
            encode_mrs(rd, operands[1])
        }
        "msr" => {
            if operands.len() != 2 {
                return Err(CodegenError::NotImplemented(
                    "msr requires 2 operands: msr sysreg, Xt".into(),
                ));
            }
            let rt = reg_number(operands[1]).ok_or_else(|| {
                CodegenError::NotImplemented(format!("invalid register: {}", operands[1]))
            })?;
            encode_msr(operands[0], rt)
        }
        "isb" | "dsb" | "dmb" => encode_barrier(mnemonic),
        "eret" | "wfi" | "wfe" | "nop" | "ret" => encode_exception(mnemonic),
        "svc" => {
            if operands.is_empty() {
                return Err(CodegenError::NotImplemented(
                    "svc requires immediate operand".into(),
                ));
            }
            let imm = operands[0]
                .trim_start_matches('#')
                .parse::<u32>()
                .map_err(|_| {
                    CodegenError::NotImplemented(format!("invalid svc immediate: {}", operands[0]))
                })?;
            Ok(encode_svc(imm))
        }
        "ldr" => {
            if operands.len() < 2 {
                return Err(CodegenError::NotImplemented(
                    "ldr requires at least 2 operands".into(),
                ));
            }
            let rt = reg_number(operands[0]).ok_or_else(|| {
                CodegenError::NotImplemented(format!("invalid register: {}", operands[0]))
            })?;
            // Parse [Xn, #imm] or [Xn]
            let (rn, imm) = parse_mem_operand(operands[1])?;
            Ok(encode_ldr(rt, rn, imm))
        }
        "str" => {
            if operands.len() < 2 {
                return Err(CodegenError::NotImplemented(
                    "str requires at least 2 operands".into(),
                ));
            }
            let rt = reg_number(operands[0]).ok_or_else(|| {
                CodegenError::NotImplemented(format!("invalid register: {}", operands[0]))
            })?;
            let (rn, imm) = parse_mem_operand(operands[1])?;
            Ok(encode_str(rt, rn, imm))
        }
        "br" => {
            if operands.is_empty() {
                return Err(CodegenError::NotImplemented(
                    "br requires register operand".into(),
                ));
            }
            let rn = reg_number(operands[0]).ok_or_else(|| {
                CodegenError::NotImplemented(format!("invalid register: {}", operands[0]))
            })?;
            Ok(encode_br(rn))
        }
        "blr" => {
            if operands.is_empty() {
                return Err(CodegenError::NotImplemented(
                    "blr requires register operand".into(),
                ));
            }
            let rn = reg_number(operands[0]).ok_or_else(|| {
                CodegenError::NotImplemented(format!("invalid register: {}", operands[0]))
            })?;
            Ok(encode_blr(rn))
        }
        "movz" | "mov" => {
            if operands.len() < 2 {
                return Err(CodegenError::NotImplemented(
                    "movz requires Xd, #imm16".into(),
                ));
            }
            let rd = reg_number(operands[0]).ok_or_else(|| {
                CodegenError::NotImplemented(format!("invalid register: {}", operands[0]))
            })?;
            let imm = parse_immediate(operands[1])?;
            Ok(encode_movz(rd, imm))
        }
        "movk" => {
            if operands.len() < 2 {
                return Err(CodegenError::NotImplemented(
                    "movk requires Xd, #imm16 [, lsl #shift]".into(),
                ));
            }
            let rd = reg_number(operands[0]).ok_or_else(|| {
                CodegenError::NotImplemented(format!("invalid register: {}", operands[0]))
            })?;
            let imm = parse_immediate(operands[1])?;
            let shift = if operands.len() > 2 {
                parse_shift(operands[2])?
            } else {
                0
            };
            Ok(encode_movk(rd, imm, shift))
        }
        "tlbi" => {
            if operands.is_empty() {
                return Err(CodegenError::NotImplemented(
                    "tlbi requires operation name".into(),
                ));
            }
            let rt = if operands.len() > 1 {
                reg_number(operands[1]).unwrap_or(31)
            } else {
                31
            };
            encode_tlbi(operands[0], rt)
        }
        _ => Err(CodegenError::NotImplemented(format!(
            "ARM64 instruction not yet supported: {mnemonic}"
        ))),
    }
}

/// Parses `[Xn, #imm]` or `[Xn]` memory operand.
fn parse_mem_operand(s: &str) -> Result<(u32, u32), CodegenError> {
    let s = s.trim().trim_matches(|c| c == '[' || c == ']');
    let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
    let rn = reg_number(parts[0]).ok_or_else(|| {
        CodegenError::NotImplemented(format!("invalid base register: {}", parts[0]))
    })?;
    let imm = if parts.len() > 1 {
        parse_immediate(parts[1])?
    } else {
        0
    };
    Ok((rn, imm))
}

/// Parses `#imm` or `0x...` immediate value.
fn parse_immediate(s: &str) -> Result<u32, CodegenError> {
    let s = s.trim().trim_start_matches('#');
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16)
            .map_err(|_| CodegenError::NotImplemented(format!("invalid hex immediate: {s}")))
    } else {
        s.parse::<u32>()
            .map_err(|_| CodegenError::NotImplemented(format!("invalid immediate: {s}")))
    }
}

/// Parses `lsl #shift` shift specifier.
fn parse_shift(s: &str) -> Result<u32, CodegenError> {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("lsl") {
        let rest = rest.trim().trim_start_matches('#');
        rest.parse::<u32>()
            .map_err(|_| CodegenError::NotImplemented(format!("invalid shift: {s}")))
    } else {
        Err(CodegenError::NotImplemented(format!(
            "unsupported shift: {s}"
        )))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_isb() {
        assert_eq!(encode_barrier("isb").unwrap(), 0xD5033FDF);
    }

    #[test]
    fn encode_dsb_sy() {
        assert_eq!(encode_barrier("dsb").unwrap(), 0xD503309F);
    }

    #[test]
    fn encode_dmb_sy() {
        assert_eq!(encode_barrier("dmb").unwrap(), 0xD50330BF);
    }

    #[test]
    fn encode_eret_instr() {
        assert_eq!(encode_exception("eret").unwrap(), 0xD69F03E0);
    }

    #[test]
    fn encode_wfi_instr() {
        assert_eq!(encode_exception("wfi").unwrap(), 0xD503207F);
    }

    #[test]
    fn encode_wfe_instr() {
        assert_eq!(encode_exception("wfe").unwrap(), 0xD503205F);
    }

    #[test]
    fn encode_nop_instr() {
        assert_eq!(encode_exception("nop").unwrap(), 0xD503201F);
    }

    #[test]
    fn encode_ret_x30() {
        assert_eq!(encode_exception("ret").unwrap(), 0xD65F03C0);
    }

    #[test]
    fn encode_svc_0() {
        assert_eq!(encode_svc(0), 0xD4000001);
    }

    #[test]
    fn encode_svc_1() {
        assert_eq!(encode_svc(1), 0xD4000021);
    }

    #[test]
    fn encode_movz_x0_42() {
        let word = encode_movz(0, 42);
        // Check Rd=0 and imm16=42
        assert_eq!(word & 0x1F, 0); // Rd = x0
        assert_eq!((word >> 5) & 0xFFFF, 42); // imm16 = 42
    }

    #[test]
    fn encode_br_x8() {
        assert_eq!(encode_br(8), 0xD61F0000 | (8 << 5));
    }

    #[test]
    fn encode_blr_x30() {
        assert_eq!(encode_blr(30), 0xD63F0000 | (30 << 5));
    }

    #[test]
    fn encode_ret_x0() {
        assert_eq!(encode_ret(0), 0xD65F0000);
    }

    #[test]
    fn reg_number_x0() {
        assert_eq!(reg_number("x0"), Some(0));
        assert_eq!(reg_number("x30"), Some(30));
        assert_eq!(reg_number("sp"), Some(31));
        assert_eq!(reg_number("xzr"), Some(31));
        assert_eq!(reg_number("lr"), Some(30));
        assert_eq!(reg_number("x31"), None); // x31 doesn't exist
        assert_eq!(reg_number("w15"), Some(15));
    }

    #[test]
    fn sysreg_sctlr_el1() {
        assert!(sysreg_encoding("sctlr_el1").is_some());
        assert!(sysreg_encoding("SCTLR_EL1").is_some()); // Case insensitive
    }

    #[test]
    fn encode_mrs_sctlr() {
        let word = encode_mrs(0, "sctlr_el1").unwrap();
        // Should be an MRS instruction
        assert_eq!(word >> 20, 0xD53 >> 0); // Top bits
        assert_eq!(word & 0x1F, 0); // Rd = x0
    }

    #[test]
    fn encode_instruction_eret() {
        assert_eq!(encode_instruction("eret", &[]).unwrap(), 0xD69F03E0);
    }

    #[test]
    fn encode_instruction_mrs() {
        let word = encode_instruction("mrs", &["x5", "sctlr_el1"]).unwrap();
        assert_eq!(word & 0x1F, 5); // Rd = x5
    }

    #[test]
    fn encode_instruction_svc() {
        let word = encode_instruction("svc", &["#0"]).unwrap();
        assert_eq!(word, 0xD4000001);
    }

    #[test]
    fn encode_instruction_wfi() {
        assert_eq!(encode_instruction("wfi", &[]).unwrap(), 0xD503207F);
    }

    #[test]
    fn encode_ldr_x0_x1_0() {
        let word = encode_ldr(0, 1, 0);
        assert_eq!(word & 0x1F, 0); // Rt = x0
        assert_eq!((word >> 5) & 0x1F, 1); // Rn = x1
    }

    #[test]
    fn encode_str_x2_sp_16() {
        let word = encode_str(2, 31, 16); // str x2, [sp, #16]
        assert_eq!(word & 0x1F, 2); // Rt = x2
        assert_eq!((word >> 5) & 0x1F, 31); // Rn = sp
    }

    #[test]
    fn is_arm64_specific_mrs() {
        assert!(is_arm64_specific("mrs"));
        assert!(is_arm64_specific("MSR"));
        assert!(is_arm64_specific("eret"));
        assert!(is_arm64_specific("wfi"));
        assert!(is_arm64_specific("isb"));
        assert!(!is_arm64_specific("add"));
        assert!(!is_arm64_specific("nop")); // nop is generic but also ARM64-specific
    }

    #[test]
    fn encode_instruction_movz() {
        let word = encode_instruction("movz", &["x0", "#0x1234"]).unwrap();
        assert_eq!(word & 0x1F, 0); // Rd = x0
        assert_eq!((word >> 5) & 0xFFFF, 0x1234);
    }

    #[test]
    fn encode_instruction_ldr() {
        let word = encode_instruction("ldr", &["x3", "[x1, #8]"]).unwrap();
        assert_eq!(word & 0x1F, 3); // Rt = x3
        assert_eq!((word >> 5) & 0x1F, 1); // Rn = x1
    }

    #[test]
    fn encode_tlbi_alle1() {
        assert_eq!(encode_tlbi("alle1", 31).unwrap(), 0xD508871F);
    }
}
