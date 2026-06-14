use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::Mutex;
use tokio::time::sleep;

use crate::config::AppConfig;
use crate::core::{AppState, ConnectionStatus};

use super::discovery::discover_squarelaunch_ws_endpoint;
use super::protocol::{parse_squarelaunch_ws_message, SquareLaunchMessage};

#[derive(Clone)]
pub struct SquareLaunchRuntime {
    state: AppState,
    inner: Arc<Mutex<SquareLaunchInner>>,
}

#[derive(Default)]
struct SquareLaunchInner {
    task: Option<tokio::task::JoinHandle<()>>,
}

impl SquareLaunchRuntime {
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            inner: Arc::new(Mutex::new(SquareLaunchInner::default())),
        }
    }

    pub async fn configure(&self, config: AppConfig) {
        let mut inner = self.inner.lock().await;
        if let Some(task) = inner.task.take() {
            task.abort();
        }

        if !config.squarelaunch_enabled {
            drop(inner);
            self.state
                .update_squarelaunch(|status| {
                    status.connection_status = ConnectionStatus::Disconnected;
                    status.last_error = None;
                })
                .await;
            return;
        }

        let state = self.state.clone();
        inner.task = Some(tokio::spawn(async move {
            run_squarelaunch_loop(config, state).await;
        }));
    }
}

async fn run_squarelaunch_loop(config: AppConfig, state: AppState) {
    if !config.squarelaunch_enabled {
        return;
    }
    loop {
        let (host, port) = if let Some(host) = config.squarelaunch_ws_host.clone() {
            (host, config.squarelaunch_ws_port)
        } else {
            state
                .update_squarelaunch(|status| {
                    status.connection_status = ConnectionStatus::Scanning;
                    status.last_error = None;
                })
                .await;
            match tokio::task::spawn_blocking(|| {
                discover_squarelaunch_ws_endpoint(Duration::from_secs(5))
            })
            .await
            {
                Ok(Ok(endpoint)) => {
                    state
                        .update_squarelaunch(|status| {
                            status.host = Some(endpoint.host.clone());
                            status.port = endpoint.port;
                            status.last_error = None;
                        })
                        .await;
                    (endpoint.host, endpoint.port)
                }
                Ok(Err(err)) => {
                    state
                        .update_squarelaunch(|status| {
                            status.connection_status = ConnectionStatus::Error;
                            status.last_error = Some(err);
                        })
                        .await;
                    sleep(Duration::from_secs(2)).await;
                    continue;
                }
                Err(err) => {
                    state
                        .update_squarelaunch(|status| {
                            status.connection_status = ConnectionStatus::Error;
                            status.last_error =
                                Some(format!("SquareLaunch discovery task failed: {err}"));
                        })
                        .await;
                    sleep(Duration::from_secs(2)).await;
                    continue;
                }
            }
        };

        let url = format!("ws://{host}:{port}");
        state
            .update_squarelaunch(|status| {
                status.connection_status = ConnectionStatus::Connecting;
                status.last_error = None;
            })
            .await;

        match tokio_tungstenite::connect_async(&url).await {
            Ok((stream, _)) => {
                state
                    .update_squarelaunch(|status| {
                        status.connection_status = ConnectionStatus::Connected;
                        status.last_error = None;
                    })
                    .await;
                pump_messages(stream, &state).await;
            }
            Err(err) => {
                state
                    .update_squarelaunch(|status| {
                        status.connection_status = ConnectionStatus::Error;
                        status.last_error =
                            Some(format!("SquareLaunch websocket connect failed: {err}"));
                    })
                    .await;
            }
        }

        state
            .update_squarelaunch(|status| {
                status.connection_status = ConnectionStatus::Disconnected;
            })
            .await;
        sleep(Duration::from_millis(750)).await;
    }
}

async fn pump_messages<S>(mut stream: S, state: &AppState)
where
    S: futures_util::Stream<
            Item = Result<
                tokio_tungstenite::tungstenite::Message,
                tokio_tungstenite::tungstenite::Error,
            >,
        > + Unpin,
{
    while let Some(message) = stream.next().await {
        match message {
            Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                match parse_squarelaunch_ws_message(&text) {
                    Ok(SquareLaunchMessage::Shot(shot)) => {
                        state
                            .update_squarelaunch(|status| {
                                status.last_shot_number = Some(shot.shot_number);
                                status.last_error = None;
                            })
                            .await;
                    }
                    Ok(SquareLaunchMessage::Status) => {}
                    Ok(SquareLaunchMessage::Other(kind)) => {
                        state
                            .update_squarelaunch(|status| {
                                status.last_error =
                                    Some(format!("ignored unknown SquareLaunch message {kind:?}"));
                            })
                            .await;
                    }
                    Err(err) => {
                        state
                            .update_squarelaunch(|status| {
                                status.last_error = Some(err);
                            })
                            .await;
                    }
                }
            }
            Ok(tokio_tungstenite::tungstenite::Message::Close(_)) => break,
            Ok(_) => {}
            Err(err) => {
                state
                    .update_squarelaunch(|status| {
                        status.last_error =
                            Some(format!("SquareLaunch websocket read failed: {err}"));
                    })
                    .await;
                break;
            }
        }
    }
}
