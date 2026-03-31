//! Safety Certification — Sprint V9 (10 tasks).
//!
//! MISRA-C and CERT-C compliance checkers, DO-178C (aerospace),
//! ISO 26262 (automotive), IEC 62304 (medical) evidence generation,
//! traceability matrix, verification coverage (MCDC-style),
//! audit trail, and certificate generation.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// V9.1: Certification Standards
// ═══════════════════════════════════════════════════════════════════════

/// Safety certification standard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CertificationStandard {
    /// MISRA-C:2012 — Motor Industry Software Reliability Association.
    MisraC,
    /// CERT-C — SEI CERT C Coding Standard.
    CertC,
    /// DO-178C — Software Considerations in Airborne Systems (aerospace).
    Do178c,
    /// ISO 26262 — Road Vehicles Functional Safety (automotive).
    Iso26262,
    /// IEC 62304 — Medical Device Software Lifecycle.
    Iec62304,
}

impl fmt::Display for CertificationStandard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MisraC => write!(f, "MISRA-C:2012"),
            Self::CertC => write!(f, "CERT-C"),
            Self::Do178c => write!(f, "DO-178C"),
            Self::Iso26262 => write!(f, "ISO 26262"),
            Self::Iec62304 => write!(f, "IEC 62304"),
        }
    }
}

/// Safety integrity level (cross-standard).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SafetyLevel {
    /// DO-178C DAL E / ISO 26262 QM — no safety requirement.
    None,
    /// DO-178C DAL D / ISO 26262 ASIL A — minor safety requirement.
    Low,
    /// DO-178C DAL C / ISO 26262 ASIL B — moderate safety requirement.
    Medium,
    /// DO-178C DAL B / ISO 26262 ASIL C — high safety requirement.
    High,
    /// DO-178C DAL A / ISO 26262 ASIL D — highest safety requirement.
    Critical,
}

impl fmt::Display for SafetyLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "None (DAL E / QM)"),
            Self::Low => write!(f, "Low (DAL D / ASIL A)"),
            Self::Medium => write!(f, "Medium (DAL C / ASIL B)"),
            Self::High => write!(f, "High (DAL B / ASIL C)"),
            Self::Critical => write!(f, "Critical (DAL A / ASIL D)"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V9.2: MISRA-C Compliance Checker
// ═══════════════════════════════════════════════════════════════════════

/// A MISRA-C rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MisraRule {
    /// Rule number (e.g., "Rule 1.3").
    pub id: String,
    /// Category.
    pub category: MisraCategory,
    /// Description.
    pub description: String,
    /// Whether this rule applies to Fajar Lang.
    pub applicable: bool,
}

/// MISRA rule categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MisraCategory {
    /// Mandatory — must be followed.
    Mandatory,
    /// Required — must be followed with documented deviation.
    Required,
    /// Advisory — should be followed.
    Advisory,
}

impl fmt::Display for MisraCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mandatory => write!(f, "Mandatory"),
            Self::Required => write!(f, "Required"),
            Self::Advisory => write!(f, "Advisory"),
        }
    }
}

/// MISRA compliance result for a source file.
#[derive(Debug, Clone)]
pub struct MisraComplianceResult {
    /// File checked.
    pub file: String,
    /// Violations found.
    pub violations: Vec<MisraViolation>,
    /// Rules checked.
    pub rules_checked: u32,
    /// Rules compliant.
    pub rules_compliant: u32,
}

/// A MISRA rule violation.
#[derive(Debug, Clone)]
pub struct MisraViolation {
    /// Rule ID.
    pub rule_id: String,
    /// Category.
    pub category: MisraCategory,
    /// Description of the violation.
    pub description: String,
    /// Source file.
    pub file: String,
    /// Line number.
    pub line: u32,
    /// Whether a deviation is documented.
    pub has_deviation: bool,
}

impl MisraComplianceResult {
    /// Returns compliance percentage.
    pub fn compliance_rate(&self) -> f64 {
        if self.rules_checked == 0 {
            return 1.0;
        }
        self.rules_compliant as f64 / self.rules_checked as f64
    }

    /// Returns true if all mandatory/required rules pass.
    pub fn is_compliant(&self) -> bool {
        !self.violations.iter().any(|v| {
            matches!(v.category, MisraCategory::Mandatory | MisraCategory::Required)
                && !v.has_deviation
        })
    }
}

/// Checks MISRA-C compliance for common Fajar Lang patterns.
pub fn check_misra_compliance(
    has_goto: bool,
    has_implicit_cast: bool,
    has_recursion: bool,
    has_dynamic_alloc: bool,
    has_null_deref: bool,
    file: &str,
) -> MisraComplianceResult {
    let mut violations = Vec::new();
    let mut rules_compliant = 0u32;
    let rules_checked = 5u32;

    // Rule 15.1: goto should not be used
    if has_goto {
        violations.push(MisraViolation {
            rule_id: "Rule 15.1".to_string(),
            category: MisraCategory::Advisory,
            description: "goto statement used".to_string(),
            file: file.to_string(),
            line: 0,
            has_deviation: false,
        });
    } else {
        rules_compliant += 1;
    }

    // Rule 10.1: no implicit type conversions
    if has_implicit_cast {
        violations.push(MisraViolation {
            rule_id: "Rule 10.1".to_string(),
            category: MisraCategory::Required,
            description: "implicit type conversion detected".to_string(),
            file: file.to_string(),
            line: 0,
            has_deviation: false,
        });
    } else {
        rules_compliant += 1;
    }

    // Rule 17.2: no recursion
    if has_recursion {
        violations.push(MisraViolation {
            rule_id: "Rule 17.2".to_string(),
            category: MisraCategory::Required,
            description: "function recursion detected".to_string(),
            file: file.to_string(),
            line: 0,
            has_deviation: false,
        });
    } else {
        rules_compliant += 1;
    }

    // Rule 21.3: no dynamic memory allocation (malloc/free)
    if has_dynamic_alloc {
        violations.push(MisraViolation {
            rule_id: "Rule 21.3".to_string(),
            category: MisraCategory::Required,
            description: "dynamic memory allocation used".to_string(),
            file: file.to_string(),
            line: 0,
            has_deviation: false,
        });
    } else {
        rules_compliant += 1;
    }

    // Rule 1.3: no undefined behavior (null dereference)
    if has_null_deref {
        violations.push(MisraViolation {
            rule_id: "Rule 1.3".to_string(),
            category: MisraCategory::Mandatory,
            description: "potential null pointer dereference".to_string(),
            file: file.to_string(),
            line: 0,
            has_deviation: false,
        });
    } else {
        rules_compliant += 1;
    }

    MisraComplianceResult {
        file: file.to_string(),
        violations,
        rules_checked,
        rules_compliant,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V9.3: CERT-C Compliance
// ═══════════════════════════════════════════════════════════════════════

/// A CERT-C rule check result.
#[derive(Debug, Clone)]
pub struct CertCResult {
    /// Rule ID (e.g., "INT32-C").
    pub rule_id: String,
    /// Description.
    pub description: String,
    /// Compliance status.
    pub status: ComplianceStatus,
    /// Source evidence.
    pub evidence: String,
}

/// Compliance status for a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComplianceStatus {
    /// Rule is fully satisfied.
    Compliant,
    /// Rule is violated.
    NonCompliant,
    /// Rule is not applicable to Fajar Lang.
    NotApplicable,
    /// Compliance with documented deviation.
    Deviation,
}

impl fmt::Display for ComplianceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compliant => write!(f, "COMPLIANT"),
            Self::NonCompliant => write!(f, "NON-COMPLIANT"),
            Self::NotApplicable => write!(f, "N/A"),
            Self::Deviation => write!(f, "DEVIATION"),
        }
    }
}

/// Checks common CERT-C rules relevant to Fajar Lang.
pub fn check_cert_c(
    has_overflow_check: bool,
    has_bounds_check: bool,
    has_null_check: bool,
    has_format_string_check: bool,
) -> Vec<CertCResult> {
    vec![
        CertCResult {
            rule_id: "INT32-C".to_string(),
            description: "Ensure integer operations do not overflow".to_string(),
            status: if has_overflow_check {
                ComplianceStatus::Compliant
            } else {
                ComplianceStatus::NonCompliant
            },
            evidence: if has_overflow_check {
                "Overflow checks present in arithmetic operations".to_string()
            } else {
                "Missing overflow checks".to_string()
            },
        },
        CertCResult {
            rule_id: "ARR38-C".to_string(),
            description: "Guarantee array indices are within valid range".to_string(),
            status: if has_bounds_check {
                ComplianceStatus::Compliant
            } else {
                ComplianceStatus::NonCompliant
            },
            evidence: if has_bounds_check {
                "Bounds checks present for array accesses".to_string()
            } else {
                "Missing bounds checks".to_string()
            },
        },
        CertCResult {
            rule_id: "EXP34-C".to_string(),
            description: "Do not dereference null pointers".to_string(),
            status: if has_null_check {
                ComplianceStatus::Compliant
            } else {
                ComplianceStatus::NonCompliant
            },
            evidence: if has_null_check {
                "Null safety enforced by type system (Option<T>)".to_string()
            } else {
                "Potential null dereference".to_string()
            },
        },
        CertCResult {
            rule_id: "FIO30-C".to_string(),
            description: "Exclude user input from format strings".to_string(),
            status: if has_format_string_check {
                ComplianceStatus::Compliant
            } else {
                ComplianceStatus::NonCompliant
            },
            evidence: if has_format_string_check {
                "Format strings are compile-time constants".to_string()
            } else {
                "Dynamic format strings detected".to_string()
            },
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// V9.4: DO-178C Evidence (Aerospace)
// ═══════════════════════════════════════════════════════════════════════

/// DO-178C Design Assurance Level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DalLevel {
    /// Level E: No safety effect.
    DalE,
    /// Level D: Minor.
    DalD,
    /// Level C: Major.
    DalC,
    /// Level B: Hazardous.
    DalB,
    /// Level A: Catastrophic.
    DalA,
}

impl fmt::Display for DalLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DalE => write!(f, "DAL E"),
            Self::DalD => write!(f, "DAL D"),
            Self::DalC => write!(f, "DAL C"),
            Self::DalB => write!(f, "DAL B"),
            Self::DalA => write!(f, "DAL A"),
        }
    }
}

/// DO-178C objective evidence.
#[derive(Debug, Clone)]
pub struct Do178cEvidence {
    /// Objective ID (e.g., "A-7.2.1").
    pub objective_id: String,
    /// Objective description.
    pub description: String,
    /// DAL applicability.
    pub applicable_dals: Vec<DalLevel>,
    /// Evidence provided.
    pub evidence: Vec<String>,
    /// Whether the objective is satisfied.
    pub satisfied: bool,
}

/// Generates DO-178C evidence for formal verification.
pub fn generate_do178c_evidence(
    dal: DalLevel,
    has_formal_specs: bool,
    has_mc_dc_coverage: bool,
    has_code_review: bool,
    verification_coverage: f64,
) -> Vec<Do178cEvidence> {
    let mut evidence = Vec::new();

    // Table A-7: Verification of Verification Process Results
    evidence.push(Do178cEvidence {
        objective_id: "A-7.2.1".to_string(),
        description: "Requirements-based test coverage".to_string(),
        applicable_dals: vec![DalLevel::DalA, DalLevel::DalB, DalLevel::DalC],
        evidence: if verification_coverage >= 0.95 {
            vec![format!(
                "Formal verification coverage: {:.1}%",
                verification_coverage * 100.0
            )]
        } else {
            vec![format!(
                "Insufficient coverage: {:.1}% (need >= 95%)",
                verification_coverage * 100.0
            )]
        },
        satisfied: dal < DalLevel::DalC || verification_coverage >= 0.95,
    });

    // Formal methods supplement (DO-333)
    evidence.push(Do178cEvidence {
        objective_id: "FM-1".to_string(),
        description: "Formal specification of high-level requirements".to_string(),
        applicable_dals: vec![DalLevel::DalA, DalLevel::DalB],
        evidence: if has_formal_specs {
            vec!["@requires/@ensures annotations on safety-critical functions".to_string()]
        } else {
            vec!["No formal specifications provided".to_string()]
        },
        satisfied: dal < DalLevel::DalB || has_formal_specs,
    });

    // MC/DC coverage
    evidence.push(Do178cEvidence {
        objective_id: "A-7.2.6".to_string(),
        description: "MC/DC structural coverage".to_string(),
        applicable_dals: vec![DalLevel::DalA],
        evidence: if has_mc_dc_coverage {
            vec!["SMT-based MCDC analysis of all decisions".to_string()]
        } else {
            vec!["MC/DC coverage not measured".to_string()]
        },
        satisfied: dal != DalLevel::DalA || has_mc_dc_coverage,
    });

    // Code review
    evidence.push(Do178cEvidence {
        objective_id: "A-5.5".to_string(),
        description: "Source code review".to_string(),
        applicable_dals: vec![DalLevel::DalA, DalLevel::DalB, DalLevel::DalC, DalLevel::DalD],
        evidence: if has_code_review {
            vec!["Code review completed and documented".to_string()]
        } else {
            vec!["Code review not performed".to_string()]
        },
        satisfied: has_code_review || dal == DalLevel::DalE,
    });

    evidence
}

// ═══════════════════════════════════════════════════════════════════════
// V9.5: ISO 26262 (Automotive)
// ═══════════════════════════════════════════════════════════════════════

/// ISO 26262 ASIL (Automotive Safety Integrity Level).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AsilLevel {
    /// QM: Quality Management (no safety requirement).
    Qm,
    /// ASIL A: Lowest safety level.
    AsilA,
    /// ASIL B.
    AsilB,
    /// ASIL C.
    AsilC,
    /// ASIL D: Highest safety level.
    AsilD,
}

impl fmt::Display for AsilLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Qm => write!(f, "QM"),
            Self::AsilA => write!(f, "ASIL A"),
            Self::AsilB => write!(f, "ASIL B"),
            Self::AsilC => write!(f, "ASIL C"),
            Self::AsilD => write!(f, "ASIL D"),
        }
    }
}

/// ISO 26262 verification method recommendation.
#[derive(Debug, Clone)]
pub struct Iso26262Method {
    /// Method name.
    pub name: String,
    /// Part/clause reference.
    pub reference: String,
    /// Required for ASIL level.
    pub required_from: AsilLevel,
    /// Whether this method is provided by Fajar Lang verification.
    pub provided: bool,
    /// Evidence description.
    pub evidence: String,
}

/// Generates ISO 26262 compliance evidence.
pub fn generate_iso26262_evidence(
    _asil: AsilLevel,
    has_static_analysis: bool,
    has_formal_verification: bool,
    has_unit_testing: bool,
    has_integration_testing: bool,
) -> Vec<Iso26262Method> {
    vec![
        Iso26262Method {
            name: "Static analysis".to_string(),
            reference: "ISO 26262-6:2018 Table 9".to_string(),
            required_from: AsilLevel::AsilA,
            provided: has_static_analysis,
            evidence: if has_static_analysis {
                "Fajar Lang type checker + analyzer".to_string()
            } else {
                "Not provided".to_string()
            },
        },
        Iso26262Method {
            name: "Formal verification".to_string(),
            reference: "ISO 26262-6:2018 Table 9".to_string(),
            required_from: AsilLevel::AsilC,
            provided: has_formal_verification,
            evidence: if has_formal_verification {
                "SMT-based formal verification with Z3".to_string()
            } else {
                "Not provided".to_string()
            },
        },
        Iso26262Method {
            name: "Unit testing".to_string(),
            reference: "ISO 26262-6:2018 Table 10".to_string(),
            required_from: AsilLevel::AsilA,
            provided: has_unit_testing,
            evidence: if has_unit_testing {
                "fj test framework with coverage".to_string()
            } else {
                "Not provided".to_string()
            },
        },
        Iso26262Method {
            name: "Integration testing".to_string(),
            reference: "ISO 26262-6:2018 Table 11".to_string(),
            required_from: AsilLevel::AsilB,
            provided: has_integration_testing,
            evidence: if has_integration_testing {
                "End-to-end pipeline tests".to_string()
            } else {
                "Not provided".to_string()
            },
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// V9.6: IEC 62304 (Medical)
// ═══════════════════════════════════════════════════════════════════════

/// IEC 62304 software safety class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SoftwareSafetyClass {
    /// Class A: No injury or damage to health.
    ClassA,
    /// Class B: Non-serious injury.
    ClassB,
    /// Class C: Death or serious injury.
    ClassC,
}

impl fmt::Display for SoftwareSafetyClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ClassA => write!(f, "Class A"),
            Self::ClassB => write!(f, "Class B"),
            Self::ClassC => write!(f, "Class C"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V9.7: Traceability Matrix
// ═══════════════════════════════════════════════════════════════════════

/// A requirement-to-verification traceability entry.
#[derive(Debug, Clone)]
pub struct TraceabilityEntry {
    /// Requirement ID.
    pub requirement_id: String,
    /// Requirement description.
    pub requirement_desc: String,
    /// Verification conditions that cover this requirement.
    pub vc_ids: Vec<String>,
    /// Test case IDs that cover this requirement.
    pub test_ids: Vec<String>,
    /// Coverage status.
    pub status: TraceStatus,
}

/// Traceability status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceStatus {
    /// Fully covered (has VCs and tests).
    FullyCovered,
    /// Partially covered (VCs only, or tests only).
    PartiallyCovered,
    /// Not covered.
    NotCovered,
}

impl fmt::Display for TraceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FullyCovered => write!(f, "FULLY COVERED"),
            Self::PartiallyCovered => write!(f, "PARTIAL"),
            Self::NotCovered => write!(f, "NOT COVERED"),
        }
    }
}

/// Traceability matrix for a project.
#[derive(Debug, Clone, Default)]
pub struct TraceabilityMatrix {
    /// Entries (one per requirement).
    pub entries: Vec<TraceabilityEntry>,
}

impl TraceabilityMatrix {
    /// Adds a traceability entry.
    pub fn add_entry(&mut self, entry: TraceabilityEntry) {
        self.entries.push(entry);
    }

    /// Returns the number of fully covered requirements.
    pub fn fully_covered_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.status == TraceStatus::FullyCovered)
            .count()
    }

    /// Returns the coverage ratio.
    pub fn coverage_ratio(&self) -> f64 {
        if self.entries.is_empty() {
            return 1.0;
        }
        self.fully_covered_count() as f64 / self.entries.len() as f64
    }

    /// Returns uncovered requirements.
    pub fn uncovered(&self) -> Vec<&TraceabilityEntry> {
        self.entries
            .iter()
            .filter(|e| e.status == TraceStatus::NotCovered)
            .collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V9.8: Verification Coverage (MC/DC-style)
// ═══════════════════════════════════════════════════════════════════════

/// Verification coverage report (MC/DC-style for decisions).
#[derive(Debug, Clone)]
pub struct VerificationCoverage {
    /// Total decisions (branch points).
    pub total_decisions: u64,
    /// Decisions covered by at least one VC.
    pub covered_decisions: u64,
    /// Total conditions within decisions.
    pub total_conditions: u64,
    /// Conditions independently shown to affect the decision outcome.
    pub mcdc_conditions: u64,
    /// Coverage per function.
    pub function_coverage: HashMap<String, FunctionCoverage>,
}

/// Per-function coverage.
#[derive(Debug, Clone)]
pub struct FunctionCoverage {
    /// Function name.
    pub name: String,
    /// Number of VCs for this function.
    pub vc_count: u32,
    /// Number of VCs proven.
    pub vc_proven: u32,
    /// Decision coverage (fraction).
    pub decision_coverage: f64,
    /// Condition coverage (fraction).
    pub condition_coverage: f64,
}

impl VerificationCoverage {
    /// Returns decision coverage ratio.
    pub fn decision_coverage(&self) -> f64 {
        if self.total_decisions == 0 {
            return 1.0;
        }
        self.covered_decisions as f64 / self.total_decisions as f64
    }

    /// Returns MC/DC coverage ratio.
    pub fn mcdc_coverage(&self) -> f64 {
        if self.total_conditions == 0 {
            return 1.0;
        }
        self.mcdc_conditions as f64 / self.total_conditions as f64
    }

    /// Returns true if coverage meets the target for a given safety level.
    pub fn meets_target(&self, level: SafetyLevel) -> bool {
        match level {
            SafetyLevel::Critical => self.mcdc_coverage() >= 0.95,
            SafetyLevel::High => self.decision_coverage() >= 0.95,
            SafetyLevel::Medium => self.decision_coverage() >= 0.90,
            SafetyLevel::Low => self.decision_coverage() >= 0.80,
            SafetyLevel::None => true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V9.9: Audit Trail
// ═══════════════════════════════════════════════════════════════════════

/// An audit trail entry.
#[derive(Debug, Clone)]
pub struct AuditEntry {
    /// Timestamp (ISO 8601).
    pub timestamp: String,
    /// Who performed the action.
    pub actor: String,
    /// What action was performed.
    pub action: AuditAction,
    /// Details.
    pub details: String,
    /// Hash of the state at this point.
    pub state_hash: String,
}

/// Audit actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditAction {
    /// Verification run started.
    VerificationStarted,
    /// Verification completed.
    VerificationCompleted,
    /// Property added or modified.
    PropertyChanged,
    /// Suppression added.
    SuppressionAdded,
    /// Certificate generated.
    CertificateGenerated,
    /// Configuration changed.
    ConfigChanged,
}

impl fmt::Display for AuditAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VerificationStarted => write!(f, "VERIFY_START"),
            Self::VerificationCompleted => write!(f, "VERIFY_COMPLETE"),
            Self::PropertyChanged => write!(f, "PROPERTY_CHANGE"),
            Self::SuppressionAdded => write!(f, "SUPPRESSION_ADD"),
            Self::CertificateGenerated => write!(f, "CERT_GENERATE"),
            Self::ConfigChanged => write!(f, "CONFIG_CHANGE"),
        }
    }
}

/// Audit trail for a project.
#[derive(Debug, Clone, Default)]
pub struct AuditTrail {
    /// Entries in chronological order.
    pub entries: Vec<AuditEntry>,
}

impl AuditTrail {
    /// Adds an audit entry.
    pub fn log(&mut self, entry: AuditEntry) {
        self.entries.push(entry);
    }

    /// Returns entries for a specific action type.
    pub fn entries_for_action(&self, action: AuditAction) -> Vec<&AuditEntry> {
        self.entries.iter().filter(|e| e.action == action).collect()
    }

    /// Returns the most recent entry.
    pub fn latest(&self) -> Option<&AuditEntry> {
        self.entries.last()
    }

    /// Returns the total number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the trail is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V9.10: Certificate Generation
// ═══════════════════════════════════════════════════════════════════════

/// A safety certificate.
#[derive(Debug, Clone)]
pub struct SafetyCertificate {
    /// Certificate ID.
    pub id: String,
    /// Certification standard.
    pub standard: CertificationStandard,
    /// Safety level.
    pub level: SafetyLevel,
    /// Project name.
    pub project: String,
    /// Version/commit hash.
    pub version: String,
    /// Issue date (ISO 8601).
    pub issue_date: String,
    /// Issuer.
    pub issuer: String,
    /// Overall verdict.
    pub verdict: CertificateVerdict,
    /// Verification summary.
    pub total_vcs: u64,
    /// VCs proven.
    pub proven_vcs: u64,
    /// Compliance results.
    pub compliance_notes: Vec<String>,
    /// Conditions/caveats.
    pub conditions: Vec<String>,
}

/// Certificate verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertificateVerdict {
    /// All requirements met.
    Pass,
    /// Requirements met with conditions.
    ConditionalPass,
    /// Requirements not met.
    Fail,
}

impl fmt::Display for CertificateVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pass => write!(f, "PASS"),
            Self::ConditionalPass => write!(f, "CONDITIONAL PASS"),
            Self::Fail => write!(f, "FAIL"),
        }
    }
}

impl SafetyCertificate {
    /// Returns verification coverage.
    pub fn verification_coverage(&self) -> f64 {
        if self.total_vcs == 0 {
            return 1.0;
        }
        self.proven_vcs as f64 / self.total_vcs as f64
    }
}

impl fmt::Display for SafetyCertificate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "====================================")?;
        writeln!(f, "  SAFETY CERTIFICATE")?;
        writeln!(f, "====================================")?;
        writeln!(f, "  ID:        {}", self.id)?;
        writeln!(f, "  Standard:  {}", self.standard)?;
        writeln!(f, "  Level:     {}", self.level)?;
        writeln!(f, "  Project:   {}", self.project)?;
        writeln!(f, "  Version:   {}", self.version)?;
        writeln!(f, "  Date:      {}", self.issue_date)?;
        writeln!(f, "  Issuer:    {}", self.issuer)?;
        writeln!(f, "  Verdict:   {}", self.verdict)?;
        writeln!(f)?;
        writeln!(
            f,
            "  Verification: {}/{} VCs ({:.1}%)",
            self.proven_vcs,
            self.total_vcs,
            self.verification_coverage() * 100.0,
        )?;
        if !self.compliance_notes.is_empty() {
            writeln!(f, "\n  Compliance Notes:")?;
            for note in &self.compliance_notes {
                writeln!(f, "    - {note}")?;
            }
        }
        if !self.conditions.is_empty() {
            writeln!(f, "\n  Conditions:")?;
            for cond in &self.conditions {
                writeln!(f, "    - {cond}")?;
            }
        }
        writeln!(f, "====================================")?;
        Ok(())
    }
}

/// Generates a safety certificate from verification results.
pub fn generate_certificate(
    standard: CertificationStandard,
    level: SafetyLevel,
    project: &str,
    version: &str,
    total_vcs: u64,
    proven_vcs: u64,
    compliance_notes: Vec<String>,
) -> SafetyCertificate {
    let coverage = if total_vcs == 0 {
        1.0
    } else {
        proven_vcs as f64 / total_vcs as f64
    };

    let verdict = match level {
        SafetyLevel::Critical => {
            if coverage >= 0.99 {
                CertificateVerdict::Pass
            } else if coverage >= 0.95 {
                CertificateVerdict::ConditionalPass
            } else {
                CertificateVerdict::Fail
            }
        }
        SafetyLevel::High => {
            if coverage >= 0.95 {
                CertificateVerdict::Pass
            } else if coverage >= 0.90 {
                CertificateVerdict::ConditionalPass
            } else {
                CertificateVerdict::Fail
            }
        }
        SafetyLevel::Medium | SafetyLevel::Low => {
            if coverage >= 0.90 {
                CertificateVerdict::Pass
            } else if coverage >= 0.80 {
                CertificateVerdict::ConditionalPass
            } else {
                CertificateVerdict::Fail
            }
        }
        SafetyLevel::None => CertificateVerdict::Pass,
    };

    let mut conditions = Vec::new();
    if verdict == CertificateVerdict::ConditionalPass {
        conditions.push(format!(
            "Coverage {:.1}% is below ideal for {} — improve before deployment",
            coverage * 100.0,
            level,
        ));
    }

    SafetyCertificate {
        id: format!("FJ-CERT-{}-001", standard),
        standard,
        level,
        project: project.to_string(),
        version: version.to_string(),
        issue_date: "2026-03-31".to_string(),
        issuer: "Fajar Lang Verification Engine".to_string(),
        verdict,
        total_vcs,
        proven_vcs,
        compliance_notes,
        conditions,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v9_1_certification_standard_display() {
        assert_eq!(format!("{}", CertificationStandard::MisraC), "MISRA-C:2012");
        assert_eq!(format!("{}", CertificationStandard::Do178c), "DO-178C");
        assert_eq!(format!("{}", CertificationStandard::Iso26262), "ISO 26262");
        assert_eq!(format!("{}", CertificationStandard::Iec62304), "IEC 62304");
    }

    #[test]
    fn v9_1_safety_level_ordering() {
        assert!(SafetyLevel::None < SafetyLevel::Low);
        assert!(SafetyLevel::Low < SafetyLevel::Medium);
        assert!(SafetyLevel::Medium < SafetyLevel::High);
        assert!(SafetyLevel::High < SafetyLevel::Critical);
    }

    #[test]
    fn v9_1_safety_level_display() {
        assert!(format!("{}", SafetyLevel::Critical).contains("DAL A"));
        assert!(format!("{}", SafetyLevel::None).contains("QM"));
    }

    #[test]
    fn v9_2_misra_full_compliance() {
        let result = check_misra_compliance(false, false, false, false, false, "safe.fj");
        assert!(result.is_compliant());
        assert!((result.compliance_rate() - 1.0).abs() < f64::EPSILON);
        assert!(result.violations.is_empty());
    }

    #[test]
    fn v9_2_misra_violations() {
        let result = check_misra_compliance(true, true, true, true, true, "unsafe.fj");
        assert!(!result.is_compliant());
        assert_eq!(result.violations.len(), 5);
        assert!((result.compliance_rate() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn v9_2_misra_partial_compliance() {
        let result = check_misra_compliance(true, false, false, false, false, "partial.fj");
        // goto is advisory, so mandatory/required rules pass
        assert!(result.is_compliant());
        assert_eq!(result.rules_compliant, 4);
    }

    #[test]
    fn v9_2_misra_category_display() {
        assert_eq!(format!("{}", MisraCategory::Mandatory), "Mandatory");
        assert_eq!(format!("{}", MisraCategory::Required), "Required");
        assert_eq!(format!("{}", MisraCategory::Advisory), "Advisory");
    }

    #[test]
    fn v9_3_cert_c_all_compliant() {
        let results = check_cert_c(true, true, true, true);
        assert_eq!(results.len(), 4);
        assert!(results
            .iter()
            .all(|r| r.status == ComplianceStatus::Compliant));
    }

    #[test]
    fn v9_3_cert_c_violations() {
        let results = check_cert_c(false, false, true, true);
        assert_eq!(results[0].status, ComplianceStatus::NonCompliant); // INT32-C
        assert_eq!(results[1].status, ComplianceStatus::NonCompliant); // ARR38-C
        assert_eq!(results[2].status, ComplianceStatus::Compliant); // EXP34-C
    }

    #[test]
    fn v9_3_compliance_status_display() {
        assert_eq!(format!("{}", ComplianceStatus::Compliant), "COMPLIANT");
        assert_eq!(
            format!("{}", ComplianceStatus::NonCompliant),
            "NON-COMPLIANT"
        );
        assert_eq!(format!("{}", ComplianceStatus::NotApplicable), "N/A");
    }

    #[test]
    fn v9_4_do178c_dal_a() {
        let evidence = generate_do178c_evidence(DalLevel::DalA, true, true, true, 0.99);
        assert!(evidence.iter().all(|e| e.satisfied));
    }

    #[test]
    fn v9_4_do178c_insufficient_coverage() {
        let evidence = generate_do178c_evidence(DalLevel::DalA, true, false, true, 0.80);
        // MC/DC not provided for DAL A
        let mcdc = evidence.iter().find(|e| e.objective_id == "A-7.2.6");
        assert!(mcdc.is_some());
        assert!(!mcdc.expect("mc/dc evidence").satisfied);
    }

    #[test]
    fn v9_4_dal_level_display() {
        assert_eq!(format!("{}", DalLevel::DalA), "DAL A");
        assert_eq!(format!("{}", DalLevel::DalE), "DAL E");
    }

    #[test]
    fn v9_5_iso26262_full() {
        let methods = generate_iso26262_evidence(AsilLevel::AsilD, true, true, true, true);
        assert!(methods.iter().all(|m| m.provided));
    }

    #[test]
    fn v9_5_iso26262_missing_formal() {
        let methods = generate_iso26262_evidence(AsilLevel::AsilD, true, false, true, true);
        let formal = methods.iter().find(|m| m.name == "Formal verification");
        assert!(formal.is_some());
        assert!(!formal.expect("formal method").provided);
    }

    #[test]
    fn v9_5_asil_display() {
        assert_eq!(format!("{}", AsilLevel::AsilD), "ASIL D");
        assert_eq!(format!("{}", AsilLevel::Qm), "QM");
    }

    #[test]
    fn v9_6_software_safety_class_display() {
        assert_eq!(format!("{}", SoftwareSafetyClass::ClassC), "Class C");
        assert_eq!(format!("{}", SoftwareSafetyClass::ClassA), "Class A");
    }

    #[test]
    fn v9_7_traceability_matrix() {
        let mut matrix = TraceabilityMatrix::default();
        matrix.add_entry(TraceabilityEntry {
            requirement_id: "REQ-001".to_string(),
            requirement_desc: "No buffer overflow".to_string(),
            vc_ids: vec!["VC-1".to_string()],
            test_ids: vec!["TEST-1".to_string()],
            status: TraceStatus::FullyCovered,
        });
        matrix.add_entry(TraceabilityEntry {
            requirement_id: "REQ-002".to_string(),
            requirement_desc: "No null deref".to_string(),
            vc_ids: vec![],
            test_ids: vec![],
            status: TraceStatus::NotCovered,
        });

        assert_eq!(matrix.fully_covered_count(), 1);
        assert!((matrix.coverage_ratio() - 0.5).abs() < f64::EPSILON);
        assert_eq!(matrix.uncovered().len(), 1);
        assert_eq!(matrix.uncovered()[0].requirement_id, "REQ-002");
    }

    #[test]
    fn v9_7_trace_status_display() {
        assert_eq!(format!("{}", TraceStatus::FullyCovered), "FULLY COVERED");
        assert_eq!(format!("{}", TraceStatus::NotCovered), "NOT COVERED");
    }

    #[test]
    fn v9_8_verification_coverage() {
        let coverage = VerificationCoverage {
            total_decisions: 100,
            covered_decisions: 95,
            total_conditions: 200,
            mcdc_conditions: 180,
            function_coverage: HashMap::new(),
        };
        assert!((coverage.decision_coverage() - 0.95).abs() < f64::EPSILON);
        assert!((coverage.mcdc_coverage() - 0.9).abs() < f64::EPSILON);
        assert!(coverage.meets_target(SafetyLevel::High));
        assert!(!coverage.meets_target(SafetyLevel::Critical)); // MCDC < 0.95
    }

    #[test]
    fn v9_8_coverage_meets_none() {
        let coverage = VerificationCoverage {
            total_decisions: 0,
            covered_decisions: 0,
            total_conditions: 0,
            mcdc_conditions: 0,
            function_coverage: HashMap::new(),
        };
        assert!(coverage.meets_target(SafetyLevel::None));
        assert!((coverage.decision_coverage() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn v9_9_audit_trail() {
        let mut trail = AuditTrail::default();
        assert!(trail.is_empty());

        trail.log(AuditEntry {
            timestamp: "2026-03-31T10:00:00Z".to_string(),
            actor: "fj-verify".to_string(),
            action: AuditAction::VerificationStarted,
            details: "Running on main.fj".to_string(),
            state_hash: "abc123".to_string(),
        });
        trail.log(AuditEntry {
            timestamp: "2026-03-31T10:00:05Z".to_string(),
            actor: "fj-verify".to_string(),
            action: AuditAction::VerificationCompleted,
            details: "10/10 VCs proven".to_string(),
            state_hash: "def456".to_string(),
        });

        assert_eq!(trail.len(), 2);
        assert!(!trail.is_empty());
        assert_eq!(
            trail
                .entries_for_action(AuditAction::VerificationStarted)
                .len(),
            1
        );
        assert!(trail.latest().is_some());
        assert_eq!(
            trail.latest().expect("latest entry").action,
            AuditAction::VerificationCompleted
        );
    }

    #[test]
    fn v9_9_audit_action_display() {
        assert_eq!(format!("{}", AuditAction::VerificationStarted), "VERIFY_START");
        assert_eq!(
            format!("{}", AuditAction::CertificateGenerated),
            "CERT_GENERATE"
        );
    }

    #[test]
    fn v9_10_certificate_pass() {
        let cert = generate_certificate(
            CertificationStandard::Do178c,
            SafetyLevel::High,
            "drone-firmware",
            "v1.2.3",
            100,
            98,
            vec!["All MISRA rules satisfied".to_string()],
        );
        assert_eq!(cert.verdict, CertificateVerdict::Pass);
        assert!((cert.verification_coverage() - 0.98).abs() < f64::EPSILON);
    }

    #[test]
    fn v9_10_certificate_conditional() {
        let cert = generate_certificate(
            CertificationStandard::Iso26262,
            SafetyLevel::High,
            "ecu-sw",
            "v2.0",
            100,
            92,
            vec![],
        );
        assert_eq!(cert.verdict, CertificateVerdict::ConditionalPass);
        assert!(!cert.conditions.is_empty());
    }

    #[test]
    fn v9_10_certificate_fail() {
        let cert = generate_certificate(
            CertificationStandard::Do178c,
            SafetyLevel::Critical,
            "flight-ctrl",
            "v0.1",
            100,
            80,
            vec![],
        );
        assert_eq!(cert.verdict, CertificateVerdict::Fail);
    }

    #[test]
    fn v9_10_certificate_display() {
        let cert = generate_certificate(
            CertificationStandard::MisraC,
            SafetyLevel::Medium,
            "iot-sensor",
            "v3.0",
            50,
            48,
            vec!["No goto".to_string()],
        );
        let s = format!("{cert}");
        assert!(s.contains("SAFETY CERTIFICATE"));
        assert!(s.contains("MISRA-C:2012"));
        assert!(s.contains("iot-sensor"));
        assert!(s.contains("PASS"));
    }

    #[test]
    fn v9_10_certificate_verdict_display() {
        assert_eq!(format!("{}", CertificateVerdict::Pass), "PASS");
        assert_eq!(
            format!("{}", CertificateVerdict::ConditionalPass),
            "CONDITIONAL PASS"
        );
        assert_eq!(format!("{}", CertificateVerdict::Fail), "FAIL");
    }
}
