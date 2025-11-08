// crates/flowcli/src/main.rs

use anyhow::Result;
use clap::{Parser, Subcommand};
use flowcore::{ExecutionEvent, Value, Workflow};
use flowruntime::FlowRuntime;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing_subscriber;

#[derive(Parser)]
#[command(name = "flow")]
#[command(about = "Flow Engine CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Execute a workflow file
    Run {
        /// Path to workflow JSON file
        #[arg(short, long)]
        file: PathBuf,
        
        /// Input data as JSON string
        #[arg(short, long)]
        input: Option<String>,
        
        /// Show verbose output
        #[arg(short, long)]
        verbose: bool,
    },
    
    /// Validate a workflow file
    Validate {
        /// Path to workflow JSON file
        file: PathBuf,
    },
    
    /// List available node types
    Nodes,
    
    /// Create a new example workflow
    Init {
        /// Output file path
        #[arg(short, long, default_value = "workflow.json")]
        output: PathBuf,
    },
}

/// Convert a serde_json::Value to flowcore::Value
fn json_to_value(json: serde_json::Value) -> Value {
    match json {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                Value::Number(f)
            } else {
                Value::Number(n.as_i64().unwrap_or(0) as f64)
            }
        }
        serde_json::Value::String(s) => Value::String(s),
        serde_json::Value::Array(arr) => {
            Value::Array(arr.into_iter().map(json_to_value).collect())
        }
        serde_json::Value::Object(obj) => {
            let map: HashMap<String, Value> = obj
                .into_iter()
                .map(|(k, v)| (k, json_to_value(v)))
                .collect();
            Value::Object(map)
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Run { file, input, verbose } => {
            // Initialize logging
            if verbose {
                tracing_subscriber::fmt()
                    .with_max_level(tracing::Level::DEBUG)
                    .init();
            } else {
                tracing_subscriber::fmt()
                    .with_max_level(tracing::Level::INFO)
                    .init();
            }
            
            run_workflow(file, input).await?;
        }
        
        Commands::Validate { file } => {
            validate_workflow(file)?;
        }
        
        Commands::Nodes => {
            list_nodes();
        }
        
        Commands::Init { output } => {
            create_example_workflow(output)?;
        }
    }
    
    Ok(())
}

async fn run_workflow(file: PathBuf, input: Option<String>) -> Result<()> {
    println!("🚀 Loading workflow from: {}", file.display());
    
    // Load workflow
    let workflow_json = std::fs::read_to_string(&file)?;
    let workflow: Workflow = serde_json::from_str(&workflow_json)?;
    
    println!("📋 Workflow: {}", workflow.name);
    println!("   Nodes: {}", workflow.nodes.len());
    println!("   Connections: {}", workflow.connections.len());
    println!();
    
    // Parse input data - convert plain JSON to Value types
    let inputs: HashMap<String, Value> = if let Some(input_str) = input {
        // Parse as plain JSON first
        let json: serde_json::Value = serde_json::from_str(&input_str)?;
        
        // Convert to HashMap<String, Value>
        if let serde_json::Value::Object(obj) = json {
            obj.into_iter()
                .map(|(k, v)| (k, json_to_value(v)))
                .collect()
        } else {
            return Err(anyhow::anyhow!("Input must be a JSON object"));
        }
    } else {
        HashMap::new()
    };
    
    // Create runtime with registered nodes
    let mut registry = flowruntime::NodeRegistry::new();
    flownodes::register_all(&mut registry);
    
    let runtime = FlowRuntime::with_registry(
        std::sync::Arc::new(registry),
        flowruntime::RuntimeConfig::default(),
    );
    
    // Subscribe to events for real-time output
    let mut events = runtime.subscribe_events();
    
    // Spawn event listener
    let event_task = tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            match event {
                ExecutionEvent::WorkflowStarted { .. } => {
                    println!("▶️  Workflow started");
                }
                ExecutionEvent::NodeStarted { node_id, node_type, .. } => {
                    println!("  ⚡ Starting node: {} ({})", node_id, node_type);
                }
                ExecutionEvent::NodeCompleted { node_id, duration_ms, .. } => {
                    println!("  ✅ Node {} completed in {}ms", node_id, duration_ms);
                }
                ExecutionEvent::NodeFailed { node_id, error, .. } => {
                    println!("  ❌ Node {} failed: {}", node_id, error);
                }
                ExecutionEvent::NodeEvent { node_id, event, .. } => {
                    match event {
                        flowcore::NodeEvent::Info { message } => {
                            println!("     ℹ️  [{}] {}", node_id, message);
                        }
                        flowcore::NodeEvent::Warning { message } => {
                            println!("     ⚠️  [{}] {}", node_id, message);
                        }
                        flowcore::NodeEvent::Progress { percent, message } => {
                            if let Some(msg) = message {
                                println!("     📊 [{}] {}% - {}", node_id, percent, msg);
                            } else {
                                println!("     📊 [{}] {}%", node_id, percent);
                            }
                        }
                        _ => {}
                    }
                }
                ExecutionEvent::WorkflowCompleted { success, duration_ms, .. } => {
                    if success {
                        println!("✨ Workflow completed successfully in {}ms", duration_ms);
                    } else {
                        println!("💥 Workflow failed after {}ms", duration_ms);
                    }
                }
            }
        }
    });
    
    // Execute workflow
    let result = runtime.execute(&workflow, inputs).await?;
    
    // Wait for events to finish printing
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    event_task.abort();
    
    println!();
    println!("📊 Execution Summary:");
    println!("   Execution ID: {}", result.execution_id);
    println!("   Completed: {}/{} nodes", result.completed_nodes, result.total_nodes);
    
    if !result.outputs.is_empty() {
        println!();
        println!("📤 Outputs:");
        for (node_id, outputs) in &result.outputs {
            if !outputs.is_empty() {
                println!("   Node {}:", node_id);
                for (key, value) in outputs {
                    println!("     {}: {:?}", key, value);
                }
            }
        }
    }
    
    Ok(())
}

fn validate_workflow(file: PathBuf) -> Result<()> {
    println!("🔍 Validating workflow: {}", file.display());
    
    let workflow_json = std::fs::read_to_string(&file)?;
    let workflow: Workflow = serde_json::from_str(&workflow_json)?;
    
    println!("✅ Workflow is valid:");
    println!("   Name: {}", workflow.name);
    println!("   Nodes: {}", workflow.nodes.len());
    println!("   Connections: {}", workflow.connections.len());
    
    // TODO: Add more validation (check for cycles, unknown node types, etc.)
    
    Ok(())
}

fn list_nodes() {
    println!("📦 Available Node Types:");
    println!();
    
    let mut registry = flowruntime::NodeRegistry::new();
    flownodes::register_all(&mut registry);
    
    for node_type in registry.list_node_types() {
        if let Some(metadata) = registry.get_metadata(&node_type) {
            println!("  • {} ({})", node_type, metadata.category);
            println!("    {}", metadata.description);
        } else {
            println!("  • {}", node_type);
        }
    }
}

fn create_example_workflow(output: PathBuf) -> Result<()> {
    use flowcore::{NodeSpec, Workflow};
    
    let mut workflow = Workflow::new("Example HTTP Workflow");
    workflow.description = Some("Fetches data from an API and logs the result".to_string());
    
    // Create nodes
    let http_node = NodeSpec::new("http.request")
        .with_name("Fetch Data")
        .with_config("method", "GET")
        .with_position(100.0, 100.0);
    
    let debug_node = NodeSpec::new("debug.log")
        .with_name("Log Response")
        .with_position(300.0, 100.0);
    
    let http_id = workflow.add_node(http_node);
    let debug_id = workflow.add_node(debug_node);
    
    // Connect them
    workflow.connect(http_id, "body", debug_id, "message");
    
    // Save to file
    let json = serde_json::to_string_pretty(&workflow)?;
    std::fs::write(&output, json)?;
    
    println!("✨ Created example workflow: {}", output.display());
    println!();
    println!("Run it with:");
    println!("  flow run --file {} --input '{{\"url\": \"https://api.github.com/zen\"}}'", output.display());
    
    Ok(())
}