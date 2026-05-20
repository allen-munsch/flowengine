// crates/flowruntime/src/loader.rs
pub struct CustomNodeLoader {
    watch_dir: PathBuf,
}

impl CustomNodeLoader {
    pub async fn load_custom_nodes(&self, registry: &mut NodeRegistry) -> Result<()> {
        for entry in std::fs::read_dir(&self.watch_dir)? {
            let path = entry?.path();
            if path.extension() == Some("json".as_ref()) {
                let node_def: CustomNodeDefinition = serde_json::from_reader(
                    std::fs::File::open(&path)?
                )?;
                
                registry.register_custom(node_def)?;
            }
        }
        Ok(())
    }
}