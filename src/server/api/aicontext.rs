//! Endpoints for managing `aicontext console` subprocesses.
//!
//! Each project path can have at most one running console. The lifecycle is:
//!   POST /api/aicontext/launch  — spawn the subprocess, return the port
//!   GET  /api/aicontext/status  — check if a console is running for a path
//!   POST /api/aicontext/stop    — kill the subprocess

use std::collections::HashMap;
use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;

use super::AppState;

/// Tracks running aicontext console subprocesses keyed by project path.
pub struct AicontextProcesses {
    inner: RwLock<HashMap<String, AicontextEntry>>,
}

struct AicontextEntry {
    child: Child,
    port: u16,
}

impl AicontextProcesses {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    /// Kill all running processes (called on server shutdown).
    pub async fn shutdown_all(&self) {
        let mut map = self.inner.write().await;
        for (_, mut entry) in map.drain() {
            let _ = entry.child.kill().await;
        }
    }
}

#[derive(Deserialize)]
pub struct LaunchRequest {
    /// The project path to launch aicontext console for.
    pub path: String,
    /// The origin URL of the AoE web dashboard (for --allowed-origin).
    pub origin: Option<String>,
}

#[derive(Serialize)]
pub struct LaunchResponse {
    pub port: u16,
    pub url: String,
}

#[derive(Deserialize)]
pub struct PathQuery {
    pub path: String,
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub running: bool,
    pub port: Option<u16>,
    pub url: Option<String>,
}

/// Find an available port by binding to port 0.
async fn find_available_port() -> std::io::Result<u16> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

/// POST /api/aicontext/launch
pub async fn aicontext_launch(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LaunchRequest>,
) -> impl IntoResponse {
    if state.read_only {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "Server is in read-only mode"})),
        )
            .into_response();
    }

    let processes = &state.aicontext_processes;

    // Check if already running for this path.
    {
        let map = processes.inner.read().await;
        if let Some(entry) = map.get(&body.path) {
            return Json(LaunchResponse {
                port: entry.port,
                url: format!("http://127.0.0.1:{}", entry.port),
            })
            .into_response();
        }
    }

    // Find available port.
    let port = match find_available_port().await {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to find available port: {}", e)})),
            )
                .into_response();
        }
    };

    // Determine the origin for --allowed-origin.
    let origin = body
        .origin
        .unwrap_or_else(|| "http://127.0.0.1:8081".to_string());

    // Spawn aicontext console with browser opening suppressed.
    // On macOS, aicontext uses NSWorkspace/open to launch the browser directly,
    // ignoring BROWSER env var. We use sandbox-exec to deny process-exec of
    // /usr/bin/open, which prevents the tab from opening.
    let sandbox_profile = std::env::temp_dir().join("aoe-no-browser.sb");
    let _ = std::fs::write(
        &sandbox_profile,
        "(version 1)\n(allow default)\n(deny process-exec (literal \"/usr/bin/open\"))\n",
    );

    // Determine the working directory: prefer runtimes/root/ if it exists.
    let cwd = {
        let runtime_root = std::path::Path::new(&body.path).join("runtimes/root");
        if runtime_root.is_dir() {
            runtime_root
        } else {
            std::path::Path::new(&body.path).to_path_buf()
        }
    };

    let child = match Command::new("sandbox-exec")
        .args([
            "-f",
            sandbox_profile.to_str().unwrap_or("/tmp/aoe-no-browser.sb"),
            "aicontext",
            "console",
            "--port",
            &port.to_string(),
            "--allowed-origin",
            &origin,
        ])
        .current_dir(&cwd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to spawn aicontext console: {}", e)})),
            )
                .into_response();
        }
    };

    // Give it a moment to start up.
    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

    let mut map = processes.inner.write().await;
    map.insert(body.path, AicontextEntry { child, port });

    Json(LaunchResponse {
        port,
        url: format!("http://127.0.0.1:{}", port),
    })
    .into_response()
}

/// GET /api/aicontext/status?path=...
pub async fn aicontext_status(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(query): axum::extract::Query<PathQuery>,
) -> Json<StatusResponse> {
    let processes = &state.aicontext_processes;
    let map = processes.inner.read().await;

    if let Some(entry) = map.get(&query.path) {
        Json(StatusResponse {
            running: true,
            port: Some(entry.port),
            url: Some(format!("http://127.0.0.1:{}", entry.port)),
        })
    } else {
        Json(StatusResponse {
            running: false,
            port: None,
            url: None,
        })
    }
}

/// POST /api/aicontext/stop
pub async fn aicontext_stop(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PathQuery>,
) -> impl IntoResponse {
    if state.read_only {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "Server is in read-only mode"})),
        )
            .into_response();
    }

    let processes = &state.aicontext_processes;
    let mut map = processes.inner.write().await;

    if let Some(mut entry) = map.remove(&body.path) {
        let _ = entry.child.kill().await;
        (StatusCode::OK, Json(serde_json::json!({"stopped": true}))).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "No running console for this path"})),
        )
            .into_response()
    }
}
