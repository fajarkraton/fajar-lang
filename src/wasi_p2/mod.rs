//! WASI Preview 2 — Component Model Implementation.
//!
//! Full WIT parser, type system, component binary format, and WASI P2 interfaces.
//! Built on top of V12 WASI P1 (8 syscalls wired into wasm compiler).
//!
//! ## Module Organization
//! - `wit_lexer` — Tokenizer for `.wit` files
//! - `wit_parser` — Recursive-descent parser producing `WitDocument`
//! - `wit_types` — WIT-to-Fajar type mapping and type system

pub mod wit_lexer;
pub mod wit_parser;
pub mod wit_types;
