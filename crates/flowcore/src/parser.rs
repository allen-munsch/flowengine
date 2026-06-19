use crate::{Workflow, FlowError};

/// Trait for parsing workflow definitions from different formats.
pub trait WorkflowParser: Send + Sync {
    /// Parse a workflow from a string.
    fn parse(&self, input: &str) -> Result<Workflow, FlowError>;

    /// The format name (e.g. "json", "yaml").
    fn format_name(&self) -> &str;
}

/// Parser for JSON workflow definitions (existing format).
pub struct JsonParser;

impl WorkflowParser for JsonParser {
    fn parse(&self, input: &str) -> Result<Workflow, FlowError> {
        serde_json::from_str(input)
            .map_err(|e| FlowError::Parse(format!("JSON parse error: {}", e)))
    }

    fn format_name(&self) -> &str {
        "json"
    }
}

/// Parser for YAML workflow definitions.
pub struct YamlParser;

impl WorkflowParser for YamlParser {
    fn parse(&self, input: &str) -> Result<Workflow, FlowError> {
        serde_yaml::from_str(input)
            .map_err(|e| FlowError::Parse(format!("YAML parse error: {}", e)))
    }

    fn format_name(&self) -> &str {
        "yaml"
    }
}

/// Auto-detect format and parse a workflow.
/// If the first non-whitespace character is `{`, tries JSON first; otherwise YAML first.
/// Falls back to the other format on failure.
pub fn parse_workflow(input: &str) -> Result<Workflow, FlowError> {
    let trimmed = input.trim_start();
    let (first, second): (&dyn WorkflowParser, &dyn WorkflowParser) = if trimmed.starts_with('{') {
        (&JsonParser, &YamlParser)
    } else {
        (&YamlParser, &JsonParser)
    };

    first.parse(input).or_else(|_| second.parse(input))
}

/// Parse a workflow from a file path, detecting format by extension.
pub fn parse_workflow_file(path: &std::path::Path, contents: &str) -> Result<Workflow, FlowError> {
    let parser: &dyn WorkflowParser = match path.extension().and_then(|e| e.to_str()) {
        Some("yaml") | Some("yml") => &YamlParser,
        _ => &JsonParser,
    };
    parser.parse(contents)
}
