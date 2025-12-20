use crate::config::load_config;
use crate::setup::start_harness;
use crate::snapshot::{create_snapshot, is_validator_running, list_snapshots, load_snapshot};
use anyhow::{Context, Result};
use axum::{
    extract::State,
    response::{Html, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use tokio::process::Child;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use std::fs;
use std::path::Path;

#[derive(Clone)]
pub struct AppState {
    pub validator_process: Arc<Mutex<Option<Child>>>,
    pub config_path: Arc<RwLock<String>>,
}

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub running: bool,
    pub rpc_url: Option<String>,
    pub current_config: String,
}

#[derive(Deserialize)]
pub struct SnapshotRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct LoadSnapshotRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct SnapshotConfigRequest {
    pub name: String,
}

#[derive(Serialize)]
pub struct SnapshotInfo {
    pub name: String,
    pub created_at: Option<String>,
    pub config_path: Option<String>,
}

#[derive(Deserialize)]
pub struct ConfigSelectRequest {
    pub path: String,
}

#[derive(Deserialize)]
pub struct SaveConfigRequest {
    pub content: serde_json::Value,
}

#[derive(Deserialize)]
pub struct CreateConfigRequest {
    pub name: String,
    pub content: serde_json::Value,
}

#[derive(Serialize)]
pub struct ConfigInfo {
    pub name: String,
    pub path: String,
}

pub async fn start_web_server(port: u16, config_path: String) -> Result<()> {
    if !std::path::Path::new(&config_path).exists() {
        println!("⚠️  Warning: Config file '{}' not found.", config_path);
    }

    if !std::path::Path::new("web/index.html").exists() {
        return Err(anyhow::anyhow!("web/index.html not found. Run from project root."));
    }

    let state = AppState {
        validator_process: Arc::new(Mutex::new(None)),
        config_path: Arc::new(RwLock::new(config_path.clone())),
    };

    let index_html = Arc::new(fs::read_to_string("web/index.html").context("Failed to read web/index.html")?);

    let app = Router::new()
        .route("/", get({
            let html = index_html.clone();
            move || {
                let html = html.clone();
                async move { Html((*html).clone()) }
            }
        }))
        .route("/api/status", get(get_status))
        .route("/api/start", post(start_validator))
        .route("/api/stop", post(stop_validator))
        .route("/api/snapshots", get(list_snapshots_api))
        .route("/api/snapshots/create", post(create_snapshot_api))
        .route("/api/snapshots/load", post(load_snapshot_api))
        .route("/api/snapshots/config", post(get_snapshot_config))
        .route("/api/config", get(get_config))
        .route("/api/configs", get(list_configs))
        .route("/api/configs/select", post(select_config))
        .route("/api/config/save", post(save_config))
        .route("/api/configs/create", post(create_config))
        .fallback_service(ServeDir::new("web"))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await.context("Failed to bind")?;
    println!("🌐 Cadenza Web UI running on http://localhost:{}", port);

    axum::serve(listener, app).await.context("Failed to start server")?;
    Ok(())
}

async fn get_status(State(state): State<AppState>) -> Json<ApiResponse<StatusResponse>> {
    let mut process = state.validator_process.lock().await;
    if let Some(child) = process.as_mut() {
        if let Ok(Some(_)) = child.try_wait() { *process = None; }
    }

    let config_path = state.config_path.read().unwrap().clone();
    let config = load_config(&config_path).ok();
    let cluster = config.as_ref().map(|c| c.cluster);

    let running = match is_validator_running() {
        Ok(run) => run,
        Err(_) => process.is_some(),
    };

    let rpc_url = match cluster {
        Some(crate::config::Cluster::Devnet) => Some("https://api.devnet.solana.com".to_string()),
        _ if running => match config {
            Some(c) => Some(format!("http://127.0.0.1:{}", c.rpc_port)),
            None => Some("http://127.0.0.1:8899".to_string()),
        },
        _ => None,
    };

    Json(ApiResponse { success: true, data: Some(StatusResponse { running, rpc_url, current_config: config_path }), error: None })
}

async fn start_validator(State(state): State<AppState>) -> Json<ApiResponse<String>> {
    let mut process = state.validator_process.lock().await;
    let config_path = state.config_path.read().unwrap().clone();

    if let Some(child) = process.as_mut() {
        if child.try_wait().map(|s| s.is_none()).unwrap_or(false) {
            return Json(ApiResponse { success: false, data: None, error: Some("Already running".to_string()) });
        }
    }

    match start_harness(&config_path).await {
        Ok(Some(child)) => {
            *process = Some(child);
            Json(ApiResponse { success: true, data: Some("Started".to_string()), error: None })
        }
        Ok(None) => Json(ApiResponse { success: true, data: Some("Devnet ready".to_string()), error: None }),
        Err(e) => Json(ApiResponse { success: false, data: None, error: Some(e.to_string()) }),
    }
}

async fn stop_validator(State(state): State<AppState>) -> Json<ApiResponse<String>> {
    let mut process = state.validator_process.lock().await;
    let config_path = state.config_path.read().unwrap().clone();

    if let Some(mut child) = process.take() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }

    for name in vec!["solana-test-validator", "solana-test-val"] {
        if cfg!(target_os = "windows") {
            let _ = std::process::Command::new("taskkill").args(["/F", "/IM", &format!("{}.exe", name)]).output();
        } else {
            let _ = std::process::Command::new("pkill").arg("-15").arg("-f").arg(name).output();
            let _ = std::process::Command::new("pkill").arg("-9").arg("-f").arg(name).output();
        }
    }

    let is_devnet = load_config(&config_path).map(|c| matches!(c.cluster, crate::config::Cluster::Devnet)).unwrap_or(false);
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    if !is_devnet && is_validator_running().unwrap_or(false) {
        return Json(ApiResponse { success: false, data: None, error: Some("Still running".to_string()) });
    }

    Json(ApiResponse { success: true, data: Some("Stopped".to_string()), error: None })
}

async fn list_snapshots_api() -> Json<ApiResponse<Vec<SnapshotInfo>>> {
    match list_snapshots() {
        Ok(snapshots) => {
            let details: Vec<SnapshotInfo> = snapshots
                .into_iter()
                .map(|name| {
                    let meta_path = Path::new("snapshots").join(&name).join("metadata.json");
                    let (created_at, config_path) = if meta_path.exists() {
                        fs::read_to_string(&meta_path)
                            .ok()
                            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                            .map(|v| {
                                let created = v
                                    .get("created_at")
                                    .and_then(|c| c.as_str())
                                    .map(|s| s.to_string());
                                let cfg = v
                                    .get("config_path")
                                    .and_then(|c| c.as_str())
                                    .map(|s| s.to_string());
                                (created, cfg)
                            })
                            .unwrap_or((None, None))
                    } else {
                        (None, None)
                    };
                    SnapshotInfo {
                        name,
                        created_at,
                        config_path,
                    }
                })
                .collect();
            Json(ApiResponse { success: true, data: Some(details), error: None })
        }
        Err(e) => Json(ApiResponse { success: false, data: None, error: Some(e.to_string()) }),
    }
}

async fn create_snapshot_api(State(state): State<AppState>, Json(req): Json<SnapshotRequest>) -> Json<ApiResponse<String>> {
    let config_path = state.config_path.read().unwrap().clone();
    // Web UI currently does not support overwrite, defaulting to false
    match create_snapshot(&req.name, &config_path, false) {
        Ok(_) => Json(ApiResponse { success: true, data: Some("Created".to_string()), error: None }),
        Err(e) => Json(ApiResponse { success: false, data: None, error: Some(e.to_string()) }),
    }
}

async fn load_snapshot_api(Json(req): Json<LoadSnapshotRequest>) -> Json<ApiResponse<String>> {
    match load_snapshot(&req.name) {
        Ok(_) => Json(ApiResponse { success: true, data: Some("Loaded".to_string()), error: None }),
        Err(e) => Json(ApiResponse { success: false, data: None, error: Some(e.to_string()) }),
    }
}

async fn get_snapshot_config(Json(req): Json<SnapshotConfigRequest>) -> Json<ApiResponse<serde_json::Value>> {
    let path = Path::new("snapshots")
        .join(&req.name)
        .join("cadenza-config.json");
    if !path.exists() {
        return Json(ApiResponse { success: false, data: None, error: Some("Config not found in snapshot".to_string()) });
    }
    match fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
    {
        Some(val) => Json(ApiResponse { success: true, data: Some(val), error: None }),
        None => Json(ApiResponse { success: false, data: None, error: Some("Failed to parse config".to_string()) }),
    }
}

async fn get_config(State(state): State<AppState>) -> Json<ApiResponse<serde_json::Value>> {
    let config_path = state.config_path.read().unwrap().clone();
    match load_config(&config_path) {
        Ok(config) => Json(ApiResponse { success: true, data: Some(serde_json::to_value(config).unwrap_or_default()), error: None }),
        Err(e) => Json(ApiResponse { success: false, data: None, error: Some(e.to_string()) }),
    }
}

async fn list_configs() -> Json<ApiResponse<Vec<ConfigInfo>>> {
    let mut configs = Vec::new();
    if let Ok(entries) = fs::read_dir(".") {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
                let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
                if name.contains("cadenza") || name.contains("config") {
                    configs.push(ConfigInfo { name: name.clone(), path: name });
                }
            }
        }
    }
    configs.sort_by(|a, b| a.name.cmp(&b.name));
    Json(ApiResponse { success: true, data: Some(configs), error: None })
}

async fn select_config(State(state): State<AppState>, Json(req): Json<ConfigSelectRequest>) -> Json<ApiResponse<String>> {
    if !std::path::Path::new(&req.path).exists() {
        return Json(ApiResponse { success: false, data: None, error: Some("Not found".to_string()) });
    }
    let mut path = state.config_path.write().unwrap();
    *path = req.path.clone();
    Json(ApiResponse { success: true, data: Some("Switched".to_string()), error: None })
}

async fn save_config(State(state): State<AppState>, Json(req): Json<SaveConfigRequest>) -> Json<ApiResponse<String>> {
    let path = state.config_path.read().unwrap().clone();
    match serde_json::to_string_pretty(&req.content) {
        Ok(serialized) => {
            if let Err(e) = fs::write(&path, serialized) {
                return Json(ApiResponse { success: false, data: None, error: Some(e.to_string()) });
            }
            Json(ApiResponse { success: true, data: Some("Saved".to_string()), error: None })
        }
        Err(e) => Json(ApiResponse { success: false, data: None, error: Some(e.to_string()) }),
    }
}

async fn create_config(State(state): State<AppState>, Json(req): Json<CreateConfigRequest>) -> Json<ApiResponse<String>> {
    if req.name.is_empty() || req.name.contains('/') || req.name.contains('\\') {
        return Json(ApiResponse { success: false, data: None, error: Some("Invalid name".to_string()) });
    }

    let filename = if req.name.ends_with(".json") { req.name.clone() } else { format!("{}.json", req.name) };
    if Path::new(&filename).exists() {
        return Json(ApiResponse { success: false, data: None, error: Some("File already exists".to_string()) });
    }

    match serde_json::to_string_pretty(&req.content) {
        Ok(serialized) => {
            if let Err(e) = fs::write(&filename, serialized) {
                return Json(ApiResponse { success: false, data: None, error: Some(e.to_string()) });
            }
            let mut path = state.config_path.write().unwrap();
            *path = filename.clone();
            Json(ApiResponse { success: true, data: Some(filename), error: None })
        }
        Err(e) => Json(ApiResponse { success: false, data: None, error: Some(e.to_string()) }),
    }
}
