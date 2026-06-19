use crate::registry::NodeRegistry;
use serde::Deserialize;
use std::path::PathBuf;

/// Loads custom nodes from JSON definitions and dynamic plugins from a watch directory.
pub struct CustomNodeLoader {
    watch_dir: PathBuf,
}

/// A node defined in a JSON file.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct CustomNodeDefinition {
    pub node_type: String,
    pub command: String,
    pub description: Option<String>,
}

impl CustomNodeLoader {
    pub fn new(watch_dir: PathBuf) -> Self {
        Self { watch_dir }
    }

    /// Scan the watch directory and register custom node definitions.
    /// For `.json` files, parse them as `CustomNodeDefinition`.
    /// For `.so` files, delegate to `DynamicLoader`.
    pub async fn load_custom_nodes(&self, registry: &mut NodeRegistry) -> Result<(), String> {
        let dir_entries = std::fs::read_dir(&self.watch_dir)
            .map_err(|e| format!("Cannot read watch dir {:?}: {}", self.watch_dir, e))?;

        for entry in dir_entries {
            let entry = entry.map_err(|e| format!("Dir entry error: {}", e))?;
            let path = entry.path();
            match path.extension().and_then(|e| e.to_str()) {
                Some("json") => {
                    let contents = std::fs::read_to_string(&path)
                        .map_err(|e| format!("Cannot read {:?}: {}", path, e))?;
                    let _def: CustomNodeDefinition = serde_json::from_str(&contents)
                        .map_err(|e| format!("Invalid JSON in {:?}: {}", path, e))?;
                    tracing::info!("Loaded custom node definition from {:?}", path);
                    // JSON-defined nodes would be shell.exec wrappers; register as needed.
                }
                Some("so") | Some("dylib") => {
                    crate::DynamicLoader::load_plugin_file(&path, registry)?;
                }
                _ => {}
            }
        }
        Ok(())
    }
}
