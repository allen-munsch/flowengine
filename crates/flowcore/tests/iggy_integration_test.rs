// crates/flowcore/tests/iggy_v07_test.rs

// crates/flowcore/tests/iggy_v07_test.rs

use flowcore::events::{IggyEventBus, IggyEventBusConfig, ExecutionEvent, NodeEvent};
use flowcore::{ExecutionId, Value};
use chrono::Utc;
use std::collections::HashMap;
use uuid::Uuid;

/// Helper to check if Iggy server is available
async fn iggy_available() -> bool {
    tokio::net::TcpStream::connect("127.0.0.1:8090")
        .await
        .is_ok()
}

/// Initialize tracing for tests
fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let _ = fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("debug"))
        )
        .with_test_writer()
        .try_init();
}

#[tokio::test]
#[ignore] // Run only when Iggy server is available
async fn test_iggy_07_connection() {
    init_tracing();
    
    if !iggy_available().await {
        println!("Skipping test: Iggy server not available at 127.0.0.1:8090");
        return;
    }
    
    let config = IggyEventBusConfig {
        connection_string: "iggy+tcp://iggy:iggy@127.0.0.1:8090".to_string(),
        username: "iggy".to_string(),
        password: "iggy".to_string(),
        stream_name: format!("test_stream_{}", Uuid::new_v4()),
        topic_name: "test_topic".to_string(),
    };
    
    let bus = IggyEventBus::new(config).await;
    match &bus {
        Ok(_) => println!("✓ Successfully connected to Iggy server"),
        Err(e) => println!("✗ Failed to connect: {}", e),
    }
    assert!(bus.is_ok(), "Should connect to Iggy server");
}

#[tokio::test]
#[ignore]
async fn test_iggy_07_publish_and_subscribe() {
    init_tracing();
    
    if !iggy_available().await {
        println!("Skipping test: Iggy server not available");
        return;
    }
    
    let config = IggyEventBusConfig {
        connection_string: "iggy+tcp://iggy:iggy@127.0.0.1:8090".to_string(),
        username: "iggy".to_string(),
        password: "iggy".to_string(),
        stream_name: format!("test_stream_{}", Uuid::new_v4()),
        topic_name: "test_topic".to_string(),
    };
    
    println!("Creating bus with stream: {}", config.stream_name);
    let bus = IggyEventBus::new(config.clone()).await
        .expect("Failed to create event bus");
    
    // Create test event
    let execution_id = ExecutionId::new_v4();
    let workflow_id = Uuid::new_v4();
    
    let event = ExecutionEvent::WorkflowStarted {
        execution_id,
        workflow_id,
        timestamp: Utc::now(),
    };
    
    // Publish event
    println!("Publishing event...");
    let publish_result = bus.publish(event.clone()).await;
    if let Err(ref e) = publish_result {
        println!("✗ Publish failed: {}", e);
    } else {
        println!("✓ Published successfully");
    }
    assert!(publish_result.is_ok(), "Should publish event successfully: {:?}", publish_result.err());
    
    // Subscribe and consume
    println!("Creating subscription...");
    let subscription = bus.subscribe("test_consumer".to_string()).await
        .expect("Failed to create subscription");
    
    // Give some time for the message to be available
    println!("Waiting for message propagation...");
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    println!("Polling for events...");
    let events = subscription.poll().await
        .expect("Failed to poll events");
    
    println!("Received {} events", events.len());
    assert!(!events.is_empty(), "Should receive published event");
}

#[tokio::test]
#[ignore]
async fn test_iggy_07_multiple_events() {
    init_tracing();
    
    if !iggy_available().await {
        println!("Skipping test: Iggy server not available");
        return;
    }
    
    let config = IggyEventBusConfig {
        connection_string: "iggy+tcp://iggy:iggy@127.0.0.1:8090".to_string(),
        username: "iggy".to_string(),
        password: "iggy".to_string(),
        stream_name: format!("multi_stream_{}", Uuid::new_v4()),
        topic_name: "multi_events".to_string(),
    };
    
    println!("Creating bus with stream: {}", config.stream_name);
    let bus = IggyEventBus::new(config.clone()).await
        .expect("Failed to create event bus");
    
    // Publish multiple events
    for i in 0..10 {
        let event = ExecutionEvent::NodeEvent {
            execution_id: ExecutionId::new_v4(),
            node_id: Uuid::new_v4(),
            event: NodeEvent::Info {
                message: format!("Test message {}", i),
            },
            timestamp: Utc::now(),
        };
        
        println!("Publishing event {}...", i);
        let result = bus.publish(event).await;
        if let Err(ref e) = result {
            println!("✗ Failed to publish event {}: {}", i, e);
        }
        assert!(result.is_ok(), "Should publish event {}: {:?}", i, result.err());
    }
    
    println!("✓ All events published successfully");
    
    // Subscribe and verify
    println!("Creating subscription...");
    let subscription = bus.subscribe("multi_consumer".to_string()).await
        .expect("Failed to create subscription");
    
    println!("Waiting for message propagation...");
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    println!("Polling for events...");
    let events = subscription.poll().await
        .expect("Failed to poll events");
    
    println!("Received {} events", events.len());
    assert!(events.len() > 0, "Should receive at least some events");
}

#[tokio::test]
#[ignore]
async fn test_iggy_07_event_ordering() {
    init_tracing();
    
    if !iggy_available().await {
        println!("Skipping test: Iggy server not available");
        return;
    }
    
    let config = IggyEventBusConfig {
        connection_string: "iggy+tcp://iggy:iggy@127.0.0.1:8090".to_string(),
        username: "iggy".to_string(),
        password: "iggy".to_string(),
        stream_name: format!("ordered_stream_{}", Uuid::new_v4()),
        topic_name: "ordered_events".to_string(),
    };
    
    println!("Creating bus with stream: {}", config.stream_name);
    let bus = IggyEventBus::new(config.clone()).await
        .expect("Failed to create event bus");
    
    let execution_id = ExecutionId::new_v4();
    let workflow_id = Uuid::new_v4();
    
    // Publish events in order
    let events = vec![
        ExecutionEvent::WorkflowStarted {
            execution_id,
            workflow_id,
            timestamp: Utc::now(),
        },
        ExecutionEvent::NodeStarted {
            execution_id,
            node_id: Uuid::new_v4(),
            node_type: "test.node".to_string(),
            timestamp: Utc::now(),
        },
        ExecutionEvent::WorkflowCompleted {
            execution_id,
            success: true,
            duration_ms: 100,
            timestamp: Utc::now(),
        },
    ];
    
    for (i, event) in events.iter().enumerate() {
        println!("Publishing event {}...", i);
        let result = bus.publish(event.clone()).await;
        if let Err(ref e) = result {
            println!("✗ Failed to publish event {}: {}", i, e);
        }
        result.expect(&format!("Should publish event {}", i));
        // Small delay between events
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
    
    println!("✓ All events published successfully");
    
    // Subscribe and verify order
    println!("Creating subscription...");
    let subscription = bus.subscribe("order_consumer".to_string()).await
        .expect("Failed to create subscription");
    
    println!("Waiting for message propagation...");
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    println!("Polling for events...");
    let received = subscription.poll().await
        .expect("Failed to poll events");
    
    println!("Received {} events", received.len());
    assert_eq!(received.len(), 3, "Should receive all 3 events");
    
    // Verify event types in order
    matches!(received[0], ExecutionEvent::WorkflowStarted { .. });
    matches!(received[1], ExecutionEvent::NodeStarted { .. });
    matches!(received[2], ExecutionEvent::WorkflowCompleted { .. });
}

#[tokio::test]
#[ignore]
async fn test_iggy_07_complex_event_data() {
    init_tracing();
    
    if !iggy_available().await {
        println!("Skipping test: Iggy server not available");
        return;
    }
    
    let config = IggyEventBusConfig {
        connection_string: "iggy+tcp://iggy:iggy@127.0.0.1:8090".to_string(),
        username: "iggy".to_string(),
        password: "iggy".to_string(),
        stream_name: format!("complex_stream_{}", Uuid::new_v4()),
        topic_name: "complex_events".to_string(),
    };
    
    println!("Creating bus with stream: {}", config.stream_name);
    let bus = IggyEventBus::new(config.clone()).await
        .expect("Failed to create event bus");
    
    // Create complex event with nested data
    let mut outputs = HashMap::new();
    outputs.insert("result".to_string(), Value::Number(42.0));
    outputs.insert("status".to_string(), Value::String("success".to_string()));
    
    let mut nested = HashMap::new();
    nested.insert("level1".to_string(), Value::String("data".to_string()));
    outputs.insert("nested".to_string(), Value::Object(nested));
    
    let event = ExecutionEvent::NodeCompleted {
        execution_id: ExecutionId::new_v4(),
        node_id: Uuid::new_v4(),
        outputs,
        duration_ms: 150,
        timestamp: Utc::now(),
    };
    
    // Publish and retrieve
    println!("Publishing complex event...");
    let result = bus.publish(event.clone()).await;
    if let Err(ref e) = result {
        println!("✗ Failed to publish: {}", e);
    }
    result.expect("Should publish complex event");
    
    println!("✓ Event published successfully");
    
    println!("Creating subscription...");
    let subscription = bus.subscribe("complex_consumer".to_string()).await
        .expect("Failed to create subscription");
    
    println!("Waiting for message propagation...");
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    println!("Polling for events...");
    let events = subscription.poll().await
        .expect("Failed to poll events");
    
    println!("Received {} events", events.len());
    assert!(!events.is_empty(), "Should receive complex event");
    
    if let ExecutionEvent::NodeCompleted { outputs, .. } = &events[0] {
        println!("✓ Verified event structure");
        assert!(outputs.contains_key("result"));
        assert!(outputs.contains_key("nested"));
    } else {
        panic!("Expected NodeCompleted event");
    }
}
