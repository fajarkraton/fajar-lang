// Nightly clippy allow-list — lints that differ between stable and nightly.
// TODO: remove each allow when the lint stabilizes and code is updated.
// - collapsible_if: Edition 2024 expanded scope (nightly 2025-03+)
#![allow(clippy::collapsible_if)]

//! # Fajar Lang
//!
//! A statically-typed systems programming language for OS development and AI/ML.
//!
//! ## Architecture
//!
//! ```text
//! Source (.fj) → Lexer → Parser → Analyzer → Interpreter
//!                                              ├── OS Runtime
//!                                              └── ML Runtime
//! ```
//!
//! ## Modules
//!
//! - [`lexer`] — Tokenization (`&str` → `Vec<Token>`)
//! - [`parser`] — Parsing (`Vec<Token>` → `Program` AST)
//! - [`analyzer`] — Semantic analysis (Phase 2+)
//! - [`interpreter`] — Tree-walking evaluation
//! - [`runtime`] — OS and ML execution backends

pub mod accelerator;
pub mod analyzer;
pub mod bsp;
pub mod codegen;
pub mod compiler;
pub mod concurrency_v2;
pub mod debugger;
pub mod debugger_v2;
pub mod demos;
pub mod dependent;
pub mod deployment;
pub mod distributed;
pub mod docgen;
pub mod ffi_v2;
pub mod formatter;
pub mod generators_v12;
pub mod gpu_codegen;
pub mod gui;
pub mod hw;
pub mod interpreter;
pub mod iot;
pub mod jit;
pub mod lexer;
pub mod lsp;
pub mod lsp_v2;
pub mod lsp_v3;
pub mod macros;
pub mod macros_v12;
pub mod ml_advanced;
pub mod package;
pub mod package_v2;
pub mod parser;
pub mod playground;
pub mod plugin;
pub mod profiler;
pub mod rt_pipeline;
pub mod rtos;
pub mod runtime;
pub mod selfhost;
pub mod stdlib;
pub mod stdlib_v3;
pub mod testing;
pub mod verify;
pub mod vm;
pub mod wasi_v12;

use analyzer::SemanticError;
use interpreter::RuntimeError;
use lexer::LexError;
use parser::ParseError;

/// Top-level error type for the Fajar Lang pipeline.
///
/// Each variant wraps the actual error types from each compilation stage.
/// Errors are collected (not fail-fast) to provide comprehensive diagnostics.
///
/// # Examples
///
/// ```
/// use fajar_lang::interpreter::Interpreter;
///
/// let mut interp = Interpreter::new();
/// let result = interp.eval_source("let x: i64 = 42");
/// assert!(result.is_ok());
/// ```
#[derive(Debug)]
pub enum FjError {
    /// Tokenization errors (e.g., unterminated string, invalid number).
    Lex(Vec<LexError>),
    /// Syntax errors (e.g., unexpected token, missing delimiter).
    Parse(Vec<ParseError>),
    /// Type/scope errors (e.g., undefined variable, type mismatch).
    Semantic(Vec<SemanticError>),
    /// Execution errors (e.g., division by zero, stack overflow).
    Runtime(RuntimeError),
}

impl std::fmt::Display for FjError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FjError::Lex(errors) => {
                for e in errors {
                    writeln!(f, "lex error: {e}")?;
                }
                Ok(())
            }
            FjError::Parse(errors) => {
                for e in errors {
                    writeln!(f, "parse error: {e}")?;
                }
                Ok(())
            }
            FjError::Semantic(errors) => {
                for e in errors {
                    writeln!(f, "semantic error: {e}")?;
                }
                Ok(())
            }
            FjError::Runtime(e) => write!(f, "runtime error: {e}"),
        }
    }
}

impl std::error::Error for FjError {}

impl From<Vec<LexError>> for FjError {
    fn from(errors: Vec<LexError>) -> Self {
        FjError::Lex(errors)
    }
}

impl From<Vec<ParseError>> for FjError {
    fn from(errors: Vec<ParseError>) -> Self {
        FjError::Parse(errors)
    }
}

impl From<Vec<SemanticError>> for FjError {
    fn from(errors: Vec<SemanticError>) -> Self {
        FjError::Semantic(errors)
    }
}

impl From<RuntimeError> for FjError {
    fn from(error: RuntimeError) -> Self {
        FjError::Runtime(error)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// miette diagnostic integration
// ═══════════════════════════════════════════════════════════════════════

use lexer::token::Span;

/// A diagnostic error that wraps any Fajar Lang error with source code
/// for beautiful miette-powered error display.
#[derive(Debug)]
pub struct FjDiagnostic {
    /// The error message.
    pub message: String,
    /// The error code (e.g., "LE001", "SE004").
    pub code: Option<String>,
    /// Severity label ("error" or "warning").
    pub severity: miette::Severity,
    /// The highlighted source span.
    pub span: Option<Span>,
    /// Help text.
    pub help: Option<String>,
    /// The source code.
    pub source_code: miette::NamedSource<String>,
}

impl std::fmt::Display for FjDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for FjDiagnostic {}

impl miette::Diagnostic for FjDiagnostic {
    fn code<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        self.code
            .as_ref()
            .map(|c| Box::new(c.clone()) as Box<dyn std::fmt::Display>)
    }

    fn severity(&self) -> Option<miette::Severity> {
        Some(self.severity)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        self.help
            .as_ref()
            .map(|h| Box::new(h.clone()) as Box<dyn std::fmt::Display>)
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        Some(&self.source_code)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        self.span.map(|s| {
            let label = miette::LabeledSpan::at(s.start..s.end, &self.message);
            Box::new(std::iter::once(label)) as Box<dyn Iterator<Item = miette::LabeledSpan>>
        })
    }
}

impl FjDiagnostic {
    /// Creates a diagnostic from a LexError.
    pub fn from_lex_error(e: &LexError, filename: &str, source: &str) -> Self {
        let (code, span, help) = match e {
            LexError::UnexpectedChar { span, .. } => (
                "LE001",
                *span,
                Some("check for typos or unsupported characters".into()),
            ),
            LexError::UnterminatedString { span, .. } => (
                "LE002",
                *span,
                Some("add closing `\"` to terminate the string".into()),
            ),
            LexError::UnterminatedBlockComment { span, .. } => (
                "LE003",
                *span,
                Some("add `*/` to close the block comment".into()),
            ),
            LexError::InvalidNumber { span, .. } => {
                ("LE004", *span, Some("check the number format".into()))
            }
            LexError::InvalidEscape { span, .. } => (
                "LE005",
                *span,
                Some("valid escapes: \\n \\t \\r \\\\ \\' \\\" \\0".into()),
            ),
            LexError::NumberOverflow { span, .. } => (
                "LE006",
                *span,
                Some("number exceeds maximum value for integer type".into()),
            ),
            LexError::EmptyCharLiteral { span, .. } => (
                "LE007",
                *span,
                Some("put a character between the quotes: 'a'".into()),
            ),
            LexError::MultiCharLiteral { span, .. } => (
                "LE008",
                *span,
                Some("character literal must contain exactly one character".into()),
            ),
            LexError::InvalidCharLiteral { span, .. } => (
                "LE004",
                *span,
                Some("check character literal syntax".into()),
            ),
            LexError::UnknownAnnotation { span, .. } => (
                "LE001",
                *span,
                Some("valid annotations: @kernel @device @safe @unsafe @ffi".into()),
            ),
        };
        FjDiagnostic {
            message: e.to_string(),
            code: Some(code.into()),
            severity: miette::Severity::Error,
            span: Some(span),
            help,
            source_code: miette::NamedSource::new(filename, source.to_string()),
        }
    }

    /// Creates a diagnostic from a ParseError.
    pub fn from_parse_error(e: &ParseError, filename: &str, source: &str) -> Self {
        let (code, span) = match e {
            ParseError::UnexpectedToken { span, .. } => ("PE001", Some(*span)),
            ParseError::ExpectedExpression { span, .. } => ("PE002", Some(*span)),
            ParseError::ExpectedType { span, .. } => ("PE003", Some(*span)),
            ParseError::ExpectedPattern { span, .. } => ("PE004", Some(*span)),
            ParseError::ExpectedIdentifier { span, .. } => ("PE005", Some(*span)),
            ParseError::UnexpectedEof { span } => ("PE006", Some(*span)),
            ParseError::InvalidPattern { span, .. } => ("PE007", Some(*span)),
            ParseError::DuplicateField { span, .. } => ("PE008", Some(*span)),
            ParseError::TrailingSeparator { span, .. } => ("PE009", Some(*span)),
            ParseError::InvalidAnnotation { span, .. } => ("PE010", Some(*span)),
            ParseError::ModuleFileNotFound { span, .. } => ("PE011", Some(*span)),
        };
        FjDiagnostic {
            message: e.to_string(),
            code: Some(code.into()),
            severity: miette::Severity::Error,
            span,
            help: None,
            source_code: miette::NamedSource::new(filename, source.to_string()),
        }
    }

    /// Creates a diagnostic from a SemanticError.
    pub fn from_semantic_error(e: &SemanticError, filename: &str, source: &str) -> Self {
        let code = match e {
            SemanticError::UndefinedVariable { .. } => "SE001",
            SemanticError::UndefinedFunction { .. } => "SE002",
            SemanticError::UndefinedType { .. } => "SE003",
            SemanticError::TypeMismatch { .. } => "SE004",
            SemanticError::ArgumentCountMismatch { .. } => "SE005",
            SemanticError::DuplicateDefinition { .. } => "SE006",
            SemanticError::ImmutableAssignment { .. } => "SE007",
            SemanticError::MissingReturn { .. } => "SE008",
            SemanticError::UnusedVariable { .. } => "SE009",
            SemanticError::UnreachableCode { .. } => "SE010",
            SemanticError::NonExhaustiveMatch { .. } => "SE011",
            SemanticError::MissingField { .. } => "SE012",
            SemanticError::BreakOutsideLoop { .. } => "SE007",
            SemanticError::ReturnOutsideFunction { .. } => "SE007",
            SemanticError::HeapAllocInKernel { .. } => "KE001",
            SemanticError::TensorInKernel { .. } => "KE002",
            SemanticError::DeviceCallInKernel { .. } => "KE003",
            SemanticError::RawPointerInDevice { .. } => "DE001",
            SemanticError::KernelCallInDevice { .. } => "DE002",
            SemanticError::FfiUnsafeType { .. } => "SE013",
            SemanticError::UseAfterMove { .. } => "ME001",
            SemanticError::MoveWhileBorrowed { .. } => "ME003",
            SemanticError::MutBorrowConflict { .. } => "ME004",
            SemanticError::ImmBorrowConflict { .. } => "ME005",
            SemanticError::TraitBoundNotSatisfied { .. } => "SE014",
            SemanticError::UnknownTrait { .. } => "SE015",
            SemanticError::CannotInferType { .. } => "SE013",
            SemanticError::TraitMethodSignatureMismatch { .. } => "SE016",
            SemanticError::TensorShapeMismatch { .. } => "TE001",
            SemanticError::HardwareAccessInSafe { .. } => "SE020",
            SemanticError::KernelCallInSafe { .. } => "SE021",
            SemanticError::DeviceCallInSafe { .. } => "SE022",
            SemanticError::AsmInSafeContext { .. } => "KE005",
            SemanticError::AsmInDeviceContext { .. } => "KE006",
            SemanticError::AwaitOutsideAsync { .. } => "SE017",
            SemanticError::NotSendType { .. } => "SE018",
            SemanticError::UnusedImport { .. } => "SE019",
            SemanticError::UnreachablePattern { .. } => "SE020",
            SemanticError::LifetimeMismatch { .. } => "SE021",
            SemanticError::LifetimeConflict { .. } => "ME009",
            SemanticError::DanglingReference { .. } => "ME010",
            SemanticError::RawPointerInNpu { .. } => "NE001",
            SemanticError::HeapAllocInNpu { .. } => "NE002",
            SemanticError::OsPrimitiveInNpu { .. } => "NE003",
            SemanticError::KernelCallInNpu { .. } => "NE004",
            SemanticError::LinearNotConsumed { .. } => "ME010",
            SemanticError::UndeclaredEffect { .. } => "EE001",
            SemanticError::UnknownEffect { .. } => "EE002",
            SemanticError::EffectForbiddenInContext { .. } => "EE006",
            SemanticError::ResumeOutsideHandler { .. } => "EE005",
            SemanticError::DuplicateEffectDecl { .. } => "EE004",
            SemanticError::MessageTooLarge { .. } => "IPC001",
            SemanticError::IpcTypeMismatch { .. } => "IPC002",
            SemanticError::IndexOutOfBounds { .. } => "SE022",
        };
        let severity = if e.is_warning() {
            miette::Severity::Warning
        } else {
            miette::Severity::Error
        };
        FjDiagnostic {
            message: e.to_string(),
            code: Some(code.into()),
            severity,
            span: Some(e.span()),
            help: e.hint(),
            source_code: miette::NamedSource::new(filename, source.to_string()),
        }
    }

    /// Creates a diagnostic from a RuntimeError (no source span).
    pub fn from_runtime_error(e: &RuntimeError, filename: &str, source: &str) -> Self {
        FjDiagnostic {
            message: e.to_string(),
            code: None,
            severity: miette::Severity::Error,
            span: None,
            help: None,
            source_code: miette::NamedSource::new(filename, source.to_string()),
        }
    }

    /// Renders this diagnostic to stderr using miette's graphical handler.
    pub fn eprint(&self) {
        use miette::GraphicalReportHandler;
        let handler = GraphicalReportHandler::new();
        let mut output = String::new();
        let _ = handler.render_report(&mut output, self);
        eprint!("{output}");
    }
}
