//! Package management for Fajar Lang projects.
//!
//! Handles `fj.toml` project manifests, project scaffolding (`fj new`),
//! and project builds (`fj build`).

pub mod manifest;
pub mod publish;
pub mod registry;
pub mod resolver;

pub use manifest::{find_project_root, ProjectConfig};
pub use publish::{publish_to_registry, validate_package};
pub use registry::{Registry, SemVer, VersionConstraint};
pub use resolver::{resolve_full, LockFile};
