use flowcore::{Node, NodeError, NodePlugin, Value};
use crate::registry::{NodeFactory, NodeMetadata, NodeRegistry};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// External symbol signatures for dynamically loaded plugins.
/// Plugins are passed as raw pointers across the FFI boundary
/// and then wrapped in Box on the host side.
/// Note: both sides of this boundary are Rust code, so fat pointers
/// (trait objects) work in practice even though they're not C-FFI-safe.
#[allow(improper_ctypes_definitions)]
type PluginCreateFn = unsafe extern "C" fn() -> *mut dyn NodePlugin;

/// Loads node plugins from `.so` files in a directory and registers them.
pub struct DynamicLoader {
    plugin_dir: PathBuf,
    _libraries: Vec<libloading::Library>,
}

impl DynamicLoader {
    /// Scan `plugin_dir` for `.so` files, load each one, and register found plugins.
    pub fn load(plugin_dir: &Path, registry: &mut NodeRegistry) -> Result<(), String> {
        let mut loader = Self {
            plugin_dir: plugin_dir.to_path_buf(),
            _libraries: Vec::new(),
        };
        loader.scan_and_register(registry)
    }

    /// Load a single plugin file and register it.
    pub fn load_plugin_file(path: &Path, registry: &mut NodeRegistry) -> Result<(), String> {
        let loader = Self {
            plugin_dir: PathBuf::new(),
            _libraries: Vec::new(),
        };
        let (factory, _lib) = loader.load_plugin(path)?;
        registry.register(Arc::new(factory));
        // Leak the library so symbols stay alive (acceptable for process-lifetime plugins).
        std::mem::forget(_lib);
        Ok(())
    }

    fn scan_and_register(&mut self, registry: &mut NodeRegistry) -> Result<(), String> {
        let entries = std::fs::read_dir(&self.plugin_dir)
            .map_err(|e| format!("Cannot read plugin dir {:?}: {}", self.plugin_dir, e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Dir entry error: {}", e))?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "so" || ext == "dylib") {
                match self.load_plugin(&path) {
                    Ok((factory, lib)) => {
                        self._libraries.push(lib);
                        registry.register(Arc::new(factory));
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load plugin {:?}: {}", path, e);
                    }
                }
            }
        }
        Ok(())
    }

    fn load_plugin(
        &self,
        path: &Path,
    ) -> Result<(PluginFactory, libloading::Library), String> {
        // SAFETY: libloading requires unsafe for dynamic symbol resolution.
        // We only call `plugin_create` which is expected to follow the extern "C" ABI.
        let lib = unsafe { libloading::Library::new(path) }
            .map_err(|e| format!("Cannot load library {:?}: {}", path, e))?;

        let plugin_create: libloading::Symbol<PluginCreateFn> = unsafe { lib.get(b"plugin_create") }
            .map_err(|e| format!("Missing 'plugin_create' symbol in {:?}: {}", path, e))?;

        let plugin: Box<dyn NodePlugin> = unsafe { Box::from_raw(plugin_create()) };
        let factory = PluginFactory { plugin };
        Ok((factory, lib))
    }
}

/// Wraps a dynamically loaded `NodePlugin` as a `NodeFactory`.
struct PluginFactory {
    plugin: Box<dyn NodePlugin>,
}

impl NodeFactory for PluginFactory {
    fn create(&self, config: &HashMap<String, Value>) -> Result<Box<dyn Node>, NodeError> {
        // NodePlugin::create_node is async, but NodeFactory::create is sync.
        // Use a minimal blocking runtime or poll once.
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
            outputs: Vec::new(), // Plugin trait doesn't separate input/output ports; could be extended
        }
    }
}
