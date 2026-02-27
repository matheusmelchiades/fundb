// fundb-server — FunDB production server: HTTP API, security, observability.

pub mod http;
pub mod observability;
pub mod security;

pub use observability::{
    Counter, FunDbMetrics, Gauge, Histogram, InMemoryExporter, MetricsRegistry,
    Span, SpanContext, SpanExporter, SpanStatus, Tracer, grafana_dashboard_json,
};
pub use security::{
    AuditAction, AuditEntry, AuditLog, AuditOutcome, Permission, RbacEngine,
    Role, SecurityError, TenantContext, TlsAcceptorStub, TlsConfig, TlsVersion,
    scope_query, validate_record_ownership,
};
