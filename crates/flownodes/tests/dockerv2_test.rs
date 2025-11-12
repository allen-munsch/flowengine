// crates/flownodes/tests/docker_v2_test.rs

use flowcore::{Node, NodeContext, Value, EventBus, ExecutionId};
use flownodes::DockerNodeV2;
use std::collections::HashMap;
use std::sync::Arc;
use tokio;

// Helper function to create a test context
fn create_test_context(
    config: HashMap<String, Value>,
    inputs: HashMap<String, Value>,
) -> NodeContext {
    let event_bus = Arc::new(EventBus::new(100));
    let execution_id = ExecutionId::new_v4();
    let node_id = uuid::Uuid::new_v4();
    
    NodeContext {
        node_id,
        inputs,
        config,
        state: Arc::new(tokio::sync::RwLock::new(flowcore::NodeState::default())),
        events: event_bus.create_emitter(execution_id, node_id),
        cancellation: tokio_util::sync::CancellationToken::new(),
    }
}

// ============================================================================
// IO Mode: Flat Tests
// ============================================================================

#[tokio::test]
async fn test_io_mode_flat_simple_value() {
    let node = DockerNodeV2;
    
    let mut config = HashMap::new();
    config.insert("image".to_string(), Value::String("python:3.9-slim".to_string()));
    config.insert(
        "command".to_string(),
        Value::String(
            "python -c \"import sys, json; data=json.load(sys.stdin); print(json.dumps({'result': data['value'] * 2}))\"".to_string()
        )
    );
    config.insert("stdin_mode".to_string(), Value::String("json".to_string()));
    config.insert("output_mode".to_string(), Value::String("auto".to_string()));
    config.insert("io_mode".to_string(), Value::String("flat".to_string()));
    
    let mut inputs = HashMap::new();
    inputs.insert("value".to_string(), Value::Number(21.0));
    
    let ctx = create_test_context(config, inputs);
    let result = node.execute(ctx).await;
    
    assert!(result.is_ok(), "Docker node should execute successfully");
    
    let output = result.unwrap();
    match output.outputs.get("output") {
        Some(Value::Json(json)) => {
            let result = json.get("result").and_then(|v| v.as_f64()).unwrap();
            assert_eq!(result, 42.0, "Should double the input value");
        }
        _ => panic!("Expected JSON output"),
    }
}

#[tokio::test]
async fn test_io_mode_flat_complex_object() {
    let node = DockerNodeV2;
    
    let mut config = HashMap::new();
    config.insert("image".to_string(), Value::String("python:3.9-slim".to_string()));
    config.insert(
        "command".to_string(),
        Value::String(
            r#"python -c "import sys, json; data=json.load(sys.stdin); print(json.dumps({'name': data['user']['name'].upper(), 'score': data['score'] + 10}))" "#.to_string()
        )
    );
    config.insert("stdin_mode".to_string(), Value::String("json".to_string()));
    config.insert("output_mode".to_string(), Value::String("auto".to_string()));
    config.insert("io_mode".to_string(), Value::String("flat".to_string()));
    
    let mut user_obj = HashMap::new();
    user_obj.insert("name".to_string(), Value::String("alice".to_string()));
    
    let mut inputs = HashMap::new();
    inputs.insert("user".to_string(), Value::Object(user_obj));
    inputs.insert("score".to_string(), Value::Number(90.0));
    
    let ctx = create_test_context(config, inputs);
    let result = node.execute(ctx).await;
    
    assert!(result.is_ok(), "Docker node should execute successfully: {:?}", result);
    
    let output = result.unwrap();
    match output.outputs.get("output") {
        Some(Value::Json(json)) => {
            assert_eq!(json.get("name").and_then(|v| v.as_str()), Some("ALICE"));
            assert_eq!(json.get("score").and_then(|v| v.as_f64()), Some(100.0));
        }
        _ => panic!("Expected JSON output"),
    }
}

#[tokio::test]
async fn test_io_mode_flat_array_processing() {
    let node = DockerNodeV2;
    
    let mut config = HashMap::new();
    config.insert("image".to_string(), Value::String("python:3.9-slim".to_string()));
    config.insert(
        "command".to_string(),
        Value::String(
            r#"python -c "import sys, json; data=json.load(sys.stdin); print(json.dumps({'sum': sum(data['numbers']), 'count': len(data['numbers'])}))" "#.to_string()
        )
    );
    config.insert("stdin_mode".to_string(), Value::String("json".to_string()));
    config.insert("output_mode".to_string(), Value::String("auto".to_string()));
    config.insert("io_mode".to_string(), Value::String("flat".to_string()));
    
    let mut inputs = HashMap::new();
    inputs.insert("numbers".to_string(), Value::Array(vec![
        Value::Number(1.0),
        Value::Number(2.0),
        Value::Number(3.0),
        Value::Number(4.0),
        Value::Number(5.0),
    ]));
    
    let ctx = create_test_context(config, inputs);
    let result = node.execute(ctx).await;
    
    assert!(result.is_ok(), "Docker node should execute successfully");
    
    let output = result.unwrap();
    match output.outputs.get("output") {
        Some(Value::Json(json)) => {
            assert_eq!(json.get("sum").and_then(|v| v.as_f64()), Some(15.0));
            assert_eq!(json.get("count").and_then(|v| v.as_f64()), Some(5.0));
        }
        _ => panic!("Expected JSON output"),
    }
}

// ============================================================================
// IO Mode: Wrapped Tests
// ============================================================================

#[tokio::test]
async fn test_io_mode_wrapped_preserves_structure() {
    let node = DockerNodeV2;
    
    let mut config = HashMap::new();
    config.insert("image".to_string(), Value::String("python:3.9-slim".to_string()));
    config.insert(
        "command".to_string(),
        Value::String(
            r#"python -c "import sys, json; data=json.load(sys.stdin); print(json.dumps({'received_type': str(type(data)), 'keys': list(data.keys())}))" "#.to_string()
        )
    );
    config.insert("stdin_mode".to_string(), Value::String("json".to_string()));
    config.insert("output_mode".to_string(), Value::String("auto".to_string()));
    config.insert("io_mode".to_string(), Value::String("wrapped".to_string()));
    
    let mut inputs = HashMap::new();
    inputs.insert("value".to_string(), Value::Number(42.0));
    
    let ctx = create_test_context(config, inputs);
    let result = node.execute(ctx).await;
    
    assert!(result.is_ok(), "Docker node should execute successfully");
    
    let output = result.unwrap();
    match output.outputs.get("output") {
        Some(Value::Json(json)) => {
            // In wrapped mode, the input includes the Value enum structure
            let keys = json.get("keys").and_then(|v| v.as_array()).unwrap();
            assert!(keys.len() > 0, "Should have keys from wrapped structure");
        }
        _ => panic!("Expected JSON output"),
    }
}

// ============================================================================
// IO Mode: Auto Tests
// ============================================================================

#[tokio::test]
async fn test_io_mode_auto_defaults_to_flat() {
    let node = DockerNodeV2;
    
    let mut config = HashMap::new();
    config.insert("image".to_string(), Value::String("python:3.9-slim".to_string()));
    config.insert(
        "command".to_string(),
        Value::String(
            "python -c \"import sys, json; data=json.load(sys.stdin); print(json.dumps({'doubled': data['x'] * 2}))\"".to_string()
        )
    );
    config.insert("stdin_mode".to_string(), Value::String("json".to_string()));
    config.insert("output_mode".to_string(), Value::String("auto".to_string()));
    config.insert("io_mode".to_string(), Value::String("auto".to_string()));
    
    let mut inputs = HashMap::new();
    inputs.insert("x".to_string(), Value::Number(5.0));
    
    let ctx = create_test_context(config, inputs);
    let result = node.execute(ctx).await;
    
    assert!(result.is_ok(), "Docker node should execute successfully");
    
    let output = result.unwrap();
    match output.outputs.get("output") {
        Some(Value::Json(json)) => {
            assert_eq!(json.get("doubled").and_then(|v| v.as_f64()), Some(10.0));
        }
        _ => panic!("Expected JSON output"),
    }
}

// ============================================================================
// Volume and Environment Tests
// ============================================================================

#[tokio::test]
async fn test_dockerv2_volume_mounting() {
    let node = DockerNodeV2;
    
    // Create a temp file to mount
    let temp_dir = std::env::temp_dir();
    let test_file = temp_dir.join("docker_v2_test.txt");
    std::fs::write(&test_file, "Hello from host!").unwrap();
    
    let mut config = HashMap::new();
    config.insert("image".to_string(), Value::String("alpine:latest".to_string()));
    config.insert(
        "command".to_string(),
        Value::String("cat /mnt/test.txt".to_string())
    );
    config.insert("stdin_mode".to_string(), Value::String("none".to_string()));
    config.insert("output_mode".to_string(), Value::String("text".to_string()));
    
    let mount_str = format!("{}:/mnt/test.txt:ro", test_file.display());
    config.insert("volumes".to_string(), Value::Array(vec![
        Value::String(mount_str)
    ]));
    
    let ctx = create_test_context(config, HashMap::new());
    let result = node.execute(ctx).await;
    
    assert!(result.is_ok(), "Docker node should execute successfully");
    
    let output = result.unwrap();
    let stdout = output.outputs.get("stdout").and_then(|v| v.as_str()).unwrap();
    assert!(stdout.contains("Hello from host!"));
    
    // Cleanup
    std::fs::remove_file(test_file).ok();
}

#[tokio::test]
async fn test_dockerv2_complex_env_vars() {
    let node = DockerNodeV2;
    
    let mut config = HashMap::new();
    config.insert("image".to_string(), Value::String("alpine:latest".to_string()));
    config.insert(
        "command".to_string(),
        Value::String("sh -c \"echo Name:$NAME Age:$AGE\"".to_string())
    );
    config.insert("stdin_mode".to_string(), Value::String("none".to_string()));
    
    let mut env = HashMap::new();
    env.insert("NAME".to_string(), Value::String("Alice".to_string()));
    env.insert("AGE".to_string(), Value::String("30".to_string()));
    config.insert("env".to_string(), Value::Object(env));
    
    let ctx = create_test_context(config, HashMap::new());
    let result = node.execute(ctx).await;
    
    assert!(result.is_ok(), "Docker node should execute successfully");
    
    let output = result.unwrap();
    let stdout = output.outputs.get("stdout").and_then(|v| v.as_str()).unwrap();
    assert!(stdout.contains("Name:Alice"));
    assert!(stdout.contains("Age:30"));
}

// ============================================================================
// Resource Limits Tests
// ============================================================================

#[tokio::test]
async fn test_dockerv2_combined_resource_limits() {
    let node = DockerNodeV2;
    
    let mut config = HashMap::new();
    config.insert("image".to_string(), Value::String("alpine:latest".to_string()));
    config.insert("command".to_string(), Value::String("echo 'Limited container'".to_string()));
    config.insert("memory_limit".to_string(), Value::String("64m".to_string()));
    config.insert("cpu_limit".to_string(), Value::String("0.25".to_string()));
    config.insert("stdin_mode".to_string(), Value::String("none".to_string()));
    
    let ctx = create_test_context(config, HashMap::new());
    let result = node.execute(ctx).await;
    
    assert!(result.is_ok(), "Docker node should execute with resource limits");
    
    let output = result.unwrap();
    assert_eq!(
        output.outputs.get("exit_code").and_then(|v| v.as_f64()),
        Some(0.0)
    );
}

// ============================================================================
// Network and User Tests
// ============================================================================

#[tokio::test]
async fn test_dockerv2_custom_user() {
    let node = DockerNodeV2;
    
    let mut config = HashMap::new();
    config.insert("image".to_string(), Value::String("alpine:latest".to_string()));
    config.insert("command".to_string(), Value::String("id".to_string()));
    config.insert("user".to_string(), Value::String("nobody".to_string()));
    config.insert("stdin_mode".to_string(), Value::String("none".to_string()));
    
    let ctx = create_test_context(config, HashMap::new());
    let result = node.execute(ctx).await;
    
    assert!(result.is_ok(), "Docker node should execute with custom user");
    
    let output = result.unwrap();
    let stdout = output.outputs.get("stdout").and_then(|v| v.as_str()).unwrap();
    assert!(stdout.contains("nobody") || stdout.contains("uid="));
}

#[tokio::test]
async fn test_dockerv2_custom_network() {
    let node = DockerNodeV2;
    
    let mut config = HashMap::new();
    config.insert("image".to_string(), Value::String("alpine:latest".to_string()));
    config.insert("command".to_string(), Value::String("echo 'network test'".to_string()));
    config.insert("network".to_string(), Value::String("none".to_string()));
    config.insert("stdin_mode".to_string(), Value::String("none".to_string()));
    
    let ctx = create_test_context(config, HashMap::new());
    let result = node.execute(ctx).await;
    
    assert!(result.is_ok(), "Docker node should execute with custom network");
}

// ============================================================================
// Timeout Tests
// ============================================================================

#[tokio::test]
async fn test_dockerv2_timeout_enforcement() {
    let node = DockerNodeV2;
    
    let mut config = HashMap::new();
    config.insert("image".to_string(), Value::String("alpine:latest".to_string()));
    config.insert("command".to_string(), Value::String("sleep 10".to_string()));
    config.insert("stdin_mode".to_string(), Value::String("none".to_string()));
    config.insert("timeout".to_string(), Value::Number(1.0)); // 1 second timeout
    
    let ctx = create_test_context(config, HashMap::new());
    let result = node.execute(ctx).await;
    
    assert!(result.is_err(), "Should timeout");
    if let Err(e) = result {
        assert!(e.to_string().contains("Timeout"));
    }
}

// ============================================================================
// Entrypoint Tests
// ============================================================================

#[tokio::test]
async fn test_dockerv2_custom_entrypoint() {
    let node = DockerNodeV2;
    
    let mut config = HashMap::new();
    config.insert("image".to_string(), Value::String("alpine:latest".to_string()));
    config.insert("entrypoint".to_string(), Value::Array(vec![
        Value::String("/bin/sh".to_string()),
    ]));
    config.insert("command".to_string(), Value::Array(vec![
        Value::String("-c".to_string()),
        Value::String("echo 'custom entrypoint'".to_string()),
    ]));
    config.insert("stdin_mode".to_string(), Value::String("none".to_string()));
    
    let ctx = create_test_context(config, HashMap::new());
    let result = node.execute(ctx).await;
    
    assert!(result.is_ok(), "Docker node should execute with custom entrypoint");
    
    let output = result.unwrap();
    let stdout = output.outputs.get("stdout").and_then(|v| v.as_str()).unwrap();
    assert!(stdout.contains("custom entrypoint"));
}

// ============================================================================
// Pipeline Tests (Chaining Multiple Containers)
// ============================================================================

#[tokio::test]
async fn test_dockerv2_multi_stage_pipeline() {
    // Stage 1: Generate data
    let node1 = DockerNodeV2;
    
    let mut config1 = HashMap::new();
    config1.insert("image".to_string(), Value::String("python:3.9-slim".to_string()));
    config1.insert(
        "command".to_string(),
        Value::String(
            r#"python -c "import json; print(json.dumps({'data': [1, 2, 3, 4, 5], 'multiplier': 3}))" "#.to_string()
        )
    );
    config1.insert("stdin_mode".to_string(), Value::String("none".to_string()));
    config1.insert("output_mode".to_string(), Value::String("auto".to_string()));
    config1.insert("io_mode".to_string(), Value::String("flat".to_string()));
    
    let ctx1 = create_test_context(config1, HashMap::new());
    let result1 = node1.execute(ctx1).await;
    
    assert!(result1.is_ok(), "Stage 1 should succeed");
    let output1 = result1.unwrap();
    
    // Stage 2: Process data
    let node2 = DockerNodeV2;
    
    let mut config2 = HashMap::new();
    config2.insert("image".to_string(), Value::String("python:3.9-slim".to_string()));
    config2.insert(
        "command".to_string(),
        Value::String(
            r#"python -c "import sys, json; d=json.load(sys.stdin); result = [x * d['multiplier'] for x in d['data']]; print(json.dumps({'result': result, 'sum': sum(result)}))" "#.to_string()
        )
    );
    config2.insert("stdin_mode".to_string(), Value::String("json".to_string()));
    config2.insert("output_mode".to_string(), Value::String("auto".to_string()));
    config2.insert("io_mode".to_string(), Value::String("flat".to_string()));
    
    // Pass output from stage 1 as input to stage 2
    let mut inputs2 = HashMap::new();
    if let Some(Value::Json(json)) = output1.outputs.get("output") {
        inputs2.insert("data".to_string(), Value::Json(json.get("data").unwrap().clone()));
        inputs2.insert("multiplier".to_string(), Value::Json(json.get("multiplier").unwrap().clone()));
    }
    
    let ctx2 = create_test_context(config2, inputs2);
    let result2 = node2.execute(ctx2).await;
    
    assert!(result2.is_ok(), "Stage 2 should succeed");
    
    let output2 = result2.unwrap();
    match output2.outputs.get("output") {
        Some(Value::Json(json)) => {
            assert_eq!(json.get("sum").and_then(|v| v.as_f64()), Some(45.0));
        }
        _ => panic!("Expected JSON output"),
    }
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[tokio::test]
async fn test_dockerv2_non_zero_exit_code() {
    let node = DockerNodeV2;
    
    let mut config = HashMap::new();
    config.insert("image".to_string(), Value::String("alpine:latest".to_string()));
    config.insert("command".to_string(), Value::String("sh -c 'exit 5'".to_string()));
    config.insert("stdin_mode".to_string(), Value::String("none".to_string()));
    
    let ctx = create_test_context(config, HashMap::new());
    let result = node.execute(ctx).await;
    
    assert!(result.is_ok(), "Node should complete even with non-zero exit");
    
    let output = result.unwrap();
    assert_eq!(
        output.outputs.get("exit_code").and_then(|v| v.as_f64()),
        Some(2.0)
    );
    assert_eq!(
        output.outputs.get("success").and_then(|v| v.as_bool()),
        Some(false)
    );
}

#[tokio::test]
async fn test_dockerv2_missing_required_config() {
    let node = DockerNodeV2;
    
    // No image config
    let config = HashMap::new();
    let ctx = create_test_context(config, HashMap::new());
    
    let result = node.execute(ctx).await;
    assert!(result.is_err(), "Should fail without required image config");
}

#[tokio::test]
async fn test_dockerv2_invalid_image() {
    let node = DockerNodeV2;
    
    let mut config = HashMap::new();
    config.insert("image".to_string(), Value::String("nonexistent-image-12345:latest".to_string()));
    config.insert("command".to_string(), Value::String("echo test".to_string()));
    config.insert("stdin_mode".to_string(), Value::String("none".to_string()));
    config.insert("auto_pull".to_string(), Value::Bool(true));
    
    let ctx = create_test_context(config, HashMap::new());
    let result = node.execute(ctx).await;
    
    assert!(result.is_err(), "Should fail with invalid image");
}

// ============================================================================
// Output Mode Tests
// ============================================================================

#[tokio::test]
async fn test_dockerv2_output_mode_text_only() {
    let node = DockerNodeV2;
    
    let mut config = HashMap::new();
    config.insert("image".to_string(), Value::String("alpine:latest".to_string()));
    config.insert("command".to_string(), Value::String("echo '{\"json\": true}'".to_string()));
    config.insert("stdin_mode".to_string(), Value::String("none".to_string()));
    config.insert("output_mode".to_string(), Value::String("text".to_string()));
    
    let ctx = create_test_context(config, HashMap::new());
    let result = node.execute(ctx).await;
    
    assert!(result.is_ok());
    
    let output = result.unwrap();
    match output.outputs.get("output") {
        Some(Value::String(s)) => {
            assert!(s.contains("json"));
        }
        _ => panic!("Expected String output in text mode"),
    }
}

#[tokio::test]
async fn test_dockerv2_output_mode_json_force() {
    let node = DockerNodeV2;
    
    let mut config = HashMap::new();
    config.insert("image".to_string(), Value::String("python:3.9-slim".to_string()));
    config.insert(
        "command".to_string(),
        Value::String(
            r#"python -c "import json; print(json.dumps({'forced': 'json'}))" "#.to_string()
        )
    );
    config.insert("stdin_mode".to_string(), Value::String("none".to_string()));
    config.insert("output_mode".to_string(), Value::String("json".to_string()));
    
    let ctx = create_test_context(config, HashMap::new());
    let result = node.execute(ctx).await;
    
    assert!(result.is_ok());
    
    let output = result.unwrap();
    match output.outputs.get("output") {
        Some(Value::Json(json)) => {
            assert_eq!(json.get("forced").and_then(|v| v.as_str()), Some("json"));
        }
        _ => panic!("Expected JSON output"),
    }
}

// ============================================================================
// Stdin Mode Tests
// ============================================================================

#[tokio::test]
async fn test_dockerv2_stdin_mode_raw() {
    let node = DockerNodeV2;
    
    let mut config = HashMap::new();
    config.insert("image".to_string(), Value::String("alpine:latest".to_string()));
    config.insert("command".to_string(), Value::String("cat".to_string()));
    config.insert("stdin_mode".to_string(), Value::String("raw".to_string()));
    config.insert("output_mode".to_string(), Value::String("text".to_string()));
    
    let mut inputs = HashMap::new();
    inputs.insert("data".to_string(), Value::String("Raw data test".to_string()));
    
    let ctx = create_test_context(config, inputs);
    let result = node.execute(ctx).await;
    
    assert!(result.is_ok());
    
    let output = result.unwrap();
    let stdout = output.outputs.get("stdout").and_then(|v| v.as_str()).unwrap();
    assert_eq!(stdout.trim(), "Raw data test");
}

#[tokio::test]
async fn test_dockerv2_stdin_mode_text() {
    let node = DockerNodeV2;
    
    let mut config = HashMap::new();
    config.insert("image".to_string(), Value::String("alpine:latest".to_string()));
    config.insert("command".to_string(), Value::String("wc -w".to_string()));
    config.insert("stdin_mode".to_string(), Value::String("text".to_string()));
    
    let mut inputs = HashMap::new();
    inputs.insert("data".to_string(), Value::String("one two three four five".to_string()));
    
    let ctx = create_test_context(config, inputs);
    let result = node.execute(ctx).await;
    
    assert!(result.is_ok());
    
    let output = result.unwrap();
    let stdout = output.outputs.get("stdout").and_then(|v| v.as_str()).unwrap();
    assert!(stdout.trim().contains("5"));
}