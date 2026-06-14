use std::sync::{Arc, Mutex};

use tauri::Emitter;

pub mod api;
pub mod config;
pub mod core;
pub mod device;
pub mod simulator;
pub mod squarelaunch;

#[derive(Clone, Default)]
struct ApiUrlState(Arc<Mutex<Option<String>>>);

#[tauri::command]
fn api_base(state: tauri::State<'_, ApiUrlState>) -> Option<String> {
    state.0.lock().ok().and_then(|url| url.clone())
}

pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "squaregolf_connector=info,tower_http=info".into()),
        )
        .try_init();

    let api_url_state = ApiUrlState::default();
    tauri::Builder::default()
        .manage(api_url_state.clone())
        .invoke_handler(tauri::generate_handler![api_base])
        .setup(move |app| {
            let handle = app.handle().clone();
            let api_url_state = api_url_state.clone();
            tauri::async_runtime::spawn(async move {
                let result = async {
                    let config = crate::config::AppConfig::from_env()?;
                    api::serve_with_ready(
                        config,
                        Some(crate::config::ConfigStore::default()),
                        move |addr| {
                            let url = format!("http://{addr}");
                            if let Ok(mut stored) = api_url_state.0.lock() {
                                *stored = Some(url.clone());
                            }
                            handle.emit("api-ready", url).map_err(|err| err.to_string())
                        },
                    )
                    .await
                }
                .await;
                if let Err(err) = result {
                    tracing::error!("API server failed: {err}");
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}
