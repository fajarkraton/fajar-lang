//! Borrowing bridge — temporary borrows from linear values, affine/linear
//! promotion/demotion, closures, generics, linear references, reborrowing.

use std::collections::HashMap;
use std::fmt;

use super::checker::Linearity;

// ═══════════════════════════════════════════════════════════════════════
// S7.1 / S7.2: Temporary Borrow from Linear
// ═══════════════════════════════════════════════════════════════════════

/// A temporary borrow from a linear value.
#[derive(Debug, Clone)]
pub struct LinearBorrow {
    /// The linear value being borrowed.
    pub source: String,
    /// Whether the borrow is mutable.
    pub mutable: bool,
    /// Scope depth where the borrow was created.
    pub borrow_scope: u32,
    /// Scope depth where the source will be consumed.
    pub consumption_scope: Option<u32>,
}

impl LinearBorrow {
    /// Checks that this borrow does not outlive the source's consumption.
    pub fn validate_lifetime(&self) -> Result<(), String> {
        if let Some(consume_scope) = self.consumption_scope {
            if self.borrow_scope > consume_scope {
                return Err(format!(
                    "borrow of linear value `{}` outlives its consumption point",
                    self.source
                ));
            }
        }
        Ok(())
    }
}

/// Tracker for active borrows from linear values.
#[derive(Debug, Clone, Default)]
pub struct LinearBorrowTracker {
    /// Active borrows: borrowed_name → source linear value name.
    pub active_borrows: HashMap<String, LinearBorrow>,
}

impl LinearBorrowTracker {
    /// Creates a new tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a borrow from a linear value.
    pub fn borrow_from(&mut self, borrow_name: &str, source: &str, mutable: bool, scope: u32) {
        self.active_borrows.insert(
            borrow_name.into(),
            LinearBorrow {
                source: source.into(),
                mutable,
                borrow_scope: scope,
                consumption_scope: None,
            },
        );
    }

    /// Marks that a linear source will be consumed at the given scope.
    pub fn mark_consumption_point(&mut self, source: &str, scope: u32) {
        for borrow in self.active_borrows.values_mut() {
            if borrow.source == source {
                borrow.consumption_scope = Some(scope);
            }
        }
    }

    /// Validates all active borrows.
    pub fn validate_all(&self) -> Vec<String> {
        self.active_borrows
            .values()
            .filter_map(|b| b.validate_lifetime().err())
            .collect()
    }

    /// Removes borrows that are no longer in scope.
    pub fn exit_scope(&mut self, scope: u32) {
        self.active_borrows.retain(|_, b| b.borrow_scope < scope);
    }

    /// Returns the number of active borrows.
    pub fn count(&self) -> usize {
        self.active_borrows.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S7.3 / S7.4: Promotion & Demotion
// ═══════════════════════════════════════════════════════════════════════

/// Result of a linearity conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConversionResult {
    /// Conversion is valid.
    Ok,
    /// Conversion requires @unsafe context.
    RequiresUnsafe,
    /// Conversion is invalid.
    Invalid(String),
}

/// Checks whether a linearity promotion (Affine → Linear) is valid.
pub fn check_promotion(from: Linearity, to: Linearity) -> ConversionResult {
    match (from, to) {
        (Linearity::Affine, Linearity::Linear) => ConversionResult::Ok,
        (Linearity::Unrestricted, Linearity::Linear) => {
            ConversionResult::Invalid("cannot promote unrestricted to linear".into())
        }
        (Linearity::Unrestricted, Linearity::Affine) => ConversionResult::Ok,
        (a, b) if a == b => ConversionResult::Ok,
        _ => ConversionResult::Invalid(format!("cannot promote {from} to {to}")),
    }
}

/// Checks whether a linearity demotion (Linear → Affine) is valid.
pub fn check_demotion(from: Linearity, to: Linearity, in_unsafe: bool) -> ConversionResult {
    match (from, to) {
        (Linearity::Linear, Linearity::Affine) => {
            if in_unsafe {
                ConversionResult::Ok
            } else {
                ConversionResult::RequiresUnsafe
            }
        }
        (Linearity::Linear, Linearity::Unrestricted) => {
            ConversionResult::Invalid("cannot demote linear to unrestricted".into())
        }
        (Linearity::Affine, Linearity::Unrestricted) => {
            if in_unsafe {
                ConversionResult::Ok
            } else {
                ConversionResult::RequiresUnsafe
            }
        }
        (a, b) if a == b => ConversionResult::Ok,
        _ => ConversionResult::Invalid(format!("cannot demote {from} to {to}")),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S7.5: Linear-Safe Closures
// ═══════════════════════════════════════════════════════════════════════

/// Closure linearity classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClosureKind {
    /// Can be called multiple times — no linear captures.
    Fn,
    /// Can be called multiple times with mutation — no linear captures.
    FnMut,
    /// Can only be called once — may capture linear values.
    FnOnce,
}

impl fmt::Display for ClosureKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClosureKind::Fn => write!(f, "Fn"),
            ClosureKind::FnMut => write!(f, "FnMut"),
            ClosureKind::FnOnce => write!(f, "FnOnce"),
        }
    }
}

/// Determines the closure kind based on captured linearities.
pub fn infer_closure_kind(captured_linearities: &[Linearity]) -> ClosureKind {
    if captured_linearities
        .iter()
        .any(|l| l.is_linear() || *l == Linearity::Affine)
    {
        ClosureKind::FnOnce
    } else {
        ClosureKind::Fn
    }
}

/// Validates that a closure capturing linear values is used at most once.
pub fn validate_closure_usage(kind: ClosureKind, call_count: u32) -> Result<(), String> {
    if kind == ClosureKind::FnOnce && call_count > 1 {
        return Err(format!(
            "closure capturing linear values (FnOnce) called {call_count} times (max 1)"
        ));
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// S7.6: Linear in Generics
// ═══════════════════════════════════════════════════════════════════════

/// A trait bound that requires a type to be linear.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearBound {
    /// Type parameter name.
    pub param_name: String,
    /// Required linearity.
    pub required: Linearity,
}

/// Checks whether a type argument satisfies a linear bound.
pub fn check_linear_bound(bound: &LinearBound, actual: Linearity) -> Result<(), String> {
    match (bound.required, actual) {
        (Linearity::Linear, Linearity::Linear) => Ok(()),
        (Linearity::Linear, other) => Err(format!(
            "type parameter `{}` requires `Linear` bound, got `{other}`",
            bound.param_name
        )),
        (Linearity::Affine, Linearity::Linear | Linearity::Affine) => Ok(()),
        (Linearity::Affine, Linearity::Unrestricted) => Err(format!(
            "type parameter `{}` requires at least `Affine`, got `Unrestricted`",
            bound.param_name
        )),
        _ => Ok(()),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S7.7 / S7.8: Linear References & Reborrowing
// ═══════════════════════════════════════════════════════════════════════

/// A linear reference type: `&linear T`.
#[derive(Debug, Clone)]
pub struct LinearRef {
    /// The referenced type.
    pub inner_type: String,
    /// Source linear value name.
    pub source: String,
    /// Whether reborrowing is allowed (always false for linear refs).
    pub allows_reborrow: bool,
}

impl LinearRef {
    /// Creates a new linear reference.
    pub fn new(inner_type: &str, source: &str) -> Self {
        Self {
            inner_type: inner_type.into(),
            source: source.into(),
            allows_reborrow: false,
        }
    }
}

/// Checks whether a reborrow of a linear reference is valid.
pub fn check_reborrow(lin_ref: &LinearRef) -> Result<(), String> {
    if !lin_ref.allows_reborrow {
        Err(format!(
            "cannot reborrow linear reference to `{}`; single borrow chain only",
            lin_ref.inner_type
        ))
    } else {
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S7.9: Linear + Ownership Interplay
// ═══════════════════════════════════════════════════════════════════════

/// Precedence rules when linear meets ownership/borrow checker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinearOwnershipRule {
    /// Linear check runs first — if it rejects, ownership check is skipped.
    LinearFirst,
    /// Ownership check runs first — if it rejects, linear check adds its error too.
    OwnershipFirst,
}

/// The default rule: linear checking takes precedence.
pub const DEFAULT_RULE: LinearOwnershipRule = LinearOwnershipRule::LinearFirst;

/// Determines the combined result of linear + ownership analysis.
pub fn combined_check(
    linear_ok: bool,
    ownership_ok: bool,
    rule: LinearOwnershipRule,
) -> (bool, &'static str) {
    match rule {
        LinearOwnershipRule::LinearFirst => {
            if !linear_ok {
                (false, "linear")
            } else if !ownership_ok {
                (false, "ownership")
            } else {
                (true, "ok")
            }
        }
        LinearOwnershipRule::OwnershipFirst => {
            if !ownership_ok {
                (false, "ownership")
            } else if !linear_ok {
                (false, "linear")
            } else {
                (true, "ok")
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S7.1 — Temporary Borrow
    #[test]
    fn s7_1_borrow_from_linear() {
        let mut tracker = LinearBorrowTracker::new();
        tracker.borrow_from("ref_h", "h", false, 1);
        assert_eq!(tracker.count(), 1);
    }

    #[test]
    fn s7_1_borrow_valid_lifetime() {
        let borrow = LinearBorrow {
            source: "h".into(),
            mutable: false,
            borrow_scope: 1,
            consumption_scope: Some(2),
        };
        assert!(borrow.validate_lifetime().is_ok());
    }

    // S7.2 — Borrow Scope Rules
    #[test]
    fn s7_2_borrow_outlives_consumption() {
        let borrow = LinearBorrow {
            source: "h".into(),
            mutable: false,
            borrow_scope: 3,
            consumption_scope: Some(2),
        };
        assert!(borrow.validate_lifetime().is_err());
    }

    #[test]
    fn s7_2_exit_scope_removes_borrows() {
        let mut tracker = LinearBorrowTracker::new();
        tracker.borrow_from("r1", "h", false, 2);
        tracker.borrow_from("r2", "h", false, 1);
        tracker.exit_scope(2);
        assert_eq!(tracker.count(), 1); // r2 stays (scope 1), r1 removed (scope 2)
    }

    // S7.3 — Promotion
    #[test]
    fn s7_3_affine_to_linear_ok() {
        assert_eq!(
            check_promotion(Linearity::Affine, Linearity::Linear),
            ConversionResult::Ok
        );
    }

    #[test]
    fn s7_3_unrestricted_to_linear_fail() {
        assert!(matches!(
            check_promotion(Linearity::Unrestricted, Linearity::Linear),
            ConversionResult::Invalid(_)
        ));
    }

    // S7.4 — Demotion
    #[test]
    fn s7_4_linear_to_affine_unsafe() {
        assert_eq!(
            check_demotion(Linearity::Linear, Linearity::Affine, false),
            ConversionResult::RequiresUnsafe
        );
        assert_eq!(
            check_demotion(Linearity::Linear, Linearity::Affine, true),
            ConversionResult::Ok
        );
    }

    #[test]
    fn s7_4_linear_to_unrestricted_fail() {
        assert!(matches!(
            check_demotion(Linearity::Linear, Linearity::Unrestricted, true),
            ConversionResult::Invalid(_)
        ));
    }

    // S7.5 — Closures
    #[test]
    fn s7_5_closure_fn_once_for_linear() {
        let captures = vec![Linearity::Linear, Linearity::Unrestricted];
        assert_eq!(infer_closure_kind(&captures), ClosureKind::FnOnce);
    }

    #[test]
    fn s7_5_closure_fn_for_unrestricted() {
        let captures = vec![Linearity::Unrestricted, Linearity::Unrestricted];
        assert_eq!(infer_closure_kind(&captures), ClosureKind::Fn);
    }

    #[test]
    fn s7_5_fn_once_called_twice_error() {
        assert!(validate_closure_usage(ClosureKind::FnOnce, 2).is_err());
        assert!(validate_closure_usage(ClosureKind::FnOnce, 1).is_ok());
    }

    // S7.6 — Generic Bounds
    #[test]
    fn s7_6_linear_bound_satisfied() {
        let bound = LinearBound {
            param_name: "T".into(),
            required: Linearity::Linear,
        };
        assert!(check_linear_bound(&bound, Linearity::Linear).is_ok());
    }

    #[test]
    fn s7_6_linear_bound_not_satisfied() {
        let bound = LinearBound {
            param_name: "T".into(),
            required: Linearity::Linear,
        };
        assert!(check_linear_bound(&bound, Linearity::Affine).is_err());
    }

    // S7.7 — Linear References
    #[test]
    fn s7_7_linear_ref_no_reborrow() {
        let lr = LinearRef::new("FileHandle", "h");
        assert!(!lr.allows_reborrow);
    }

    // S7.8 — Reborrowing
    #[test]
    fn s7_8_reborrow_rejected() {
        let lr = LinearRef::new("FileHandle", "h");
        assert!(check_reborrow(&lr).is_err());
    }

    // S7.9 — Ownership Interplay
    #[test]
    fn s7_9_linear_first_both_ok() {
        let (ok, reason) = combined_check(true, true, LinearOwnershipRule::LinearFirst);
        assert!(ok);
        assert_eq!(reason, "ok");
    }

    #[test]
    fn s7_9_linear_first_linear_fails() {
        let (ok, reason) = combined_check(false, true, LinearOwnershipRule::LinearFirst);
        assert!(!ok);
        assert_eq!(reason, "linear");
    }

    #[test]
    fn s7_9_ownership_first_ownership_fails() {
        let (ok, reason) = combined_check(true, false, LinearOwnershipRule::OwnershipFirst);
        assert!(!ok);
        assert_eq!(reason, "ownership");
    }

    // S7.10 — Additional
    #[test]
    fn s7_10_same_linearity_conversion_ok() {
        assert_eq!(
            check_promotion(Linearity::Linear, Linearity::Linear),
            ConversionResult::Ok
        );
        assert_eq!(
            check_demotion(Linearity::Affine, Linearity::Affine, false),
            ConversionResult::Ok
        );
    }

    #[test]
    fn s7_10_closure_kind_display() {
        assert_eq!(ClosureKind::Fn.to_string(), "Fn");
        assert_eq!(ClosureKind::FnMut.to_string(), "FnMut");
        assert_eq!(ClosureKind::FnOnce.to_string(), "FnOnce");
    }

    #[test]
    fn s7_10_validate_all_borrows() {
        let mut tracker = LinearBorrowTracker::new();
        tracker.borrow_from("r1", "h", false, 1);
        tracker.mark_consumption_point("h", 2);
        let errors = tracker.validate_all();
        assert!(errors.is_empty());
    }

    #[test]
    fn s7_10_affine_closure_is_fn_once() {
        let captures = vec![Linearity::Affine];
        assert_eq!(infer_closure_kind(&captures), ClosureKind::FnOnce);
    }
}
