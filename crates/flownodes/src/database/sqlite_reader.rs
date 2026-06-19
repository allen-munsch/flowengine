//! SQLite reader node — executes queries against SQLite databases
//!
//! Config fields:
//! - connection_string (String) — path to SQLite DB file or ":memory:"
//! - query (String) — SQL query to execute
//! - timeout_ms (Option<u64>) — optional query timeout
//! - max_rows (Option<u64>) — optional row limit
//!
//! Outputs:
//! - rows (Value::List) — query results as Vec<Value>, each row a Value::Map of column→value
//! - row_count (Value::Number) — number of rows returned

use async_trait::async_trait;
use flowcore::{Node, NodeContext, NodeError, NodeOutput, Value};
use flowcore_macros::NodeConfig;
use flowruntime::{NodeFactory, NodeMetadata, PortDefinition};
use std::collections::HashMap;

pub struct SqliteReaderNode;

#[derive(Debug, Clone, NodeConfig)]
struct SqliteReaderConfig {
    connection_string: String,
    query: String,
    #[config(default = "30000")]
    timeout_ms: u64,
    #[config(default = "10000")]
    max_rows: u64,
}

#[async_trait]
impl Node for SqliteReaderNode {
    fn node_type(&self) -> &str {
        "sqlite.reader"
    }

    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        let config = SqliteReaderConfig::try_from(&ctx.config)
            .map_err(|e| NodeError::Configuration(e))?;

        ctx.events.info(format!(
            "Executing SQL query on '{}'...",
            config.connection_string
        ));

        let conn = rusqlite::Connection::open(&config.connection_string)
            .map_err(|e| {
                NodeError::ExecutionFailed(format!(
                    "Failed to open SQLite database '{}': {}",
                    config.connection_string, e
                ))
            })?;

        conn.busy_timeout(std::time::Duration::from_millis(config.timeout_ms))
            .map_err(|e| {
                NodeError::ExecutionFailed(format!(
                    "Failed to set busy timeout: {}",
                    e
                ))
            })?;

        let mut stmt = conn.prepare(&config.query).map_err(|e| {
            NodeError::ExecutionFailed(format!(
                "Failed to prepare query: {}",
                e
            ))
        })?;

        let column_names: Vec<String> = stmt
            .column_names()
            .iter()
            .map(|c| c.to_string())
            .collect();

        let rows: Vec<Value> = stmt
            .query_map((), |row| {
                let mut map = serde_json::Map::new();
                for (i, col_name) in column_names.iter().enumerate() {
                    let val: rusqlite::types::Value = row.get_unwrap(i);
                    let json_val = rusqlite_value_to_json(val);
                    map.insert(col_name.clone(), json_val);
                }
                Ok(serde_json::Value::Object(map))
            })
            .map_err(|e| {
                NodeError::ExecutionFailed(format!(
                    "Query execution failed: {}",
                    e
                ))
            })?
            .take(config.max_rows as usize)
            .filter_map(|r| r.ok())
            .map(|json_val| Value::Json(json_val))
            .collect();

        let row_count = rows.len() as f64;

        ctx.events.info(format!(
            "Query returned {} row(s)",
            row_count
        ));

        Ok(NodeOutput::new()
            .with_output("rows", Value::Array(rows))
            .with_output("row_count", Value::Number(row_count)))
    }
}

fn rusqlite_value_to_json(val: rusqlite::types::Value) -> serde_json::Value {
    use rusqlite::types::Value as SqlVal;
    match val {
        SqlVal::Null => serde_json::Value::Null,
        SqlVal::Integer(i) => serde_json::Value::Number(i.into()),
        SqlVal::Real(f) => {
            serde_json::Number::from_f64(f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null)
        }
        SqlVal::Text(s) => serde_json::Value::String(s),
        SqlVal::Blob(b) => {
            serde_json::Value::String(base64_encode(&b))
        }
    }
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

pub struct SqliteReaderNodeFactory;

impl NodeFactory for SqliteReaderNodeFactory {
    fn create(
        &self,
        _config: &HashMap<String, Value>,
    ) -> Result<Box<dyn Node>, NodeError> {
        Ok(Box::new(SqliteReaderNode))
    }

    fn node_type(&self) -> &str {
        "sqlite.reader"
    }

    fn metadata(&self) -> NodeMetadata {
        NodeMetadata {
            description: "Executes SQL queries against SQLite databases and emits rows as output".to_string(),
            category: "database".to_string(),
            inputs: vec![],
            outputs: vec![
                PortDefinition {
                    name: "rows".to_string(),
                    description: "Query results as a list of JSON objects".to_string(),
                    required: false,
                },
                PortDefinition {
                    name: "row_count".to_string(),
                    description: "Number of rows returned".to_string(),
                    required: false,
                },
            ],
        }
    }
}
