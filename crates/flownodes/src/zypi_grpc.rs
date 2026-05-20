//! Zypi gRPC client for FlowEngine
//!
//! Provides a typed gRPC client for calling Zypi's sandbox execution service
//! on port 4001. Falls back to REST (port 4000) when gRPC is unavailable.
//!
//! Usage:
//! ```ignore
//! let client = ZypiGrpcClient::new("http://localhost:4001");
//! let result = client.execute(command, image, timeout, env).await;
//! ```

use flowproto::zypi::v1::{
    zypi_service_client::ZypiServiceClient,
    CreateSessionRequest,
    SandboxExecRequest,
    SessionExecRequest,
};
use tonic::transport::Channel;
use tracing::{info, warn};

/// Result from a sandbox execution (normalized across gRPC and REST)
#[derive(Debug, Clone)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub sandbox_id: String,
    pub timed_out: bool,
    pub session_id: Option<String>,
    pub container_id: Option<String>,
    pub ip: Option<String>,
}

/// Result from session creation
#[derive(Debug, Clone)]
pub struct SessionResult {
    pub session_id: String,
    pub container_id: String,
    pub ip: String,
    pub image: String,
    pub status: String,
    pub created_at: String,
}

/// gRPC client for Zypi sandbox service.
///
/// Tries to connect lazily — if the connection fails, all methods
/// return an error so the caller can fall back to REST.
pub struct ZypiGrpcClient {
    endpoint: String,
}

impl ZypiGrpcClient {
    /// Create a new client.
    /// `endpoint` should be a tonic-compatible URI, e.g. `http://localhost:4001`.
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
        }
    }

    /// Get a connected client, or return an error.
    async fn connect(&self) -> Result<ZypiServiceClient<Channel>, tonic::transport::Error> {
        let channel = tonic::transport::Endpoint::from_shared(self.endpoint.clone())?
            .connect()
            .await?;
        Ok(ZypiServiceClient::new(channel))
    }

    /// Execute a one-shot command in a Firecracker sandbox.
    pub async fn execute(
        &self,
        command: Vec<String>,
        image: &str,
        timeout_secs: u32,
        env: std::collections::HashMap<String, String>,
        workdir: Option<&str>,
        memory_mb: Option<u32>,
        vcpus: Option<u32>,
    ) -> Result<ExecResult, String> {
        let mut client = self.connect().await.map_err(|e| {
            format!("gRPC connect failed: {}", e)
        })?;

        let request = SandboxExecRequest {
            command,
            image: image.to_string(),
            timeout_secs,
            memory_mb,
            vcpus,
            env,
            workdir: workdir.map(|s| s.to_string()),
            files: std::collections::HashMap::new(),
            agent_id: None,
            stream: false,
        };

        info!("[Zypi gRPC] Executing: {:?}", request.command);

        let response = client
            .execute(request)
            .await
            .map_err(|e| format!("gRPC Execute error: {}", e))?
            .into_inner();

        Ok(ExecResult {
            stdout: response.stdout,
            stderr: response.stderr,
            exit_code: response.exit_code,
            duration_ms: response.duration_ms,
            sandbox_id: response.sandbox_id,
            timed_out: response.timed_out,
            session_id: None,
            container_id: None,
            ip: None,
        })
    }

    /// Execute a command in an existing session.
    pub async fn session_exec(
        &self,
        session_id: &str,
        command: Vec<String>,
        timeout_secs: u32,
        env: std::collections::HashMap<String, String>,
        workdir: Option<&str>,
    ) -> Result<ExecResult, String> {
        let mut client = self.connect().await.map_err(|e| {
            format!("gRPC connect failed: {}", e)
        })?;

        let request = SessionExecRequest {
            session_id: session_id.to_string(),
            command,
            timeout_secs: Some(timeout_secs),
            env,
            workdir: workdir.map(|s| s.to_string()),
            stream: false,
        };

        info!("[Zypi gRPC] Session exec {}: {:?}", session_id, request.command);

        let response = client
            .session_exec(request)
            .await
            .map_err(|e| format!("gRPC SessionExec error: {}", e))?
            .into_inner();

        Ok(ExecResult {
            stdout: response.stdout,
            stderr: response.stderr,
            exit_code: response.exit_code,
            duration_ms: response.duration_ms,
            sandbox_id: response.sandbox_id,
            timed_out: response.timed_out,
            session_id: Some(session_id.to_string()),
            container_id: None,
            ip: None,
        })
    }

    /// Create a long-lived sandbox session.
    pub async fn create_session(
        &self,
        image: &str,
        vcpus: u32,
        memory_mb: u32,
        agent_id: Option<&str>,
    ) -> Result<SessionResult, String> {
        let mut client = self.connect().await.map_err(|e| {
            format!("gRPC connect failed: {}", e)
        })?;

        let request = CreateSessionRequest {
            agent_id: agent_id.map(|s| s.to_string()),
            image: image.to_string(),
            vcpus: Some(vcpus),
            memory_mb: Some(memory_mb),
            metadata: std::collections::HashMap::new(),
        };

        info!("[Zypi gRPC] Creating session: {}", image);

        let response = client
            .create_session(request)
            .await
            .map_err(|e| format!("gRPC CreateSession error: {}", e))?
            .into_inner();

        Ok(SessionResult {
            session_id: response.session_id,
            container_id: response.container_id,
            ip: response.ip,
            image: response.image,
            status: response.status,
            created_at: response.created_at,
        })
    }
}

/// Helper: try gRPC first, return result or error for REST fallback.
///
/// Returns `Ok(result)` if gRPC succeeded, `Err(err)` if gRPC failed
/// (caller should fall back to REST).
pub async fn try_grpc_or_err<T>(
    grpc_fn: impl std::future::Future<Output = Result<T, String>>,
) -> Result<T, String> {
    match grpc_fn.await {
        Ok(result) => {
            info!("✅ Zypi gRPC call succeeded");
            Ok(result)
        }
        Err(e) => {
            warn!("⚠️  Zypi gRPC failed (will fall back to REST): {}", e);
            Err(e)
        }
    }
}
