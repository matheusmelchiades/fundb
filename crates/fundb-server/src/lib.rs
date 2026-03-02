// fundb-server — FunDB production server: HTTP API, security, observability.

pub mod handler;
pub mod http;
pub mod observability;
pub mod security;

pub use observability::{
    grafana_dashboard_json, Counter, FunDbMetrics, Gauge, Histogram, InMemoryExporter,
    MetricsRegistry, Span, SpanContext, SpanExporter, SpanStatus, Tracer,
};
pub use security::{
    scope_query, validate_record_ownership, AuditAction, AuditEntry, AuditLog, AuditOutcome,
    Permission, RbacEngine, Role, SecurityError, TenantContext, TlsAcceptorStub, TlsConfig,
    TlsVersion,
};
