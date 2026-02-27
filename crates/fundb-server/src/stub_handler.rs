//! Stub query handler for STORY-1-4.
//!
//! Handles a small number of hard-coded queries so that `psql` can connect and
//! receive sensible results while the real SQL engine is not yet implemented.

use async_trait::async_trait;
use fundb_protocol::{ConnContext, FieldDescription, QueryError, QueryHandler, QueryResult};

/// A minimal handler that answers a handful of well-known queries.
pub struct StubHandler;

#[async_trait]
impl QueryHandler for StubHandler {
    async fn execute(&self, query: &str, _ctx: &ConnContext) -> Result<QueryResult, QueryError> {
        let q = query.trim().to_uppercase();

        if q.starts_with("SELECT 1") {
            return Ok(QueryResult {
                columns: vec![FieldDescription {
                    name: "?column?".into(),
                    type_oid: 23, // int4
                }],
                rows: vec![vec![Some("1".into())]],
                command_tag: "SELECT 1".into(),
            });
        }

        if q.contains("VERSION") {
            return Ok(QueryResult {
                columns: vec![FieldDescription {
                    name: "version".into(),
                    type_oid: 25, // text
                }],
                rows: vec![vec![Some("FunDB 0.1.0".into())]],
                command_tag: "SELECT 1".into(),
            });
        }

        if q.is_empty() {
            return Ok(QueryResult {
                columns: vec![],
                rows: vec![],
                command_tag: String::new(),
            });
        }

        // Unknown query: return empty result set (real parser arrives in Sprint 3).
        Ok(QueryResult {
            columns: vec![],
            rows: vec![],
            command_tag: "OK".into(),
        })
    }
}
