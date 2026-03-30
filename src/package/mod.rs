//! Package management for Fajar Lang projects.
//!
//! Handles `fj.toml` project manifests, project scaffolding (`fj new`),
//! and project builds (`fj build`).

pub mod audit;
pub mod client;
pub mod docgen;
pub mod documentation;
pub mod manifest;
pub mod portal;
pub mod pubgrub;
pub mod publish;
pub mod registry;
pub mod registry_cli;
pub mod registry_db;
pub mod resolver;
pub mod sbom;
pub mod server;
pub mod signing;
pub mod v12;
pub mod verification;

pub use manifest::{ProjectConfig, find_project_root};
pub use publish::{publish_to_registry, validate_package};
pub use registry::{Registry, SemVer, VersionConstraint};
pub use resolver::{LockFile, resolve_full};
