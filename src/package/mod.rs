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

// Re-export deployment utilities (containers, observability, runtime mgmt, security).
pub use crate::deployment;

// Re-export package v2 features (workspaces, build scripts, conditional compilation).
pub use crate::package_v2;

/// Returns the list of deployment subsystem names.
pub fn deployment_subsystems() -> Vec<&'static str> {
    vec!["containers", "observability", "runtime_mgmt", "security"]
}

/// Returns the list of package v2 feature names.
pub fn package_v2_features() -> Vec<&'static str> {
    vec![
        "workspaces",
        "build_scripts",
        "conditional",
        "cross_compile",
    ]
}
