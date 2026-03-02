use std::sync::Arc;

use fundb_server::{
    scope_query, validate_record_ownership, AuditAction, AuditEntry, AuditLog, AuditOutcome,
    Counter, Histogram, InMemoryExporter, MetricsRegistry, Permission, RbacEngine, Role,
    TenantContext, TlsConfig, TlsVersion, Tracer,
};
use uuid::Uuid;

/// Helper: create a TenantContext.
fn ctx(tenant_id: Uuid, roles: Vec<Role>) -> TenantContext {
    TenantContext {
        tenant_id,
        agent_id: Uuid::now_v7(),
        roles,
    }
}

/// Helper: create an AuditEntry.
fn audit_entry(tenant_id: Uuid, action: AuditAction, outcome: AuditOutcome) -> AuditEntry {
    AuditEntry {
        id: Uuid::now_v7(),
        timestamp: 0,
        tenant_id,
        agent_id: Uuid::now_v7(),
        action,
        collection: Some("test".to_string()),
        outcome,
        detail: "test entry".to_string(),
    }
}

// ---------------------------------------------------------------------------
// 1. RBAC — Admin has full access
// ---------------------------------------------------------------------------
#[test]
fn test_rbac_admin_full_access() {
    let engine = RbacEngine::new();
    let admin_ctx = ctx(Uuid::now_v7(), vec![Role::Admin]);

    for perm in [
        Permission::Read,
        Permission::Write,
        Permission::Delete,
        Permission::Grant,
        Permission::Admin,
    ] {
        assert!(
            engine.check(&admin_ctx, "any_collection", &perm),
            "Admin should have {:?} on any collection",
            perm
        );
    }
}

// ---------------------------------------------------------------------------
// 2. RBAC — ReadOnly denied write
// ---------------------------------------------------------------------------
#[test]
fn test_rbac_readonly_denied_write() {
    let engine = RbacEngine::new();
    let readonly_ctx = ctx(Uuid::now_v7(), vec![Role::ReadOnly]);

    assert!(
        engine.check(&readonly_ctx, "data", &Permission::Read),
        "ReadOnly should have Read permission"
    );
    assert!(
        !engine.check(&readonly_ctx, "data", &Permission::Write),
        "ReadOnly should NOT have Write permission"
    );
    assert!(
        !engine.check(&readonly_ctx, "data", &Permission::Delete),
        "ReadOnly should NOT have Delete permission"
    );
}

// ---------------------------------------------------------------------------
// 3. RBAC — custom role scoped to collection
// ---------------------------------------------------------------------------
#[test]
fn test_rbac_custom_role_scoped() {
    let mut engine = RbacEngine::new();
    let analyst_role = Role::Custom("analyst".to_string());

    // Grant analyst Read on "metrics" only
    engine.grant("metrics", analyst_role.clone(), Permission::Read);

    let analyst_ctx = ctx(Uuid::now_v7(), vec![analyst_role.clone()]);

    assert!(
        engine.check(&analyst_ctx, "metrics", &Permission::Read),
        "analyst should read metrics"
    );
    assert!(
        !engine.check(&analyst_ctx, "metrics", &Permission::Write),
        "analyst should not write metrics"
    );
    assert!(
        !engine.check(&analyst_ctx, "other_collection", &Permission::Read),
        "analyst should not read other collections"
    );
}

// ---------------------------------------------------------------------------
// 4. RBAC — grant → revoke lifecycle
// ---------------------------------------------------------------------------
#[test]
fn test_rbac_grant_revoke_lifecycle() {
    let mut engine = RbacEngine::new();
    let role = Role::Custom("temp".to_string());
    let temp_ctx = ctx(Uuid::now_v7(), vec![role.clone()]);

    // Initially no access
    assert!(
        !engine.check(&temp_ctx, "secret", &Permission::Read),
        "temp role should not have access initially"
    );

    // Grant
    engine.grant("secret", role.clone(), Permission::Read);
    assert!(
        engine.check(&temp_ctx, "secret", &Permission::Read),
        "temp role should have Read after grant"
    );

    // Revoke
    engine.revoke("secret", role.clone(), Permission::Read);
    assert!(
        !engine.check(&temp_ctx, "secret", &Permission::Read),
        "temp role should not have Read after revoke"
    );
}

// ---------------------------------------------------------------------------
// 5. Tenant isolation — scope_query
// ---------------------------------------------------------------------------
#[test]
fn test_tenant_isolation_scope_query() {
    let tenant_id = Uuid::now_v7();
    let tenant_ctx = ctx(tenant_id, vec![Role::ReadWrite]);

    let query = "SELECT * FROM orders";
    let scoped = scope_query(query, &tenant_ctx).unwrap();

    assert!(
        scoped.contains(&tenant_id.to_string()),
        "scoped query should contain tenant UUID: {}",
        scoped
    );
    assert!(
        scoped.contains("_tenant_id"),
        "scoped query should reference _tenant_id"
    );
}

// ---------------------------------------------------------------------------
// 6. Tenant isolation — record ownership violation
// ---------------------------------------------------------------------------
#[test]
fn test_tenant_isolation_record_ownership() {
    let tenant_a = Uuid::now_v7();
    let tenant_b = Uuid::now_v7();
    let ctx_a = ctx(tenant_a, vec![Role::ReadWrite]);

    // Record belongs to tenant B, context is tenant A
    let result = validate_record_ownership(tenant_b, &ctx_a);
    assert!(
        result.is_err(),
        "accessing another tenant's record should fail"
    );

    // Record belongs to same tenant
    let result = validate_record_ownership(tenant_a, &ctx_a);
    assert!(
        result.is_ok(),
        "accessing own tenant's record should succeed"
    );
}

// ---------------------------------------------------------------------------
// 7. Audit log — filter by tenant
// ---------------------------------------------------------------------------
#[test]
fn test_audit_log_filters_by_tenant() {
    let log = AuditLog::new(100);
    let tenant_a = Uuid::now_v7();
    let tenant_b = Uuid::now_v7();

    // Log 10 entries for tenant A
    for _ in 0..10 {
        log.record(audit_entry(
            tenant_a,
            AuditAction::Query,
            AuditOutcome::Allowed,
        ));
    }

    // Log 5 entries for tenant B
    for _ in 0..5 {
        log.record(audit_entry(
            tenant_b,
            AuditAction::Write,
            AuditOutcome::Allowed,
        ));
    }

    let a_entries = log.entries_for_tenant(tenant_a);
    let b_entries = log.entries_for_tenant(tenant_b);

    assert_eq!(a_entries.len(), 10, "tenant A should have 10 entries");
    assert_eq!(b_entries.len(), 5, "tenant B should have 5 entries");
}

// ---------------------------------------------------------------------------
// 8. Audit log — FIFO eviction
// ---------------------------------------------------------------------------
#[test]
fn test_audit_log_fifo_eviction() {
    let log = AuditLog::new(5);
    let tenant = Uuid::now_v7();

    for i in 0..8 {
        let mut entry = audit_entry(tenant, AuditAction::Query, AuditOutcome::Allowed);
        entry.detail = format!("entry_{}", i);
        log.record(entry);
    }

    assert_eq!(log.len(), 5, "log should cap at max_size=5");

    // Oldest entries should be evicted — last 5 should remain (entry_3..entry_7)
    let recent = log.recent(5);
    assert_eq!(recent.len(), 5);
    assert_eq!(
        recent[0].detail, "entry_3",
        "oldest surviving entry should be entry_3"
    );
    assert_eq!(
        recent[4].detail, "entry_7",
        "newest entry should be entry_7"
    );
}

// ---------------------------------------------------------------------------
// 9. Metrics — Prometheus text format
// ---------------------------------------------------------------------------
#[test]
fn test_metrics_prometheus_format() {
    let mut registry = MetricsRegistry::new();

    let counter = Counter::new("test_counter", "A test counter");
    counter.increment();
    counter.increment();
    registry.register_counter(counter);

    let histogram = Histogram::new("test_histogram", "A test histogram", vec![1.0, 5.0, 10.0]);
    histogram.observe(3.0);
    histogram.observe(7.0);
    registry.register_histogram(histogram);

    let text = registry.render_text();

    assert!(
        text.contains("# HELP test_counter"),
        "should have HELP line for counter"
    );
    assert!(
        text.contains("# TYPE test_counter counter"),
        "should have TYPE line for counter"
    );
    assert!(
        text.contains("# HELP test_histogram"),
        "should have HELP line for histogram"
    );
    assert!(
        text.contains("# TYPE test_histogram histogram"),
        "should have TYPE line for histogram"
    );
    assert!(text.contains("test_counter"), "should contain counter data");
    assert!(
        text.contains("test_histogram_bucket"),
        "should contain histogram buckets"
    );
}

// ---------------------------------------------------------------------------
// 10. Tracer — parent/child spans
// ---------------------------------------------------------------------------
#[test]
fn test_tracer_parent_child_spans() {
    let exporter = Arc::new(InMemoryExporter::new());
    let tracer = Tracer::new(
        Arc::clone(&exporter) as Arc<dyn fundb_server::SpanExporter>,
        1.0,
    );

    // Create parent span
    let mut parent = tracer.start_span("query", None);
    parent.set_attribute("collection", "users");
    let parent_ctx = parent.context.clone();
    tracer.finish_span(parent);

    // Create child span
    let mut child = tracer.start_child("scan", &parent_ctx);
    child.set_attribute("index", "btree");
    tracer.finish_span(child);

    let exported = exporter.exported_spans();
    assert_eq!(exported.len(), 2, "should export 2 spans");

    // Both should share the same trace_id
    assert_eq!(
        exported[0].trace_id, exported[1].trace_id,
        "parent and child should share trace_id"
    );

    // Child should reference parent
    assert_eq!(
        exported[1].parent_span_id,
        Some(exported[0].span_id),
        "child's parent_span_id should point to parent's span_id"
    );
}

// ---------------------------------------------------------------------------
// 11. TLS config validation
// ---------------------------------------------------------------------------
#[test]
fn test_tls_config_validation() {
    let config = TlsConfig::new("/path/to/cert.pem", "/path/to/key.pem");
    assert!(
        config.is_tls13_only(),
        "default TLS config should be TLS 1.3 only"
    );

    let mut config12 = TlsConfig::new("/cert.pem", "/key.pem");
    config12.min_version = TlsVersion::Tls12;
    assert!(
        !config12.is_tls13_only(),
        "TLS 1.2 config should not be TLS 1.3 only"
    );

    // Validate should succeed (stub)
    assert!(config.validate().is_ok(), "validation should pass for stub");
}
