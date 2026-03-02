//! Real FunDB query handler — replaces the stub handler with full
//! parse → bind → execute pipeline.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use fundb_executor::batch::RecordBatch;
use fundb_executor::Executor;
use fundb_protocol::{ConnContext, FieldDescription, QueryError, QueryHandler, QueryResult};
use fundb_sql::binder::Catalog;
use serde::Deserialize;
use tokio::sync::RwLock;

/// A generic value type for deserializing MessagePack data columns.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum DataValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Array(Vec<DataValue>),
    Null,
}

impl fmt::Display for DataValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataValue::Bool(b) => write!(f, "{}", b),
            DataValue::Int(i) => write!(f, "{}", i),
            DataValue::Float(v) => write!(f, "{}", v),
            DataValue::Str(s) => write!(f, "{}", s),
            DataValue::Array(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            DataValue::Null => write!(f, ""),
        }
    }
}

/// Production query handler that routes SQL through the full FunDB pipeline.
pub struct FunDBHandler {
    executor: Executor,
    catalog: Arc<RwLock<Catalog>>,
}

impl FunDBHandler {
    pub fn new(executor: Executor, catalog: Arc<RwLock<Catalog>>) -> Self {
        Self { executor, catalog }
    }
}

#[async_trait]
impl QueryHandler for FunDBHandler {
    async fn execute(&self, query: &str, _ctx: &ConnContext) -> Result<QueryResult, QueryError> {
        let q = query.trim();

        // Fast-path: empty query
        if q.is_empty() {
            return Ok(QueryResult {
                columns: vec![],
                rows: vec![],
                command_tag: String::new(),
            });
        }

        // Fast-path: SELECT <literal> without FROM (e.g. SELECT 1, SELECT 2)
        let upper = q.to_uppercase();
        if upper.starts_with("SELECT ") && !upper.contains("FROM") {
            let rest = q[7..].trim().trim_end_matches(';').trim();
            // Simple integer literal
            if let Ok(n) = rest.parse::<i64>() {
                return Ok(QueryResult {
                    columns: vec![FieldDescription {
                        name: "?column?".into(),
                        type_oid: 23,
                    }],
                    rows: vec![vec![Some(n.to_string())]],
                    command_tag: "SELECT 1".into(),
                });
            }
        }

        // Fast-path: VERSION()
        if upper.contains("VERSION()") || upper.contains("VERSION ()") {
            return Ok(QueryResult {
                columns: vec![FieldDescription {
                    name: "version".into(),
                    type_oid: 25,
                }],
                rows: vec![vec![Some("FunDB 0.1.0".into())]],
                command_tag: "SELECT 1".into(),
            });
        }

        // Fast-path: _schema system table (used by VSCode extension)
        // Query: SELECT column_name FROM _schema WHERE collection = '<name>'
        if upper.contains("FROM _SCHEMA") {
            if let Some(col_name) = extract_schema_collection(q) {
                // Scan the collection for one record to discover columns
                let plan = fundb_sql::LogicalPlan::Limit {
                    input: Box::new(fundb_sql::LogicalPlan::Scan {
                        collection: col_name.clone(),
                        predicate: None,
                        projections: vec![fundb_sql::Expr::Star],
                    }),
                    n: 1,
                };
                let batch = self.executor.execute(plan).await.map_err(|e| QueryError {
                    code: "XX000".into(),
                    message: format!("schema scan error: {}", e),
                })?;
                let mut col_names: Vec<String> = vec!["_id".to_string()];
                if !batch.is_empty() {
                    let data = &batch.records[0].data;
                    if !data.is_empty() {
                        if let Ok(map) = rmp_serde::from_slice::<HashMap<String, DataValue>>(data) {
                            let mut user_cols: Vec<String> = map.keys().cloned().collect();
                            user_cols.sort();
                            col_names.extend(user_cols);
                        }
                    }
                }
                col_names.push("_confidence".to_string());
                let count = col_names.len();
                let rows: Vec<Vec<Option<String>>> = col_names
                    .into_iter()
                    .map(|n| vec![Some(n)])
                    .collect();
                return Ok(QueryResult {
                    columns: vec![FieldDescription {
                        name: "column_name".into(),
                        type_oid: 25,
                    }],
                    rows,
                    command_tag: format!("SELECT {}", count),
                });
            }
        }

        // Fast-path: _collections system table (used by VSCode extension explorer)
        if upper.contains("FROM _COLLECTIONS") {
            let catalog = self.catalog.read().await;
            let names = catalog.list_collections();
            let rows: Vec<Vec<Option<String>>> = names
                .into_iter()
                .map(|n| vec![Some(n)])
                .collect();
            let count = rows.len();
            return Ok(QueryResult {
                columns: vec![FieldDescription {
                    name: "name".into(),
                    type_oid: 25,
                }],
                rows,
                command_tag: format!("SELECT {}", count),
            });
        }

        // Parse SQL
        let stmt = fundb_sql::parse(q).map_err(|e| QueryError {
            code: "42601".into(),
            message: format!("syntax error: {}", e.message),
        })?;

        // Determine statement type and extract collection for catalog registration
        let is_insert = matches!(stmt, fundb_sql::Statement::Insert(_));
        let insert_collection = match &stmt {
            fundb_sql::Statement::Insert(ins) => Some(ins.table.clone()),
            _ => None,
        };

        // For CREATE COLLECTION, register it in the catalog
        if let fundb_sql::Statement::CreateCollection(ref create) = stmt {
            let mut catalog = self.catalog.write().await;
            catalog.add_collection(&create.name);
        }

        // Bind
        let catalog = self.catalog.read().await;
        let plan = fundb_sql::bind(stmt, &catalog).map_err(|e| QueryError {
            code: "42P01".into(),
            message: e.to_string(),
        })?;
        drop(catalog);

        // Register INSERT collection in catalog so subsequent SELECTs can find it
        if let Some(collection) = insert_collection {
            let mut catalog = self.catalog.write().await;
            catalog.add_collection(&collection);
        }

        // Execute
        let batch = self.executor.execute(plan).await.map_err(|e| QueryError {
            code: "XX000".into(),
            message: format!("execution error: {}", e),
        })?;

        // Convert result
        if is_insert {
            Ok(QueryResult {
                columns: vec![],
                rows: vec![],
                command_tag: format!("INSERT 0 {}", count_values_in_query(q)),
            })
        } else {
            batch_to_query_result(batch)
        }
    }
}

/// Extract collection name from a _schema query like:
/// `SELECT column_name FROM _schema WHERE collection = 'users'`
fn extract_schema_collection(query: &str) -> Option<String> {
    let lower = query.to_lowercase();
    // Look for collection = 'name' or collection = "name"
    if let Some(pos) = lower.find("collection") {
        let rest = &query[pos..];
        // Find the opening quote
        if let Some(q_start) = rest.find('\'').or_else(|| rest.find('"')) {
            let after_quote = &rest[q_start + 1..];
            let quote_char = rest.as_bytes()[q_start] as char;
            if let Some(q_end) = after_quote.find(quote_char) {
                return Some(after_quote[..q_end].to_string());
            }
        }
    }
    None
}

/// Count the number of value rows in an INSERT query for the command tag.
fn count_values_in_query(query: &str) -> usize {
    let upper = query.to_uppercase();
    if let Some(pos) = upper.find("VALUES") {
        let after_values = &query[pos + 6..];
        let mut count = 0;
        let mut depth = 0;
        for ch in after_values.chars() {
            match ch {
                '(' if depth == 0 => {
                    count += 1;
                    depth += 1;
                }
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
        }
        count.max(1)
    } else {
        1
    }
}

/// Convert a RecordBatch (from SELECT) into a QueryResult for the wire protocol.
fn batch_to_query_result(batch: RecordBatch) -> Result<QueryResult, QueryError> {
    if batch.is_empty() {
        return Ok(QueryResult {
            columns: vec![],
            rows: vec![],
            command_tag: "SELECT 0".into(),
        });
    }

    // System columns always present
    let mut all_columns: Vec<String> = vec!["_id".to_string()];

    // Discover user columns from the first record's MessagePack data
    let first_data = &batch.records[0].data;
    let user_columns: Vec<String> = if !first_data.is_empty() {
        if let Ok(map) = rmp_serde::from_slice::<HashMap<String, DataValue>>(first_data) {
            let mut cols: Vec<String> = map.keys().cloned().collect();
            cols.sort();
            cols
        } else {
            vec![]
        }
    } else {
        vec![]
    };
    all_columns.extend(user_columns.clone());
    all_columns.push("_confidence".to_string());

    // Build field descriptions (all as text for simplicity)
    let columns: Vec<FieldDescription> = all_columns
        .iter()
        .map(|name| FieldDescription {
            name: name.clone(),
            type_oid: 25, // text
        })
        .collect();

    // Build rows
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(batch.len());
    for record in &batch.records {
        let mut row: Vec<Option<String>> = Vec::with_capacity(all_columns.len());

        // _id
        row.push(Some(record._id.to_string()));

        // User data columns
        let data_map: HashMap<String, DataValue> = if !record.data.is_empty() {
            rmp_serde::from_slice(&record.data).unwrap_or_default()
        } else {
            HashMap::new()
        };

        for col in &user_columns {
            match data_map.get(col) {
                Some(DataValue::Null) | None => row.push(None),
                Some(v) => row.push(Some(v.to_string())),
            }
        }

        // _confidence
        row.push(Some(format!("{:.2}", record._confidence)));

        rows.push(row);
    }

    Ok(QueryResult {
        columns,
        rows,
        command_tag: format!("SELECT {}", batch.len()),
    })
}
