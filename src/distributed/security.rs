//! Security & Multi-Tenancy — Sprint D9: TLS everywhere, mTLS, certificate
//! rotation, RBAC, resource quotas, audit logging, secrets management,
//! network policies, sandboxed execution, data encryption in transit.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// D9.1: TLS Everywhere (mTLS)
// ═══════════════════════════════════════════════════════════════════════

/// TLS protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsVersion {
    /// TLS 1.2.
    Tls12,
    /// TLS 1.3.
    Tls13,
}

impl fmt::Display for TlsVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TlsVersion::Tls12 => write!(f, "TLS 1.2"),
            TlsVersion::Tls13 => write!(f, "TLS 1.3"),
        }
    }
}

/// TLS configuration for a node.
#[derive(Debug, Clone)]
pub struct TlsConfig {
    /// Whether TLS is enabled.
    pub enabled: bool,
    /// Minimum TLS version.
    pub min_version: TlsVersion,
    /// Certificate path.
    pub cert_path: String,
    /// Private key path.
    pub key_path: String,
    /// CA certificate path (for mTLS verification).
    pub ca_cert_path: Option<String>,
    /// Whether mutual TLS is required.
    pub mtls_required: bool,
    /// Allowed cipher suites.
    pub cipher_suites: Vec<String>,
}

impl Default for TlsConfig {
    fn default() -> Self {
        TlsConfig {
            enabled: true,
            min_version: TlsVersion::Tls13,
            cert_path: "/etc/fj/tls/cert.pem".to_string(),
            key_path: "/etc/fj/tls/key.pem".to_string(),
            ca_cert_path: Some("/etc/fj/tls/ca.pem".to_string()),
            mtls_required: true,
            cipher_suites: vec![
                "TLS_AES_256_GCM_SHA384".to_string(),
                "TLS_CHACHA20_POLY1305_SHA256".to_string(),
            ],
        }
    }
}

impl TlsConfig {
    /// Validates the TLS configuration.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.enabled {
            if self.cert_path.is_empty() {
                errors.push("cert_path is required when TLS is enabled".to_string());
            }
            if self.key_path.is_empty() {
                errors.push("key_path is required when TLS is enabled".to_string());
            }
            if self.mtls_required && self.ca_cert_path.is_none() {
                errors.push("ca_cert_path is required for mTLS".to_string());
            }
            if self.cipher_suites.is_empty() {
                errors.push("at least one cipher suite must be specified".to_string());
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D9.2: Certificate Rotation
// ═══════════════════════════════════════════════════════════════════════

/// A TLS certificate record.
#[derive(Debug, Clone)]
pub struct Certificate {
    /// Certificate fingerprint (SHA-256 hex).
    pub fingerprint: String,
    /// Subject common name.
    pub subject_cn: String,
    /// Issuer common name.
    pub issuer_cn: String,
    /// Not-before timestamp (ms since epoch).
    pub not_before_ms: u64,
    /// Not-after timestamp (ms since epoch).
    pub not_after_ms: u64,
    /// Whether this certificate is the active one.
    pub active: bool,
}

impl Certificate {
    /// Returns true if the certificate is valid at the given time.
    pub fn is_valid(&self, now_ms: u64) -> bool {
        now_ms >= self.not_before_ms && now_ms < self.not_after_ms
    }

    /// Returns the remaining validity in milliseconds, or 0 if expired.
    pub fn remaining_ms(&self, now_ms: u64) -> u64 {
        self.not_after_ms.saturating_sub(now_ms)
    }
}

/// Manages certificate rotation.
#[derive(Debug)]
pub struct CertRotator {
    /// Certificate history (newest first).
    pub certificates: Vec<Certificate>,
    /// Rotation threshold: rotate when remaining validity is below this (ms).
    pub rotation_threshold_ms: u64,
}

impl CertRotator {
    /// Creates a new certificate rotator.
    pub fn new(rotation_threshold_ms: u64) -> Self {
        CertRotator {
            certificates: Vec::new(),
            rotation_threshold_ms,
        }
    }

    /// Adds a new certificate.
    pub fn add_certificate(&mut self, cert: Certificate) {
        // Deactivate the previous active cert
        for c in &mut self.certificates {
            c.active = false;
        }
        self.certificates.insert(0, cert);
    }

    /// Returns the currently active certificate.
    pub fn active(&self) -> Option<&Certificate> {
        self.certificates.iter().find(|c| c.active)
    }

    /// Checks if rotation is needed at the given time.
    pub fn needs_rotation(&self, now_ms: u64) -> bool {
        if let Some(active) = self.active() {
            active.remaining_ms(now_ms) < self.rotation_threshold_ms
        } else {
            true // No active cert
        }
    }

    /// Returns the number of stored certificates.
    pub fn cert_count(&self) -> usize {
        self.certificates.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D9.3: RBAC (Role-Based Access Control)
// ═══════════════════════════════════════════════════════════════════════

/// A role in the cluster RBAC system.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ClusterRole {
    /// Full cluster control.
    Admin,
    /// Can schedule and manage tasks.
    Scheduler,
    /// Can execute tasks.
    Worker,
    /// Read-only access.
    Reader,
    /// Custom named role.
    Custom(String),
}

impl fmt::Display for ClusterRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClusterRole::Admin => write!(f, "admin"),
            ClusterRole::Scheduler => write!(f, "scheduler"),
            ClusterRole::Worker => write!(f, "worker"),
            ClusterRole::Reader => write!(f, "reader"),
            ClusterRole::Custom(name) => write!(f, "custom:{name}"),
        }
    }
}

/// A permission that can be granted to a role.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Permission {
    /// Submit tasks.
    TaskSubmit,
    /// Cancel/kill tasks.
    TaskCancel,
    /// View task status.
    TaskView,
    /// Manage cluster nodes (join/leave).
    ClusterManage,
    /// View cluster status.
    ClusterView,
    /// Access secrets.
    SecretsRead,
    /// Modify secrets.
    SecretsWrite,
    /// View audit logs.
    AuditRead,
    /// Manage users and roles.
    UserManage,
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Permission::TaskSubmit => write!(f, "task:submit"),
            Permission::TaskCancel => write!(f, "task:cancel"),
            Permission::TaskView => write!(f, "task:view"),
            Permission::ClusterManage => write!(f, "cluster:manage"),
            Permission::ClusterView => write!(f, "cluster:view"),
            Permission::SecretsRead => write!(f, "secrets:read"),
            Permission::SecretsWrite => write!(f, "secrets:write"),
            Permission::AuditRead => write!(f, "audit:read"),
            Permission::UserManage => write!(f, "user:manage"),
        }
    }
}

/// RBAC policy engine.
#[derive(Debug)]
pub struct RbacPolicy {
    /// Role -> set of permissions.
    role_permissions: HashMap<String, Vec<Permission>>,
    /// Identity -> assigned roles.
    identity_roles: HashMap<String, Vec<ClusterRole>>,
}

impl RbacPolicy {
    /// Creates a new RBAC policy with default role definitions.
    pub fn new() -> Self {
        let mut policy = RbacPolicy {
            role_permissions: HashMap::new(),
            identity_roles: HashMap::new(),
        };

        // Default role definitions
        policy.define_role(
            &ClusterRole::Admin,
            vec![
                Permission::TaskSubmit,
                Permission::TaskCancel,
                Permission::TaskView,
                Permission::ClusterManage,
                Permission::ClusterView,
                Permission::SecretsRead,
                Permission::SecretsWrite,
                Permission::AuditRead,
                Permission::UserManage,
            ],
        );
        policy.define_role(
            &ClusterRole::Scheduler,
            vec![
                Permission::TaskSubmit,
                Permission::TaskCancel,
                Permission::TaskView,
                Permission::ClusterView,
            ],
        );
        policy.define_role(
            &ClusterRole::Worker,
            vec![Permission::TaskView, Permission::ClusterView],
        );
        policy.define_role(
            &ClusterRole::Reader,
            vec![Permission::TaskView, Permission::ClusterView],
        );

        policy
    }

    /// Defines permissions for a role.
    pub fn define_role(&mut self, role: &ClusterRole, permissions: Vec<Permission>) {
        self.role_permissions.insert(role.to_string(), permissions);
    }

    /// Assigns a role to an identity.
    pub fn assign_role(&mut self, identity: &str, role: ClusterRole) {
        self.identity_roles
            .entry(identity.to_string())
            .or_default()
            .push(role);
    }

    /// Checks if an identity has a specific permission.
    pub fn check_permission(&self, identity: &str, permission: &Permission) -> bool {
        if let Some(roles) = self.identity_roles.get(identity) {
            for role in roles {
                if let Some(perms) = self.role_permissions.get(&role.to_string()) {
                    if perms.contains(permission) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Returns all permissions for an identity.
    pub fn identity_permissions(&self, identity: &str) -> Vec<&Permission> {
        let mut all_perms = Vec::new();
        if let Some(roles) = self.identity_roles.get(identity) {
            for role in roles {
                if let Some(perms) = self.role_permissions.get(&role.to_string()) {
                    for p in perms {
                        if !all_perms.contains(&p) {
                            all_perms.push(p);
                        }
                    }
                }
            }
        }
        all_perms
    }

    /// Returns the number of defined roles.
    pub fn role_count(&self) -> usize {
        self.role_permissions.len()
    }
}

impl Default for RbacPolicy {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D9.4: Resource Quotas
// ═══════════════════════════════════════════════════════════════════════

/// Resource quota for a tenant.
#[derive(Debug, Clone)]
pub struct ResourceQuota {
    /// Tenant name.
    pub tenant: String,
    /// Maximum CPU cores.
    pub max_cpu: u32,
    /// Maximum memory in MB.
    pub max_memory_mb: u64,
    /// Maximum GPU count.
    pub max_gpu: u32,
    /// Maximum concurrent tasks.
    pub max_tasks: u32,
    /// Maximum storage in MB.
    pub max_storage_mb: u64,
}

/// Current resource usage for a tenant.
#[derive(Debug, Clone, Default)]
pub struct ResourceUsage {
    /// Current CPU cores in use.
    pub cpu: u32,
    /// Current memory in use (MB).
    pub memory_mb: u64,
    /// Current GPUs in use.
    pub gpu: u32,
    /// Current active tasks.
    pub tasks: u32,
    /// Current storage used (MB).
    pub storage_mb: u64,
}

/// Manages per-tenant resource quotas.
#[derive(Debug, Default)]
pub struct QuotaManager {
    /// Quotas per tenant.
    quotas: HashMap<String, ResourceQuota>,
    /// Current usage per tenant.
    usage: HashMap<String, ResourceUsage>,
}

impl QuotaManager {
    /// Creates a new quota manager.
    pub fn new() -> Self {
        QuotaManager::default()
    }

    /// Sets a quota for a tenant.
    pub fn set_quota(&mut self, quota: ResourceQuota) {
        self.quotas.insert(quota.tenant.clone(), quota);
    }

    /// Returns the quota for a tenant.
    pub fn get_quota(&self, tenant: &str) -> Option<&ResourceQuota> {
        self.quotas.get(tenant)
    }

    /// Returns the current usage for a tenant.
    pub fn get_usage(&self, tenant: &str) -> Option<&ResourceUsage> {
        self.usage.get(tenant)
    }

    /// Checks if a resource request would exceed the quota.
    pub fn check(&self, tenant: &str, cpu: u32, memory_mb: u64, gpu: u32) -> Result<(), String> {
        let quota = self
            .quotas
            .get(tenant)
            .ok_or_else(|| format!("no quota defined for tenant: {tenant}"))?;
        let usage = self.usage.get(tenant).cloned().unwrap_or_default();

        if usage.cpu + cpu > quota.max_cpu {
            return Err(format!(
                "CPU quota exceeded: {} + {} > {}",
                usage.cpu, cpu, quota.max_cpu
            ));
        }
        if usage.memory_mb + memory_mb > quota.max_memory_mb {
            return Err(format!(
                "memory quota exceeded: {} + {} > {}",
                usage.memory_mb, memory_mb, quota.max_memory_mb
            ));
        }
        if usage.gpu + gpu > quota.max_gpu {
            return Err(format!(
                "GPU quota exceeded: {} + {} > {}",
                usage.gpu, gpu, quota.max_gpu
            ));
        }
        Ok(())
    }

    /// Allocates resources for a tenant.
    pub fn allocate(
        &mut self,
        tenant: &str,
        cpu: u32,
        memory_mb: u64,
        gpu: u32,
    ) -> Result<(), String> {
        self.check(tenant, cpu, memory_mb, gpu)?;
        let usage = self.usage.entry(tenant.to_string()).or_default();
        usage.cpu += cpu;
        usage.memory_mb += memory_mb;
        usage.gpu += gpu;
        usage.tasks += 1;
        Ok(())
    }

    /// Releases resources for a tenant.
    pub fn release(&mut self, tenant: &str, cpu: u32, memory_mb: u64, gpu: u32) {
        if let Some(usage) = self.usage.get_mut(tenant) {
            usage.cpu = usage.cpu.saturating_sub(cpu);
            usage.memory_mb = usage.memory_mb.saturating_sub(memory_mb);
            usage.gpu = usage.gpu.saturating_sub(gpu);
            if usage.tasks > 0 {
                usage.tasks -= 1;
            }
        }
    }

    /// Returns the number of tenants with quotas.
    pub fn tenant_count(&self) -> usize {
        self.quotas.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D9.5: Audit Logging
// ═══════════════════════════════════════════════════════════════════════

/// An audit event category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditCategory {
    /// Authentication events.
    Auth,
    /// Authorization (permission check) events.
    Authz,
    /// Resource management events.
    Resource,
    /// Task lifecycle events.
    Task,
    /// Configuration changes.
    Config,
    /// Security events (certificate rotation, policy changes).
    Security,
}

impl fmt::Display for AuditCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuditCategory::Auth => write!(f, "AUTH"),
            AuditCategory::Authz => write!(f, "AUTHZ"),
            AuditCategory::Resource => write!(f, "RESOURCE"),
            AuditCategory::Task => write!(f, "TASK"),
            AuditCategory::Config => write!(f, "CONFIG"),
            AuditCategory::Security => write!(f, "SECURITY"),
        }
    }
}

/// An audit log entry.
#[derive(Debug, Clone)]
pub struct AuditEntry {
    /// Entry ID.
    pub id: u64,
    /// Timestamp (ms since epoch).
    pub timestamp_ms: u64,
    /// Category.
    pub category: AuditCategory,
    /// Identity that performed the action.
    pub identity: String,
    /// Action performed.
    pub action: String,
    /// Resource acted upon.
    pub resource: String,
    /// Whether the action was allowed.
    pub allowed: bool,
    /// Additional details.
    pub details: String,
}

/// An audit log.
#[derive(Debug, Default)]
pub struct AuditLog {
    /// All entries.
    entries: Vec<AuditEntry>,
    /// Next entry ID.
    next_id: u64,
}

impl AuditLog {
    /// Creates a new empty audit log.
    pub fn new() -> Self {
        AuditLog {
            entries: Vec::new(),
            next_id: 1,
        }
    }

    /// Records an audit entry.
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &mut self,
        timestamp_ms: u64,
        category: AuditCategory,
        identity: &str,
        action: &str,
        resource: &str,
        allowed: bool,
        details: &str,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push(AuditEntry {
            id,
            timestamp_ms,
            category,
            identity: identity.to_string(),
            action: action.to_string(),
            resource: resource.to_string(),
            allowed,
            details: details.to_string(),
        });
        id
    }

    /// Returns entries filtered by category.
    pub fn by_category(&self, category: &AuditCategory) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.category == *category)
            .collect()
    }

    /// Returns entries filtered by identity.
    pub fn by_identity(&self, identity: &str) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.identity == identity)
            .collect()
    }

    /// Returns denied entries (for security review).
    pub fn denied_entries(&self) -> Vec<&AuditEntry> {
        self.entries.iter().filter(|e| !e.allowed).collect()
    }

    /// Returns the total number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the log is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D9.6: Secrets Management
// ═══════════════════════════════════════════════════════════════════════

/// A secret stored in the secrets vault.
#[derive(Debug, Clone)]
pub struct Secret {
    /// Secret name.
    pub name: String,
    /// Encrypted value (in production; plaintext here for simulation).
    pub value: Vec<u8>,
    /// Version number.
    pub version: u32,
    /// Created timestamp (ms since epoch).
    pub created_ms: u64,
    /// Allowed identities that can read this secret.
    pub allowed_readers: Vec<String>,
}

/// A secrets vault.
#[derive(Debug, Default)]
pub struct SecretsVault {
    /// Secrets indexed by name.
    secrets: HashMap<String, Secret>,
}

impl SecretsVault {
    /// Creates a new empty vault.
    pub fn new() -> Self {
        SecretsVault::default()
    }

    /// Stores a secret, incrementing the version if it already exists.
    pub fn store(
        &mut self,
        name: &str,
        value: Vec<u8>,
        created_ms: u64,
        allowed_readers: Vec<String>,
    ) {
        let version = self.secrets.get(name).map_or(1, |s| s.version + 1);
        self.secrets.insert(
            name.to_string(),
            Secret {
                name: name.to_string(),
                value,
                version,
                created_ms,
                allowed_readers,
            },
        );
    }

    /// Retrieves a secret if the identity is allowed to read it.
    pub fn get(&self, name: &str, identity: &str) -> Result<&Secret, String> {
        let secret = self
            .secrets
            .get(name)
            .ok_or_else(|| format!("secret not found: {name}"))?;
        if secret
            .allowed_readers
            .iter()
            .any(|r| r == identity || r == "*")
        {
            Ok(secret)
        } else {
            Err(format!(
                "identity '{identity}' not authorized to read secret '{name}'"
            ))
        }
    }

    /// Deletes a secret.
    pub fn delete(&mut self, name: &str) -> bool {
        self.secrets.remove(name).is_some()
    }

    /// Lists all secret names.
    pub fn list_names(&self) -> Vec<&str> {
        self.secrets.keys().map(|s| s.as_str()).collect()
    }

    /// Returns the number of stored secrets.
    pub fn count(&self) -> usize {
        self.secrets.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D9.7: Network Policies
// ═══════════════════════════════════════════════════════════════════════

/// A network policy rule.
#[derive(Debug, Clone)]
pub struct NetworkPolicyRule {
    /// Source identity or CIDR.
    pub source: String,
    /// Destination identity or CIDR.
    pub destination: String,
    /// Port (0 = any).
    pub port: u16,
    /// Protocol (tcp, udp, any).
    pub protocol: String,
    /// Whether this rule allows or denies traffic.
    pub action: NetworkAction,
}

/// Network policy action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkAction {
    /// Allow traffic.
    Allow,
    /// Deny traffic.
    Deny,
}

impl fmt::Display for NetworkAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetworkAction::Allow => write!(f, "Allow"),
            NetworkAction::Deny => write!(f, "Deny"),
        }
    }
}

/// A network policy engine.
#[derive(Debug, Default)]
pub struct NetworkPolicyEngine {
    /// Policy rules in evaluation order.
    pub rules: Vec<NetworkPolicyRule>,
    /// Default action when no rule matches.
    pub default_action: Option<NetworkAction>,
}

impl NetworkPolicyEngine {
    /// Creates a new engine with a default-deny policy.
    pub fn new_default_deny() -> Self {
        NetworkPolicyEngine {
            rules: Vec::new(),
            default_action: Some(NetworkAction::Deny),
        }
    }

    /// Creates a new engine with a default-allow policy.
    pub fn new_default_allow() -> Self {
        NetworkPolicyEngine {
            rules: Vec::new(),
            default_action: Some(NetworkAction::Allow),
        }
    }

    /// Adds a rule.
    pub fn add_rule(&mut self, rule: NetworkPolicyRule) {
        self.rules.push(rule);
    }

    /// Evaluates whether traffic from source to destination on port is allowed.
    pub fn evaluate(&self, source: &str, destination: &str, port: u16) -> NetworkAction {
        for rule in &self.rules {
            let source_match = rule.source == "*" || rule.source == source;
            let dest_match = rule.destination == "*" || rule.destination == destination;
            let port_match = rule.port == 0 || rule.port == port;

            if source_match && dest_match && port_match {
                return rule.action;
            }
        }
        self.default_action.unwrap_or(NetworkAction::Deny)
    }

    /// Returns the number of rules.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D9.8: Sandboxed Execution
// ═══════════════════════════════════════════════════════════════════════

/// Sandbox constraints for a task.
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Maximum CPU time in milliseconds.
    pub max_cpu_ms: u64,
    /// Maximum memory in MB.
    pub max_memory_mb: u64,
    /// Maximum disk I/O in MB.
    pub max_disk_mb: u64,
    /// Whether network access is allowed.
    pub allow_network: bool,
    /// Whether filesystem write is allowed.
    pub allow_fs_write: bool,
    /// Allowed syscalls (empty = all blocked).
    pub allowed_syscalls: Vec<String>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        SandboxConfig {
            max_cpu_ms: 60_000,
            max_memory_mb: 512,
            max_disk_mb: 100,
            allow_network: false,
            allow_fs_write: false,
            allowed_syscalls: vec![
                "read".to_string(),
                "write".to_string(),
                "mmap".to_string(),
                "exit".to_string(),
            ],
        }
    }
}

/// Result of a sandbox check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxViolation {
    /// CPU limit exceeded.
    CpuExceeded { used_ms: u64, limit_ms: u64 },
    /// Memory limit exceeded.
    MemoryExceeded { used_mb: u64, limit_mb: u64 },
    /// Disk limit exceeded.
    DiskExceeded { used_mb: u64, limit_mb: u64 },
    /// Disallowed network access.
    NetworkDenied,
    /// Disallowed filesystem write.
    FsWriteDenied,
    /// Disallowed syscall.
    SyscallDenied(String),
}

impl fmt::Display for SandboxViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SandboxViolation::CpuExceeded { used_ms, limit_ms } => {
                write!(f, "CPU exceeded: {used_ms}ms > {limit_ms}ms")
            }
            SandboxViolation::MemoryExceeded { used_mb, limit_mb } => {
                write!(f, "Memory exceeded: {used_mb}MB > {limit_mb}MB")
            }
            SandboxViolation::DiskExceeded { used_mb, limit_mb } => {
                write!(f, "Disk exceeded: {used_mb}MB > {limit_mb}MB")
            }
            SandboxViolation::NetworkDenied => write!(f, "Network access denied"),
            SandboxViolation::FsWriteDenied => write!(f, "Filesystem write denied"),
            SandboxViolation::SyscallDenied(name) => {
                write!(f, "Syscall denied: {name}")
            }
        }
    }
}

/// A sandbox enforcer that checks resource usage against limits.
#[derive(Debug)]
pub struct SandboxEnforcer {
    /// Sandbox configuration.
    pub config: SandboxConfig,
    /// Violations detected.
    pub violations: Vec<SandboxViolation>,
}

impl SandboxEnforcer {
    /// Creates a new sandbox enforcer.
    pub fn new(config: SandboxConfig) -> Self {
        SandboxEnforcer {
            config,
            violations: Vec::new(),
        }
    }

    /// Checks CPU usage.
    pub fn check_cpu(&mut self, used_ms: u64) -> bool {
        if used_ms > self.config.max_cpu_ms {
            self.violations.push(SandboxViolation::CpuExceeded {
                used_ms,
                limit_ms: self.config.max_cpu_ms,
            });
            false
        } else {
            true
        }
    }

    /// Checks memory usage.
    pub fn check_memory(&mut self, used_mb: u64) -> bool {
        if used_mb > self.config.max_memory_mb {
            self.violations.push(SandboxViolation::MemoryExceeded {
                used_mb,
                limit_mb: self.config.max_memory_mb,
            });
            false
        } else {
            true
        }
    }

    /// Checks if a syscall is allowed.
    pub fn check_syscall(&mut self, name: &str) -> bool {
        if self.config.allowed_syscalls.iter().any(|s| s == name) {
            true
        } else {
            self.violations
                .push(SandboxViolation::SyscallDenied(name.to_string()));
            false
        }
    }

    /// Checks if network access is allowed.
    pub fn check_network(&mut self) -> bool {
        if self.config.allow_network {
            true
        } else {
            self.violations.push(SandboxViolation::NetworkDenied);
            false
        }
    }

    /// Returns true if no violations have occurred.
    pub fn is_clean(&self) -> bool {
        self.violations.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D9.9: Data Encryption in Transit
// ═══════════════════════════════════════════════════════════════════════

/// Encryption algorithm for data in transit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitEncryption {
    /// No encryption.
    None,
    /// AES-256-GCM.
    Aes256Gcm,
    /// ChaCha20-Poly1305.
    ChaCha20Poly1305,
}

impl fmt::Display for TransitEncryption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransitEncryption::None => write!(f, "None"),
            TransitEncryption::Aes256Gcm => write!(f, "AES-256-GCM"),
            TransitEncryption::ChaCha20Poly1305 => write!(f, "ChaCha20-Poly1305"),
        }
    }
}

/// Simulates encrypting data for transit.
pub fn encrypt_transit(data: &[u8], algorithm: TransitEncryption) -> Vec<u8> {
    match algorithm {
        TransitEncryption::None => data.to_vec(),
        TransitEncryption::Aes256Gcm | TransitEncryption::ChaCha20Poly1305 => {
            // Simulated: prepend algorithm tag + nonce placeholder + XOR with 0x42
            let mut buf = Vec::with_capacity(data.len() + 13);
            buf.push(algorithm as u8);
            buf.extend_from_slice(&[0u8; 12]); // 12-byte nonce placeholder
            for &b in data {
                buf.push(b ^ 0x42);
            }
            buf
        }
    }
}

/// Simulates decrypting data from transit.
pub fn decrypt_transit(data: &[u8], algorithm: TransitEncryption) -> Result<Vec<u8>, String> {
    match algorithm {
        TransitEncryption::None => Ok(data.to_vec()),
        TransitEncryption::Aes256Gcm | TransitEncryption::ChaCha20Poly1305 => {
            if data.len() < 13 {
                return Err("encrypted data too short".to_string());
            }
            Ok(data[13..].iter().map(|b| b ^ 0x42).collect())
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D9.10: Integration — Security Manager
// ═══════════════════════════════════════════════════════════════════════

/// A security manager combining all D9 security features.
#[derive(Debug)]
pub struct SecurityManager {
    /// TLS configuration.
    pub tls: TlsConfig,
    /// Certificate rotator.
    pub cert_rotator: CertRotator,
    /// RBAC policy.
    pub rbac: RbacPolicy,
    /// Resource quotas.
    pub quotas: QuotaManager,
    /// Audit log.
    pub audit: AuditLog,
    /// Secrets vault.
    pub secrets: SecretsVault,
    /// Network policies.
    pub network: NetworkPolicyEngine,
    /// Transit encryption algorithm.
    pub transit_encryption: TransitEncryption,
}

impl SecurityManager {
    /// Creates a new security manager with default settings.
    pub fn new() -> Self {
        SecurityManager {
            tls: TlsConfig::default(),
            cert_rotator: CertRotator::new(7 * 24 * 3600 * 1000), // 7 days
            rbac: RbacPolicy::new(),
            quotas: QuotaManager::new(),
            audit: AuditLog::new(),
            secrets: SecretsVault::new(),
            network: NetworkPolicyEngine::new_default_deny(),
            transit_encryption: TransitEncryption::Aes256Gcm,
        }
    }

    /// Performs an authenticated, authorized action with audit logging.
    pub fn authorized_action(
        &mut self,
        identity: &str,
        permission: &Permission,
        resource: &str,
        action: &str,
        timestamp_ms: u64,
    ) -> bool {
        let allowed = self.rbac.check_permission(identity, permission);
        self.audit.record(
            timestamp_ms,
            AuditCategory::Authz,
            identity,
            action,
            resource,
            allowed,
            &format!("permission={permission}, allowed={allowed}"),
        );
        allowed
    }

    /// Returns a security status summary.
    pub fn status_summary(&self) -> String {
        format!(
            "tls={}, mtls={}, roles={}, tenants={}, secrets={}, audit_entries={}, network_rules={}",
            self.tls.enabled,
            self.tls.mtls_required,
            self.rbac.role_count(),
            self.quotas.tenant_count(),
            self.secrets.count(),
            self.audit.len(),
            self.network.rule_count(),
        )
    }
}

impl Default for SecurityManager {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // D9.1 — TLS Config
    #[test]
    fn d9_1_tls_config_default_valid() {
        let config = TlsConfig::default();
        assert!(config.validate().is_ok());
        assert!(config.mtls_required);
        assert_eq!(config.min_version, TlsVersion::Tls13);
    }

    #[test]
    fn d9_1_tls_config_missing_ca() {
        let mut config = TlsConfig::default();
        config.ca_cert_path = None;
        let errors = config.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("ca_cert_path")));
    }

    // D9.2 — Certificate Rotation
    #[test]
    fn d9_2_cert_rotation_check() {
        let mut rotator = CertRotator::new(7 * 24 * 3600 * 1000); // 7 days in ms
        rotator.add_certificate(Certificate {
            fingerprint: "abc123".to_string(),
            subject_cn: "fj-node-1".to_string(),
            issuer_cn: "fj-ca".to_string(),
            not_before_ms: 0,
            not_after_ms: 30 * 24 * 3600 * 1000, // 30 days
            active: true,
        });

        // Day 1: no rotation needed
        assert!(!rotator.needs_rotation(1 * 24 * 3600 * 1000));

        // Day 25: within 7-day threshold -> needs rotation
        assert!(rotator.needs_rotation(25 * 24 * 3600 * 1000));
    }

    #[test]
    fn d9_2_cert_validity() {
        let cert = Certificate {
            fingerprint: "def456".to_string(),
            subject_cn: "test".to_string(),
            issuer_cn: "ca".to_string(),
            not_before_ms: 1000,
            not_after_ms: 5000,
            active: true,
        };
        assert!(cert.is_valid(3000));
        assert!(!cert.is_valid(5000)); // Expired
        assert_eq!(cert.remaining_ms(3000), 2000);
    }

    // D9.3 — RBAC
    #[test]
    fn d9_3_rbac_admin_has_all_permissions() {
        let mut rbac = RbacPolicy::new();
        rbac.assign_role("root@cluster", ClusterRole::Admin);

        assert!(rbac.check_permission("root@cluster", &Permission::TaskSubmit));
        assert!(rbac.check_permission("root@cluster", &Permission::SecretsWrite));
        assert!(rbac.check_permission("root@cluster", &Permission::UserManage));
    }

    #[test]
    fn d9_3_rbac_reader_limited() {
        let mut rbac = RbacPolicy::new();
        rbac.assign_role("viewer@cluster", ClusterRole::Reader);

        assert!(rbac.check_permission("viewer@cluster", &Permission::TaskView));
        assert!(!rbac.check_permission("viewer@cluster", &Permission::TaskSubmit));
        assert!(!rbac.check_permission("viewer@cluster", &Permission::SecretsRead));
    }

    #[test]
    fn d9_3_rbac_unknown_identity() {
        let rbac = RbacPolicy::new();
        assert!(!rbac.check_permission("nobody", &Permission::TaskView));
    }

    // D9.4 — Resource Quotas
    #[test]
    fn d9_4_quota_enforcement() {
        let mut qm = QuotaManager::new();
        qm.set_quota(ResourceQuota {
            tenant: "team-a".to_string(),
            max_cpu: 8,
            max_memory_mb: 16384,
            max_gpu: 2,
            max_tasks: 10,
            max_storage_mb: 100_000,
        });

        assert!(qm.allocate("team-a", 4, 8192, 1).is_ok());
        assert!(qm.allocate("team-a", 4, 8192, 1).is_ok());
        assert!(qm.allocate("team-a", 1, 0, 0).is_err()); // CPU exceeded
    }

    #[test]
    fn d9_4_quota_release() {
        let mut qm = QuotaManager::new();
        qm.set_quota(ResourceQuota {
            tenant: "team-b".to_string(),
            max_cpu: 4,
            max_memory_mb: 8192,
            max_gpu: 1,
            max_tasks: 5,
            max_storage_mb: 50_000,
        });

        qm.allocate("team-b", 4, 4096, 1).unwrap();
        qm.release("team-b", 2, 2048, 0);
        assert!(qm.allocate("team-b", 2, 2048, 0).is_ok());
    }

    // D9.5 — Audit Logging
    #[test]
    fn d9_5_audit_record_and_filter() {
        let mut audit = AuditLog::new();
        audit.record(
            1000,
            AuditCategory::Auth,
            "user@cluster",
            "login",
            "cluster",
            true,
            "success",
        );
        audit.record(
            2000,
            AuditCategory::Authz,
            "user@cluster",
            "submit_task",
            "task-1",
            false,
            "denied: insufficient permissions",
        );

        assert_eq!(audit.len(), 2);
        assert_eq!(audit.by_category(&AuditCategory::Auth).len(), 1);
        assert_eq!(audit.denied_entries().len(), 1);
    }

    // D9.6 — Secrets Management
    #[test]
    fn d9_6_secrets_store_and_retrieve() {
        let mut vault = SecretsVault::new();
        vault.store(
            "db_password",
            b"s3cret".to_vec(),
            1000,
            vec!["admin".to_string()],
        );

        let secret = vault.get("db_password", "admin").unwrap();
        assert_eq!(secret.value, b"s3cret");
        assert_eq!(secret.version, 1);

        // Unauthorized reader
        assert!(vault.get("db_password", "hacker").is_err());
    }

    #[test]
    fn d9_6_secrets_versioning() {
        let mut vault = SecretsVault::new();
        vault.store("key", b"v1".to_vec(), 1000, vec!["*".to_string()]);
        vault.store("key", b"v2".to_vec(), 2000, vec!["*".to_string()]);

        let secret = vault.get("key", "anyone").unwrap();
        assert_eq!(secret.version, 2);
        assert_eq!(secret.value, b"v2");
    }

    // D9.7 — Network Policies
    #[test]
    fn d9_7_network_policy_default_deny() {
        let engine = NetworkPolicyEngine::new_default_deny();
        assert_eq!(
            engine.evaluate("worker-1", "scheduler", 9000),
            NetworkAction::Deny
        );
    }

    #[test]
    fn d9_7_network_policy_rules() {
        let mut engine = NetworkPolicyEngine::new_default_deny();
        engine.add_rule(NetworkPolicyRule {
            source: "worker-1".to_string(),
            destination: "scheduler".to_string(),
            port: 9000,
            protocol: "tcp".to_string(),
            action: NetworkAction::Allow,
        });

        assert_eq!(
            engine.evaluate("worker-1", "scheduler", 9000),
            NetworkAction::Allow
        );
        assert_eq!(
            engine.evaluate("worker-1", "scheduler", 8080),
            NetworkAction::Deny
        );
        assert_eq!(
            engine.evaluate("attacker", "scheduler", 9000),
            NetworkAction::Deny
        );
    }

    // D9.8 — Sandboxed Execution
    #[test]
    fn d9_8_sandbox_cpu_check() {
        let mut sb = SandboxEnforcer::new(SandboxConfig::default());
        assert!(sb.check_cpu(30_000));
        assert!(!sb.check_cpu(120_000)); // Exceeds 60s limit
        assert_eq!(sb.violations.len(), 1);
    }

    #[test]
    fn d9_8_sandbox_syscall_check() {
        let mut sb = SandboxEnforcer::new(SandboxConfig::default());
        assert!(sb.check_syscall("read"));
        assert!(!sb.check_syscall("execve")); // Not in allowed list
        assert!(!sb.is_clean());
    }

    // D9.9 — Data Encryption in Transit
    #[test]
    fn d9_9_encrypt_decrypt_aes() {
        let data = b"sensitive model weights";
        let encrypted = encrypt_transit(data, TransitEncryption::Aes256Gcm);
        assert_ne!(encrypted, data);
        let decrypted = decrypt_transit(&encrypted, TransitEncryption::Aes256Gcm).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn d9_9_encrypt_none_passthrough() {
        let data = b"plaintext";
        let encrypted = encrypt_transit(data, TransitEncryption::None);
        assert_eq!(encrypted, data);
    }

    // D9.10 — Security Manager
    #[test]
    fn d9_10_security_manager_authorized_action() {
        let mut mgr = SecurityManager::new();
        mgr.rbac.assign_role("admin@fj", ClusterRole::Admin);
        mgr.rbac.assign_role("reader@fj", ClusterRole::Reader);

        assert!(mgr.authorized_action(
            "admin@fj",
            &Permission::TaskSubmit,
            "task-1",
            "submit",
            1000
        ));
        assert!(!mgr.authorized_action(
            "reader@fj",
            &Permission::TaskSubmit,
            "task-2",
            "submit",
            2000
        ));
        assert_eq!(mgr.audit.len(), 2);
        assert_eq!(mgr.audit.denied_entries().len(), 1);
    }

    #[test]
    fn d9_10_security_manager_status() {
        let mgr = SecurityManager::new();
        let status = mgr.status_summary();
        assert!(status.contains("tls=true"));
        assert!(status.contains("mtls=true"));
    }
}
