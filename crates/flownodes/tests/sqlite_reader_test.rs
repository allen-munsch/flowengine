use flowcore::{Node, NodeContext, Value};
use flowcore::events::EventEmitter;
use flownodes::database::sqlite_reader::SqliteReaderNode;
use std::collections::HashMap;
use uuid::Uuid;

fn create_test_context(config: HashMap<String, Value>) -> NodeContext {
    let (tx, _rx) = tokio::sync::broadcast::channel(16);
    let exec_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();
    let events = EventEmitter::new(exec_id, node_id, tx);
    let mut ctx = NodeContext::new(node_id, events);
    ctx.config = config;
    ctx
}

#[tokio::test]
async fn test_sqlite_reader_in_memory() {
    let node = SqliteReaderNode;

    let mut config = HashMap::new();
    config.insert("connection_string".to_string(), Value::String(":memory:".to_string()));
    config.insert("query".to_string(), Value::String(
        "SELECT 1 AS id, 'hello' AS name UNION ALL SELECT 2, 'world'".to_string()
    ));

    let ctx = create_test_context(config);

    let result = node.execute(ctx).await;
    assert!(result.is_ok(), "SQLite reader should execute successfully: {:?}", result.err());

    let output = result.unwrap();
    let rows = output.outputs.get("rows").expect("Should have rows output");
    let row_count = output.outputs.get("row_count").expect("Should have row_count output");

    assert_eq!(row_count.as_f64(), Some(2.0), "Should return 2 rows");

    if let Value::Array(arr) = rows {
        assert_eq!(arr.len(), 2);
        if let Value::Json(serde_json::Value::Object(map)) = &arr[0] {
            assert_eq!(map.get("id").and_then(|v| v.as_i64()), Some(1));
            assert_eq!(map.get("name").and_then(|v| v.as_str()), Some("hello"));
        } else {
            panic!("First row should be a Json Object");
        }
    } else {
        panic!("rows should be an Array");
    }
}

#[tokio::test]
async fn test_sqlite_reader_create_table_and_query() {
    let node = SqliteReaderNode;

    // Create a table
    {
        let mut config = HashMap::new();
        config.insert("connection_string".to_string(), Value::String(":memory:".to_string()));
        config.insert("query".to_string(), Value::String(
            "CREATE TABLE test (id INTEGER PRIMARY KEY, value TEXT)".to_string()
        ));
        let ctx = create_test_context(config);
        let result = node.execute(ctx).await;
        assert!(result.is_ok(), "CREATE TABLE should succeed");
        let output = result.unwrap();
        let row_count = output.outputs.get("row_count").and_then(|v| v.as_f64());
        assert_eq!(row_count, Some(0.0));
    }

    // Insert and query
    {
        let mut config = HashMap::new();
        config.insert("connection_string".to_string(), Value::String(":memory:".to_string()));
        config.insert("query".to_string(), Value::String(
            "INSERT INTO test VALUES (1, 'alpha'), (2, 'beta'); SELECT * FROM test ORDER BY id".to_string()
        ));
        let ctx = create_test_context(config);
        let result = node.execute(ctx).await;
        assert!(result.is_ok(), "INSERT + SELECT should succeed");
        let output = result.unwrap();
        let rows = output.outputs.get("rows").expect("Should have rows");
        if let Value::Array(arr) = rows {
            assert_eq!(arr.len(), 2);
        } else {
            panic!("rows should be an Array");
        }
    }
}

#[tokio::test]
async fn test_sqlite_reader_missing_connection_string() {
    let node = SqliteReaderNode;

    let mut config = HashMap::new();
    config.insert("query".to_string(), Value::String("SELECT 1".to_string()));

    let ctx = create_test_context(config);

    let result = node.execute(ctx).await;
    assert!(result.is_err(), "Should fail without connection_string");
}

#[tokio::test]
async fn test_sqlite_reader_node_type() {
    let node = SqliteReaderNode;
    assert_eq!(node.node_type(), "sqlite.reader");
}

#[tokio::test]
async fn test_sqlite_reader_factory() {
    use flowruntime::NodeFactory;
    let factory = flownodes::database::sqlite_reader::SqliteReaderNodeFactory;

    assert_eq!(factory.node_type(), "sqlite.reader");

    let node = factory.create(&HashMap::new()).expect("Factory should create node");
    assert_eq!(node.node_type(), "sqlite.reader");

    let metadata = factory.metadata();
    assert_eq!(metadata.category, "database");
    assert_eq!(metadata.outputs.len(), 2);
}
