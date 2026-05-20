//! Generated gRPC types for FlowEngine and Zypi services.
//!
//! Contains:
//! - `flowengine::v1` — FlowEngine gRPC service (port 3001)
//! - `zypi::v1` — Zypi gRPC client types (port 4001)

pub mod flowengine {
    pub mod v1 {
        tonic::include_proto!("flowengine.v1");
    }
}

pub mod zypi {
    pub mod v1 {
        tonic::include_proto!("zypi.v1");
    }
}
