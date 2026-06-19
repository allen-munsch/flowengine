//! Standard node library
//! 
//! Collection of built-in nodes for common operations

mod api_call;
mod browser;
pub mod database;
mod debug;
mod docker;
mod docker_v2;
mod flow;
mod http;
mod shell;
mod time;
mod transform;
mod zypi;

pub use api_call::ApiCallNode;
pub use browser::BrowserRenderNode;
pub use database::sqlite_reader::SqliteReaderNode;
pub use debug::DebugNode;
pub use docker::{DockerNode, DockerNodeFactory};
pub use docker_v2::{DockerNodeV2, DockerNodeV2Factory};
pub use flow::branch::BranchNode;
pub use flow::merge::MergeNode;
pub use http::HttpRequestNode;
pub use shell::ShellExecNode;
pub use time::DelayNode;
pub use transform::{JsonParseNode, JsonStringifyNode};
pub use zypi::{ZypiExecNode, ZypiSessionCreateNode};
use flowruntime::NodeRegistry;

use std::sync::Arc;

/// Register all standard nodes with a registry
pub fn register_all(registry: &mut NodeRegistry) {
    registry.register(Arc::new(api_call::ApiCallNodeFactory));
    registry.register(Arc::new(browser::BrowserRenderNodeFactory));
    registry.register(Arc::new(database::sqlite_reader::SqliteReaderNodeFactory));
    registry.register(Arc::new(debug::DebugNodeFactory));
    registry.register(Arc::new(docker::DockerNodeFactory));
    registry.register(Arc::new(docker_v2::DockerNodeV2Factory));
    registry.register(Arc::new(flow::branch::BranchNodeFactory));
    registry.register(Arc::new(flow::merge::MergeNodeFactory));
    registry.register(Arc::new(http::HttpRequestNodeFactory));
    registry.register(Arc::new(shell::ShellExecNodeFactory));
    registry.register(Arc::new(time::DelayNodeFactory));
    registry.register(Arc::new(transform::JsonParseNodeFactory));
    registry.register(Arc::new(transform::JsonStringifyNodeFactory));
    registry.register(Arc::new(zypi::ZypiExecNodeFactory));
    registry.register(Arc::new(zypi::ZypiSessionCreateNodeFactory));
}
