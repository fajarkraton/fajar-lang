//! Intel AMX (Advanced Matrix Extensions) code generation.
//!
//! Generates AMX tile instructions for accelerated matrix multiplication:
//! - TDPBF16PS — BF16 tile matmul → FP32 accumulator
//! - TDPBSSD — INT8 tile matmul → INT32 accumulator
//! - TDPFP16PS — FP16 tile matmul → FP32 accumulator
//! - TILELOADD / TILESTORED — tile ↔ memory
//! - LDTILECFG — configure tile registers
//!
//! # Tile Registers
//!
//! AMX provides 8 tile registers (TMM0-TMM7), each up to 1KB:
//! - Max dimensions: 16 rows × 64 bytes (configurable per register)
//! - BF16: 16×32 elements per tile
//! - INT8: 16×64 elements per tile
//! - FP16: 16×32 elements per tile

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// AMX Feature Set
// ═══════════════════════════════════════════════════════════════════════

/// AMX feature flags detected from CPUID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmxFeatures {
    /// AMX-TILE: Base tile architecture (CPUID.7.0:EDX[24]).
    pub amx_tile: bool,
    /// AMX-BF16: BF16 tile matmul (CPUID.7.0:EDX[22]).
    pub amx_bf16: bool,
    /// AMX-INT8: INT8 tile matmul (CPUID.7.0:EDX[25]).
    pub amx_int8: bool,
    /// AMX-FP16: FP16 tile matmul (CPUID.7.1:EAX[21]).
    pub amx_fp16: bool,
    /// AMX-COMPLEX: Complex number tile ops.
    pub amx_complex: bool,
}

impl AmxFeatures {
    /// All features enabled (for testing).
    pub fn all() -> Self {
        Self {
            amx_tile: true,
            amx_bf16: true,
            amx_int8: true,
            amx_fp16: true,
            amx_complex: true,
        }
    }

    /// No features enabled.
    pub fn none() -> Self {
        Self {
            amx_tile: false,
            amx_bf16: false,
            amx_int8: false,
            amx_fp16: false,
            amx_complex: false,
        }
    }

    /// Whether any AMX is available.
    pub fn has_amx(&self) -> bool {
        self.amx_tile
    }

    /// Whether BF16 tile matmul is available.
    pub fn has_bf16(&self) -> bool {
        self.amx_tile && self.amx_bf16
    }

    /// Whether INT8 tile matmul is available.
    pub fn has_int8(&self) -> bool {
        self.amx_tile && self.amx_int8
    }

    /// Whether FP16 tile matmul is available.
    pub fn has_fp16(&self) -> bool {
        self.amx_tile && self.amx_fp16
    }
}

impl fmt::Display for AmxFeatures {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.amx_tile {
            return write!(f, "AMX: None");
        }
        let mut parts = vec!["AMX-TILE"];
        if self.amx_bf16 {
            parts.push("BF16");
        }
        if self.amx_int8 {
            parts.push("INT8");
        }
        if self.amx_fp16 {
            parts.push("FP16");
        }
        if self.amx_complex {
            parts.push("COMPLEX");
        }
        write!(f, "AMX: {}", parts.join(", "))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tile Registers
// ═══════════════════════════════════════════════════════════════════════

/// AMX tile register (TMM0-TMM7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileRegister {
    /// Register index (0-7).
    pub index: u8,
}

impl TileRegister {
    /// Creates a tile register reference.
    pub fn new(index: u8) -> Result<Self, String> {
        if index > 7 {
            return Err(format!("tile register index {} out of range [0, 7]", index));
        }
        Ok(Self { index })
    }

    /// Returns the register name (e.g., "tmm0").
    pub fn name(&self) -> String {
        format!("tmm{}", self.index)
    }
}

impl fmt::Display for TileRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tmm{}", self.index)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tile Configuration
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for a single tile register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileConfig {
    /// Number of rows (1-16).
    pub rows: u8,
    /// Number of bytes per row (1-64).
    pub cols_bytes: u8,
}

impl TileConfig {
    /// Creates a tile configuration.
    pub fn new(rows: u8, cols_bytes: u8) -> Result<Self, String> {
        if rows == 0 || rows > 16 {
            return Err(format!("tile rows {} out of range [1, 16]", rows));
        }
        if cols_bytes == 0 || cols_bytes > 64 {
            return Err(format!(
                "tile cols_bytes {} out of range [1, 64]",
                cols_bytes
            ));
        }
        Ok(Self { rows, cols_bytes })
    }

    /// BF16 tile: 16 rows × 32 BF16 elements (64 bytes).
    pub fn bf16_full() -> Self {
        Self {
            rows: 16,
            cols_bytes: 64,
        }
    }

    /// INT8 tile: 16 rows × 64 INT8 elements (64 bytes).
    pub fn int8_full() -> Self {
        Self {
            rows: 16,
            cols_bytes: 64,
        }
    }

    /// FP16 tile: 16 rows × 32 FP16 elements (64 bytes).
    pub fn fp16_full() -> Self {
        Self {
            rows: 16,
            cols_bytes: 64,
        }
    }

    /// FP32 accumulator tile: 16 rows × 16 FP32 elements (64 bytes).
    pub fn fp32_accumulator() -> Self {
        Self {
            rows: 16,
            cols_bytes: 64,
        }
    }

    /// Total size in bytes.
    pub fn size_bytes(&self) -> u32 {
        self.rows as u32 * self.cols_bytes as u32
    }

    /// Number of BF16 elements per row.
    pub fn bf16_cols(&self) -> u32 {
        self.cols_bytes as u32 / 2
    }

    /// Number of INT8 elements per row.
    pub fn int8_cols(&self) -> u32 {
        self.cols_bytes as u32
    }
}

/// Full tile configuration for all 8 registers (LDTILECFG structure).
///
/// This is the 64-byte configuration block loaded by LDTILECFG.
#[derive(Debug, Clone)]
pub struct TileCfg {
    /// Configuration for each tile register.
    pub tiles: [Option<TileConfig>; 8],
    /// Palette ID (always 1 for current AMX).
    pub palette: u8,
}

impl TileCfg {
    /// Creates an empty tile configuration.
    pub fn new() -> Self {
        Self {
            tiles: [None; 8],
            palette: 1,
        }
    }

    /// Sets configuration for a tile register.
    pub fn set_tile(&mut self, index: u8, config: TileConfig) -> Result<(), String> {
        if index > 7 {
            return Err(format!("tile index {} out of range [0, 7]", index));
        }
        self.tiles[index as usize] = Some(config);
        Ok(())
    }

    /// Serializes to the 64-byte TILECFG structure for LDTILECFG.
    pub fn to_bytes(&self) -> [u8; 64] {
        let mut bytes = [0u8; 64];
        bytes[0] = self.palette;
        // Bytes 16-23: rows for tiles 0-7
        // Bytes 48-63: cols_bytes for tiles 0-7 (as u16, little-endian)
        for (i, tile) in self.tiles.iter().enumerate() {
            if let Some(tc) = tile {
                bytes[16 + i] = tc.rows;
                let col_offset = 48 + i * 2;
                bytes[col_offset] = tc.cols_bytes;
                bytes[col_offset + 1] = 0;
            }
        }
        bytes
    }

    /// Number of configured tiles.
    pub fn active_tile_count(&self) -> usize {
        self.tiles.iter().filter(|t| t.is_some()).count()
    }
}

impl Default for TileCfg {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// AMX Instructions
// ═══════════════════════════════════════════════════════════════════════

/// AMX instruction mnemonic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmxOp {
    /// Load tile configuration.
    Ldtilecfg,
    /// Store tile configuration.
    Sttilecfg,
    /// Load tile from memory.
    Tileloadd,
    /// Store tile to memory.
    Tilestored,
    /// Zero a tile register.
    Tilezero,
    /// Release tile state.
    Tilerelease,
    /// BF16 tile matmul → FP32.
    Tdpbf16ps,
    /// INT8 signed tile matmul → INT32.
    Tdpbssd,
    /// INT8 unsigned×signed tile matmul → INT32.
    Tdpbusd,
    /// FP16 tile matmul → FP32.
    Tdpfp16ps,
}

impl fmt::Display for AmxOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            AmxOp::Ldtilecfg => "ldtilecfg",
            AmxOp::Sttilecfg => "sttilecfg",
            AmxOp::Tileloadd => "tileloadd",
            AmxOp::Tilestored => "tilestored",
            AmxOp::Tilezero => "tilezero",
            AmxOp::Tilerelease => "tilerelease",
            AmxOp::Tdpbf16ps => "tdpbf16ps",
            AmxOp::Tdpbssd => "tdpbssd",
            AmxOp::Tdpbusd => "tdpbusd",
            AmxOp::Tdpfp16ps => "tdpfp16ps",
        };
        write!(f, "{}", name)
    }
}

/// An emitted AMX instruction.
#[derive(Debug, Clone)]
pub struct AmxInstruction {
    /// Instruction opcode.
    pub op: AmxOp,
    /// Destination tile (if applicable).
    pub dst: Option<TileRegister>,
    /// Source tiles (0-2).
    pub srcs: Vec<TileRegister>,
}

impl fmt::Display for AmxInstruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.op)?;
        if let Some(dst) = &self.dst {
            write!(f, " {}", dst)?;
        }
        for src in &self.srcs {
            write!(f, ", {}", src)?;
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// AMX Code Emitter
// ═══════════════════════════════════════════════════════════════════════

/// AMX instruction emitter for tile matrix operations.
#[derive(Debug)]
pub struct AmxEmitter {
    /// Available AMX features.
    pub features: AmxFeatures,
    /// Tile configuration.
    pub tile_cfg: TileCfg,
    /// Emitted instructions.
    instructions: Vec<AmxInstruction>,
    /// Next available tile register.
    next_tile: u8,
    /// Whether XSAVE state needs to be saved.
    pub needs_xsave: bool,
}

impl AmxEmitter {
    /// Creates a new AMX emitter.
    pub fn new(features: AmxFeatures) -> Self {
        Self {
            features,
            tile_cfg: TileCfg::new(),
            instructions: Vec::new(),
            next_tile: 0,
            needs_xsave: false,
        }
    }

    /// Allocates a tile register.
    pub fn alloc_tile(&mut self) -> Result<TileRegister, String> {
        if self.next_tile >= 8 {
            return Err("tile register exhaustion (8 max)".to_string());
        }
        let reg = TileRegister::new(self.next_tile)?;
        self.next_tile += 1;
        Ok(reg)
    }

    /// Returns emitted instructions.
    pub fn instructions(&self) -> &[AmxInstruction] {
        &self.instructions
    }

    /// Emits LDTILECFG to configure tile registers.
    pub fn emit_ldtilecfg(&mut self) -> Result<(), String> {
        if !self.features.amx_tile {
            return Err("AMX-TILE required for ldtilecfg".to_string());
        }
        self.instructions.push(AmxInstruction {
            op: AmxOp::Ldtilecfg,
            dst: None,
            srcs: vec![],
        });
        Ok(())
    }

    /// Emits TILEZERO to clear a tile register.
    pub fn emit_tilezero(&mut self, tile: TileRegister) -> Result<(), String> {
        if !self.features.amx_tile {
            return Err("AMX-TILE required for tilezero".to_string());
        }
        self.instructions.push(AmxInstruction {
            op: AmxOp::Tilezero,
            dst: Some(tile),
            srcs: vec![],
        });
        Ok(())
    }

    /// Emits TILELOADD to load a tile from memory.
    pub fn emit_tileloadd(&mut self, tile: TileRegister) -> Result<(), String> {
        if !self.features.amx_tile {
            return Err("AMX-TILE required for tileloadd".to_string());
        }
        self.instructions.push(AmxInstruction {
            op: AmxOp::Tileloadd,
            dst: Some(tile),
            srcs: vec![],
        });
        Ok(())
    }

    /// Emits TILESTORED to store a tile to memory.
    pub fn emit_tilestored(&mut self, tile: TileRegister) -> Result<(), String> {
        if !self.features.amx_tile {
            return Err("AMX-TILE required for tilestored".to_string());
        }
        self.instructions.push(AmxInstruction {
            op: AmxOp::Tilestored,
            dst: None,
            srcs: vec![tile],
        });
        Ok(())
    }

    /// Emits TDPBF16PS: C += A (BF16) * B (BF16) → C (FP32).
    pub fn emit_tdpbf16ps(
        &mut self,
        dst: TileRegister,
        src1: TileRegister,
        src2: TileRegister,
    ) -> Result<(), String> {
        if !self.features.has_bf16() {
            return Err("AMX-BF16 required for tdpbf16ps".to_string());
        }
        self.instructions.push(AmxInstruction {
            op: AmxOp::Tdpbf16ps,
            dst: Some(dst),
            srcs: vec![src1, src2],
        });
        Ok(())
    }

    /// Emits TDPBSSD: C += A (INT8) * B (INT8) → C (INT32).
    pub fn emit_tdpbssd(
        &mut self,
        dst: TileRegister,
        src1: TileRegister,
        src2: TileRegister,
    ) -> Result<(), String> {
        if !self.features.has_int8() {
            return Err("AMX-INT8 required for tdpbssd".to_string());
        }
        self.instructions.push(AmxInstruction {
            op: AmxOp::Tdpbssd,
            dst: Some(dst),
            srcs: vec![src1, src2],
        });
        Ok(())
    }

    /// Emits TDPFP16PS: C += A (FP16) * B (FP16) → C (FP32).
    pub fn emit_tdpfp16ps(
        &mut self,
        dst: TileRegister,
        src1: TileRegister,
        src2: TileRegister,
    ) -> Result<(), String> {
        if !self.features.has_fp16() {
            return Err("AMX-FP16 required for tdpfp16ps".to_string());
        }
        self.instructions.push(AmxInstruction {
            op: AmxOp::Tdpfp16ps,
            dst: Some(dst),
            srcs: vec![src1, src2],
        });
        Ok(())
    }

    /// Emits TILERELEASE to release all tile state.
    pub fn emit_tilerelease(&mut self) -> Result<(), String> {
        if !self.features.amx_tile {
            return Err("AMX-TILE required for tilerelease".to_string());
        }
        self.instructions.push(AmxInstruction {
            op: AmxOp::Tilerelease,
            dst: None,
            srcs: vec![],
        });
        Ok(())
    }

    /// Sets up tiles for a BF16 16×32 × 32×16 matmul.
    ///
    /// Uses 3 tiles: tmm0 (A: BF16), tmm1 (B: BF16), tmm2 (C: FP32 accum).
    pub fn setup_bf16_matmul(
        &mut self,
    ) -> Result<(TileRegister, TileRegister, TileRegister), String> {
        let a = self.alloc_tile()?;
        let b = self.alloc_tile()?;
        let c = self.alloc_tile()?;

        self.tile_cfg.set_tile(a.index, TileConfig::bf16_full())?;
        self.tile_cfg.set_tile(b.index, TileConfig::bf16_full())?;
        self.tile_cfg
            .set_tile(c.index, TileConfig::fp32_accumulator())?;

        self.emit_ldtilecfg()?;
        self.emit_tilezero(c)?;

        Ok((a, b, c))
    }

    /// Emits a complete BF16 tile matmul: load A, load B, multiply into C, store C.
    pub fn emit_bf16_matmul(
        &mut self,
        a: TileRegister,
        b: TileRegister,
        c: TileRegister,
    ) -> Result<(), String> {
        self.emit_tileloadd(a)?;
        self.emit_tileloadd(b)?;
        self.emit_tdpbf16ps(c, a, b)?;
        self.emit_tilestored(c)?;
        Ok(())
    }

    /// Marks that XSAVE/XRSTOR is needed across function calls.
    pub fn mark_xsave_needed(&mut self) {
        self.needs_xsave = true;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amx_features_all() {
        let f = AmxFeatures::all();
        assert!(f.has_amx());
        assert!(f.has_bf16());
        assert!(f.has_int8());
        assert!(f.has_fp16());
    }

    #[test]
    fn amx_features_none() {
        let f = AmxFeatures::none();
        assert!(!f.has_amx());
        assert!(!f.has_bf16());
    }

    #[test]
    fn amx_features_display() {
        let f = AmxFeatures::all();
        let s = f.to_string();
        assert!(s.contains("AMX-TILE"));
        assert!(s.contains("BF16"));
        assert!(s.contains("INT8"));
    }

    #[test]
    fn tile_register_valid() {
        let t = TileRegister::new(0).unwrap();
        assert_eq!(t.name(), "tmm0");

        let t = TileRegister::new(7).unwrap();
        assert_eq!(t.name(), "tmm7");
    }

    #[test]
    fn tile_register_invalid() {
        assert!(TileRegister::new(8).is_err());
    }

    #[test]
    fn tile_config_bf16() {
        let tc = TileConfig::bf16_full();
        assert_eq!(tc.rows, 16);
        assert_eq!(tc.cols_bytes, 64);
        assert_eq!(tc.size_bytes(), 1024);
        assert_eq!(tc.bf16_cols(), 32);
    }

    #[test]
    fn tile_config_int8() {
        let tc = TileConfig::int8_full();
        assert_eq!(tc.int8_cols(), 64);
        assert_eq!(tc.size_bytes(), 1024);
    }

    #[test]
    fn tile_config_validation() {
        assert!(TileConfig::new(0, 64).is_err());
        assert!(TileConfig::new(17, 64).is_err());
        assert!(TileConfig::new(16, 0).is_err());
        assert!(TileConfig::new(16, 65).is_err());
        assert!(TileConfig::new(16, 64).is_ok());
    }

    #[test]
    fn tile_cfg_serialization() {
        let mut cfg = TileCfg::new();
        cfg.set_tile(0, TileConfig::bf16_full()).unwrap();
        cfg.set_tile(1, TileConfig::bf16_full()).unwrap();
        cfg.set_tile(2, TileConfig::fp32_accumulator()).unwrap();

        let bytes = cfg.to_bytes();
        assert_eq!(bytes[0], 1); // palette
        assert_eq!(bytes[16], 16); // tile 0 rows
        assert_eq!(bytes[17], 16); // tile 1 rows
        assert_eq!(bytes[18], 16); // tile 2 rows
        assert_eq!(bytes[48], 64); // tile 0 cols
    }

    #[test]
    fn tile_cfg_active_count() {
        let mut cfg = TileCfg::new();
        assert_eq!(cfg.active_tile_count(), 0);

        cfg.set_tile(0, TileConfig::bf16_full()).unwrap();
        cfg.set_tile(3, TileConfig::int8_full()).unwrap();
        assert_eq!(cfg.active_tile_count(), 2);
    }

    #[test]
    fn emit_ldtilecfg() {
        let mut e = AmxEmitter::new(AmxFeatures::all());
        e.emit_ldtilecfg().unwrap();
        assert_eq!(e.instructions()[0].op, AmxOp::Ldtilecfg);
    }

    #[test]
    fn emit_ldtilecfg_requires_amx() {
        let mut e = AmxEmitter::new(AmxFeatures::none());
        assert!(e.emit_ldtilecfg().is_err());
    }

    #[test]
    fn emit_tdpbf16ps() {
        let mut e = AmxEmitter::new(AmxFeatures::all());
        let a = e.alloc_tile().unwrap();
        let b = e.alloc_tile().unwrap();
        let c = e.alloc_tile().unwrap();

        e.emit_tdpbf16ps(c, a, b).unwrap();
        assert_eq!(e.instructions()[0].op, AmxOp::Tdpbf16ps);
        let s = e.instructions()[0].to_string();
        assert!(s.contains("tdpbf16ps"));
        assert!(s.contains("tmm2"));
    }

    #[test]
    fn emit_tdpbssd() {
        let mut e = AmxEmitter::new(AmxFeatures::all());
        let a = e.alloc_tile().unwrap();
        let b = e.alloc_tile().unwrap();
        let c = e.alloc_tile().unwrap();

        e.emit_tdpbssd(c, a, b).unwrap();
        assert_eq!(e.instructions()[0].op, AmxOp::Tdpbssd);
    }

    #[test]
    fn emit_tdpfp16ps() {
        let mut e = AmxEmitter::new(AmxFeatures::all());
        let a = e.alloc_tile().unwrap();
        let b = e.alloc_tile().unwrap();
        let c = e.alloc_tile().unwrap();

        e.emit_tdpfp16ps(c, a, b).unwrap();
        assert_eq!(e.instructions()[0].op, AmxOp::Tdpfp16ps);
    }

    #[test]
    fn emit_tdpbf16ps_requires_feature() {
        let features = AmxFeatures {
            amx_tile: true,
            amx_bf16: false,
            ..AmxFeatures::none()
        };
        let mut e = AmxEmitter::new(features);
        let a = TileRegister::new(0).unwrap();
        let b = TileRegister::new(1).unwrap();
        let c = TileRegister::new(2).unwrap();
        assert!(e.emit_tdpbf16ps(c, a, b).is_err());
    }

    #[test]
    fn emit_tileloadd_tilestored() {
        let mut e = AmxEmitter::new(AmxFeatures::all());
        let t = e.alloc_tile().unwrap();
        e.emit_tileloadd(t).unwrap();
        e.emit_tilestored(t).unwrap();
        assert_eq!(e.instructions().len(), 2);
        assert_eq!(e.instructions()[0].op, AmxOp::Tileloadd);
        assert_eq!(e.instructions()[1].op, AmxOp::Tilestored);
    }

    #[test]
    fn emit_tilezero() {
        let mut e = AmxEmitter::new(AmxFeatures::all());
        let t = e.alloc_tile().unwrap();
        e.emit_tilezero(t).unwrap();
        assert_eq!(e.instructions()[0].op, AmxOp::Tilezero);
    }

    #[test]
    fn emit_tilerelease() {
        let mut e = AmxEmitter::new(AmxFeatures::all());
        e.emit_tilerelease().unwrap();
        assert_eq!(e.instructions()[0].op, AmxOp::Tilerelease);
    }

    #[test]
    fn setup_bf16_matmul() {
        let mut e = AmxEmitter::new(AmxFeatures::all());
        let (a, b, c) = e.setup_bf16_matmul().unwrap();
        assert_eq!(a.index, 0);
        assert_eq!(b.index, 1);
        assert_eq!(c.index, 2);
        assert_eq!(e.tile_cfg.active_tile_count(), 3);
        // ldtilecfg + tilezero
        assert_eq!(e.instructions().len(), 2);
    }

    #[test]
    fn emit_bf16_matmul_complete() {
        let mut e = AmxEmitter::new(AmxFeatures::all());
        let (a, b, c) = e.setup_bf16_matmul().unwrap();
        e.emit_bf16_matmul(a, b, c).unwrap();
        // setup: ldtilecfg + tilezero = 2
        // matmul: tileloadd(a) + tileloadd(b) + tdpbf16ps + tilestored = 4
        assert_eq!(e.instructions().len(), 6);
    }

    #[test]
    fn tile_register_allocation_exhaustion() {
        let mut e = AmxEmitter::new(AmxFeatures::all());
        for i in 0..8 {
            let t = e.alloc_tile().unwrap();
            assert_eq!(t.index, i);
        }
        assert!(e.alloc_tile().is_err());
    }

    #[test]
    fn xsave_flag() {
        let mut e = AmxEmitter::new(AmxFeatures::all());
        assert!(!e.needs_xsave);
        e.mark_xsave_needed();
        assert!(e.needs_xsave);
    }

    #[test]
    fn amx_op_display() {
        assert_eq!(AmxOp::Tdpbf16ps.to_string(), "tdpbf16ps");
        assert_eq!(AmxOp::Tdpbssd.to_string(), "tdpbssd");
        assert_eq!(AmxOp::Ldtilecfg.to_string(), "ldtilecfg");
        assert_eq!(AmxOp::Tilerelease.to_string(), "tilerelease");
    }

    #[test]
    fn amx_instruction_display() {
        let inst = AmxInstruction {
            op: AmxOp::Tdpbf16ps,
            dst: Some(TileRegister::new(2).unwrap()),
            srcs: vec![TileRegister::new(0).unwrap(), TileRegister::new(1).unwrap()],
        };
        assert_eq!(inst.to_string(), "tdpbf16ps tmm2, tmm0, tmm1");
    }
}
