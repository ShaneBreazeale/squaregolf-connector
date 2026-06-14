use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};
use utoipa::ToSchema;

use crate::config::AppConfig;
use protocol::parser::ParsedBallMetrics;

pub mod protocol;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub api_port: u16,
    pub device: DeviceStatus,
    pub gspro: SimulatorStatus,
    pub infinite_tees: SimulatorStatus,
    pub squarelaunch: SquareLaunchStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStatus {
    pub connection_status: ConnectionStatus,
    pub device_name: Option<String>,
    pub battery_level: Option<u8>,
    pub last_error: Option<String>,
    pub last_ball_metrics: Option<BallMetrics>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionStatus {
    Disconnected,
    Scanning,
    Connecting,
    Connected,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BallMetrics {
    pub speed_mps: f64,
    pub launch_angle: f64,
    pub horizontal_angle: f64,
    pub total_spin: f64,
    pub spin_axis: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SquareLaunchStatus {
    pub enabled: bool,
    pub connection_status: ConnectionStatus,
    pub host: Option<String>,
    pub port: u16,
    pub last_error: Option<String>,
    pub last_shot_number: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SimulatorStatus {
    pub enabled: bool,
    pub connection_status: ConnectionStatus,
    pub host: String,
    pub port: u16,
    pub last_error: Option<String>,
    pub last_shot_number: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum UiEvent {
    Status(AppStatus),
}

#[derive(Clone)]
pub struct AppState {
    inner: Arc<RwLock<AppStatus>>,
    events: broadcast::Sender<UiEvent>,
}

impl AppState {
    pub fn new(config: &AppConfig) -> Self {
        let status = AppStatus {
            api_port: config.api_port,
            device: DeviceStatus {
                connection_status: ConnectionStatus::Disconnected,
                device_name: None,
                battery_level: None,
                last_error: None,
                last_ball_metrics: None,
            },
            gspro: SimulatorStatus {
                enabled: config.gspro_enabled,
                connection_status: ConnectionStatus::Disconnected,
                host: config.gspro_host.clone(),
                port: config.gspro_port,
                last_error: None,
                last_shot_number: None,
            },
            infinite_tees: SimulatorStatus {
                enabled: config.infinite_tees_enabled,
                connection_status: ConnectionStatus::Disconnected,
                host: config.infinite_tees_host.clone(),
                port: config.infinite_tees_port,
                last_error: None,
                last_shot_number: None,
            },
            squarelaunch: SquareLaunchStatus {
                enabled: config.squarelaunch_enabled,
                connection_status: ConnectionStatus::Disconnected,
                host: config.squarelaunch_ws_host.clone(),
                port: config.squarelaunch_ws_port,
                last_error: None,
                last_shot_number: None,
            },
        };
        let (events, _) = broadcast::channel(128);
        Self {
            inner: Arc::new(RwLock::new(status)),
            events,
        }
    }

    pub async fn status(&self) -> AppStatus {
        self.inner.read().await.clone()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<UiEvent> {
        self.events.subscribe()
    }

    pub async fn update_squarelaunch<F>(&self, update: F)
    where
        F: FnOnce(&mut SquareLaunchStatus),
    {
        let mut status = self.inner.write().await;
        update(&mut status.squarelaunch);
        let _ = self.events.send(UiEvent::Status(status.clone()));
    }

    pub async fn update_device<F>(&self, update: F)
    where
        F: FnOnce(&mut DeviceStatus),
    {
        let mut status = self.inner.write().await;
        update(&mut status.device);
        let _ = self.events.send(UiEvent::Status(status.clone()));
    }

    pub async fn set_last_ball_metrics(&self, metrics: ParsedBallMetrics) {
        let mut status = self.inner.write().await;
        status.device.last_ball_metrics = Some(BallMetrics {
            speed_mps: metrics.ball_speed_mps,
            launch_angle: metrics.vertical_angle,
            horizontal_angle: metrics.horizontal_angle,
            total_spin: f64::from(metrics.total_spin_rpm),
            spin_axis: metrics.spin_axis,
        });
        let _ = self.events.send(UiEvent::Status(status.clone()));
    }

    pub async fn update_gspro<F>(&self, update: F)
    where
        F: FnOnce(&mut SimulatorStatus),
    {
        let mut status = self.inner.write().await;
        update(&mut status.gspro);
        let _ = self.events.send(UiEvent::Status(status.clone()));
    }

    pub async fn update_infinite_tees<F>(&self, update: F)
    where
        F: FnOnce(&mut SimulatorStatus),
    {
        let mut status = self.inner.write().await;
        update(&mut status.infinite_tees);
        let _ = self.events.send(UiEvent::Status(status.clone()));
    }
}
