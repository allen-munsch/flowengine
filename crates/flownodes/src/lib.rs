//! Standard node library
//! 
//! Collection of built-in nodes for common operations

mod debug;
mod http;
mod transform;
mod time;
mod docker;
mod docker_v2;

pub use debug::DebugNode;
pub use docker::{DockerNode, DockerNodeFactory};
pub use docker_v2::{DockerNodeV2, DockerNodeV2Factory};
pub use http::HttpRequestNode;
pub use transform::{JsonParseNode, JsonStringifyNode};
pub use time::DelayNode;
use flowruntime::NodeRegistry;

use std::sync::Arc;

/// Register all standard nodes with a registry
pub fn register_all(registry: &mut NodeRegistry) {
    registry.register(Arc::new(debug::DebugNodeFactory));
    registry.register(Arc::new(docker::DockerNodeFactory));
    registry.register(Arc::new(docker_v2::DockerNodeV2Factory));
    registry.register(Arc::new(http::HttpRequestNodeFactory));
    registry.register(Arc::new(transform::JsonParseNodeFactory));
    registry.register(Arc::new(transform::JsonStringifyNodeFactory));
    registry.register(Arc::new(time::DelayNodeFactory));
}
