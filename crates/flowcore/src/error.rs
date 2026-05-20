use thiserror::Error;

#[derive(Error, Debug)]
pub enum FlowError {
    #[error("Node error: {0}")]
    Node(#[from] NodeError),
    
    #[error("Workflow error: {0}")]
    Workflow(#[from] WorkflowError),
    
    #[error("Execution error: {0}")]
    Execution(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Error, Debug, Clone)]
pub enum NodeError {
    #[error("Missing required input: {0}")]
    MissingInput(String),
    
    #[error("Invalid input type for '{field}': expected {expected}, got {actual}")]
    InvalidInputType {
        field: String,
        expected: String,
        actual: String,
    },
    
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    
    #[error("Node initialization failed: {0}")]
    InitializationFailed(String),
    
    #[error("Timeout after {seconds}s")]
    Timeout { seconds: u64 },
    
    #[error("Cancelled")]
    Cancelled,
}

#[derive(Error, Debug)]
pub enum WorkflowError {
    #[error("Workflow not found: {0}")]
    NotFound(String),
    
    #[error("Invalid workflow: {0}")]
    Invalid(String),
    
    #[error("Cyclic dependency detected")]
    CyclicDependency,
    
    #[error("Node not found: {0}")]
    NodeNotFound(String),
    
    #[error("Unknown node type: {0}")]
    UnknownNodeType(String),
    
    #[error("Invalid connection: {0}")]
    InvalidConnection(String),
}
