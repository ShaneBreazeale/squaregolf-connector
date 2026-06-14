use std::net::{Ipv4Addr, SocketAddr};

use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa::ToSchema;
use utoipa_swagger_ui::SwaggerUi;

use crate::config::{AppConfig, ConfigStore};
use crate::core::{AppState, AppStatus, UiEvent};
use crate::device::runtime::{DeviceConnectOptions, DeviceRuntime};
use crate::simulator::runtime::SimulatorRuntime;
use crate::squarelaunch;

#[derive(OpenApi)]
#[openapi(
    paths(
        health,
        get_status,
        connect_device,
        disconnect_device,
        connect_gspro,
        disconnect_gspro,
        connect_infinite_tees,
        disconnect_infinite_tees,
        get_config,
        update_config,
    ),
    components(schemas(
        AppConfig,
        AppStatus,
        crate::core::DeviceStatus,
        crate::core::ConnectionStatus,
        crate::core::BallMetrics,
        crate::core::SimulatorStatus,
        crate::core::SquareLaunchStatus,
        ConfigUpdate,
        DeviceConnectRequest,
        ActionAccepted,
    )),
    tags(
        (name = "status", description = "Connector status and health"),
        (name = "device", description = "SquareGolf device controls"),
        (name = "config", description = "Runtime connector configuration")
    )
)]
struct ApiDoc;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActionAccepted {
    accepted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigUpdate {
    pub gspro_enabled: Option<bool>,
    pub gspro_host: Option<String>,
    pub gspro_port: Option<u16>,
    pub infinite_tees_enabled: Option<bool>,
    pub infinite_tees_host: Option<String>,
    pub infinite_tees_port: Option<u16>,
    pub squarelaunch_enabled: Option<bool>,
    pub squarelaunch_ws_host: Option<String>,
    pub squarelaunch_ws_port: Option<u16>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeviceConnectRequest {
    pub device_name: Option<String>,
    pub device_address: Option<String>,
}

#[derive(Clone)]
pub struct ApiState {
    app: AppState,
    device: DeviceRuntime,
    simulators: SimulatorRuntime,
    config_store: Option<ConfigStore>,
}

pub async fn spawn_from_env(app: tauri::AppHandle) -> Result<(), String> {
    let config = AppConfig::from_env()?;
    serve_with_ready(config, Some(ConfigStore::default()), move |addr| {
        app.emit("api-ready", format!("http://{addr}"))
            .map_err(|err| err.to_string())
    })
    .await
}

pub async fn serve(config: AppConfig) -> Result<(), String> {
    serve_with_store(config, Some(ConfigStore::default())).await
}

pub async fn serve_with_store(
    config: AppConfig,
    config_store: Option<ConfigStore>,
) -> Result<(), String> {
    serve_with_ready(config, config_store, |_| Ok(())).await
}

pub async fn serve_with_ready<F>(
    mut config: AppConfig,
    config_store: Option<ConfigStore>,
    ready: F,
) -> Result<(), String>
where
    F: FnOnce(SocketAddr) -> Result<(), String>,
{
    let (addr, listener) = bind_api_listener(config.api_port).await?;
    config.api_port = addr.port();

    let state = AppState::new(&config);
    squarelaunch::runtime::spawn(config.clone(), state.clone());

    let router = router_with_store(state, config_store);

    ready(addr)?;
    tracing::info!("API server listening on http://{addr}");
    axum::serve(listener, router)
        .await
        .map_err(|err| format!("API server failed: {err}"))
}

async fn bind_api_listener(
    preferred_port: u16,
) -> Result<(SocketAddr, tokio::net::TcpListener), String> {
    let mut last_error = None;
    for offset in 0..100u16 {
        let Some(port) = preferred_port.checked_add(offset) else {
            break;
        };
        let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => {
                if port != preferred_port {
                    tracing::warn!(
                        "API port {preferred_port} was unavailable; using http://{addr}"
                    );
                }
                return Ok((addr, listener));
            }
            Err(err) => {
                last_error = Some(format!("bind API server on {addr}: {err}"));
            }
        }
    }
    Err(last_error.unwrap_or_else(|| {
        format!("bind API server near port {preferred_port}: no candidate ports")
    }))
}

pub fn router(state: AppState) -> Router {
    let simulators = SimulatorRuntime::new(state.clone());
    router_with_simulators(state, simulators)
}

pub fn router_with_store(state: AppState, config_store: Option<ConfigStore>) -> Router {
    let simulators = SimulatorRuntime::new(state.clone());
    router_with_simulators_and_store(state, simulators, config_store)
}

pub fn router_with_simulators(state: AppState, simulators: SimulatorRuntime) -> Router {
    router_with_simulators_and_store(state, simulators, None)
}

pub fn router_with_simulators_and_store(
    state: AppState,
    simulators: SimulatorRuntime,
    config_store: Option<ConfigStore>,
) -> Router {
    let api_state = ApiState {
        app: state.clone(),
        device: DeviceRuntime::new(state, simulators.clone()),
        simulators,
        config_store,
    };
    Router::new()
        .route("/api/health", get(health))
        .route("/api/status", get(get_status))
        .route("/api/device/connect", post(connect_device))
        .route("/api/device/disconnect", post(disconnect_device))
        .route("/api/gspro/connect", post(connect_gspro))
        .route("/api/gspro/disconnect", post(disconnect_gspro))
        .route("/api/infinitetees/connect", post(connect_infinite_tees))
        .route(
            "/api/infinitetees/disconnect",
            post(disconnect_infinite_tees),
        )
        .route("/api/config", get(get_config).post(update_config))
        .route("/ws", get(ws_handler))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(CorsLayer::permissive())
        .with_state(api_state)
}

#[utoipa::path(
    get,
    path = "/api/health",
    tag = "status",
    responses((status = OK, body = ActionAccepted))
)]
async fn health() -> Json<ActionAccepted> {
    Json(ActionAccepted { accepted: true })
}

#[utoipa::path(
    get,
    path = "/api/status",
    tag = "status",
    responses((status = OK, body = AppStatus))
)]
async fn get_status(State(state): State<ApiState>) -> Json<AppStatus> {
    Json(state.app.status().await)
}

#[utoipa::path(
    post,
    path = "/api/device/connect",
    tag = "device",
    request_body = DeviceConnectRequest,
    responses((status = ACCEPTED, body = ActionAccepted))
)]
async fn connect_device(State(state): State<ApiState>, body: Bytes) -> impl IntoResponse {
    let request = if body.is_empty() {
        DeviceConnectRequest::default()
    } else {
        match serde_json::from_slice::<DeviceConnectRequest>(&body) {
            Ok(request) => request,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ActionAccepted { accepted: false }),
                );
            }
        }
    };
    state
        .device
        .connect(DeviceConnectOptions {
            device_name: request.device_name,
            device_address: request.device_address,
        })
        .await;
    (
        StatusCode::ACCEPTED,
        Json(ActionAccepted { accepted: true }),
    )
}

async fn save_current_config(state: &ApiState) -> Result<(), String> {
    let Some(store) = &state.config_store else {
        return Ok(());
    };
    let status = state.app.status().await;
    store.save(&AppConfig {
        api_port: status.api_port,
        gspro_host: status.gspro.host,
        gspro_port: status.gspro.port,
        gspro_enabled: status.gspro.enabled,
        infinite_tees_host: status.infinite_tees.host,
        infinite_tees_port: status.infinite_tees.port,
        infinite_tees_enabled: status.infinite_tees.enabled,
        squarelaunch_ws_host: status.squarelaunch.host,
        squarelaunch_ws_port: status.squarelaunch.port,
        squarelaunch_enabled: status.squarelaunch.enabled,
    })
}

#[utoipa::path(
    post,
    path = "/api/device/disconnect",
    tag = "device",
    responses((status = ACCEPTED, body = ActionAccepted))
)]
async fn disconnect_device(State(state): State<ApiState>) -> impl IntoResponse {
    state.device.disconnect().await;
    (
        StatusCode::ACCEPTED,
        Json(ActionAccepted { accepted: true }),
    )
}

#[utoipa::path(
    post,
    path = "/api/gspro/connect",
    tag = "config",
    responses(
        (status = ACCEPTED, body = ActionAccepted),
        (status = BAD_GATEWAY, body = ActionAccepted)
    )
)]
async fn connect_gspro(State(state): State<ApiState>) -> impl IntoResponse {
    match state.simulators.connect_gspro().await {
        Ok(()) => (
            StatusCode::ACCEPTED,
            Json(ActionAccepted { accepted: true }),
        ),
        Err(_) => (
            StatusCode::BAD_GATEWAY,
            Json(ActionAccepted { accepted: false }),
        ),
    }
}

#[utoipa::path(
    post,
    path = "/api/gspro/disconnect",
    tag = "config",
    responses((status = ACCEPTED, body = ActionAccepted))
)]
async fn disconnect_gspro(State(state): State<ApiState>) -> impl IntoResponse {
    state.simulators.disconnect_gspro().await;
    (
        StatusCode::ACCEPTED,
        Json(ActionAccepted { accepted: true }),
    )
}

#[utoipa::path(
    post,
    path = "/api/infinitetees/connect",
    tag = "config",
    responses(
        (status = ACCEPTED, body = ActionAccepted),
        (status = BAD_GATEWAY, body = ActionAccepted)
    )
)]
async fn connect_infinite_tees(State(state): State<ApiState>) -> impl IntoResponse {
    match state.simulators.connect_infinite_tees().await {
        Ok(()) => (
            StatusCode::ACCEPTED,
            Json(ActionAccepted { accepted: true }),
        ),
        Err(_) => (
            StatusCode::BAD_GATEWAY,
            Json(ActionAccepted { accepted: false }),
        ),
    }
}

#[utoipa::path(
    post,
    path = "/api/infinitetees/disconnect",
    tag = "config",
    responses((status = ACCEPTED, body = ActionAccepted))
)]
async fn disconnect_infinite_tees(State(state): State<ApiState>) -> impl IntoResponse {
    state.simulators.disconnect_infinite_tees().await;
    (
        StatusCode::ACCEPTED,
        Json(ActionAccepted { accepted: true }),
    )
}

#[utoipa::path(
    get,
    path = "/api/config",
    tag = "config",
    responses((status = OK, body = AppConfig))
)]
async fn get_config(State(state): State<ApiState>) -> Json<AppConfig> {
    let status = state.app.status().await;
    Json(AppConfig {
        api_port: status.api_port,
        gspro_host: status.gspro.host,
        gspro_port: status.gspro.port,
        gspro_enabled: status.gspro.enabled,
        infinite_tees_host: status.infinite_tees.host,
        infinite_tees_port: status.infinite_tees.port,
        infinite_tees_enabled: status.infinite_tees.enabled,
        squarelaunch_ws_host: status.squarelaunch.host,
        squarelaunch_ws_port: status.squarelaunch.port,
        squarelaunch_enabled: status.squarelaunch.enabled,
    })
}

#[utoipa::path(
    post,
    path = "/api/config",
    tag = "config",
    request_body = ConfigUpdate,
    responses((status = ACCEPTED, body = ActionAccepted))
)]
async fn update_config(
    State(state): State<ApiState>,
    Json(update): Json<ConfigUpdate>,
) -> impl IntoResponse {
    if update.gspro_enabled.is_some() || update.gspro_host.is_some() || update.gspro_port.is_some()
    {
        state
            .app
            .update_gspro(|status| {
                if let Some(enabled) = update.gspro_enabled {
                    status.enabled = enabled;
                }
                if let Some(host) = update.gspro_host.clone() {
                    let trimmed = host.trim();
                    if !trimmed.is_empty() {
                        status.host = trimmed.to_string();
                    }
                }
                if let Some(port) = update.gspro_port {
                    status.port = port.max(1);
                }
            })
            .await;
    }
    if update.infinite_tees_enabled.is_some()
        || update.infinite_tees_host.is_some()
        || update.infinite_tees_port.is_some()
    {
        state
            .app
            .update_infinite_tees(|status| {
                if let Some(enabled) = update.infinite_tees_enabled {
                    status.enabled = enabled;
                }
                if let Some(host) = update.infinite_tees_host.clone() {
                    let trimmed = host.trim();
                    if !trimmed.is_empty() {
                        status.host = trimmed.to_string();
                    }
                }
                if let Some(port) = update.infinite_tees_port {
                    status.port = port.max(1);
                }
            })
            .await;
    }
    state
        .app
        .update_squarelaunch(|status| {
            if let Some(enabled) = update.squarelaunch_enabled {
                status.enabled = enabled;
            }
            if let Some(host) = update.squarelaunch_ws_host {
                let trimmed = host.trim();
                status.host = (!trimmed.is_empty()).then(|| trimmed.to_string());
            }
            if let Some(port) = update.squarelaunch_ws_port {
                status.port = port.max(1);
            }
        })
        .await;
    match save_current_config(&state).await {
        Ok(()) => (
            StatusCode::ACCEPTED,
            Json(ActionAccepted { accepted: true }),
        ),
        Err(err) => {
            tracing::error!("failed to save config: {err}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ActionAccepted { accepted: false }),
            )
        }
    }
}

async fn ws_handler(State(state): State<ApiState>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| ui_socket(socket, state.app))
}

async fn ui_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let initial = UiEvent::Status(state.status().await);
    if let Ok(text) = serde_json::to_string(&initial) {
        if sender.send(Message::Text(text.into())).await.is_err() {
            return;
        }
    }

    let mut events = state.subscribe();
    loop {
        tokio::select! {
            event = events.recv() => {
                let Ok(event) = event else { break; };
                let Ok(text) = serde_json::to_string(&event) else { continue; };
                if sender.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }
            inbound = receiver.next() => {
                match inbound {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
        }
    }
}
