use std::sync::Arc;

use tokio::sync::Mutex;

use crate::core::protocol::parser::{ParsedBallMetrics, ParsedClubMetrics};
use crate::core::{AppState, ConnectionStatus};

use super::client::JsonTcpClient;
use super::open_connect::{
    ready_payload, shot_payload_from_metrics, shot_payload_with_club_metrics,
};

#[derive(Clone)]
pub struct SimulatorRuntime {
    state: AppState,
    inner: Arc<Mutex<RuntimeInner>>,
}

#[derive(Default)]
struct RuntimeInner {
    gspro: SimulatorConnection,
    infinite_tees: SimulatorConnection,
}

#[derive(Default)]
struct SimulatorConnection {
    client: Option<JsonTcpClient>,
    shot_number: u64,
}

impl SimulatorRuntime {
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            inner: Arc::new(Mutex::new(RuntimeInner::default())),
        }
    }

    pub async fn connect_gspro(&self) -> Result<(), String> {
        let status = self.state.status().await.gspro;
        self.state
            .update_gspro(|status| {
                status.connection_status = ConnectionStatus::Connecting;
                status.last_error = None;
            })
            .await;
        match JsonTcpClient::connect(&status.host, status.port).await {
            Ok(client) => {
                let mut inner = self.inner.lock().await;
                inner.gspro.client = Some(client);
                self.state
                    .update_gspro(|status| {
                        status.connection_status = ConnectionStatus::Connected;
                        status.last_error = None;
                    })
                    .await;
                Ok(())
            }
            Err(err) => {
                self.state
                    .update_gspro(|status| {
                        status.connection_status = ConnectionStatus::Error;
                        status.last_error = Some(err.clone());
                    })
                    .await;
                Err(err)
            }
        }
    }

    pub async fn disconnect_gspro(&self) {
        let mut inner = self.inner.lock().await;
        inner.gspro.client = None;
        drop(inner);
        self.state
            .update_gspro(|status| {
                status.connection_status = ConnectionStatus::Disconnected;
            })
            .await;
    }

    pub async fn connect_infinite_tees(&self) -> Result<(), String> {
        let status = self.state.status().await.infinite_tees;
        self.state
            .update_infinite_tees(|status| {
                status.connection_status = ConnectionStatus::Connecting;
                status.last_error = None;
            })
            .await;
        match JsonTcpClient::connect(&status.host, status.port).await {
            Ok(client) => {
                let mut inner = self.inner.lock().await;
                inner.infinite_tees.client = Some(client);
                self.state
                    .update_infinite_tees(|status| {
                        status.connection_status = ConnectionStatus::Connected;
                        status.last_error = None;
                    })
                    .await;
                Ok(())
            }
            Err(err) => {
                self.state
                    .update_infinite_tees(|status| {
                        status.connection_status = ConnectionStatus::Error;
                        status.last_error = Some(err.clone());
                    })
                    .await;
                Err(err)
            }
        }
    }

    pub async fn disconnect_infinite_tees(&self) {
        let mut inner = self.inner.lock().await;
        inner.infinite_tees.client = None;
        drop(inner);
        self.state
            .update_infinite_tees(|status| {
                status.connection_status = ConnectionStatus::Disconnected;
            })
            .await;
    }

    pub async fn send_ball_metrics_to_connected(
        &self,
        metrics: &ParsedBallMetrics,
    ) -> Result<(), String> {
        self.state.set_last_ball_metrics(metrics.clone()).await;

        let mut errors = Vec::new();
        let mut inner = self.inner.lock().await;
        if inner.gspro.client.is_some() {
            match inner.gspro.send_ball(metrics).await {
                Ok(shot_number) => {
                    self.state
                        .update_gspro(|status| {
                            status.last_shot_number = Some(shot_number);
                            status.last_error = None;
                        })
                        .await;
                }
                Err(err) => {
                    errors.push(format!("GSPro: {err}"));
                    inner.gspro.client = None;
                    self.state
                        .update_gspro(|status| {
                            status.connection_status = ConnectionStatus::Error;
                            status.last_error = Some(err);
                        })
                        .await;
                }
            }
        }
        if inner.infinite_tees.client.is_some() {
            match inner.infinite_tees.send_ball(metrics).await {
                Ok(shot_number) => {
                    self.state
                        .update_infinite_tees(|status| {
                            status.last_shot_number = Some(shot_number);
                            status.last_error = None;
                        })
                        .await;
                }
                Err(err) => {
                    errors.push(format!("Infinite Tees: {err}"));
                    inner.infinite_tees.client = None;
                    self.state
                        .update_infinite_tees(|status| {
                            status.connection_status = ConnectionStatus::Error;
                            status.last_error = Some(err);
                        })
                        .await;
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }

    pub async fn send_club_metrics_to_connected(
        &self,
        metrics: &ParsedClubMetrics,
    ) -> Result<(), String> {
        let mut errors = Vec::new();
        let mut inner = self.inner.lock().await;
        if inner.gspro.client.is_some() {
            if let Err(err) = inner.gspro.send_club(metrics).await {
                errors.push(format!("GSPro: {err}"));
            }
        }
        if inner.infinite_tees.client.is_some() {
            if let Err(err) = inner.infinite_tees.send_club(metrics).await {
                errors.push(format!("Infinite Tees: {err}"));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }

    pub async fn send_ready_to_connected(&self, ready: bool) -> Result<(), String> {
        let mut errors = Vec::new();
        let mut inner = self.inner.lock().await;
        if inner.gspro.client.is_some() {
            if let Err(err) = inner.gspro.send_ready(ready).await {
                errors.push(format!("GSPro: {err}"));
            }
        }
        if inner.infinite_tees.client.is_some() {
            if let Err(err) = inner.infinite_tees.send_ready(ready).await {
                errors.push(format!("Infinite Tees: {err}"));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }
}

impl SimulatorConnection {
    async fn send_ball(&mut self, metrics: &ParsedBallMetrics) -> Result<u64, String> {
        self.shot_number += 1;
        let shot_number = self.shot_number;
        let payload = shot_payload_from_metrics(metrics, shot_number);
        let client = self
            .client
            .as_mut()
            .ok_or_else(|| "not connected".to_string())?;
        client.send_json(&payload).await?;
        Ok(shot_number)
    }

    async fn send_club(&mut self, metrics: &ParsedClubMetrics) -> Result<(), String> {
        let payload = shot_payload_with_club_metrics(metrics, self.shot_number);
        let client = self
            .client
            .as_mut()
            .ok_or_else(|| "not connected".to_string())?;
        client.send_json(&payload).await
    }

    async fn send_ready(&mut self, ready: bool) -> Result<(), String> {
        let payload = ready_payload(ready, self.shot_number);
        let client = self
            .client
            .as_mut()
            .ok_or_else(|| "not connected".to_string())?;
        client.send_json(&payload).await
    }
}
