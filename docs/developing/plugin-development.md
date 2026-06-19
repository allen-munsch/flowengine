# Plugin Development

How to build and distribute nodes as dynamically-loaded plugins.

## Architecture Overview

Plugins are compiled to shared libraries (`.so` on Linux, `.dylib` on macOS) and loaded at runtime via `libloading`. Each plugin exports a `NodePlugin` trait implementation that the host wraps as a `NodeFactory` and registers in the `NodeRegistry`.

```
Plugin .so
    └── extern "C" plugin_create() → Box<dyn NodePlugin>
            ├── create_node() → Box<dyn Node>
            ├── metadata() → PluginMetadata
            ├── node_type() → &str
            └── port_definitions() → Vec<PortDefinition>
```

## `NodePlugin` Trait

```rust
use flowcore::{Node, NodeError, NodePlugin, PluginMetadata, Value, PortDefinition};

#[async_trait]
pub trait NodePlugin: Send + Sync {
    /// Create a new node instance for a workflow execution.
    async fn create_node(
        &self,
        config: &HashMap<String, Value>
    ) -> Result<Box<dyn Node>, NodeError>;

    /// Metadata about this plugin node type.
    fn metadata(&self) -> PluginMetadata;

    /// The node type string used in workflow definitions.
    fn node_type(&self) -> &str;

    /// Port definitions for this node type.
    fn port_definitions(&self) -> Vec<PortDefinition>;
}
```

## `PluginMetadata` Struct

```rust
pub struct PluginMetadata {
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
}
```

## `extern "C"` Exports

Every plugin must export exactly two C-ABI functions:

```rust
#[no_mangle]
pub extern "C" fn plugin_create() -> *mut dyn NodePlugin {
    let plugin = Box::new(MyPlugin::new());
    Box::into_raw(plugin)
}

#[no_mangle]
pub extern "C" fn plugin_version() -> u32 {
    1
}
```

`plugin_create` returns a raw pointer to the plugin. The host takes ownership and wraps it in a `PluginFactory`. `plugin_version` is used for compatibility checks.

## Directory Scanning

The `DynamicLoader` scans a `plugin_dir` for `.so` / `.dylib` files and registers each one:

```rust
use flowruntime::DynamicLoader;
use std::path::Path;

let mut registry = NodeRegistry::new();
DynamicLoader::load(Path::new("/etc/flow/plugins"), &mut registry)?;
```

The `CustomNodeLoader` (used by the runtime's watch directory) does the same:

```rust
use flowruntime::CustomNodeLoader;

let loader = CustomNodeLoader::new(PathBuf::from("./plugins"));
loader.load_custom_nodes(&mut registry).await?;
```

`RuntimeConfig` does not have a `plugin_dir` — plugin loading is done programmatically via `DynamicLoader` or `CustomNodeLoader` before creating the `FlowRuntime`.

## Cargo.toml Setup

```toml
[package]
name = "my-flow-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
flowcore = { path = "../flowengine/crates/flowcore" }
async-trait = "0.1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

### Important: One `NodePlugin` Per `.so`

The loader registers a single `NodeFactory` per `.so` based on `plugin.node_type()`. If you need multiple node types from one library, you have two options:

1. **Multiple `.so` files** — one per node type
2. **Facade plugin** — a single `NodePlugin` that dispatches to sub-nodes based on a config field

## Full Example: Hash Node Plugin

### Plugin Source (`hash_plugin.rs`)

```rust
use async_trait::async_trait;
use flowcore::{
    Node, NodeContext, NodeError, NodeOutput, NodePlugin, Value,
    PortDefinition, PluginMetadata,
};
use std::collections::{BTreeMap, HashMap};
use sha2::{Sha256, Digest};

pub struct HashPlugin;

impl HashPlugin {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl NodePlugin for HashPlugin {
    async fn create_node(
        &self,
        _config: &HashMap<String, Value>,
    ) -> Result<Box<dyn Node>, NodeError> {
        Ok(Box::new(HashNode::new()))
    }

    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "Hash Node".to_string(),
            description: "Computes SHA-256 hash of input".to_string(),
            version: "1.0.0".to_string(),
            author: "Your Name".to_string(),
        }
    }

    fn node_type(&self) -> &str {
        "crypto.sha256"
    }

    fn port_definitions(&self) -> Vec<PortDefinition> {
        vec![
            PortDefinition {
                name: "input".to_string(),
                description: "Data to hash".to_string(),
                required: true,
            },
            PortDefinition {
                name: "hash".to_string(),
                description: "Hex-encoded SHA-256 hash".to_string(),
                required: false,
            },
        ]
    }
}

pub struct HashNode;

impl HashNode {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Node for HashNode {
    fn node_type(&self) -> &str {
        "crypto.sha256"
    }

    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        let input = ctx.require_input("input")?
            .as_str()
            .ok_or_else(|| NodeError::InvalidInputType {
                field: "input".to_string(),
                expected: "string".to_string(),
                actual: "other".to_string(),
            })?;

        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        Ok(NodeOutput::new()
            .with_output("hash", hash))
    }
}

// Mandatory exports
#[no_mangle]
pub extern "C" fn plugin_create() -> *mut dyn NodePlugin {
    let plugin = Box::new(HashPlugin::new());
    Box::into_raw(plugin)
}

#[no_mangle]
pub extern "C" fn plugin_version() -> u32 {
    1
}
```

### Building

```bash
cargo build --release
# Output: target/release/libhash_plugin.so
```

### Installing

Copy the `.so` to the plugin directory:

```bash
mkdir -p /etc/flow/plugins
cp target/release/libhash_plugin.so /etc/flow/plugins/
```

The runtime loads it automatically on next start if the directory is configured.

### Using in a Workflow

```json
{
  "node_type": "crypto.sha256",
  "config": {}
}
```

Connect `input` port to a source; `hash` port carries the hex-encoded SHA-256 digest.

## Plugin Config Format

Plugin nodes can support configuration — the `create_node` method receives the workflow config `HashMap<String, Value>`:

```rust
async fn create_node(
    &self,
    config: &HashMap<String, Value>,
) -> Result<Box<dyn Node>, NodeError> {
    let algorithm = config.get("algorithm")
        .and_then(|v| v.as_str())
        .unwrap_or("sha256");

    Ok(Box::new(HashNode::new(algorithm.to_string())))
}
```

In the workflow:

```json
{
  "node_type": "crypto.sha256",
  "config": {
    "algorithm": "sha512"
  }
}
```

Use the `#[derive(NodeConfig)]` macro from `flowcore_macros` for type-safe config extraction (see [node-development.md](node-development.md#config-driven-node)).

## Registration into NodeRegistry

When loaded, the plugin's factory wraps the `NodePlugin`:

```rust
struct PluginFactory {
    plugin: Box<dyn NodePlugin>,
}

impl NodeFactory for PluginFactory {
    fn create(&self, config: &HashMap<String, Value>) -> Result<Box<dyn Node>, NodeError> {
        futures::executor::block_on(self.plugin.create_node(config))
    }

    fn node_type(&self) -> &str {
        self.plugin.node_type()
    }

    fn metadata(&self) -> NodeMetadata {
        let meta = self.plugin.metadata();
        NodeMetadata {
            description: meta.description,
            category: "plugin".to_string(),
            inputs: self.plugin.port_definitions(),
            outputs: Vec::new(),
        }
    }
}
```

All plugins get the `"plugin"` category. If two plugins register the same `node_type`, the last one wins (no duplicate detection).

## Debugging and Troubleshooting

### Plugin not loading

```bash
# Check file extension
file /etc/flow/plugins/*.so

# Verify symbols
nm -D /etc/flow/plugins/libhash_plugin.so | grep plugin
```

### Missing symbol errors

The loader expects exactly `plugin_create` and `plugin_version` with C ABI linkage. If missing, check:
- `#[no_mangle]` is present
- `extern "C"` is used
- The function signature exactly matches the expected types

### ABI compatibility

Both the plugin and the host must be compiled with the same Rust compiler version. Mismatched versions can cause subtle memory corruption. When in doubt, rebuild both.

### Memory safety

The `DynamicLoader::load_plugin_file` leaks the `libloading::Library` handle via `std::mem::forget` so that function pointers remain valid for the process lifetime. This is intentional for plugins that live for the duration of the process.

### Logging

Plugins can log via `tracing`:

```rust
use tracing::{info, warn, error};

tracing::info!("Plugin hash node executing");
```

## See Also

- [node-development.md](node-development.md) — building nodes with the `Node` trait
- [architecture.md](architecture.md) — crate structure and execution model
