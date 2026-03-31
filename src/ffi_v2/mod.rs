//! FFI v2 — C++, Python, and Rust interop for Fajar Lang.
//!
//! Access entire ecosystems (PyTorch, OpenCV, Tokio) without reimplementing.

pub mod cpp;
pub mod cpp_smart_ptr;
pub mod cpp_stl;
pub mod cpp_templates;
pub mod python;
pub mod python_async;
pub mod python_numpy;
pub mod rust_bridge;
pub mod rust_traits;
pub mod bindgen;
pub mod build_system;
pub mod safety;
pub mod docs;
