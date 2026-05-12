//! Dependent types — const generics + type-level integers (production surface).
//!
//! v35.7.2 + Action C extension (2026-05-12): The entire dependent-types
//! research surface has been removed per Compass §5.1 verdict ("Bekukan;
//! mungkin tidak kembali"). Three modules deleted:
//!
//! - `tensor_shapes` (v35.7.2, commit 94c61998) — research code for
//!   compile-time tensor shape checking; zero production consumers.
//!   B0: `docs/TENSOR_SHAPES_LOAD_BEARING_B0_FINDINGS.md`.
//! - `arrays` (Action C extension) — research code for compile-time array
//!   length checking + bounds proof; zero production consumers.
//! - `patterns` (Action C extension) — research code for dependent
//!   pattern matching, exhaustiveness, refinement; zero production consumers.
//!   Joint B0: `docs/ARRAYS_PATTERNS_LOAD_BEARING_B0_FINDINGS.md`.
//!   Decision: `docs/decisions/2026-05-12-arrays-patterns-deletion.md`.
//!
//! Only `nat` survives — it is load-bearing for const generics
//! (consumed by `src/const_generics.rs`) which IS production scope.
//!
//! Re-entry: if dependent types are ever reintroduced, recover deleted
//! files via `git log --diff-filter=D -- src/dependent/<name>.rs` and
//! revisit the Compass §5.1 verdict first.

pub mod nat;
