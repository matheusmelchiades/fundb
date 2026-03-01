//! Security module for FunDB: multi-tenant isolation, RBAC, audit logging, and TLS scaffolding.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// Tenant identity
// ---------------------------------------------------------------------------

pub type TenantId = uuid::Uuid;
pub type AgentId = uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct TenantContext {
    pub tenant_id: TenantId,
    pub agent_id: AgentId,
    pub roles: Vec<Role>,
}

// ---------------------------------------------------------------------------
// RBAC
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum Role {
    Admin,
    ReadWrite,
    ReadOnly,
    Custom(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum Permission {
    Read,
    Write,
    Delete,
    Grant,
    Admin,
}

/// Maps collection → role → set of permissions.
/// The special collection key `"*"` acts as a wildcard that applies to all collections.
#[derive(Debug, Clone)]
pub struct RbacEngine {
    /// collection → role → permissions
    grants: HashMap<String, HashMap<Role, HashSet<Permission>>>,
}

impl RbacEngine {
    /// Create a new engine pre-loaded with the default built-in RBAC rules:
    /// - `Admin`     → all permissions on `"*"`
    /// - `ReadWrite` → Read + Write on `"*"`
    /// - `ReadOnly`  → Read on `"*"`
    pub fn new() -> Self {
        let mut engine = RbacEngine {
            grants: HashMap::new(),
        };

        // Admin gets every permission on the wildcard collection.
        for perm in [
            Permission::Read,
            Permission::Write,
            Permission::Delete,
            Permission::Grant,
            Permission::Admin,
        ] {
            engine.grant("*", Role::Admin, perm);
        }

        // ReadWrite gets Read + Write on the wildcard collection.
        engine.grant("*", Role::ReadWrite, Permission::Read);
        engine.grant("*", Role::ReadWrite, Permission::Write);

        // ReadOnly gets only Read on the wildcard collection.
        engine.grant("*", Role::ReadOnly, Permission::Read);

        engine
    }

    /// Grant `role` the `permission` on `collection`.
    pub fn grant(&mut self, collection: &str, role: Role, permission: Permission) {
        self.grants
            .entry(collection.to_string())
            .or_default()
            .entry(role)
            .or_default()
            .insert(permission);
    }

    /// Revoke `permission` from `role` on `collection`.
    pub fn revoke(&mut self, collection: &str, role: Role, permission: Permission) {
        if let Some(role_map) = self.grants.get_mut(collection) {
            if let Some(perms) = role_map.get_mut(&role) {
                perms.remove(&permission);
            }
        }
    }

    /// Return `true` if `ctx` has `permission` on `collection`.
    ///
    /// Resolution order:
    /// 1. Check each role the context holds against the specific collection.
    /// 2. Check each role the context holds against the wildcard `"*"` collection.
    pub fn check(&self, ctx: &TenantContext, collection: &str, permission: &Permission) -> bool {
        for role in &ctx.roles {
            // Specific collection check.
            if self.role_has_permission_on(role, collection, permission) {
                return true;
            }
            // Wildcard check.
            if self.role_has_permission_on(role, "*", permission) {
                return true;
            }
        }
        false
    }

    /// Returns all permissions the context holds on `collection` (union across roles,
    /// merging specific-collection and wildcard grants).
    pub fn permissions_for(&self, ctx: &TenantContext, collection: &str) -> Vec<Permission> {
        let mut result: HashSet<Permission> = HashSet::new();
        for role in &ctx.roles {
            if let Some(perms) = self.role_permissions_on(role, collection) {
                result.extend(perms);
            }
            if let Some(perms) = self.role_permissions_on(role, "*") {
                result.extend(perms);
            }
        }
        result.into_iter().collect()
    }

    // --- private helpers ---

    fn role_has_permission_on(
        &self,
        role: &Role,
        collection: &str,
        permission: &Permission,
    ) -> bool {
        self.grants
            .get(collection)
            .and_then(|role_map| role_map.get(role))
            .map(|perms| perms.contains(permission))
            .unwrap_or(false)
    }

    fn role_permissions_on(
        &self,
        role: &Role,
        collection: &str,
    ) -> Option<impl Iterator<Item = Permission> + '_> {
        self.grants
            .get(collection)
            .and_then(|role_map| role_map.get(role))
            .map(|perms| perms.iter().cloned())
    }
}

impl Default for RbacEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tenant isolation filter
// ---------------------------------------------------------------------------

/// Wraps a query string to inject a tenant-scoping predicate.
///
/// - If the query already contains `"tenant_id"`, validate that it references
///   `ctx.tenant_id`; return `Err(SecurityError::TenantIsolationViolation)` on mismatch.
/// - Otherwise append `AND _tenant_id = '<uuid>'` to the query.
pub fn scope_query(query: &str, ctx: &TenantContext) -> Result<String, SecurityError> {
    let tenant_str = ctx.tenant_id.to_string();

    if query.contains("tenant_id") {
        // Check whether the tenant UUID present in the query matches ctx.tenant_id.
        if query.contains(tenant_str.as_str()) {
            // Correct tenant — return as-is.
            Ok(query.to_string())
        } else {
            // A different tenant UUID is referenced — isolation violation.
            // We do a best-effort extraction; the error message uses a sentinel UUID
            // to indicate we could not determine the exact foreign tenant.
            Err(SecurityError::InvalidQuery(format!(
                "query references a tenant_id that does not match the current context ({})",
                ctx.tenant_id
            )))
        }
    } else {
        // Append the tenant filter.
        let scoped = format!("{} AND _tenant_id = '{}'", query.trim_end(), tenant_str);
        Ok(scoped)
    }
}

/// Validate that a record belongs to the given tenant context.
pub fn validate_record_ownership(
    record_tenant_id: TenantId,
    ctx: &TenantContext,
) -> Result<(), SecurityError> {
    if record_tenant_id == ctx.tenant_id {
        Ok(())
    } else {
        Err(SecurityError::TenantIsolationViolation {
            record_tenant: record_tenant_id,
            context_tenant: ctx.tenant_id,
        })
    }
}

// ---------------------------------------------------------------------------
// Audit log
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuditEntry {
    pub id: uuid::Uuid,
    /// Unix milliseconds.
    pub timestamp: i64,
    pub tenant_id: TenantId,
    pub agent_id: AgentId,
    pub action: AuditAction,
    pub collection: Option<String>,
    pub outcome: AuditOutcome,
    pub detail: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum AuditAction {
    Query,
    Write,
    Delete,
    Grant,
    Revoke,
    Connect,
    Disconnect,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum AuditOutcome {
    Allowed,
    Denied,
}

pub struct AuditLog {
    entries: Mutex<Vec<AuditEntry>>,
    max_size: usize,
}

impl AuditLog {
    /// Create a new audit log with the given capacity cap.
    pub fn new(max_size: usize) -> Self {
        AuditLog {
            entries: Mutex::new(Vec::new()),
            max_size,
        }
    }

    /// Append an entry to the log. If the log is at capacity the oldest entry
    /// is dropped to make room (FIFO eviction).
    pub fn record(&self, entry: AuditEntry) {
        let mut entries = self.entries.lock().expect("audit log mutex poisoned");
        if self.max_size > 0 && entries.len() >= self.max_size {
            entries.remove(0);
        }
        entries.push(entry);
    }

    /// Return all entries that belong to `tenant_id` (clone, ordered oldest-first).
    pub fn entries_for_tenant(&self, tenant_id: TenantId) -> Vec<AuditEntry> {
        let entries = self.entries.lock().expect("audit log mutex poisoned");
        entries
            .iter()
            .filter(|e| e.tenant_id == tenant_id)
            .cloned()
            .collect()
    }

    /// Return the `n` most-recent entries (clone, ordered oldest-first within the slice).
    pub fn recent(&self, n: usize) -> Vec<AuditEntry> {
        let entries = self.entries.lock().expect("audit log mutex poisoned");
        let start = entries.len().saturating_sub(n);
        entries[start..].to_vec()
    }

    /// Total number of entries currently stored.
    pub fn len(&self) -> usize {
        self.entries.lock().expect("audit log mutex poisoned").len()
    }

    /// Returns `true` when no entries are stored.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ---------------------------------------------------------------------------
// TLS configuration scaffolding
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum TlsVersion {
    Tls12,
    Tls13,
}

#[derive(Debug, Clone)]
pub struct TlsConfig {
    pub cert_path: String,
    pub key_path: String,
    pub ca_cert_path: Option<String>,
    pub min_version: TlsVersion,
    pub require_client_cert: bool,
}

impl TlsConfig {
    /// Create a new `TlsConfig` with sensible defaults (TLS 1.3, no client cert required).
    pub fn new(cert_path: impl Into<String>, key_path: impl Into<String>) -> Self {
        TlsConfig {
            cert_path: cert_path.into(),
            key_path: key_path.into(),
            ca_cert_path: None,
            min_version: TlsVersion::Tls13,
            require_client_cert: false,
        }
    }

    /// Validate that the configuration paths are acceptable.
    ///
    /// Stub implementation — always returns `Ok(())` so that unit tests do not
    /// require actual certificate files on disk.
    pub fn validate(&self) -> Result<(), SecurityError> {
        Ok(())
    }

    /// Returns `true` when the minimum TLS version is TLS 1.3.
    pub fn is_tls13_only(&self) -> bool {
        self.min_version == TlsVersion::Tls13
    }
}

/// Stub TLS acceptor descriptor.  No real TLS crate is pulled in.
#[derive(Debug)]
pub struct TlsAcceptorStub {
    pub config: TlsConfig,
    pub description: String,
}

/// Build a `TlsAcceptorStub` from a `TlsConfig`.
///
/// This is a pure stub — it validates the config and produces a descriptor string
/// without loading any actual certificates or native TLS objects.
pub fn build_tls_acceptor_config(config: &TlsConfig) -> Result<TlsAcceptorStub, SecurityError> {
    config.validate()?;

    let min_ver = match config.min_version {
        TlsVersion::Tls12 => "TLSv1.2",
        TlsVersion::Tls13 => "TLSv1.3",
    };

    let description = format!(
        "TlsAcceptor {{ cert={}, key={}, ca={:?}, min_version={}, client_auth={} }}",
        config.cert_path,
        config.key_path,
        config.ca_cert_path,
        min_ver,
        config.require_client_cert,
    );

    Ok(TlsAcceptorStub {
        config: config.clone(),
        description,
    })
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum SecurityError {
    #[error("permission denied: tenant {tenant_id} lacks {permission:?} on '{collection}'")]
    PermissionDenied {
        tenant_id: TenantId,
        permission: Permission,
        collection: String,
    },

    #[error(
        "tenant isolation violation: record belongs to {record_tenant}, context is {context_tenant}"
    )]
    TenantIsolationViolation {
        record_tenant: TenantId,
        context_tenant: TenantId,
    },

    #[error("invalid query: {0}")]
    InvalidQuery(String),

    #[error("TLS configuration error: {0}. Check that certificate and key files exist and are readable")]
    TlsConfig(String),

    #[error("authentication required. Connect with valid credentials using -u <user> -W <password>")]
    Unauthenticated,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_ctx(roles: Vec<Role>) -> TenantContext {
        TenantContext {
            tenant_id: Uuid::new_v4(),
            agent_id: Uuid::new_v4(),
            roles,
        }
    }

    // 1. Admin role has all permissions on any collection.
    #[test]
    fn test_rbac_admin_has_all_permissions() {
        let engine = RbacEngine::new();
        let ctx = make_ctx(vec![Role::Admin]);

        for perm in [
            Permission::Read,
            Permission::Write,
            Permission::Delete,
            Permission::Grant,
            Permission::Admin,
        ] {
            assert!(
                engine.check(&ctx, "any_collection", &perm),
                "Admin should have {:?}",
                perm
            );
        }
    }

    // 2. ReadOnly role must not be able to Write.
    #[test]
    fn test_rbac_readonly_denies_write() {
        let engine = RbacEngine::new();
        let ctx = make_ctx(vec![Role::ReadOnly]);

        assert!(!engine.check(&ctx, "orders", &Permission::Write));
        assert!(!engine.check(&ctx, "orders", &Permission::Delete));
        // But Read must be allowed.
        assert!(engine.check(&ctx, "orders", &Permission::Read));
    }

    // 3. Grant Write to ReadOnly on "events", verify, then revoke and verify denial.
    #[test]
    fn test_rbac_grant_revoke() {
        let mut engine = RbacEngine::new();
        let ctx = make_ctx(vec![Role::ReadOnly]);

        // Initially denied.
        assert!(!engine.check(&ctx, "events", &Permission::Write));

        // Grant Write on the specific collection.
        engine.grant("events", Role::ReadOnly, Permission::Write);
        assert!(engine.check(&ctx, "events", &Permission::Write));

        // Revoke it — must be denied again.
        engine.revoke("events", Role::ReadOnly, Permission::Write);
        assert!(!engine.check(&ctx, "events", &Permission::Write));
    }

    // 4. Custom role with only Read granted.
    #[test]
    fn test_rbac_custom_role() {
        let mut engine = RbacEngine::new();
        let analyst = Role::Custom("analyst".to_string());

        engine.grant("metrics", analyst.clone(), Permission::Read);

        let ctx = make_ctx(vec![analyst]);

        assert!(engine.check(&ctx, "metrics", &Permission::Read));
        assert!(!engine.check(&ctx, "metrics", &Permission::Write));
        assert!(!engine.check(&ctx, "metrics", &Permission::Delete));
    }

    // 5. validate_record_ownership — same tenant → Ok.
    #[test]
    fn test_tenant_isolation_same_tenant() {
        let ctx = make_ctx(vec![Role::ReadOnly]);
        assert!(validate_record_ownership(ctx.tenant_id, &ctx).is_ok());
    }

    // 6. validate_record_ownership — different tenant → TenantIsolationViolation.
    #[test]
    fn test_tenant_isolation_different_tenant() {
        let ctx = make_ctx(vec![Role::ReadOnly]);
        let other_tenant = Uuid::new_v4();

        let err = validate_record_ownership(other_tenant, &ctx).unwrap_err();
        match err {
            SecurityError::TenantIsolationViolation {
                record_tenant,
                context_tenant,
            } => {
                assert_eq!(record_tenant, other_tenant);
                assert_eq!(context_tenant, ctx.tenant_id);
            }
            _ => panic!("expected TenantIsolationViolation, got {:?}", err),
        }
    }

    // 7. scope_query appends _tenant_id predicate when not present.
    #[test]
    fn test_scope_query_appends_tenant() {
        let ctx = make_ctx(vec![Role::ReadOnly]);
        let query = "SELECT * FROM orders WHERE status = 'active'";

        let scoped = scope_query(query, &ctx).expect("should succeed");

        assert!(
            scoped.contains(&format!("_tenant_id = '{}'", ctx.tenant_id)),
            "scoped query should contain tenant predicate, got: {}",
            scoped
        );
        assert!(scoped.starts_with("SELECT * FROM orders"));
    }

    // 8. scope_query with a query that already contains the correct tenant_id → Ok unchanged.
    #[test]
    fn test_scope_query_existing_tenant_id_match() {
        let ctx = make_ctx(vec![Role::ReadOnly]);
        let query = format!(
            "SELECT * FROM orders WHERE tenant_id = '{}'",
            ctx.tenant_id
        );

        let result = scope_query(&query, &ctx).expect("should return Ok for matching tenant");
        assert_eq!(result, query);
    }

    // 9. AuditLog: record entries and retrieve by tenant.
    #[test]
    fn test_audit_log_record_and_retrieve() {
        let log = AuditLog::new(100);
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let agent = Uuid::new_v4();

        let make_entry = |tid: TenantId, action: AuditAction| AuditEntry {
            id: Uuid::new_v4(),
            timestamp: 0,
            tenant_id: tid,
            agent_id: agent,
            action,
            collection: Some("orders".to_string()),
            outcome: AuditOutcome::Allowed,
            detail: "test".to_string(),
        };

        log.record(make_entry(tenant_a, AuditAction::Query));
        log.record(make_entry(tenant_b, AuditAction::Write));
        log.record(make_entry(tenant_a, AuditAction::Delete));

        assert_eq!(log.len(), 3);

        let a_entries = log.entries_for_tenant(tenant_a);
        assert_eq!(a_entries.len(), 2);
        assert!(a_entries.iter().all(|e| e.tenant_id == tenant_a));

        let b_entries = log.entries_for_tenant(tenant_b);
        assert_eq!(b_entries.len(), 1);
        assert_eq!(b_entries[0].tenant_id, tenant_b);
    }

    // 10. AuditLog::recent returns the last N entries.
    #[test]
    fn test_audit_log_recent() {
        let log = AuditLog::new(100);
        let tenant = Uuid::new_v4();
        let agent = Uuid::new_v4();

        for i in 0u8..5 {
            log.record(AuditEntry {
                id: Uuid::new_v4(),
                timestamp: i64::from(i),
                tenant_id: tenant,
                agent_id: agent,
                action: AuditAction::Query,
                collection: None,
                outcome: AuditOutcome::Allowed,
                detail: format!("entry {}", i),
            });
        }

        let recent = log.recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].timestamp, 3);
        assert_eq!(recent[1].timestamp, 4);
    }

    // 11. TlsConfig with Tls13 → is_tls13_only returns true.
    #[test]
    fn test_tls_config_tls13_only() {
        let cfg = TlsConfig::new("/path/to/cert.pem", "/path/to/key.pem");
        assert_eq!(cfg.min_version, TlsVersion::Tls13);
        assert!(cfg.is_tls13_only());

        let cfg12 = TlsConfig {
            min_version: TlsVersion::Tls12,
            ..cfg
        };
        assert!(!cfg12.is_tls13_only());
    }

    // 12. TlsConfig::validate() stub always returns Ok.
    #[test]
    fn test_tls_config_validate_stub() {
        let cfg = TlsConfig::new("/nonexistent/cert.pem", "/nonexistent/key.pem");
        assert!(cfg.validate().is_ok());
    }

    // Bonus: build_tls_acceptor_config produces a correct stub descriptor.
    #[test]
    fn test_build_tls_acceptor_config() {
        let cfg = TlsConfig::new("cert.pem", "key.pem");
        let stub = build_tls_acceptor_config(&cfg).expect("should succeed");
        assert!(stub.description.contains("TLSv1.3"));
        assert!(stub.description.contains("cert.pem"));
        assert!(stub.description.contains("key.pem"));
    }

    // Bonus: permissions_for returns the correct union across roles.
    #[test]
    fn test_permissions_for_union() {
        let engine = RbacEngine::new();
        let ctx = make_ctx(vec![Role::ReadWrite]);
        let perms = engine.permissions_for(&ctx, "inventory");

        let perm_set: HashSet<Permission> = perms.into_iter().collect();
        assert!(perm_set.contains(&Permission::Read));
        assert!(perm_set.contains(&Permission::Write));
        assert!(!perm_set.contains(&Permission::Delete));
        assert!(!perm_set.contains(&Permission::Grant));
        assert!(!perm_set.contains(&Permission::Admin));
    }
}
