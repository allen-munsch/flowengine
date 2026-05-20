// crates/flownodes/src/docker/factory.rs
pub struct ConfigurableDockerNodeFactory;

impl NodeFactory for ConfigurableDockerNodeFactory {
    fn create(&self, config: &HashMap<String, Value>) -> Result<Box<dyn Node>, NodeError> {
        let image = config.get("image")
            .and_then(|v| v.as_str())
            .ok_or_else(|| NodeError::Configuration("Missing 'image' config".into()))?;
        
        // Build Docker node from config
        DockerNodeBuilder::new(image)
            .with_optional_command(config.get("command"))
            .with_optional_env(config.get("env"))
            .build()
    }
    
    fn node_type(&self) -> &str { "docker.custom" }
}