//! Fixed-point arithmetic — Q8.8 and Q16.16 types for FPU-less targets.
//!
//! All arithmetic without floating point hardware.
//! Useful for MCU targets without FPU.

use std::fmt;
use std::ops;

// ═══════════════════════════════════════════════════════════════════════
// Q8.8 — 16-bit fixed-point (8 integer bits, 8 fractional bits)
// ═══════════════════════════════════════════════════════════════════════

/// Q8.8 fixed-point number: 8 bits integer, 8 bits fraction.
///
/// Range: approximately -128.0 to 127.996
/// Resolution: 1/256 ≈ 0.00390625
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Q8_8 {
    /// Raw i16 representation.
    raw: i16,
}

impl Q8_8 {
    /// Fractional bits.
    const FRAC_BITS: u32 = 8;
    /// Scale factor (1 << 8 = 256).
    const SCALE: i16 = 1 << Self::FRAC_BITS;

    /// Creates a Q8.8 from an integer value.
    pub fn from_int(val: i8) -> Self {
        Self {
            raw: (val as i16) << Self::FRAC_BITS,
        }
    }

    /// Creates a Q8.8 from a floating-point value.
    pub fn from_f64(val: f64) -> Self {
        Self {
            raw: (val * Self::SCALE as f64).round() as i16,
        }
    }

    /// Converts to f64.
    pub fn to_f64(self) -> f64 {
        self.raw as f64 / Self::SCALE as f64
    }

    /// Returns the raw i16 representation.
    pub fn raw(self) -> i16 {
        self.raw
    }

    /// Creates from raw i16 value.
    pub fn from_raw(raw: i16) -> Self {
        Self { raw }
    }

    /// Absolute value.
    pub fn abs(self) -> Self {
        Self {
            raw: self.raw.wrapping_abs(),
        }
    }

    /// Zero constant.
    pub const ZERO: Self = Self { raw: 0 };

    /// One constant.
    pub const ONE: Self = Self {
        raw: 1 << Self::FRAC_BITS,
    };

    /// Division (returns zero on divide-by-zero).
    pub fn checked_div(self, other: Self) -> Self {
        if other.raw == 0 {
            return Self { raw: 0 };
        }
        let result = ((self.raw as i32) << Self::FRAC_BITS) / other.raw as i32;
        Self { raw: result as i16 }
    }
}

impl ops::Add for Q8_8 {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            raw: self.raw.wrapping_add(other.raw),
        }
    }
}

impl ops::Sub for Q8_8 {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self {
            raw: self.raw.wrapping_sub(other.raw),
        }
    }
}

impl ops::Mul for Q8_8 {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        let result = (self.raw as i32 * other.raw as i32) >> Self::FRAC_BITS;
        Self { raw: result as i16 }
    }
}

impl ops::Div for Q8_8 {
    type Output = Self;
    fn div(self, other: Self) -> Self {
        self.checked_div(other)
    }
}

impl fmt::Debug for Q8_8 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Q8_8({:.4})", self.to_f64())
    }
}

impl fmt::Display for Q8_8 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.4}", self.to_f64())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Q16.16 — 32-bit fixed-point (16 integer bits, 16 fractional bits)
// ═══════════════════════════════════════════════════════════════════════

/// Q16.16 fixed-point number: 16 bits integer, 16 bits fraction.
///
/// Range: approximately -32768.0 to 32767.99998
/// Resolution: 1/65536 ≈ 0.0000153
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Q16_16 {
    /// Raw i32 representation.
    raw: i32,
}

impl Q16_16 {
    /// Fractional bits.
    const FRAC_BITS: u32 = 16;
    /// Scale factor (1 << 16 = 65536).
    const SCALE: i32 = 1 << Self::FRAC_BITS;

    /// Creates a Q16.16 from an integer value.
    pub fn from_int(val: i16) -> Self {
        Self {
            raw: (val as i32) << Self::FRAC_BITS,
        }
    }

    /// Creates a Q16.16 from a floating-point value.
    pub fn from_f64(val: f64) -> Self {
        Self {
            raw: (val * Self::SCALE as f64).round() as i32,
        }
    }

    /// Converts to f64.
    pub fn to_f64(self) -> f64 {
        self.raw as f64 / Self::SCALE as f64
    }

    /// Returns the raw i32 representation.
    pub fn raw(self) -> i32 {
        self.raw
    }

    /// Creates from raw i32 value.
    pub fn from_raw(raw: i32) -> Self {
        Self { raw }
    }

    /// Absolute value.
    pub fn abs(self) -> Self {
        Self {
            raw: self.raw.wrapping_abs(),
        }
    }

    /// Zero constant.
    pub const ZERO: Self = Self { raw: 0 };

    /// One constant.
    pub const ONE: Self = Self {
        raw: 1 << Self::FRAC_BITS,
    };

    /// Division (returns zero on divide-by-zero).
    pub fn checked_div(self, other: Self) -> Self {
        if other.raw == 0 {
            return Self { raw: 0 };
        }
        let result = ((self.raw as i64) << Self::FRAC_BITS) / other.raw as i64;
        Self { raw: result as i32 }
    }
}

impl ops::Add for Q16_16 {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            raw: self.raw.wrapping_add(other.raw),
        }
    }
}

impl ops::Sub for Q16_16 {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self {
            raw: self.raw.wrapping_sub(other.raw),
        }
    }
}

impl ops::Mul for Q16_16 {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        let result = (self.raw as i64 * other.raw as i64) >> Self::FRAC_BITS;
        Self { raw: result as i32 }
    }
}

impl ops::Div for Q16_16 {
    type Output = Self;
    fn div(self, other: Self) -> Self {
        self.checked_div(other)
    }
}

impl fmt::Debug for Q16_16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Q16_16({:.6})", self.to_f64())
    }
}

impl fmt::Display for Q16_16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.6}", self.to_f64())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Fixed-point matmul
// ═══════════════════════════════════════════════════════════════════════

/// Fixed-point matrix multiplication using Q16.16 arithmetic.
///
/// `a` shape: `[M, K]`, `b` shape: `[K, N]` → result: `[M, N]`.
/// All operations use integer arithmetic only (no FPU).
pub fn fixed_matmul(
    a: &[Q16_16],
    a_rows: usize,
    a_cols: usize,
    b: &[Q16_16],
    b_cols: usize,
) -> Vec<Q16_16> {
    let mut result = vec![Q16_16::ZERO; a_rows * b_cols];
    for i in 0..a_rows {
        for j in 0..b_cols {
            let mut acc = Q16_16::ZERO;
            for k in 0..a_cols {
                acc = acc + a[i * a_cols + k] * b[k * b_cols + j];
            }
            result[i * b_cols + j] = acc;
        }
    }
    result
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Q8.8 ──

    #[test]
    fn q8_8_from_int() {
        let v = Q8_8::from_int(5);
        assert!((v.to_f64() - 5.0).abs() < 1e-6);
    }

    #[test]
    fn q8_8_from_f64() {
        let v = Q8_8::from_f64(3.5);
        assert!((v.to_f64() - 3.5).abs() < 0.01);
    }

    #[test]
    fn q8_8_negative() {
        let v = Q8_8::from_f64(-2.25);
        assert!((v.to_f64() - (-2.25)).abs() < 0.01);
    }

    #[test]
    fn q8_8_add() {
        let a = Q8_8::from_f64(1.5);
        let b = Q8_8::from_f64(2.25);
        let c = a + b;
        assert!((c.to_f64() - 3.75).abs() < 0.01);
    }

    #[test]
    fn q8_8_sub() {
        let a = Q8_8::from_f64(5.0);
        let b = Q8_8::from_f64(3.5);
        let c = a - b;
        assert!((c.to_f64() - 1.5).abs() < 0.01);
    }

    #[test]
    fn q8_8_mul() {
        let a = Q8_8::from_f64(2.5);
        let b = Q8_8::from_f64(3.0);
        let c = a * b;
        assert!((c.to_f64() - 7.5).abs() < 0.05);
    }

    #[test]
    fn q8_8_div() {
        let a = Q8_8::from_f64(7.5);
        let b = Q8_8::from_f64(2.5);
        let c = a / b;
        assert!((c.to_f64() - 3.0).abs() < 0.05);
    }

    #[test]
    fn q8_8_div_by_zero() {
        let a = Q8_8::from_f64(1.0);
        let b = Q8_8::ZERO;
        let c = a.checked_div(b);
        assert_eq!(c.raw(), 0);
    }

    #[test]
    fn q8_8_abs() {
        let v = Q8_8::from_f64(-3.5);
        assert!((v.abs().to_f64() - 3.5).abs() < 0.01);
    }

    #[test]
    fn q8_8_constants() {
        assert_eq!(Q8_8::ZERO.to_f64(), 0.0);
        assert_eq!(Q8_8::ONE.to_f64(), 1.0);
    }

    // ── Q16.16 ──

    #[test]
    fn q16_16_from_int() {
        let v = Q16_16::from_int(42);
        assert!((v.to_f64() - 42.0).abs() < 1e-6);
    }

    #[test]
    fn q16_16_from_f64() {
        let v = Q16_16::from_f64(3.14159);
        assert!((v.to_f64() - 3.14159).abs() < 0.0001);
    }

    #[test]
    fn q16_16_negative() {
        let v = Q16_16::from_f64(-100.5);
        assert!((v.to_f64() - (-100.5)).abs() < 0.001);
    }

    #[test]
    fn q16_16_add() {
        let a = Q16_16::from_f64(1.5);
        let b = Q16_16::from_f64(2.25);
        let c = a + b;
        assert!((c.to_f64() - 3.75).abs() < 0.001);
    }

    #[test]
    fn q16_16_sub() {
        let a = Q16_16::from_f64(10.0);
        let b = Q16_16::from_f64(3.75);
        let c = a - b;
        assert!((c.to_f64() - 6.25).abs() < 0.001);
    }

    #[test]
    fn q16_16_mul() {
        let a = Q16_16::from_f64(2.5);
        let b = Q16_16::from_f64(4.0);
        let c = a * b;
        assert!((c.to_f64() - 10.0).abs() < 0.001);
    }

    #[test]
    fn q16_16_div() {
        let a = Q16_16::from_f64(10.0);
        let b = Q16_16::from_f64(4.0);
        let c = a / b;
        assert!((c.to_f64() - 2.5).abs() < 0.001);
    }

    #[test]
    fn q16_16_div_by_zero() {
        let a = Q16_16::from_f64(1.0);
        let b = Q16_16::ZERO;
        let c = a.checked_div(b);
        assert_eq!(c.raw(), 0);
    }

    #[test]
    fn q16_16_abs() {
        let v = Q16_16::from_f64(-7.25);
        assert!((v.abs().to_f64() - 7.25).abs() < 0.001);
    }

    #[test]
    fn q16_16_constants() {
        assert_eq!(Q16_16::ZERO.to_f64(), 0.0);
        assert_eq!(Q16_16::ONE.to_f64(), 1.0);
    }

    #[test]
    fn q16_16_precision() {
        // Q16.16 should have better precision than Q8.8
        let v = Q16_16::from_f64(0.001);
        assert!((v.to_f64() - 0.001).abs() < 0.0001);
    }

    // ── Fixed-point matmul ──

    #[test]
    fn fixed_matmul_2x2() {
        // [[1, 2], [3, 4]] @ [[5, 6], [7, 8]] = [[19, 22], [43, 50]]
        let a: Vec<Q16_16> = [1.0, 2.0, 3.0, 4.0]
            .iter()
            .map(|&v| Q16_16::from_f64(v))
            .collect();
        let b: Vec<Q16_16> = [5.0, 6.0, 7.0, 8.0]
            .iter()
            .map(|&v| Q16_16::from_f64(v))
            .collect();

        let result = fixed_matmul(&a, 2, 2, &b, 2);

        assert!((result[0].to_f64() - 19.0).abs() < 0.01);
        assert!((result[1].to_f64() - 22.0).abs() < 0.01);
        assert!((result[2].to_f64() - 43.0).abs() < 0.01);
        assert!((result[3].to_f64() - 50.0).abs() < 0.01);
    }

    #[test]
    fn fixed_matmul_identity() {
        let a: Vec<Q16_16> = [1.5, 2.5, 3.5, 4.5]
            .iter()
            .map(|&v| Q16_16::from_f64(v))
            .collect();
        let eye: Vec<Q16_16> = [1.0, 0.0, 0.0, 1.0]
            .iter()
            .map(|&v| Q16_16::from_f64(v))
            .collect();

        let result = fixed_matmul(&a, 2, 2, &eye, 2);

        assert!((result[0].to_f64() - 1.5).abs() < 0.01);
        assert!((result[1].to_f64() - 2.5).abs() < 0.01);
        assert!((result[2].to_f64() - 3.5).abs() < 0.01);
        assert!((result[3].to_f64() - 4.5).abs() < 0.01);
    }

    #[test]
    fn fixed_matmul_non_square() {
        // [1, 2, 3] @ [[1], [2], [3]] = [14]
        let a: Vec<Q16_16> = [1.0, 2.0, 3.0]
            .iter()
            .map(|&v| Q16_16::from_f64(v))
            .collect();
        let b: Vec<Q16_16> = [1.0, 2.0, 3.0]
            .iter()
            .map(|&v| Q16_16::from_f64(v))
            .collect();

        let result = fixed_matmul(&a, 1, 3, &b, 1);

        assert!((result[0].to_f64() - 14.0).abs() < 0.01);
    }
}
