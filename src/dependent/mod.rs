//! Dependent types — const generics, type-level integers, compile-time shape
//! verification for arrays.
//!
//! v35.7.2 (2026-05-12): `tensor_shapes` module removed per Compass §5.1
//! dependent-types verdict and `docs/TENSOR_SHAPES_LOAD_BEARING_B0_FINDINGS.md`
//! (dead in production — only consumed by sprint DT3 tests, never by
//! production analyzer/codegen). `arrays` + `patterns` follow the same
//! dead pattern; their deletion deferred to a follow-up Action C if/when
//! Fajar decides to commit fully to the Compass §5.1 dep-types freeze.

pub mod arrays;
pub mod nat;
pub mod patterns;
