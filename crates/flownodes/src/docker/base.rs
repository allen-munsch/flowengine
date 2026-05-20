// crates/flownodes/src/docker/base.rs
pub struct DockerNodeBuilder {
    image: String,
    command: Option<String>,
    env: HashMap<String, String>,
    input_mapping: InputMapping,
    output_mapping: OutputMapping,
}

impl DockerNodeBuilder {
    pub fn new(image: impl Into<String>) -> Self { ... }
    pub fn with_command(mut self, cmd: impl Into<String>) -> Self { ... }
    pub fn with_env(mut self, key: impl Into<String>, val: impl Into<String>) -> Self { ... }
    pub fn build(self) -> Box<dyn Node> { ... }
}