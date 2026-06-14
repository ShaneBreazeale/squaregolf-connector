use std::sync::Arc;
use std::time::Duration;

use btleplug::api::{Central, Characteristic, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Manager, Peripheral};
use futures_util::StreamExt;
use tokio::sync::{oneshot, Mutex};
use uuid::Uuid;

use crate::core::protocol::parser::{
    parse_omni_shot_club_metrics, parse_shot_ball_metrics, parse_shot_club_metrics,
};
use crate::core::{AppState, ConnectionStatus};
use crate::simulator::runtime::SimulatorRuntime;

const DEVICE_PREFIX: &str = "SquareGolf";
const NOTIFICATION_CHAR_UUID: &str = "86602102-6b7e-439a-bdd1-489a3213e9bb";
const BATTERY_LEVEL_CHAR_UUID: &str = "00002a19-0000-1000-8000-00805f9b34fb";
const SCAN_TICK: Duration = Duration::from_millis(500);

#[derive(Clone)]
pub struct DeviceRuntime {
    state: AppState,
    simulators: SimulatorRuntime,
    inner: Arc<Mutex<DeviceInner>>,
}

#[derive(Default)]
struct DeviceInner {
    stop: Option<oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
}

#[derive(Debug, Clone, Default)]
pub struct DeviceConnectOptions {
    pub device_name: Option<String>,
    pub device_address: Option<String>,
    pub emulator: bool,
}

impl DeviceRuntime {
    pub fn new(state: AppState, simulators: SimulatorRuntime) -> Self {
        Self {
            state,
            simulators,
            inner: Arc::new(Mutex::new(DeviceInner::default())),
        }
    }

    pub async fn connect(&self, options: DeviceConnectOptions) {
        self.disconnect().await;

        if options.emulator {
            self.state
                .update_device(|status| {
                    status.connection_status = ConnectionStatus::Connected;
                    status.device_name = Some(
                        options
                            .device_name
                            .unwrap_or_else(|| "SquareGolf Emulator".to_string()),
                    );
                    status.battery_level = Some(100);
                    status.last_error = None;
                })
                .await;
            return;
        }

        let (stop_tx, stop_rx) = oneshot::channel();
        self.state
            .update_device(|status| {
                status.connection_status = ConnectionStatus::Scanning;
                status.last_error = None;
            })
            .await;

        let state = self.state.clone();
        let simulators = self.simulators.clone();
        let task = tokio::spawn(async move {
            if let Err(err) = run_ble_connection(state.clone(), simulators, options, stop_rx).await
            {
                state
                    .update_device(|status| {
                        status.connection_status = ConnectionStatus::Error;
                        status.last_error = Some(err);
                    })
                    .await;
            }
        });

        let mut inner = self.inner.lock().await;
        inner.stop = Some(stop_tx);
        inner.task = Some(task);
    }

    pub async fn disconnect(&self) {
        let mut inner = self.inner.lock().await;
        if let Some(stop) = inner.stop.take() {
            let _ = stop.send(());
        }
        let _ = inner.task.take();
        drop(inner);

        self.state
            .update_device(|status| {
                status.connection_status = ConnectionStatus::Disconnected;
                status.device_name = None;
                status.battery_level = None;
            })
            .await;
    }

    pub async fn emulate_notification(&self, value: &[u8]) {
        handle_squaregolf_notification(&self.state, &self.simulators, value).await;
    }
}

async fn run_ble_connection(
    state: AppState,
    simulators: SimulatorRuntime,
    options: DeviceConnectOptions,
    mut stop_rx: oneshot::Receiver<()>,
) -> Result<(), String> {
    let manager = Manager::new()
        .await
        .map_err(|err| format!("initialize Bluetooth manager: {err}"))?;
    let adapter = manager
        .adapters()
        .await
        .map_err(|err| format!("list Bluetooth adapters: {err}"))?
        .into_iter()
        .next()
        .ok_or_else(|| "no Bluetooth adapter found".to_string())?;

    adapter
        .start_scan(ScanFilter::default())
        .await
        .map_err(|err| format!("start Bluetooth scan: {err}"))?;

    let peripheral = loop {
        tokio::select! {
            _ = &mut stop_rx => {
                let _ = adapter.stop_scan().await;
                return Ok(());
            }
            _ = tokio::time::sleep(SCAN_TICK) => {
                if let Some(peripheral) = find_squaregolf_peripheral(&adapter, &options).await? {
                    break peripheral;
                }
            }
        }
    };

    let _ = adapter.stop_scan().await;
    state
        .update_device(|status| {
            status.connection_status = ConnectionStatus::Connecting;
        })
        .await;

    peripheral
        .connect()
        .await
        .map_err(|err| format!("connect to SquareGolf device: {err}"))?;
    peripheral
        .discover_services()
        .await
        .map_err(|err| format!("discover SquareGolf services: {err}"))?;

    let notify_uuid = parse_uuid(NOTIFICATION_CHAR_UUID)?;
    let battery_uuid = parse_uuid(BATTERY_LEVEL_CHAR_UUID)?;
    let chars = peripheral.characteristics();
    let notify_char = chars
        .iter()
        .find(|ch| ch.uuid == notify_uuid)
        .cloned()
        .ok_or_else(|| "SquareGolf notification characteristic not found".to_string())?;
    let battery_char = chars.iter().find(|ch| ch.uuid == battery_uuid).cloned();

    peripheral
        .subscribe(&notify_char)
        .await
        .map_err(|err| format!("subscribe to SquareGolf notifications: {err}"))?;
    if let Some(ch) = battery_char.as_ref() {
        let _ = peripheral.subscribe(ch).await;
        if let Ok(value) = peripheral.read(ch).await {
            update_battery(&state, &value).await;
        }
    }

    let device_name = peripheral
        .properties()
        .await
        .ok()
        .flatten()
        .and_then(|properties| properties.local_name)
        .unwrap_or_else(|| DEVICE_PREFIX.to_string());
    state
        .update_device(|status| {
            status.connection_status = ConnectionStatus::Connected;
            status.device_name = Some(device_name);
            status.last_error = None;
        })
        .await;

    let mut notifications = peripheral
        .notifications()
        .await
        .map_err(|err| format!("open SquareGolf notification stream: {err}"))?;
    loop {
        tokio::select! {
            _ = &mut stop_rx => {
                disconnect_peripheral(&peripheral, &notify_char, battery_char.as_ref()).await;
                return Ok(());
            }
            notification = notifications.next() => {
                let Some(notification) = notification else { break; };
                if notification.uuid == battery_uuid {
                    update_battery(&state, &notification.value).await;
                } else {
                    handle_squaregolf_notification(&state, &simulators, &notification.value).await;
                }
            }
        }
    }

    disconnect_peripheral(&peripheral, &notify_char, battery_char.as_ref()).await;
    state
        .update_device(|status| {
            status.connection_status = ConnectionStatus::Disconnected;
        })
        .await;
    Ok(())
}

async fn find_squaregolf_peripheral(
    adapter: &btleplug::platform::Adapter,
    options: &DeviceConnectOptions,
) -> Result<Option<Peripheral>, String> {
    for peripheral in adapter
        .peripherals()
        .await
        .map_err(|err| format!("list Bluetooth peripherals: {err}"))?
    {
        let Some(properties) = peripheral
            .properties()
            .await
            .map_err(|err| format!("read Bluetooth peripheral properties: {err}"))?
        else {
            continue;
        };

        let local_name = properties.local_name.unwrap_or_default();
        let address = properties.address.to_string();
        let name_matches = options
            .device_name
            .as_ref()
            .map(|name| local_name == *name)
            .unwrap_or_else(|| local_name.starts_with(DEVICE_PREFIX));
        let address_matches = options
            .device_address
            .as_ref()
            .map(|candidate| address.eq_ignore_ascii_case(candidate))
            .unwrap_or(true);

        if name_matches && address_matches {
            return Ok(Some(peripheral));
        }
    }
    Ok(None)
}

async fn handle_squaregolf_notification(
    state: &AppState,
    simulators: &SimulatorRuntime,
    value: &[u8],
) {
    let bytes = value
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>();
    let byte_refs = bytes.iter().map(String::as_str).collect::<Vec<_>>();
    if bytes.len() < 2 || bytes[0] != "11" {
        return;
    }

    match bytes[1].as_str() {
        "02" => match parse_shot_ball_metrics(&byte_refs) {
            Ok(metrics) => {
                if let Err(err) = simulators.send_ball_metrics_to_connected(&metrics).await {
                    state
                        .update_device(|status| {
                            status.last_error = Some(format!("send ball metrics: {err}"));
                        })
                        .await;
                }
            }
            Err(err) => {
                state
                    .update_device(|status| {
                        status.last_error = Some(format!("parse ball metrics: {err}"));
                    })
                    .await;
            }
        },
        "07" => {
            let parsed = if bytes.len() >= 19 {
                parse_omni_shot_club_metrics(&byte_refs)
            } else {
                parse_shot_club_metrics(&byte_refs)
            };
            match parsed {
                Ok(metrics) => {
                    if let Err(err) = simulators.send_club_metrics_to_connected(&metrics).await {
                        state
                            .update_device(|status| {
                                status.last_error = Some(format!("send club metrics: {err}"));
                            })
                            .await;
                    }
                }
                Err(err) => {
                    state
                        .update_device(|status| {
                            status.last_error = Some(format!("parse club metrics: {err}"));
                        })
                        .await;
                }
            }
        }
        _ => {}
    }
}

async fn update_battery(state: &AppState, value: &[u8]) {
    if let Some(level) = value.first().copied() {
        state
            .update_device(|status| {
                status.battery_level = Some(level);
            })
            .await;
    }
}

async fn disconnect_peripheral(
    peripheral: &Peripheral,
    notify_char: &Characteristic,
    battery_char: Option<&Characteristic>,
) {
    let _ = peripheral.unsubscribe(notify_char).await;
    if let Some(ch) = battery_char {
        let _ = peripheral.unsubscribe(ch).await;
    }
    let _ = peripheral.disconnect().await;
}

fn parse_uuid(value: &str) -> Result<Uuid, String> {
    Uuid::parse_str(value).map_err(|err| format!("parse UUID {value}: {err}"))
}
