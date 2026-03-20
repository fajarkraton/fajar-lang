//! Workspaces — workspace definition, shared dependencies, workspace
//! build/test, inter-package deps, inheritance, selective build,
//! metadata, virtual workspaces.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S21.1: Workspace Definition
// ═══════════════════════════════════════════════════════════════════════

/// A workspace definition from root `fj.toml`.
#[derive(Debug, Clone)]
pub struct Workspace {
    /// Root directory of the workspace.
    pub root: String,
    /// Member package paths (relative to root).
    pub members: Vec<String>,
    /// Exclude patterns.
    pub exclude: Vec<String>,
    /// Whether this is a virtual workspace (no root package).
    pub is_virtual: bool,
    /// Shared dependencies.
    pub shared_deps: HashMap<String, DepSpec>,
    /// Workspace metadata.
    pub metadata: HashMap<String, String>,
    /// Inherited fields.
    pub inheritance: WorkspaceInheritance,
}

impl Workspace {
    /// Creates a new workspace.
    pub fn new(root: &str, members: Vec<String>) -> Self {
        Self {
            root: root.to_string(),
            members,
            exclude: Vec::new(),
            is_virtual: false,
            shared_deps: HashMap::new(),
            metadata: HashMap::new(),
            inheritance: WorkspaceInheritance::default(),
        }
    }

    /// Creates a virtual workspace (no root package).
    pub fn new_virtual(root: &str, members: Vec<String>) -> Self {
        let mut ws = Self::new(root, members);
        ws.is_virtual = true;
        ws
    }

    /// Checks if a path is a member of this workspace.
    pub fn is_member(&self, path: &str) -> bool {
        self.members.iter().any(|m| m == path)
    }

    /// Adds a shared dependency.
    pub fn add_shared_dep(&mut self, name: &str, spec: DepSpec) {
        self.shared_deps.insert(name.to_string(), spec);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S21.2: Shared Dependencies
// ═══════════════════════════════════════════════════════════════════════

/// A dependency specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepSpec {
    /// Version requirement (semver).
    pub version: String,
    /// Optional features.
    pub features: Vec<String>,
    /// Whether it's optional.
    pub optional: bool,
}

impl DepSpec {
    /// Creates a simple version-only dep.
    pub fn version(ver: &str) -> Self {
        Self {
            version: ver.to_string(),
            features: Vec::new(),
            optional: false,
        }
    }

    /// Creates a dep with features.
    pub fn with_features(ver: &str, features: Vec<String>) -> Self {
        Self {
            version: ver.to_string(),
            features,
            optional: false,
        }
    }
}

/// Resolves shared dependencies for all workspace members.
pub fn resolve_shared_deps(
    workspace: &Workspace,
    member_deps: &HashMap<String, Vec<(String, String)>>,
) -> HashMap<String, DepSpec> {
    let mut resolved = workspace.shared_deps.clone();

    // Merge member-specific deps
    for deps in member_deps.values() {
        for (name, version) in deps {
            resolved
                .entry(name.clone())
                .or_insert_with(|| DepSpec::version(version));
        }
    }

    resolved
}

// ═══════════════════════════════════════════════════════════════════════
// S21.3-S21.4: Workspace Build & Test
// ═══════════════════════════════════════════════════════════════════════

/// A workspace member package.
#[derive(Debug, Clone)]
pub struct WorkspaceMember {
    /// Package name.
    pub name: String,
    /// Package path (relative to workspace root).
    pub path: String,
    /// Dependencies on other workspace members.
    pub local_deps: Vec<String>,
}

/// Computes build order using topological sort.
pub fn build_order(members: &[WorkspaceMember]) -> Result<Vec<usize>, WorkspaceError> {
    let n = members.len();
    let mut in_degree = vec![0usize; n];
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];

    let name_to_idx: HashMap<&str, usize> = members
        .iter()
        .enumerate()
        .map(|(i, m)| (m.name.as_str(), i))
        .collect();

    for (i, member) in members.iter().enumerate() {
        for dep in &member.local_deps {
            if let Some(&dep_idx) = name_to_idx.get(dep.as_str()) {
                adj[dep_idx].push(i);
                in_degree[i] += 1;
            }
        }
    }

    let mut queue: Vec<usize> = (0..n).filter(|&i| in_degree[i] == 0).collect();
    let mut order = Vec::with_capacity(n);

    while let Some(node) = queue.pop() {
        order.push(node);
        for &next in &adj[node] {
            in_degree[next] -= 1;
            if in_degree[next] == 0 {
                queue.push(next);
            }
        }
    }

    if order.len() == n {
        Ok(order)
    } else {
        Err(WorkspaceError::CyclicDependency)
    }
}

/// Workspace error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceError {
    /// Cyclic dependency between workspace members.
    CyclicDependency,
    /// Member not found.
    MemberNotFound(String),
    /// Duplicate member names.
    DuplicateMember(String),
    /// Invalid workspace configuration.
    InvalidConfig(String),
}

impl fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkspaceError::CyclicDependency => {
                write!(f, "Cyclic dependency detected in workspace members")
            }
            WorkspaceError::MemberNotFound(name) => {
                write!(f, "Workspace member not found: {name}")
            }
            WorkspaceError::DuplicateMember(name) => {
                write!(f, "Duplicate workspace member: {name}")
            }
            WorkspaceError::InvalidConfig(msg) => {
                write!(f, "Invalid workspace config: {msg}")
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S21.5: Inter-Package Dependencies
// ═══════════════════════════════════════════════════════════════════════

/// A local path dependency between workspace members.
#[derive(Debug, Clone)]
pub struct PathDep {
    /// Package name.
    pub name: String,
    /// Relative path (e.g., "../sibling").
    pub path: String,
}

/// Resolves a path dependency within the workspace.
pub fn resolve_path_dep(workspace: &Workspace, dep: &PathDep) -> Result<String, WorkspaceError> {
    // Find the member whose name matches
    if workspace.is_member(&dep.path) {
        Ok(format!("{}/{}", workspace.root, dep.path))
    } else {
        // Try matching by name
        workspace
            .members
            .iter()
            .find(|m| m.ends_with(&dep.name) || *m == &dep.name)
            .map(|m| format!("{}/{m}", workspace.root))
            .ok_or_else(|| WorkspaceError::MemberNotFound(dep.name.clone()))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S21.6: Workspace Inheritance
// ═══════════════════════════════════════════════════════════════════════

/// Fields that can be inherited from workspace root.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceInheritance {
    /// Edition (e.g., "2026").
    pub edition: Option<String>,
    /// License (e.g., "MIT").
    pub license: Option<String>,
    /// Repository URL.
    pub repository: Option<String>,
    /// Authors list.
    pub authors: Vec<String>,
    /// Rust version (MSRV equivalent).
    pub fj_version: Option<String>,
}

/// Applies inheritance — member inherits from workspace if not set locally.
pub fn apply_inheritance(
    workspace_inherit: &WorkspaceInheritance,
    member_edition: &Option<String>,
    member_license: &Option<String>,
) -> (String, String) {
    let edition = member_edition
        .clone()
        .or_else(|| workspace_inherit.edition.clone())
        .unwrap_or_else(|| "2026".to_string());
    let license = member_license
        .clone()
        .or_else(|| workspace_inherit.license.clone())
        .unwrap_or_else(|| "MIT".to_string());
    (edition, license)
}

// ═══════════════════════════════════════════════════════════════════════
// S21.7: Selective Build
// ═══════════════════════════════════════════════════════════════════════

/// Finds a member and all its transitive local dependencies.
pub fn select_with_deps(
    members: &[WorkspaceMember],
    target: &str,
) -> Result<Vec<usize>, WorkspaceError> {
    let name_to_idx: HashMap<&str, usize> = members
        .iter()
        .enumerate()
        .map(|(i, m)| (m.name.as_str(), i))
        .collect();

    let &start = name_to_idx
        .get(target)
        .ok_or_else(|| WorkspaceError::MemberNotFound(target.to_string()))?;

    let mut selected = Vec::new();
    let mut visited = vec![false; members.len()];
    let mut stack = vec![start];

    while let Some(idx) = stack.pop() {
        if visited[idx] {
            continue;
        }
        visited[idx] = true;
        selected.push(idx);

        for dep in &members[idx].local_deps {
            if let Some(&dep_idx) = name_to_idx.get(dep.as_str()) {
                if !visited[dep_idx] {
                    stack.push(dep_idx);
                }
            }
        }
    }

    // Return in dependency order (deps first)
    selected.reverse();
    Ok(selected)
}

// ═══════════════════════════════════════════════════════════════════════
// S21.8-S21.9: Metadata & Virtual Workspace (covered above)
// ═══════════════════════════════════════════════════════════════════════

/// Validates a workspace configuration.
pub fn validate_workspace(workspace: &Workspace) -> Result<(), WorkspaceError> {
    if workspace.members.is_empty() {
        return Err(WorkspaceError::InvalidConfig(
            "Workspace must have at least one member".into(),
        ));
    }

    // Check for duplicate member names
    let mut seen = std::collections::HashSet::new();
    for member in &workspace.members {
        if !seen.insert(member.clone()) {
            return Err(WorkspaceError::DuplicateMember(member.clone()));
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S21.1 — Workspace Definition
    #[test]
    fn s21_1_workspace_new() {
        let ws = Workspace::new("/project", vec!["core".into(), "cli".into()]);
        assert_eq!(ws.members.len(), 2);
        assert!(!ws.is_virtual);
        assert!(ws.is_member("core"));
        assert!(!ws.is_member("other"));
    }

    // S21.2 — Shared Dependencies
    #[test]
    fn s21_2_shared_deps() {
        let mut ws = Workspace::new("/proj", vec!["a".into(), "b".into()]);
        ws.add_shared_dep("serde", DepSpec::version("1.0"));
        assert_eq!(ws.shared_deps.get("serde").unwrap().version, "1.0");
    }

    #[test]
    fn s21_2_resolve_shared() {
        let mut ws = Workspace::new("/proj", vec!["a".into()]);
        ws.add_shared_dep("serde", DepSpec::version("1.0"));
        let mut member_deps = HashMap::new();
        member_deps.insert("a".into(), vec![("tokio".into(), "1.0".into())]);
        let resolved = resolve_shared_deps(&ws, &member_deps);
        assert!(resolved.contains_key("serde"));
        assert!(resolved.contains_key("tokio"));
    }

    // S21.3 — Build Order
    #[test]
    fn s21_3_build_order() {
        let members = vec![
            WorkspaceMember {
                name: "cli".into(),
                path: "cli".into(),
                local_deps: vec!["core".into()],
            },
            WorkspaceMember {
                name: "core".into(),
                path: "core".into(),
                local_deps: vec![],
            },
        ];
        let order = build_order(&members).unwrap();
        // core (idx=1) should come before cli (idx=0)
        let core_pos = order.iter().position(|&i| i == 1).unwrap();
        let cli_pos = order.iter().position(|&i| i == 0).unwrap();
        assert!(core_pos < cli_pos);
    }

    #[test]
    fn s21_3_cyclic_dependency() {
        let members = vec![
            WorkspaceMember {
                name: "a".into(),
                path: "a".into(),
                local_deps: vec!["b".into()],
            },
            WorkspaceMember {
                name: "b".into(),
                path: "b".into(),
                local_deps: vec!["a".into()],
            },
        ];
        assert!(matches!(
            build_order(&members),
            Err(WorkspaceError::CyclicDependency)
        ));
    }

    // S21.5 — Inter-Package Dependencies
    #[test]
    fn s21_5_path_dep_resolve() {
        let ws = Workspace::new("/proj", vec!["core".into(), "cli".into()]);
        let dep = PathDep {
            name: "core".into(),
            path: "core".into(),
        };
        let resolved = resolve_path_dep(&ws, &dep).unwrap();
        assert!(resolved.contains("core"));
    }

    // S21.6 — Workspace Inheritance
    #[test]
    fn s21_6_inheritance() {
        let inherit = WorkspaceInheritance {
            edition: Some("2026".into()),
            license: Some("MIT".into()),
            ..Default::default()
        };
        let (edition, license) = apply_inheritance(&inherit, &None, &None);
        assert_eq!(edition, "2026");
        assert_eq!(license, "MIT");
    }

    #[test]
    fn s21_6_member_overrides() {
        let inherit = WorkspaceInheritance {
            edition: Some("2026".into()),
            license: Some("MIT".into()),
            ..Default::default()
        };
        let (edition, license) =
            apply_inheritance(&inherit, &Some("2025".into()), &Some("Apache-2.0".into()));
        assert_eq!(edition, "2025");
        assert_eq!(license, "Apache-2.0");
    }

    // S21.7 — Selective Build
    #[test]
    fn s21_7_select_with_deps() {
        let members = vec![
            WorkspaceMember {
                name: "core".into(),
                path: "core".into(),
                local_deps: vec![],
            },
            WorkspaceMember {
                name: "net".into(),
                path: "net".into(),
                local_deps: vec!["core".into()],
            },
            WorkspaceMember {
                name: "cli".into(),
                path: "cli".into(),
                local_deps: vec!["net".into()],
            },
        ];
        let selected = select_with_deps(&members, "cli").unwrap();
        assert_eq!(selected.len(), 3); // cli + net + core
    }

    #[test]
    fn s21_7_select_leaf() {
        let members = vec![
            WorkspaceMember {
                name: "core".into(),
                path: "core".into(),
                local_deps: vec![],
            },
            WorkspaceMember {
                name: "other".into(),
                path: "other".into(),
                local_deps: vec![],
            },
        ];
        let selected = select_with_deps(&members, "core").unwrap();
        assert_eq!(selected.len(), 1);
    }

    // S21.8-S21.9 — Metadata & Virtual Workspace
    #[test]
    fn s21_8_workspace_metadata() {
        let mut ws = Workspace::new("/proj", vec!["a".into()]);
        ws.metadata.insert("ci".into(), "github-actions".into());
        assert_eq!(ws.metadata.get("ci").unwrap(), "github-actions");
    }

    #[test]
    fn s21_9_virtual_workspace() {
        let ws = Workspace::new_virtual("/proj", vec!["a".into(), "b".into()]);
        assert!(ws.is_virtual);
    }

    // S21.10 — Validation
    #[test]
    fn s21_10_validate_ok() {
        let ws = Workspace::new("/proj", vec!["a".into()]);
        assert!(validate_workspace(&ws).is_ok());
    }

    #[test]
    fn s21_10_validate_empty() {
        let ws = Workspace::new("/proj", vec![]);
        assert!(matches!(
            validate_workspace(&ws),
            Err(WorkspaceError::InvalidConfig(_))
        ));
    }

    #[test]
    fn s21_10_validate_duplicate() {
        let ws = Workspace::new("/proj", vec!["a".into(), "a".into()]);
        assert!(matches!(
            validate_workspace(&ws),
            Err(WorkspaceError::DuplicateMember(_))
        ));
    }

    #[test]
    fn s21_10_error_display() {
        assert!(
            WorkspaceError::CyclicDependency
                .to_string()
                .contains("Cyclic")
        );
        assert!(
            WorkspaceError::MemberNotFound("x".into())
                .to_string()
                .contains("x")
        );
    }
}
