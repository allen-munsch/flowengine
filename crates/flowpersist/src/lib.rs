//! Workflow persistence with SQLite
//!
//! Provides:
//! - Store/load workflow definitions
//! - Node-level result caching with content-fingerprint
//! - Workflow execution history
//! - Cache invalidation

use flowcore::{Value, Workflow};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub node_type: String,
    pub config_hash: String,
    pub input_hash: String,
    pub outputs: HashMap<String, Value>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub ttl_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub workflow_name: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub success: bool,
    pub duration_ms: u64,
    pub completed_nodes: usize,
    pub total_nodes: usize,
}

/// Persistent store backed by SQLite
pub struct PersistentStore {
    db: Arc<Mutex<Connection>>,
}

impl PersistentStore {
    /// Open or create a SQLite database
    pub fn open(path: impl AsRef<Path>) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        let store = Self {
            db: Arc::new(Mutex::new(conn)),
        };
        store.initialize_tables()?;
        Ok(store)
    }

    /// Create in-memory store (for testing)
    pub fn in_memory() -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        let store = Self {
            db: Arc::new(Mutex::new(conn)),
        };
        store.initialize_tables()?;
        Ok(store)
    }

    fn initialize_tables(&self) -> Result<(), rusqlite::Error> {
        let conn = self.db.blocking_lock();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS workflows (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                definition_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS executions (
                id TEXT PRIMARY KEY,
                workflow_id TEXT NOT NULL,
                workflow_name TEXT NOT NULL,
                started_at TEXT NOT NULL,
                completed_at TEXT,
                success INTEGER NOT NULL DEFAULT 0,
                duration_ms INTEGER NOT NULL DEFAULT 0,
                completed_nodes INTEGER NOT NULL DEFAULT 0,
                total_nodes INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (workflow_id) REFERENCES workflows(id)
            );

            CREATE TABLE IF NOT EXISTS node_cache (
                id TEXT PRIMARY KEY,
                node_type TEXT NOT NULL,
                config_hash TEXT NOT NULL,
                input_hash TEXT NOT NULL,
                outputs_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                ttl_seconds INTEGER,
                UNIQUE(node_type, config_hash, input_hash)
            );

            CREATE INDEX IF NOT EXISTS idx_executions_workflow
                ON executions(workflow_id);
            CREATE INDEX IF NOT EXISTS idx_executions_started
                ON executions(started_at);
            CREATE INDEX IF NOT EXISTS idx_node_cache_lookup
                ON node_cache(node_type, config_hash, input_hash);
            ",
        )?;
        Ok(())
    }

    // ── Workflow persistence ──

    pub fn save_workflow(&self, workflow: &Workflow) -> Result<(), rusqlite::Error> {
        let conn = self.db.blocking_lock();
        let json = serde_json::to_string(workflow).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(e))
        })?;
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT OR REPLACE INTO workflows (id, name, description, definition_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, COALESCE((SELECT created_at FROM workflows WHERE id = ?1), ?5), ?5)",
            params![
                workflow.id.to_string(),
                workflow.name,
                workflow.description,
                json,
                now,
            ],
        )?;
        Ok(())
    }

    pub fn load_workflow(&self, id: Uuid) -> Result<Option<Workflow>, rusqlite::Error> {
        let conn = self.db.blocking_lock();
        let mut stmt =
            conn.prepare("SELECT definition_json FROM workflows WHERE id = ?1")?;
        let mut rows = stmt.query_map(params![id.to_string()], |row| {
            row.get::<_, String>(0)
        })?;

        match rows.next() {
            Some(Ok(json)) => {
                let workflow: Workflow = serde_json::from_str(&json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
                })?;
                Ok(Some(workflow))
            }
            _ => Ok(None),
        }
    }

    pub fn list_workflows(&self) -> Result<Vec<(Uuid, String)>, rusqlite::Error> {
        let conn = self.db.blocking_lock();
        let mut stmt =
            conn.prepare("SELECT id, name FROM workflows ORDER BY updated_at DESC")?;
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            Ok((Uuid::parse_str(&id).unwrap_or_default(), name))
        })?;

        rows.collect()
    }

    pub fn delete_workflow(&self, id: Uuid) -> Result<bool, rusqlite::Error> {
        let conn = self.db.blocking_lock();
        let count = conn.execute(
            "DELETE FROM workflows WHERE id = ?1",
            params![id.to_string()],
        )?;
        Ok(count > 0)
    }

    // ── Execution history ──

    pub fn record_execution(
        &self,
        record: &ExecutionRecord,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.db.blocking_lock();
        conn.execute(
            "INSERT INTO executions (id, workflow_id, workflow_name, started_at, completed_at, success, duration_ms, completed_nodes, total_nodes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                record.id.to_string(),
                record.workflow_id.to_string(),
                record.workflow_name,
                record.started_at.to_rfc3339(),
                record.completed_at.map(|t| t.to_rfc3339()),
                record.success as i32,
                record.duration_ms as i64,
                record.completed_nodes as i64,
                record.total_nodes as i64,
            ],
        )?;
        Ok(())
    }

    pub fn get_execution_history(
        &self,
        workflow_id: Option<Uuid>,
        limit: usize,
    ) -> Result<Vec<ExecutionRecord>, rusqlite::Error> {
        let conn = self.db.blocking_lock();
        let query = if workflow_id.is_some() {
            "SELECT id, workflow_id, workflow_name, started_at, completed_at, success, duration_ms, completed_nodes, total_nodes
             FROM executions WHERE workflow_id = ?1 ORDER BY started_at DESC LIMIT ?2"
        } else {
            "SELECT id, workflow_id, workflow_name, started_at, completed_at, success, duration_ms, completed_nodes, total_nodes
             FROM executions ORDER BY started_at DESC LIMIT ?1"
        };

        let mut stmt = conn.prepare(query)?;

        let rows: rusqlite::Result<Vec<ExecutionRecord>> = if let Some(wf_id) = workflow_id {
            stmt.query_map(
                params![wf_id.to_string(), limit as i64],
                |row| Self::row_to_record(row),
            )?
            .collect()
        } else {
            stmt.query_map(params![limit as i64], |row| {
                Self::row_to_record(row)
            })?
            .collect()
        };

        Ok(rows?)
    }

    fn row_to_record(
        row: &rusqlite::Row,
    ) -> rusqlite::Result<ExecutionRecord> {
        Ok(ExecutionRecord {
            id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
            workflow_id: Uuid::parse_str(&row.get::<_, String>(1)?)
                .unwrap_or_default(),
            workflow_name: row.get(2)?,
            started_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                .map(|t| t.with_timezone(&chrono::Utc))
                .unwrap_or_default(),
            completed_at: row
                .get::<_, Option<String>>(4)?
                .and_then(|s| {
                    chrono::DateTime::parse_from_rfc3339(&s)
                        .map(|t| t.with_timezone(&chrono::Utc))
                        .ok()
                }),
            success: row.get::<_, i32>(5)? != 0,
            duration_ms: row.get::<_, i64>(6)? as u64,
            completed_nodes: row.get::<_, i64>(7)? as usize,
            total_nodes: row.get::<_, i64>(8)? as usize,
        })
    }

    // ── Node result caching ──

    /// Compute a content hash for inputs and config
    pub fn compute_hash(data: &HashMap<String, Value>) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut keys: Vec<_> = data.keys().collect();
        keys.sort();

        let mut hasher = DefaultHasher::new();
        for key in keys {
            key.hash(&mut hasher);
            if let Some(val) = data.get(key) {
                format!("{:?}", val).hash(&mut hasher);
            }
        }
        format!("{:x}", hasher.finish())
    }

    pub fn get_cached_result(
        &self,
        node_type: &str,
        config_hash: &str,
        input_hash: &str,
    ) -> Result<Option<HashMap<String, Value>>, rusqlite::Error> {
        let conn = self.db.blocking_lock();
        let mut stmt = conn.prepare(
            "SELECT outputs_json, created_at, ttl_seconds FROM node_cache
             WHERE node_type = ?1 AND config_hash = ?2 AND input_hash = ?3",
        )?;

        let mut rows = stmt.query_map(
            params![node_type, config_hash, input_hash],
            |row| {
                let json: String = row.get(0)?;
                let created_at_str: String = row.get(1)?;
                let ttl: Option<i64> = row.get(2)?;
                Ok((json, created_at_str, ttl))
            },
        )?;

        if let Some(Ok((json_str, created_at_str, ttl_seconds))) = rows.next() {
            // Check TTL
            if let Some(ttl) = ttl_seconds {
                let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|t| t.with_timezone(&chrono::Utc))
                    .unwrap_or_default();
                let age = chrono::Utc::now()
                    .signed_duration_since(created_at)
                    .num_seconds();
                if age > ttl {
                    return Ok(None); // Expired
                }
            }

            let outputs: HashMap<String, Value> =
                serde_json::from_str(&json_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
            return Ok(Some(outputs));
        }

        Ok(None)
    }

    pub fn cache_result(
        &self,
        node_type: &str,
        config_hash: &str,
        input_hash: &str,
        outputs: &HashMap<String, Value>,
        ttl_seconds: Option<i64>,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.db.blocking_lock();
        let id = Uuid::new_v4().to_string();
        let json = serde_json::to_string(outputs).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(e))
        })?;
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT OR REPLACE INTO node_cache (id, node_type, config_hash, input_hash, outputs_json, created_at, ttl_seconds)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, node_type, config_hash, input_hash, json, now, ttl_seconds],
        )?;
        Ok(())
    }

    pub fn invalidate_cache(
        &self,
        node_type: Option<&str>,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self.db.blocking_lock();
        if let Some(nt) = node_type {
            conn.execute("DELETE FROM node_cache WHERE node_type = ?1", params![nt])
        } else {
            conn.execute("DELETE FROM node_cache", [])
        }
    }

    pub fn cache_stats(&self) -> Result<(usize, String), rusqlite::Error> {
        let conn = self.db.blocking_lock();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM node_cache",
            [],
            |row| row.get(0),
        )?;
        let newest: String = conn
            .query_row(
                "SELECT COALESCE(MAX(created_at), 'never') FROM node_cache",
                [],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "unknown".to_string());
        Ok((count as usize, newest))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flowcore::Workflow;

    #[test]
    fn test_workflow_persistence() {
        let store = PersistentStore::in_memory().unwrap();
        let workflow = Workflow::new("test");

        store.save_workflow(&workflow).unwrap();

        let loaded = store.load_workflow(workflow.id).unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().name, "test");
    }

    #[test]
    fn test_cache_hit() {
        let store = PersistentStore::in_memory().unwrap();

        let outputs: HashMap<String, Value> =
            [("result".to_string(), Value::Number(42.0))]
                .into_iter()
                .collect();

        store
            .cache_result("shell.exec", "config_hash", "input_hash", &outputs, Some(3600))
            .unwrap();

        let cached = store
            .get_cached_result("shell.exec", "config_hash", "input_hash")
            .unwrap();
        assert!(cached.is_some());
        assert_eq!(
            cached.unwrap().get("result"),
            Some(&Value::Number(42.0))
        );
    }

    #[test]
    fn test_cache_miss() {
        let store = PersistentStore::in_memory().unwrap();
        let cached = store
            .get_cached_result("shell.exec", "no_match", "no_match")
            .unwrap();
        assert!(cached.is_none());
    }
}
